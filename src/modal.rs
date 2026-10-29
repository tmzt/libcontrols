//! Modals: a persistent stack of modal layers that trap input while open.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use libmsdf::core::sdf::{
    DRAW_TYPE_MSDF_TEXT, DRAW_TYPE_TEXT, DRAW_TYPE_TEXTURE, sdf_eval_with_params,
};
use libmsdf::drawlist::{DrawList, SdfFrame, SdfInstance, SdfKind};

use crate::control::{Control, HostedControl};
use crate::event::{
    ControlAction, ControlPart, EventContext, GestureEvent, InputEvent, Key, KeyboardEvent,
    Modifiers, MouseButton, MouseEvent,
};
use crate::filter::{FilterContext, Filtered, InputFilter};
use crate::flags::NodeFlags;
use crate::rect::Rect;
use crate::router::{EventResponse as RouterResponse, NodeId, Router};
use crate::symbol::Symbol;
use crate::traits::{DrawContext, DrawControl};

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
    /// A close control of a `ModalDialog` was activated; carries its node in
    /// the dialog's content router.
    Control(NodeId),
    /// The modal asked to close itself (`ControlAction::Dismiss` with no
    /// control).
    Requested,
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
    /// Which push made this entry, so a layer can tell its own entry from a
    /// later one with the same id.
    pub(crate) serial: u64,
}

/// What a push did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum PushOutcome {
    /// A new entry, with its serial.
    Opened(u64),
    /// Queued behind an entry with the same id that is still closing.
    Queued,
    /// The id is already open.
    Refused,
}

#[derive(Debug, Default)]
pub(crate) struct ModalInner {
    default_transition: ModalTransition,
    pub(crate) entries: Vec<ModalEntry>,
    pending: Vec<(Symbol, ModalTransition)>,
    pub(crate) events: Vec<ModalEvent>,
    next_serial: u64,
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

    fn push(&mut self, id: Symbol, transition: ModalTransition) -> PushOutcome {
        if self.is_live(id) {
            return PushOutcome::Refused;
        }
        if self.entries.iter().any(|e| e.id == id) {
            self.pending.retain(|(p, _)| *p != id);
            self.pending.push((id, transition));
            return PushOutcome::Queued;
        }
        let open = transition.opens_at_once();
        self.next_serial += 1;
        let serial = self.next_serial;
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
            serial,
        });
        if open {
            self.events.push(ModalEvent::Opened { id });
        }
        PushOutcome::Opened(serial)
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
        if progress.is_nan() {
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

    /// The transition pushes use when none is given.
    pub fn default_transition(&self) -> ModalTransition {
        self.read(|inner| inner.default_transition)
    }

    /// Open `id` over the top with `transition`. Pushing goes through
    /// `Router::push_modal`, so that every entry has a layer.
    pub(crate) fn push_with(&self, id: Symbol, transition: ModalTransition) -> PushOutcome {
        self.change(|inner| inner.push(id, transition))
    }

    /// Close the top (reported as `Replaced`) and open `id` over it at once.
    pub(crate) fn replace_top_with(&self, id: Symbol, transition: ModalTransition) -> PushOutcome {
        self.change(|inner| {
            if inner.is_live(id) {
                return PushOutcome::Refused;
            }
            if let Some(top) = inner.top_index() {
                inner.begin_close(top, DismissReason::Replaced);
            }
            inner.push(id, transition)
        })
    }

    /// Close the live entry `id`, wherever it is, with `by`. A request from a
    /// layer's filter or its modal: it closes that layer, not whatever is top.
    pub(crate) fn dismiss_entry(&self, id: Symbol, by: DismissReason) {
        self.change(|inner| {
            if let Some(index) = inner
                .entries
                .iter()
                .position(|e| e.id == id && e.phase != ModalPhase::Closing)
            {
                inner.begin_close(index, by);
            }
        });
    }

    /// Queue an event (a refused dismissal).
    pub(crate) fn push_event(&self, event: ModalEvent) {
        self.change(|inner| inner.events.push(event));
    }

    /// Read the entries under the lock.
    pub(crate) fn read_entries<R>(&self, read: impl FnOnce(&ModalInner) -> R) -> R {
        self.read(read)
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

/// Where a layer is, for the modal drawing or handling an event: set by the
/// stack in `DrawContext::layer` and `EventContext::layer`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LayerView {
    /// The stack entry.
    pub id: Symbol,
    pub phase: ModalPhase,
    /// 0.0 closed .. 1.0 open.
    pub progress: f32,
    /// 0 is the bottom of the stack.
    pub depth: usize,
    pub is_top: bool,
}

/// How a pushed modal behaves on the stack.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ModalOptions {
    /// The dim drawn under it, over everything beneath.
    pub backdrop: Option<Backdrop>,
    /// Which user actions dismiss it.
    pub dismiss: DismissOn,
    /// While true, user dismissals are refused and reported as
    /// `ModalEvent::DismissRefused`; the app's own dismissals still close it.
    pub hold_open: bool,
}

impl Default for ModalOptions {
    fn default() -> Self {
        Self {
            backdrop: Some(Backdrop::default()),
            dismiss: DismissOn::default(),
            hold_open: false,
        }
    }
}

impl ModalOptions {
    /// The same options with this backdrop.
    #[must_use]
    pub fn with_backdrop(mut self, backdrop: Option<Backdrop>) -> Self {
        self.backdrop = backdrop;
        self
    }

    /// The same options with these dismiss rules.
    #[must_use]
    pub fn with_dismiss(mut self, dismiss: DismissOn) -> Self {
        self.dismiss = dismiss;
        self
    }

    /// The same options, held open or not.
    #[must_use]
    pub fn with_hold_open(mut self, hold: bool) -> Self {
        self.hold_open = hold;
        self
    }
}

/// What a layer filter needs to dismiss its layer.
#[derive(Clone, Debug)]
pub struct LayerContext {
    pub state: Arc<ModalState>,
    /// The entry.
    pub id: Symbol,
    pub options: ModalOptions,
}

impl LayerContext {
    /// Ask for this layer to close: refused and reported while held open.
    pub fn request(&self, by: DismissReason) {
        if self.options.hold_open {
            self.state
                .push_event(ModalEvent::DismissRefused { id: self.id, by });
        } else {
            self.state.dismiss_entry(self.id, by);
        }
    }
}

/// Anything that can be pushed on a `ModalStack`: a dialog, a self-drawn
/// menu, a shaped window. A modal draws itself; while it is on the stack its
/// contexts carry a `LayerView` to animate from.
pub trait Modal: Control + Any {
    /// Backdrop, dismiss rules, hold-open.
    fn options(&self) -> ModalOptions {
        ModalOptions::default()
    }

    /// The filter this layer is asked through while it is on top. The
    /// default blocks (or, by its options, dismisses on) pointer events
    /// outside the control's bounds; a shaped modal returns its own.
    fn filter_layer(&self, layer: &LayerContext) -> Box<dyn LayerFilter> {
        Box::new(ModalLayerFilter::new(self.bounds(), layer.clone()))
    }

    /// Another layer was pushed over this one: whatever it was doing with the
    /// pointer (hover, a drag in its content) is over. Default: `set_hover(None)`.
    fn covered(&self) {
        self.set_hover(None);
    }
}

/// A filter a layer owns. It may look at what its layer drew each frame, so
/// its shape can follow the layer's animation.
pub trait LayerFilter: InputFilter {
    /// Called by the stack after it draws this filter's layer, with the list
    /// and the range of instances the layer pushed (its backdrop excluded).
    fn drawn(&self, _list: &DrawList, _range: Range<usize>) {}

    /// Another layer was pushed over this one: forget any press in progress.
    fn reset(&self) {}
}

/// What is inside a layer, for its filter.
pub trait LayerShape {
    fn contains(&self, pos: [f32; 2]) -> bool;

    /// See `LayerFilter::drawn`.
    fn drawn(&self, _list: &DrawList, _range: Range<usize>) {}
}

impl LayerShape for Rect {
    fn contains(&self, pos: [f32; 2]) -> bool {
        Rect::contains(self, pos)
    }
}

/// The standard layer filter: inside its shape (or while a press that began
/// inside is held) events go to the modal; outside they are blocked, and an
/// outside tap dismisses if the options say so. Escape and Back dismiss.
pub struct ModalLayerFilter<S: LayerShape> {
    shape: S,
    layer: LayerContext,
    press_inside: Cell<Option<MouseButton>>,
    press_outside: Cell<Option<MouseButton>>,
    tapped_outside: Cell<bool>,
}

impl<S: LayerShape> ModalLayerFilter<S> {
    pub fn new(shape: S, layer: LayerContext) -> Self {
        Self {
            shape,
            layer,
            press_inside: Cell::new(None),
            press_outside: Cell::new(None),
            tapped_outside: Cell::new(false),
        }
    }

    /// The shape it tests.
    pub fn shape(&self) -> &S {
        &self.shape
    }

    fn mouse(&self, event: &MouseEvent) -> Filtered {
        if let Some(button) = self.press_inside.get() {
            if matches!(*event, MouseEvent::Up { button: b, .. } if b == button) {
                self.press_inside.set(None);
            }
            return Filtered::Pass;
        }
        let inside = self.shape.contains(event.pos());
        let outside_tap = self.layer.options.dismiss.outside_tap;
        match *event {
            MouseEvent::Down { button, .. } => {
                self.tapped_outside.set(false);
                if inside {
                    self.press_inside.set(Some(button));
                    Filtered::Pass
                } else {
                    self.press_outside.set(Some(button));
                    Filtered::Consumed(None)
                }
            }
            MouseEvent::Up { button, .. } if self.press_outside.get() == Some(button) => {
                self.press_outside.set(None);
                if !inside {
                    self.tapped_outside.set(true);
                    if outside_tap {
                        self.layer.request(DismissReason::OutsideTap);
                    }
                }
                Filtered::Consumed(None)
            }
            MouseEvent::Click { .. } if !inside => {
                // A shell that sends Down, Up and Click has already tapped.
                if !self.tapped_outside.replace(false)
                    && self.press_outside.get().is_none()
                    && outside_tap
                {
                    self.layer.request(DismissReason::OutsideTap);
                }
                Filtered::Consumed(None)
            }
            _ if inside => Filtered::Pass,
            _ => Filtered::Consumed(None),
        }
    }
}

impl<S: LayerShape> InputFilter for ModalLayerFilter<S> {
    fn filter(&self, event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        match event {
            InputEvent::Keyboard(key) if key.is_down() => {
                let dismiss = self.layer.options.dismiss;
                match key.key() {
                    Some(Key::Escape) if dismiss.escape => {
                        self.layer.request(DismissReason::Escape);
                        Filtered::Consumed(None)
                    }
                    Some(Key::Back) if dismiss.back => {
                        self.layer.request(DismissReason::Back);
                        Filtered::Consumed(None)
                    }
                    _ => Filtered::Pass,
                }
            }
            InputEvent::Keyboard(_) | InputEvent::Gesture(_) => Filtered::Pass,
            InputEvent::Mouse(mouse) => self.mouse(mouse),
            InputEvent::Modifiers(_) => Filtered::Consumed(None),
        }
    }
}

impl<S: LayerShape> LayerFilter for ModalLayerFilter<S> {
    fn drawn(&self, list: &DrawList, range: Range<usize>) {
        self.shape.drawn(list, range);
    }

    fn reset(&self) {
        self.press_inside.set(None);
        self.press_outside.set(None);
        self.tapped_outside.set(false);
    }
}

/// A layer filter whose inside is the shape the layer draws, or an alpha
/// mask.
pub type ShapedLayerFilter = ModalLayerFilter<LayerMask>;

enum MaskKind {
    Drawn {
        inside: f32,
        frame: RefCell<Option<SdfFrame>>,
    },
    Texture {
        rect: Rect,
        width: u32,
        height: u32,
        alpha: Arc<[u8]>,
        threshold: u8,
    },
}

/// The shape of a `ShapedLayerFilter`.
pub struct LayerMask(MaskKind);

impl LayerMask {
    /// The layer's own SDF output. Each frame the instances it drew are
    /// lowered and kept; a point is inside where any filled shape among them
    /// has distance <= `inside`, evaluated on the CPU with the shader's math.
    /// Text glyphs and textures do not count as shape.
    pub fn drawn(inside: f32) -> Self {
        Self(MaskKind::Drawn {
            inside,
            frame: RefCell::new(None),
        })
    }

    /// A CPU copy of the alpha texture drawn for the layer, `width` by
    /// `height`, stretched over `rect`: inside where the alpha is at least
    /// `threshold`. Outside `rect` is outside.
    pub fn texture(rect: Rect, width: u32, height: u32, alpha: Arc<[u8]>, threshold: u8) -> Self {
        Self(MaskKind::Texture {
            rect,
            width,
            height,
            alpha,
            threshold,
        })
    }
}

impl LayerShape for LayerMask {
    fn contains(&self, pos: [f32; 2]) -> bool {
        match &self.0 {
            MaskKind::Drawn { inside, frame } => frame.borrow().as_ref().is_some_and(|f| {
                f.draws.iter().any(|cmd| {
                    let ty = cmd.draw_type();
                    if ty == DRAW_TYPE_TEXT || ty == DRAW_TYPE_TEXTURE || ty == DRAW_TYPE_MSDF_TEXT
                    {
                        return false;
                    }
                    let (distance, _) = sdf_eval_with_params(cmd, pos[0], pos[1], &f.param_bank);
                    distance <= *inside
                })
            }),
            MaskKind::Texture {
                rect,
                width,
                height,
                alpha,
                threshold,
            } => {
                if !rect.contains(pos)
                    || *width == 0
                    || *height == 0
                    || rect.w <= 0.0
                    || rect.h <= 0.0
                {
                    return false;
                }
                let u = ((pos[0] - rect.x) / rect.w * *width as f32) as u32;
                let v = ((pos[1] - rect.y) / rect.h * *height as f32) as u32;
                let (u, v) = (u.min(width - 1), v.min(height - 1));
                alpha
                    .get(v as usize * *width as usize + u as usize)
                    .is_some_and(|&a| a >= *threshold)
            }
        }
    }

    fn drawn(&self, list: &DrawList, range: Range<usize>) {
        if let MaskKind::Drawn { frame, .. } = &self.0 {
            let mut own = DrawList::new();
            for instance in list.instances.get(range).unwrap_or(&[]) {
                own.push(*instance);
            }
            *frame.borrow_mut() = Some(own.lower());
        }
    }
}

/// The built-in modal: a dialog with an app-drawn frame and a content router.
pub struct ModalDialog {
    viewport: Rect,
    frame_rect: Rect,
    content_rect: Rect,
    frame: Box<dyn ModalFrame>,
    options: ModalOptions,
    close_controls: Vec<NodeId>,
    reveal: ContentReveal,
    clip_content: bool,
    initial_focus: Option<NodeId>,
    focus_seeded: Cell<bool>,
    content: Router,
}

impl ModalDialog {
    /// A dialog occupying `frame`, its controls laid out in `content`, its
    /// backdrop over `viewport`.
    pub fn new(viewport: Rect, frame: Rect, content: Rect) -> Self {
        Self {
            viewport,
            frame_rect: frame,
            content_rect: content,
            frame: Box::new(PanelFrame::default()),
            options: ModalOptions::default(),
            close_controls: Vec::new(),
            reveal: ContentReveal::default(),
            clip_content: true,
            initial_focus: None,
            focus_seeded: Cell::new(false),
            content: Router::new(),
        }
    }

    /// A dialog of `size` centred in `viewport`, its content inset by `padding`.
    pub fn centered(viewport: Rect, size: [f32; 2], padding: f32) -> Self {
        let [cx, cy] = viewport.center();
        let frame = Rect::new(cx - size[0] * 0.5, cy - size[1] * 0.5, size[0], size[1]);
        Self::new(viewport, frame, frame.pad(-padding))
    }

    /// Draw with the app's frame instead of `PanelFrame`.
    #[must_use]
    pub fn with_frame(mut self, frame: impl ModalFrame + 'static) -> Self {
        self.frame = Box::new(frame);
        self
    }

    /// Set its options.
    #[must_use]
    pub fn with_options(mut self, options: ModalOptions) -> Self {
        self.options = options;
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

    /// Activating this node of `content()` dismisses the dialog with
    /// `DismissReason::Control(id)`.
    #[must_use]
    pub fn with_close_control(mut self, id: NodeId) -> Self {
        self.close_controls.push(id);
        self
    }

    /// The node of `content()` focused the first time the dialog takes input.
    #[must_use]
    pub fn with_initial_focus(mut self, id: NodeId) -> Self {
        self.initial_focus = Some(id);
        self
    }

    /// The dialog's controls.
    pub fn content(&self) -> &Router {
        &self.content
    }

    /// The dialog's controls, to add and edit.
    pub fn content_mut(&mut self) -> &mut Router {
        &mut self.content
    }

    /// The area it occupies.
    pub fn frame_rect(&self) -> Rect {
        self.frame_rect
    }

    /// The area its controls are laid out in.
    pub fn content_rect(&self) -> Rect {
        self.content_rect
    }

    fn view(&self, layer: Option<LayerView>) -> ModalView {
        let layer = layer.unwrap_or(LayerView {
            id: Symbol::intern(""),
            phase: ModalPhase::Open,
            progress: 1.0,
            depth: 0,
            is_top: true,
        });
        ModalView {
            id: layer.id,
            phase: layer.phase,
            progress: layer.progress,
            viewport: self.viewport,
            frame: self.frame_rect,
            content: self.content_rect,
            depth: layer.depth,
            is_top: layer.is_top,
        }
    }

    /// Apply the initial focus once, the first time the dialog takes input.
    fn seed_focus(&self) {
        if !self.focus_seeded.replace(true) && self.initial_focus.is_some() {
            self.content.set_focus(self.initial_focus);
        }
    }

    /// Turn an inner response into the dialog's answer: a close control's
    /// activation becomes a dismissal.
    fn answer(&self, response: RouterResponse) -> Option<ControlAction> {
        if let (Some(target), Some(ControlAction::Activated { .. })) =
            (response.target, &response.action)
            && self.close_controls.contains(&target)
        {
            return Some(ControlAction::Dismiss {
                control: Some(target),
            });
        }
        response.action
    }
}

impl DrawControl for ModalDialog {
    /// Frame, content (clipped, revealed by its phase), the top's content
    /// overlays, then frame-over-content.
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let view = self.view(ctx.layer);
        self.frame.draw_frame(list, ctx, &view);
        if self.reveal != ContentReveal::WhenOpen || view.phase == ModalPhase::Open {
            let start = list.instances.len();
            if self.clip_content {
                list.push_clip(self.content_rect.pos(), self.content_rect.size());
            }
            self.content.draw(list, ctx);
            if self.clip_content {
                list.push_clip_end();
            }
            if view.is_top {
                self.content.draw_overlay(list, ctx);
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

impl HostedControl for ModalDialog {
    fn bounds(&self) -> Rect {
        self.frame_rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.frame_rect
            .contains(pos)
            .then_some((ControlPart::Body, 0.0))
    }

    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.seed_focus();
        self.content.set_modifiers(ctx.modifiers);
        self.answer(self.content.deliver_mouse(*event))
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.seed_focus();
        self.content.set_modifiers(ctx.modifiers);
        self.answer(self.content.deliver_keyboard(*event))
    }

    fn handle_gesture(&self, event: &GestureEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.seed_focus();
        self.content.set_modifiers(ctx.modifiers);
        self.answer(self.content.deliver_gesture(*event))
    }

    fn set_hover(&self, hover: Option<ControlPart>) {
        if hover.is_none() {
            self.content.clear_hover();
        }
    }
}

impl Modal for ModalDialog {
    fn options(&self) -> ModalOptions {
        self.options
    }

    fn covered(&self) {
        self.content.clear_hover();
        self.content.cancel_grab();
    }
}

/// Why a modal could not be pushed or the stack installed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModalError {
    /// `push_modal` on a router with no installed `ModalStack`.
    NoStack,
    /// `ModalStack::install` twice in one router.
    AlreadyInstalled,
    /// `push_modal` with an id that is open.
    AlreadyOpen,
}

impl std::fmt::Display for ModalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NoStack => "no modal stack is installed in this router",
            Self::AlreadyInstalled => "this router already has a modal stack",
            Self::AlreadyOpen => "a modal with this id is open",
        })
    }
}

impl std::error::Error for ModalError {}

struct Layer {
    id: Symbol,
    serial: u64,
    modal: Box<dyn Modal>,
    options: ModalOptions,
    filter: Box<dyn LayerFilter>,
}

/// The z-order stack of modal layers. Installed once in a root router, it
/// owns its layers; each layer owns its filter, so popping one is a drop.
pub struct ModalStack {
    state: Arc<ModalState>,
    viewport: Rect,
    layers: RefCell<Vec<Layer>>,
    pending: RefCell<Vec<(Symbol, Box<dyn Modal>)>>,
    /// Modals attached under an entry id, made into a layer whenever that
    /// entry is pushed on the state alone (a `PopupMenu`'s `open`).
    attached: RefCell<Vec<(Symbol, AttachedModal)>>,
}

/// Makes the modal of an attached entry's layer.
pub(crate) type AttachedModal = Box<dyn Fn() -> Box<dyn Modal>>;

impl ModalStack {
    /// The alias the stack is installed under.
    pub const NAME: &'static str = "libcontrols/ModalStack";

    /// Add a stack to `router` as a root named `NAME`, kept above every app
    /// root. Its id is returned.
    pub fn install(
        router: &mut Router,
        state: Arc<ModalState>,
        viewport: Rect,
    ) -> Result<NodeId, ModalError> {
        let stack = Self {
            state,
            viewport,
            layers: RefCell::new(Vec::new()),
            pending: RefCell::new(Vec::new()),
            attached: RefCell::new(Vec::new()),
        };
        router
            .add_reserved(Symbol::intern(Self::NAME), Box::new(stack))
            .map_err(|_| ModalError::AlreadyInstalled)
    }

    /// The shared state.
    pub fn state(&self) -> &Arc<ModalState> {
        &self.state
    }

    /// How many layers the stack holds (closing ones included).
    pub fn len(&self) -> usize {
        self.sync();
        self.layers.borrow().len()
    }

    /// Whether the stack holds no layer.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The modal of the live layer `id`, if it is a `T`.
    pub fn modal<T: Modal>(&self, id: impl Into<Symbol>) -> Option<std::cell::Ref<'_, T>> {
        let id = id.into();
        self.sync();
        let layers = self.layers.borrow();
        let index = layers.iter().rposition(|l| l.id == id)?;
        std::cell::Ref::filter_map(layers, |l| {
            let any: &dyn Any = l[index].modal.as_ref();
            any.downcast_ref::<T>()
        })
        .ok()
    }

    /// The modal of the live layer `id`, mutably, if it is a `T`.
    pub fn modal_mut<T: Modal>(&mut self, id: impl Into<Symbol>) -> Option<&mut T> {
        let id = id.into();
        self.sync();
        let layers = self.layers.get_mut();
        let layer = layers.iter_mut().rev().find(|l| l.id == id)?;
        let any: &mut dyn Any = layer.modal.as_mut();
        any.downcast_mut::<T>()
    }

    /// Attach `make` under `id`: whenever an entry `id` is live with no
    /// layer and no queued push, its layer's modal is `make()`. Attaching
    /// an id again replaces it.
    pub(crate) fn attach(&self, id: Symbol, make: AttachedModal) {
        let mut attached = self.attached.borrow_mut();
        attached.retain(|(a, _)| *a != id);
        attached.push((id, make));
    }

    /// A new modal for the attached entry `id`.
    fn make_attached(&self, id: Symbol) -> Option<Box<dyn Modal>> {
        let attached = self.attached.try_borrow().ok()?;
        attached
            .iter()
            .find(|(a, _)| *a == id)
            .map(|(_, make)| make())
    }

    fn make_layer(&self, id: Symbol, serial: u64, modal: Box<dyn Modal>) -> Layer {
        let options = modal.options();
        let layer = LayerContext {
            state: Arc::clone(&self.state),
            id,
            options,
        };
        let filter = modal.filter_layer(&layer);
        // A new top: the layers below lose the pointer, and any press or drag
        // they had in progress.
        for below in self.layers.borrow().iter() {
            below.modal.covered();
            below.filter.reset();
        }
        Layer {
            id,
            serial,
            modal,
            options,
            filter,
        }
    }

    fn push(&self, id: Symbol, modal: Box<dyn Modal>, outcome: PushOutcome) {
        match outcome {
            PushOutcome::Opened(serial) => {
                let layer = self.make_layer(id, serial, modal);
                self.layers.borrow_mut().push(layer);
            }
            PushOutcome::Queued => {
                let mut pending = self.pending.borrow_mut();
                pending.retain(|(p, _)| *p != id);
                pending.push((id, modal));
            }
            PushOutcome::Refused => {}
        }
        self.sync();
    }

    /// Drop the layers whose entry has finished closing, and make the
    /// layers of queued pushes whose entry now exists.
    fn sync(&self) {
        let (live, queued): (Vec<(Symbol, u64)>, Vec<Symbol>) = self.state.read_entries(|inner| {
            (
                inner.entries.iter().map(|e| (e.id, e.serial)).collect(),
                inner.pending.iter().map(|(id, _)| *id).collect(),
            )
        });
        let Ok(mut layers) = self.layers.try_borrow_mut() else {
            return;
        };
        // Closed layers are moved out and dropped after the borrow ends, so a
        // modal whose Drop asks the stack anything finds it free.
        let (keep, closed): (Vec<Layer>, Vec<Layer>) = std::mem::take(&mut *layers)
            .into_iter()
            .partition(|l| live.iter().any(|&(_, s)| s == l.serial));
        *layers = keep;
        drop(layers);
        drop(closed);
        let Ok(layers) = self.layers.try_borrow_mut() else {
            return;
        };
        let missing: Vec<(Symbol, u64)> = live
            .iter()
            .filter(|&&(_, s)| !layers.iter().any(|l| l.serial == s))
            .copied()
            .collect();
        drop(layers);
        for (id, serial) in missing {
            let queued = {
                let mut pending = self.pending.borrow_mut();
                pending
                    .iter()
                    .position(|(p, _)| *p == id)
                    .map(|i| pending.remove(i).1)
            };
            if let Some(modal) = queued.or_else(|| self.make_attached(id)) {
                let layer = self.make_layer(id, serial, modal);
                let mut layers = self.layers.borrow_mut();
                let at = live
                    .iter()
                    .position(|&(_, s)| s == serial)
                    .map_or(layers.len(), |p| p.min(layers.len()));
                layers.insert(at, layer);
            }
        }
        // A queued push the state no longer has (dismiss_all cleared it)
        // drops its modal.
        let cancelled: Vec<(Symbol, Box<dyn Modal>)> = match self.pending.try_borrow_mut() {
            Ok(mut pending) => {
                let (keep, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut *pending)
                    .into_iter()
                    .partition(|(id, _)| queued.contains(id));
                *pending = keep;
                gone
            }
            Err(_) => Vec::new(),
        };
        drop(cancelled);
    }

    /// The serial of the top entry, while any entry exists; `Some(0)` when
    /// every entry is closing (input is still the stack's).
    pub(crate) fn top_serial(&self) -> Option<u64> {
        if !self.state.captures_input() {
            return None;
        }
        self.state
            .read_entries(|inner| Some(inner.top_index().map_or(0, |i| inner.entries[i].serial)))
    }

    /// Each layer's view, bottom to top, by serial.
    fn views(&self) -> Vec<(u64, LayerView)> {
        self.state.read_entries(|inner| {
            let top = inner.top_index();
            inner
                .entries
                .iter()
                .enumerate()
                .map(|(depth, e)| {
                    (
                        e.serial,
                        LayerView {
                            id: e.id,
                            phase: e.phase,
                            progress: e.progress,
                            depth,
                            is_top: top == Some(depth),
                        },
                    )
                })
                .collect()
        })
    }

    /// The layer chain: `None` when the stack is empty (route as usual);
    /// otherwise the top layer's answer, and nothing beneath sees the event.
    pub(crate) fn route(
        &self,
        event: InputEvent,
        filter_ctx: &FilterContext,
        modifiers: Modifiers,
    ) -> Option<LayerOutcome> {
        self.sync();
        if !self.state.captures_input() {
            return None;
        }
        let views = self.views();
        let Some((serial, view)) = views.iter().copied().find(|(_, v)| v.is_top) else {
            return Some(LayerOutcome::Consumed(None));
        };
        let layers = self.layers.borrow();
        let Some(layer) = layers.iter().find(|l| l.serial == serial) else {
            return Some(LayerOutcome::Consumed(None));
        };
        match layer.filter.filter(&event, filter_ctx) {
            Filtered::Consumed(action) => {
                // The pointer is outside the modal's shape: it is not hovered.
                if matches!(event, InputEvent::Mouse(_)) {
                    layer.modal.set_hover(None);
                }
                return Some(LayerOutcome::Consumed(action));
            }
            Filtered::Pass | Filtered::Redirect(_) => {}
        }
        if view.phase != ModalPhase::Open {
            return Some(LayerOutcome::Consumed(None));
        }
        let mut ctx = EventContext::new(ControlPart::Body).with_modifiers(modifiers);
        ctx.layer = Some(view);
        let action = match event {
            InputEvent::Mouse(mouse) => {
                if let Some((part, _)) = layer.modal.hit_test(mouse.pos()) {
                    ctx = ctx.with_part(part);
                }
                layer.modal.handle_mouse(&mouse, &ctx)
            }
            InputEvent::Keyboard(key) => layer.modal.handle_keyboard(&key, &ctx),
            InputEvent::Gesture(gesture) => layer.modal.handle_gesture(&gesture, &ctx),
            InputEvent::Modifiers(_) => None,
        };
        if let Some(ControlAction::Dismiss { control }) = action {
            let by = control.map_or(DismissReason::Requested, DismissReason::Control);
            LayerContext {
                state: Arc::clone(&self.state),
                id: layer.id,
                options: layer.options,
            }
            .request(by);
        }
        Some(LayerOutcome::Handled(action))
    }
}

/// What the layer chain did with an event.
pub(crate) enum LayerOutcome {
    /// Blocked (or spent by the filter), with an action for the app.
    Consumed(Option<ControlAction>),
    /// Given to the top modal, which answered this.
    Handled(Option<ControlAction>),
}

impl DrawControl for ModalStack {
    fn draw(&self, _list: &mut DrawList, _ctx: &DrawContext) {}

    /// Every layer bottom to top: its backdrop, its modal (both passes) with
    /// its `LayerView`, then its filter is shown what the modal drew.
    fn draw_overlay(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.sync();
        let views = self.views();
        let layers = self.layers.borrow();
        for (serial, view) in views {
            let Some(layer) = layers.iter().find(|l| l.serial == serial) else {
                continue;
            };
            if let Some(backdrop) = layer.options.backdrop {
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
            let mut own = ctx.clone();
            own.layer = Some(view);
            let start = list.instances.len();
            layer.modal.draw(list, &own);
            layer.modal.draw_overlay(list, &own);
            layer.filter.drawn(list, start..list.instances.len());
        }
    }
}

impl HostedControl for ModalStack {
    fn bounds(&self) -> Rect {
        self.viewport
    }

    /// Never hit: the stack takes input through the layer chain only.
    fn hit_test(&self, _pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        None
    }

    fn handle_mouse(&self, _event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        None
    }

    fn handle_keyboard(
        &self,
        _event: &KeyboardEvent,
        _ctx: &EventContext,
    ) -> Option<ControlAction> {
        None
    }

    fn default_flags(&self) -> NodeFlags {
        NodeFlags::NONE
    }
}

impl Router {
    /// The installed modal stack, if any.
    pub(crate) fn modal_stack(&self) -> Option<(NodeId, &ModalStack)> {
        let id = self.find_alias(Symbol::lookup(ModalStack::NAME)?)?;
        Some((id, self.get::<ModalStack>(id)?))
    }

    /// Push `modal` as entry `id` with the stack's default transition.
    pub fn push_modal(
        &mut self,
        id: impl Into<Symbol>,
        modal: impl Modal,
    ) -> Result<(), ModalError> {
        let (_, stack) = self.modal_stack().ok_or(ModalError::NoStack)?;
        let t = stack.state.default_transition();
        self.push_modal_with(id, modal, t)
    }

    /// Push `modal` as entry `id` with `transition`.
    pub fn push_modal_with(
        &mut self,
        id: impl Into<Symbol>,
        modal: impl Modal,
        transition: ModalTransition,
    ) -> Result<(), ModalError> {
        let id = id.into();
        let (_, stack) = self.modal_stack().ok_or(ModalError::NoStack)?;
        let outcome = stack.state.push_with(id, transition);
        if outcome == PushOutcome::Refused {
            return Err(ModalError::AlreadyOpen);
        }
        stack.push(id, Box::new(modal), outcome);
        self.layer_opened();
        Ok(())
    }

    /// Close the top (reported as `Replaced`) and push `modal` over it.
    pub fn replace_top_modal(
        &mut self,
        id: impl Into<Symbol>,
        modal: impl Modal,
    ) -> Result<(), ModalError> {
        let id = id.into();
        let (_, stack) = self.modal_stack().ok_or(ModalError::NoStack)?;
        let t = stack.state.default_transition();
        let outcome = stack.state.replace_top_with(id, t);
        if outcome == PushOutcome::Refused {
            return Err(ModalError::AlreadyOpen);
        }
        stack.push(id, Box::new(modal), outcome);
        self.layer_opened();
        Ok(())
    }
}
