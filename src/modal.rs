//! Modals: a persistent stack of modal layers that trap input while open.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

use crate::event::{
    ControlAction, ControlPart, EventContext, EventResponse, FilterContext, Filtered, GestureEvent,
    InputEvent, Key, KeyboardEvent, Modifiers, MouseButton, MouseEvent,
};
use crate::handle::ControlHandle;
use crate::host::{Control, ControlHost, ControlKey, DragGrab, HostedControl, InputFilter};
use crate::rect::Rect;
use crate::symbol::Symbol;
use crate::traits::{DrawContext, DrawControl};

/// The most modals the stack holds at once, entries still closing included.
pub const MAX_MODAL_DEPTH: usize = 4;

/// Where a modal is in its life.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ModalPhase {
    /// Not on the stack.
    #[default]
    Closed,
    /// On its way in; takes no input yet.
    Opening,
    /// Fully open; its content takes input when it is the top.
    Open,
    /// On its way out; never the top again.
    Closing,
}

/// How progress moves between 0 and 1.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ModalTransition {
    /// Progress jumps; push and dismiss finish their transition at once.
    Instant,
    /// Progress moves linearly with [`ModalState::advance`]. A duration of
    /// zero or less behaves as `Instant` for that direction.
    Timed { open_secs: f32, close_secs: f32 },
    /// The app sets progress with [`ModalState::set_progress`]; `advance`
    /// does nothing.
    Driven,
}

impl Default for ModalTransition {
    /// `Timed { open_secs: 0.18, close_secs: 0.12 }`.
    fn default() -> Self {
        Self::Timed {
            open_secs: 0.18,
            close_secs: 0.12,
        }
    }
}

impl ModalTransition {
    fn opens_at_once(self) -> bool {
        match self {
            Self::Instant => true,
            Self::Timed { open_secs, .. } => open_secs <= 0.0,
            Self::Driven => false,
        }
    }

    fn closes_at_once(self) -> bool {
        match self {
            Self::Instant => true,
            Self::Timed { close_secs, .. } => close_secs <= 0.0,
            Self::Driven => false,
        }
    }
}

/// Why a modal began to close.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DismissReason {
    /// A press and release both outside the top's frame.
    OutsideTap,
    /// `Key::Escape`.
    Escape,
    /// `Key::Back`, the platform back action.
    Back,
    /// A content control registered with `Modal::with_close_control` was
    /// activated; carries its name.
    Control(Symbol),
    /// The app called `pop`, `close`, `dismiss(DismissReason::App)` or
    /// `dismiss_all(DismissReason::App)`.
    App,
    /// The app called `replace_top`; the outgoing top closes with this reason.
    Replaced,
}

/// Something the modal layer reports. Taken once with [`ModalState::take_events`].
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ModalEvent {
    /// The opening transition finished; content now takes input.
    Opened { id: Symbol },
    /// The modal began to close, and why.
    Dismissed { id: Symbol, by: DismissReason },
    /// The closing transition finished; the entry is gone. Every `Dismissed`
    /// is followed by exactly one `Closed` with the same id and reason.
    Closed { id: Symbol, by: DismissReason },
    /// A user dismissal was refused because the modal is held open. The
    /// modal stays as it was.
    DismissRefused { id: Symbol, by: DismissReason },
}

/// One entry of the stack, as [`ModalState::stack`] reports it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ModalEntryView {
    pub id: Symbol,
    pub phase: ModalPhase,
    pub progress: f32,
    pub is_top: bool,
}

#[derive(Debug)]
pub(crate) struct ModalEntry {
    pub(crate) id: Symbol,
    pub(crate) phase: ModalPhase,
    pub(crate) progress: f32,
    transition: ModalTransition,
    reason: Option<DismissReason>,
    pub(crate) focus: Option<ControlKey>,
    pub(crate) drag: Option<DragGrab>,
    pub(crate) hover: Option<ControlKey>,
    pub(crate) focus_seeded: bool,
    pub(crate) outside_press: Option<MouseButton>,
}

#[derive(Debug, Default)]
pub(crate) struct ModalInner {
    default_transition: ModalTransition,
    pub(crate) entries: Vec<ModalEntry>,
    pending: Vec<(Symbol, ModalTransition)>,
    pub(crate) events: Vec<ModalEvent>,
}

impl ModalInner {
    fn is_live(&self, id: Symbol) -> bool {
        self.entries
            .iter()
            .any(|e| e.id == id && e.phase != ModalPhase::Closing)
    }

    pub(crate) fn top_index(&self) -> Option<usize> {
        self.entries
            .iter()
            .rposition(|e| e.phase != ModalPhase::Closing)
    }

    /// The entry an id names: the live one if there is one, else one closing.
    pub(crate) fn find(&self, id: Symbol) -> Option<usize> {
        self.entries
            .iter()
            .rposition(|e| e.id == id && e.phase != ModalPhase::Closing)
            .or_else(|| self.entries.iter().rposition(|e| e.id == id))
    }

    fn push(&mut self, id: Symbol, transition: ModalTransition) -> bool {
        if self.is_live(id) {
            return false;
        }
        if self.entries.iter().any(|e| e.id == id) {
            self.pending.retain(|(p, _)| *p != id);
            self.pending.push((id, transition));
            return true;
        }
        if self.entries.len() >= MAX_MODAL_DEPTH {
            return false;
        }
        let open = transition.opens_at_once();
        self.entries.push(ModalEntry {
            id,
            phase: if open {
                ModalPhase::Open
            } else {
                ModalPhase::Opening
            },
            progress: if open { 1.0 } else { 0.0 },
            transition,
            reason: None,
            focus: None,
            drag: None,
            hover: None,
            focus_seeded: false,
            outside_press: None,
        });
        if open {
            self.events.push(ModalEvent::Opened { id });
        }
        true
    }

    fn begin_close(&mut self, index: usize, by: DismissReason) {
        let entry = &mut self.entries[index];
        entry.phase = ModalPhase::Closing;
        entry.reason = Some(by);
        let id = entry.id;
        let at_once = entry.transition.closes_at_once();
        self.events.push(ModalEvent::Dismissed { id, by });
        if at_once {
            self.entries[index].progress = 0.0;
            self.finish_close(index);
        }
    }

    fn finish_close(&mut self, index: usize) {
        let entry = self.entries.remove(index);
        let by = entry.reason.unwrap_or(DismissReason::App);
        self.events.push(ModalEvent::Closed { id: entry.id, by });
        if let Some(pos) = self.pending.iter().position(|(p, _)| *p == entry.id) {
            let (id, transition) = self.pending.remove(pos);
            self.push(id, transition);
        }
    }

    /// Apply the "progress reaches 1 / 0" rules to the entry at `index`.
    fn settle(&mut self, index: usize) {
        let entry = &mut self.entries[index];
        match entry.phase {
            ModalPhase::Opening if entry.progress >= 1.0 => {
                entry.phase = ModalPhase::Open;
                let id = entry.id;
                self.events.push(ModalEvent::Opened { id });
            }
            ModalPhase::Closing if entry.progress <= 0.0 => self.finish_close(index),
            _ => {}
        }
    }

    fn set_progress(&mut self, index: usize, progress: f32) {
        if !progress.is_finite() {
            return;
        }
        self.entries[index].progress = progress.clamp(0.0, 1.0);
        self.settle(index);
    }

    fn dismiss(&mut self, by: DismissReason) {
        if let Some(top) = self.top_index() {
            self.begin_close(top, by);
        }
    }
}

/// The modal stack, shared between the app and every rebuild of the host.
#[derive(Debug, Default)]
pub struct ModalState {
    inner: Mutex<ModalInner>,
    /// Whether any entry exists, kept in step with the lock so that
    /// `captures_input` needs none.
    occupied: AtomicBool,
}

impl ModalState {
    /// An empty stack with the default transition.
    pub fn new() -> Self {
        Self::default()
    }

    /// The same stack, with `transition` as the default for pushes.
    #[must_use]
    pub fn with_transition(self, transition: ModalTransition) -> Self {
        self.inner.lock().unwrap().default_transition = transition;
        self
    }

    /// Run `change` under the lock, then bring `occupied` up to date.
    pub(crate) fn change<R>(&self, change: impl FnOnce(&mut ModalInner) -> R) -> R {
        let mut inner = self.inner.lock().unwrap();
        let result = change(&mut inner);
        self.occupied
            .store(!inner.entries.is_empty(), Ordering::Release);
        result
    }

    fn read<R>(&self, read: impl FnOnce(&ModalInner) -> R) -> R {
        read(&self.inner.lock().unwrap())
    }

    /// Open `id` over the top with the default transition. False if it is
    /// already open or the stack is full.
    pub fn push(&self, id: impl Into<Symbol>) -> bool {
        let id = id.into();
        self.change(|inner| {
            let t = inner.default_transition;
            inner.push(id, t)
        })
    }

    /// Open `id` over the top with `transition`.
    pub fn push_with(&self, id: impl Into<Symbol>, transition: ModalTransition) -> bool {
        let id = id.into();
        self.change(|inner| inner.push(id, transition))
    }

    /// Close the top (reported as `Replaced`) and open `id` over it at once.
    pub fn replace_top(&self, id: impl Into<Symbol>) -> bool {
        let id = id.into();
        let t = self.read(|inner| inner.default_transition);
        self.replace_top_with(id, t)
    }

    /// `replace_top` with `transition` for the incoming modal.
    pub fn replace_top_with(&self, id: impl Into<Symbol>, transition: ModalTransition) -> bool {
        let id = id.into();
        self.change(|inner| {
            if inner.is_live(id) {
                return false;
            }
            let Some(top) = inner.top_index() else {
                return inner.push(id, transition);
            };
            if inner.entries.len() >= MAX_MODAL_DEPTH {
                return false;
            }
            inner.begin_close(top, DismissReason::Replaced);
            inner.push(id, transition)
        })
    }

    /// Close the top only. Ignores any hold-open: this is the app's call.
    pub fn dismiss(&self, by: DismissReason) {
        self.change(|inner| inner.dismiss(by));
    }

    /// `dismiss(DismissReason::App)`.
    pub fn pop(&self) {
        self.dismiss(DismissReason::App);
    }

    /// Close the entry `id` names, wherever it is on the stack.
    pub fn close(&self, id: impl Into<Symbol>) {
        let id = id.into();
        self.change(|inner| {
            if let Some(index) = inner
                .entries
                .iter()
                .position(|e| e.id == id && e.phase != ModalPhase::Closing)
            {
                inner.begin_close(index, DismissReason::App);
            }
        });
    }

    /// Close every open entry, top first, and drop any queued push.
    pub fn dismiss_all(&self, by: DismissReason) {
        self.change(|inner| {
            inner.pending.clear();
            for index in (0..inner.entries.len()).rev() {
                if inner.entries[index].phase != ModalPhase::Closing {
                    inner.begin_close(index, by);
                }
            }
        });
    }

    /// Move every `Timed` entry on by `dt_secs`. True while any entry is
    /// still opening or closing.
    pub fn advance(&self, dt_secs: f32) -> bool {
        let dt = if dt_secs.is_finite() {
            dt_secs.max(0.0)
        } else {
            0.0
        };
        self.change(|inner| {
            // Work out every step first: settling one entry can remove it and
            // push a queued one, which would shift the indices of the rest.
            let steps: Vec<(Symbol, ModalPhase, f32)> = inner
                .entries
                .iter()
                .filter_map(|e| match (e.transition, e.phase) {
                    (ModalTransition::Timed { open_secs, .. }, ModalPhase::Opening) => Some((
                        e.id,
                        e.phase,
                        e.progress + dt / open_secs.max(f32::MIN_POSITIVE),
                    )),
                    (ModalTransition::Timed { close_secs, .. }, ModalPhase::Closing) => Some((
                        e.id,
                        e.phase,
                        e.progress - dt / close_secs.max(f32::MIN_POSITIVE),
                    )),
                    _ => None,
                })
                .collect();
            for (id, phase, progress) in steps {
                if let Some(index) = inner
                    .entries
                    .iter()
                    .position(|e| e.id == id && e.phase == phase)
                {
                    inner.set_progress(index, progress);
                }
            }
            inner
                .entries
                .iter()
                .any(|e| matches!(e.phase, ModalPhase::Opening | ModalPhase::Closing))
        })
    }

    /// Set the progress of the entry `id` names (for `Driven` entries; works
    /// for any).
    pub fn set_progress(&self, id: impl Into<Symbol>, progress: f32) {
        let id = id.into();
        self.change(|inner| {
            if let Some(index) = inner.find(id) {
                inner.set_progress(index, progress);
            }
        });
    }

    /// Set the progress of the top entry.
    pub fn set_top_progress(&self, progress: f32) {
        self.change(|inner| {
            if let Some(index) = inner.top_index() {
                inner.set_progress(index, progress);
            }
        });
    }

    /// The top entry's id: the uppermost entry not closing.
    pub fn top(&self) -> Option<Symbol> {
        self.read(|inner| inner.top_index().map(|i| inner.entries[i].id))
    }

    /// How many entries there are, closing ones included.
    pub fn depth(&self) -> usize {
        self.read(|inner| inner.entries.len())
    }

    /// Whether `id` is on the stack and not closing.
    pub fn contains(&self, id: impl Into<Symbol>) -> bool {
        let id = id.into();
        self.read(|inner| inner.is_live(id))
    }

    /// The phase of the entry `id` names; `Closed` if there is none.
    pub fn phase(&self, id: impl Into<Symbol>) -> ModalPhase {
        let id = id.into();
        self.read(|inner| {
            inner
                .find(id)
                .map_or(ModalPhase::Closed, |i| inner.entries[i].phase)
        })
    }

    /// The progress of the entry `id` names; 0 if there is none.
    pub fn progress(&self, id: impl Into<Symbol>) -> f32 {
        let id = id.into();
        self.read(|inner| inner.find(id).map_or(0.0, |i| inner.entries[i].progress))
    }

    /// Every entry, bottom to top.
    pub fn stack(&self) -> Vec<ModalEntryView> {
        self.read(|inner| {
            let top = inner.top_index();
            inner
                .entries
                .iter()
                .enumerate()
                .map(|(i, e)| ModalEntryView {
                    id: e.id,
                    phase: e.phase,
                    progress: e.progress,
                    is_top: top == Some(i),
                })
                .collect()
        })
    }

    /// Whether any entry exists, closing ones included. Takes no lock.
    pub fn captures_input(&self) -> bool {
        self.occupied.load(Ordering::Acquire)
    }

    /// Whether there is a top and it is fully open.
    pub fn top_takes_input(&self) -> bool {
        self.read(|inner| {
            inner
                .top_index()
                .is_some_and(|i| inner.entries[i].phase == ModalPhase::Open)
        })
    }

    /// Whether any entry is opening or closing.
    pub fn is_animating(&self) -> bool {
        self.read(|inner| {
            inner
                .entries
                .iter()
                .any(|e| matches!(e.phase, ModalPhase::Opening | ModalPhase::Closing))
        })
    }

    /// Every event since the last call, in the order they happened.
    pub fn take_events(&self) -> Vec<ModalEvent> {
        self.change(|inner| std::mem::take(&mut inner.events))
    }

    /// The filter that traps input for this stack, redirecting every event
    /// to the hosted `ModalStack` named `target` while any modal exists.
    pub fn filter(self: &Arc<Self>, target: impl Into<Symbol>) -> ModalFilter {
        ModalFilter::new(Arc::clone(self), target)
    }
}

/// Traps input while any modal exists by redirecting every event to the
/// hosted `ModalStack` registered under `target`. Created once from the state
/// and kept in app state beside the `Arc`; borrowed by every host build.
#[derive(Clone, Debug)]
pub struct ModalFilter {
    state: Arc<ModalState>,
    target: Symbol,
}

impl ModalFilter {
    /// A filter for `state` that redirects to the control named `target`.
    pub fn new(state: Arc<ModalState>, target: impl Into<Symbol>) -> Self {
        Self {
            state,
            target: target.into(),
        }
    }

    /// The stack this filter traps input for.
    pub fn state(&self) -> &Arc<ModalState> {
        &self.state
    }

    /// The name of the `ModalStack` control it redirects to.
    pub fn target(&self) -> Symbol {
        self.target
    }
}

impl InputFilter for ModalFilter {
    fn filter(&self, _event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        if self.state.captures_input() {
            Filtered::Redirect(ControlKey::Named(self.target))
        } else {
            Filtered::Pass
        }
    }
}

/// Everything a frame painter needs to draw one frame of one modal.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ModalView {
    pub id: Symbol,
    pub phase: ModalPhase,
    pub progress: f32,
    pub viewport: Rect,
    pub frame: Rect,
    pub content: Rect,
    /// 0 is the bottom of the stack.
    pub depth: usize,
    pub is_top: bool,
}

/// The app-drawn look of a modal: its background, its animation, its shape.
pub trait ModalFrame {
    /// Draw the frame (parchment, open book, panel) under the content.
    fn draw_frame(&self, list: &mut DrawList, ctx: &DrawContext, view: &ModalView);

    /// Draw anything over the content (a page-curl shadow, a ribbon).
    fn draw_over_content(&self, _list: &mut DrawList, _ctx: &DrawContext, _view: &ModalView) {}

    /// Whether a point is on the modal for tap-outside purposes.
    fn contains(&self, pos: [f32; 2], view: &ModalView) -> bool {
        view.frame.contains(pos)
    }
}

impl<F: ModalFrame + ?Sized> ModalFrame for &F {
    fn draw_frame(&self, list: &mut DrawList, ctx: &DrawContext, view: &ModalView) {
        (**self).draw_frame(list, ctx, view);
    }

    fn draw_over_content(&self, list: &mut DrawList, ctx: &DrawContext, view: &ModalView) {
        (**self).draw_over_content(list, ctx, view);
    }

    fn contains(&self, pos: [f32; 2], view: &ModalView) -> bool {
        (**self).contains(pos, view)
    }
}

/// The built-in frame: a rounded box with an optional border, in theme
/// colours, faded by the modal's progress.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PanelFrame {
    /// `None`: the theme's background colour.
    pub bg_color: Option<[f32; 4]>,
    /// `None`: the theme's accent colour.
    pub border_color: Option<[f32; 4]>,
    /// 0 draws no border.
    pub border_thickness: f32,
    /// `None`: the theme's corner radius.
    pub corner_radius: Option<f32>,
}

impl Default for PanelFrame {
    fn default() -> Self {
        Self {
            bg_color: None,
            border_color: None,
            border_thickness: 1.0,
            corner_radius: None,
        }
    }
}

fn faded(mut color: [f32; 4], by: f32) -> [f32; 4] {
    color[3] *= by;
    color
}

impl ModalFrame for PanelFrame {
    fn draw_frame(&self, list: &mut DrawList, ctx: &DrawContext, view: &ModalView) {
        let radius = self.corner_radius.unwrap_or(ctx.theme.corner_radius);
        let kind = if radius > 0.0 {
            SdfKind::RoundedBox { radius }
        } else {
            SdfKind::Box
        };
        list.push(SdfInstance {
            kind,
            position: view.frame.pos(),
            size: view.frame.size(),
            color: faded(self.bg_color.unwrap_or(ctx.theme.bg_color), view.progress),
            anim: 0,
        });
        if self.border_thickness > 0.0 {
            list.push(SdfInstance {
                kind: SdfKind::Outline {
                    radius,
                    thickness: self.border_thickness,
                },
                position: view.frame.pos(),
                size: view.frame.size(),
                color: faded(
                    self.border_color.unwrap_or(ctx.theme.accent_color),
                    view.progress,
                ),
                anim: 0,
            });
        }
    }
}

/// The dim drawn over everything beneath a modal.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Backdrop {
    pub color: [f32; 4],
    /// Multiply the colour's alpha by the modal's progress.
    pub fade_with_progress: bool,
}

impl Default for Backdrop {
    /// Black at 45%, fading with progress.
    fn default() -> Self {
        Self {
            color: [0.0, 0.0, 0.0, 0.45],
            fade_with_progress: true,
        }
    }
}

/// How the content controls appear during a transition.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ContentReveal {
    /// Drawn only when open (a book whose pages appear once it has opened).
    WhenOpen,
    /// Drawn in every visible phase with alpha multiplied by progress.
    #[default]
    Fade,
    /// Drawn unchanged in every visible phase; the frame painter does the rest.
    Always,
}

/// Which user actions dismiss the top modal. Close controls are listed on
/// [`Modal`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DismissOn {
    pub outside_tap: bool,
    pub escape: bool,
    pub back: bool,
}

impl Default for DismissOn {
    /// Every action dismisses.
    fn default() -> Self {
        Self {
            outside_tap: true,
            escape: true,
            back: true,
        }
    }
}

/// One modal, built with the host for the stack entry it names: its content
/// controls, its frame and the rules for dismissing it.
pub struct Modal<'a> {
    state: Arc<ModalState>,
    id: Symbol,
    viewport: Rect,
    frame_rect: Rect,
    content_rect: Rect,
    frame: Box<dyn ModalFrame + 'a>,
    backdrop: Option<Backdrop>,
    dismiss: DismissOn,
    hold_open: bool,
    close_controls: Vec<Symbol>,
    reveal: ContentReveal,
    clip_content: bool,
    initial_focus: Option<ControlKey>,
    children: RefCell<ControlHost<'a>>,
}

impl<'a> Modal<'a> {
    /// A modal for the entry `id`, occupying `frame` with its controls laid
    /// out in `content`; its backdrop covers `viewport`.
    pub fn new(
        state: Arc<ModalState>,
        id: impl Into<Symbol>,
        viewport: Rect,
        frame: Rect,
        content: Rect,
    ) -> Self {
        Self {
            state,
            id: id.into(),
            viewport,
            frame_rect: frame,
            content_rect: content,
            frame: Box::new(PanelFrame::default()),
            backdrop: Some(Backdrop::default()),
            dismiss: DismissOn::default(),
            hold_open: false,
            close_controls: Vec::new(),
            reveal: ContentReveal::default(),
            clip_content: true,
            initial_focus: None,
            children: RefCell::new(ControlHost::builder().build()),
        }
    }

    /// A modal of `size` centred in `viewport`, its content inset by `padding`.
    pub fn centered(
        state: Arc<ModalState>,
        id: impl Into<Symbol>,
        viewport: Rect,
        size: [f32; 2],
        padding: f32,
    ) -> Self {
        let [cx, cy] = viewport.center();
        let frame = Rect::new(cx - size[0] * 0.5, cy - size[1] * 0.5, size[0], size[1]);
        Self::new(state, id, viewport, frame, frame.pad(-padding))
    }

    /// Draw with the app's frame instead of [`PanelFrame`].
    #[must_use]
    pub fn with_frame<F: ModalFrame + 'a>(mut self, frame: F) -> Self {
        self.frame = Box::new(frame);
        self
    }

    /// Set or remove the backdrop.
    #[must_use]
    pub fn with_backdrop(mut self, backdrop: Option<Backdrop>) -> Self {
        self.backdrop = backdrop;
        self
    }

    /// Choose which user actions dismiss it.
    #[must_use]
    pub fn with_dismiss(mut self, dismiss: DismissOn) -> Self {
        self.dismiss = dismiss;
        self
    }

    /// While true, user dismissals are refused and reported as
    /// [`ModalEvent::DismissRefused`]; the app's own dismissals still close it.
    #[must_use]
    pub fn with_hold_open(mut self, hold: bool) -> Self {
        self.hold_open = hold;
        self
    }

    /// Activating the content control named `id` dismisses the modal with
    /// [`DismissReason::Control`]. May be called more than once.
    #[must_use]
    pub fn with_close_control(mut self, id: impl Into<Symbol>) -> Self {
        self.close_controls.push(id.into());
        self
    }

    /// Choose how the content appears during a transition.
    #[must_use]
    pub fn with_reveal(mut self, reveal: ContentReveal) -> Self {
        self.reveal = reveal;
        self
    }

    /// Whether the content's first draw pass is clipped to the content rect.
    #[must_use]
    pub fn with_clip_content(mut self, clip: bool) -> Self {
        self.clip_content = clip;
        self
    }

    /// The content control to focus when the modal is pushed.
    #[must_use]
    pub fn with_initial_focus(mut self, key: ControlKey) -> Self {
        self.initial_focus = Some(key);
        self
    }

    /// Add a content control.
    #[must_use]
    pub fn with_control<C: Control + 'a>(mut self, control: ControlHandle<'a, C>) -> Self {
        self.children.get_mut().add_control(control);
        self
    }

    /// Add a named content control.
    #[must_use]
    pub fn with_named_control<C: Control + 'a>(
        mut self,
        id: impl Into<Symbol>,
        control: ControlHandle<'a, C>,
    ) -> Self {
        self.children.get_mut().add_named_control(id, control);
        self
    }

    /// Add a content control owned by the modal.
    #[must_use]
    pub fn with_custom_control<H: Control + 'a>(mut self, control: H) -> Self {
        self.children.get_mut().add_control(control);
        self
    }

    /// The stack entry this modal is built for.
    pub fn id(&self) -> Symbol {
        self.id
    }

    /// The stack it belongs to.
    pub fn state(&self) -> &Arc<ModalState> {
        &self.state
    }

    /// The area it visibly occupies.
    pub fn frame_rect(&self) -> Rect {
        self.frame_rect
    }

    /// The area its controls are laid out in.
    pub fn content_rect(&self) -> Rect {
        self.content_rect
    }

    /// Where the modal is now; `None` when the state has no entry for it.
    pub fn view(&self) -> Option<ModalView> {
        self.state.read(|inner| {
            let depth = inner.find(self.id)?;
            let entry = &inner.entries[depth];
            Some(ModalView {
                id: self.id,
                phase: entry.phase,
                progress: entry.progress,
                viewport: self.viewport,
                frame: self.frame_rect,
                content: self.content_rect,
                depth,
                is_top: inner.top_index() == Some(depth),
            })
        })
    }

    /// Route one event to this modal as the top, with the modifiers the
    /// outer host saw: Escape and Back dismiss it, a tap outside its frame
    /// dismisses it, and its content gets the rest while it is open.
    /// `ModalStack` calls it; public so a modal can be tested on its own.
    pub fn deliver_event(&self, event: InputEvent, modifiers: Modifiers) -> EventResponse {
        let consumed = EventResponse::consumed(None, None);
        let Some(view) = self.view() else {
            return consumed;
        };
        let mut children = self.children.borrow_mut();
        self.load(&mut children);
        children.set_modifiers(modifiers);
        let response = self.route(&mut children, &view, event);
        let (focus, drag, hover) = (
            children.focused_key(),
            children.active_drag(),
            children.hovered_key(),
        );
        self.with_entry(|entry| {
            entry.focus = focus;
            entry.drag = drag;
            entry.hover = hover;
        });
        response.unwrap_or(consumed)
    }

    /// Tell the hovered content control, if any, that the pointer left, and
    /// record no hover for this entry. `ModalStack` calls it on every modal
    /// that is not the top.
    pub fn clear_hover(&self) {
        let Some(hover) = self
            .state
            .read(|inner| inner.find(self.id).map(|i| inner.entries[i].hover))
        else {
            return;
        };
        let mut children = self.children.borrow_mut();
        children.set_hovered_key(hover);
        children.clear_hover();
        self.with_entry(|entry| entry.hover = None);
    }

    /// Change this modal's entry under the state's lock, if it has one.
    fn with_entry(&self, change: impl FnOnce(&mut ModalEntry)) {
        self.state.change(|inner| {
            if let Some(index) = inner.find(self.id) {
                change(&mut inner.entries[index]);
            }
        });
    }

    /// Put the content host's focus, drag and hover back as the entry
    /// remembers them; the first time after a push, use the initial focus.
    fn load(&self, children: &mut ControlHost<'_>) {
        let initial = self.initial_focus;
        let mut carried = None;
        self.state.change(|inner| {
            if let Some(index) = inner.find(self.id) {
                let entry = &mut inner.entries[index];
                if !entry.focus_seeded {
                    entry.focus_seeded = true;
                    entry.focus = initial;
                }
                carried = Some((entry.focus, entry.drag, entry.hover));
            }
        });
        if let Some((focus, drag, hover)) = carried {
            children.set_focused_key(focus);
            children.set_active_drag(drag);
            children.set_hovered_key(hover);
        }
    }

    /// A dismissal the user asked for: refused and reported while held open.
    fn request(&self, by: DismissReason) {
        if self.hold_open {
            let id = self.id;
            self.state
                .change(|inner| inner.events.push(ModalEvent::DismissRefused { id, by }));
        } else {
            self.state.dismiss(by);
        }
    }

    /// The modal's handling of one event; `None` means consumed.
    fn route(
        &self,
        children: &mut ControlHost<'_>,
        view: &ModalView,
        event: InputEvent,
    ) -> Option<EventResponse> {
        if let InputEvent::Keyboard(key_event) = event
            && key_event.is_down()
            && matches!(view.phase, ModalPhase::Opening | ModalPhase::Open)
        {
            match key_event.key() {
                Some(Key::Escape) if self.dismiss.escape => {
                    self.request(DismissReason::Escape);
                    return None;
                }
                Some(Key::Back) if self.dismiss.back => {
                    self.request(DismissReason::Back);
                    return None;
                }
                _ => {}
            }
        }
        if view.phase != ModalPhase::Open {
            return None;
        }
        if let InputEvent::Mouse(mouse) = event
            && children.active_drag().is_none()
            && self.tap_outside(view, mouse)
        {
            return None;
        }
        let response = children.deliver_event(event);
        if let (Some(id), Some(ControlAction::Activated { .. })) =
            (response.target_id, &response.action)
            && self.close_controls.contains(&id)
        {
            self.request(DismissReason::Control(id));
        }
        Some(response)
    }

    /// Handle a pointer event that concerns the area outside the frame.
    /// True when the event is spent here and must not reach the content.
    fn tap_outside(&self, view: &ModalView, mouse: MouseEvent) -> bool {
        let outside = !self.frame.contains(mouse.pos(), view);
        let pressed = self.state.read(|inner| {
            inner
                .find(self.id)
                .and_then(|i| inner.entries[i].outside_press)
        });
        match mouse {
            MouseEvent::Down { button, .. } if outside => {
                self.with_entry(|entry| entry.outside_press = Some(button));
                true
            }
            MouseEvent::Up { button, .. } if pressed == Some(button) => {
                self.with_entry(|entry| entry.outside_press = None);
                if outside && self.dismiss.outside_tap {
                    self.request(DismissReason::OutsideTap);
                }
                true
            }
            MouseEvent::Click { .. } if outside && pressed.is_none() => {
                if self.dismiss.outside_tap {
                    self.request(DismissReason::OutsideTap);
                }
                true
            }
            _ => outside,
        }
    }
}

impl DrawControl for Modal<'_> {
    /// Backdrop, frame, content, content overlays (top only), then
    /// frame-over-content. Draws nothing when the state has no entry for it.
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let Some(view) = self.view() else {
            return;
        };
        if let Some(backdrop) = self.backdrop {
            let by = if backdrop.fade_with_progress {
                view.progress
            } else {
                1.0
            };
            list.push(SdfInstance {
                kind: SdfKind::Box,
                position: self.viewport.pos(),
                size: self.viewport.size(),
                color: faded(backdrop.color, by),
                anim: 0,
            });
        }
        self.frame.draw_frame(list, ctx, &view);
        if self.reveal != ContentReveal::WhenOpen || view.phase == ModalPhase::Open {
            let start = list.instances.len();
            let children = self.children.borrow();
            if self.clip_content {
                list.push_clip(self.content_rect.pos(), self.content_rect.size());
            }
            children.draw_controls(list, ctx);
            if self.clip_content {
                list.push_clip_end();
            }
            if view.is_top {
                children.draw_control_overlays(list, ctx);
            }
            if self.reveal == ContentReveal::Fade {
                for instance in &mut list.instances[start..] {
                    instance.color[3] *= view.progress;
                }
            }
        }
        self.frame.draw_over_content(list, ctx, &view);
    }
}

/// Every modal on the stack, as one hosted control. Added to the host by
/// name, last, so it draws above every base control; never registered as a
/// filter. It is reached only through the [`ModalFilter`]'s redirect, and
/// hands each event to the top modal.
pub struct ModalStack<'a> {
    state: Arc<ModalState>,
    viewport: Rect,
    modals: Vec<Modal<'a>>,
}

impl<'a> ModalStack<'a> {
    /// An empty stack control for `state`, drawing over `viewport`.
    pub fn new(state: Arc<ModalState>, viewport: Rect) -> Self {
        Self {
            state,
            viewport,
            modals: Vec::new(),
        }
    }

    /// Add the modal built for one stack entry; a second modal for the same
    /// id replaces the first.
    #[must_use]
    pub fn with_modal(mut self, modal: Modal<'a>) -> Self {
        self.modals.retain(|m| m.id != modal.id);
        self.modals.push(modal);
        self
    }

    /// The stack it draws and routes for.
    pub fn state(&self) -> &Arc<ModalState> {
        &self.state
    }

    /// The modals built for this host build, in the order they were added.
    pub fn modals(&self) -> &[Modal<'a>] {
        &self.modals
    }

    /// The modal built for the top entry, if one was.
    pub fn top_modal(&self) -> Option<&Modal<'a>> {
        let top = self.state.top()?;
        self.modals.iter().find(|m| m.id == top)
    }

    /// A handle for registering this stack with a host.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }

    fn to_top(&self, event: InputEvent, modifiers: Modifiers) -> Option<ControlAction> {
        self.top_modal()?.deliver_event(event, modifiers).action
    }
}

impl HostedControl for ModalStack<'_> {
    fn bounds(&self) -> Rect {
        self.viewport
    }

    /// Never hit: the stack takes input only by redirect.
    fn hit_test(&self, _pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        None
    }

    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction> {
        let top = self.state.top();
        for modal in self.modals.iter().filter(|m| Some(m.id) != top) {
            modal.clear_hover();
        }
        self.to_top(InputEvent::Mouse(*event), ctx.modifiers)
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.to_top(InputEvent::Keyboard(*event), ctx.modifiers)
    }

    fn handle_gesture(&self, event: &GestureEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.to_top(InputEvent::Gesture(*event), ctx.modifiers)
    }

    /// Tab in the base host never stops on the stack.
    fn accepts_focus(&self) -> bool {
        false
    }
}

impl DrawControl for ModalStack<'_> {
    fn draw(&self, _list: &mut DrawList, _ctx: &DrawContext) {}

    /// Every entry's modal, bottom to top, after every base control's overlay.
    fn draw_overlay(&self, list: &mut DrawList, ctx: &DrawContext) {
        for entry in self.state.stack() {
            if let Some(modal) = self.modals.iter().find(|m| m.id == entry.id) {
                modal.draw(list, ctx);
            }
        }
    }
}
