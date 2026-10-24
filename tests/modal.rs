//! The modal stack: state, the layer chain, modals and the installed stack.
//!
//! Plan 03's tests, ported to the persistent router (plan 05, section 8):
//! modals are pushed with `Router::push_modal`, live on the installed
//! `ModalStack`, and are asked through the stack's top layer filter.

mod common;

use std::cell::{Ref, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::{Call, Recorder};
use libcontrols::router::EventResponse;
use libcontrols::{
    Backdrop, Button, ContentReveal, ControlAction, ControlPart, DismissOn, DismissReason,
    DrawContext, DrawControl, Edit, EventContext, FocusEvent, GestureEvent, HostedControl,
    InputEvent, Key, KeyboardEvent, LayerContext, LayerFilter, LayerShape, Modal, ModalDialog,
    ModalError, ModalEvent, ModalFrame, ModalLayerFilter, ModalOptions, ModalPhase, ModalStack,
    ModalState, ModalTransition, ModalView, Modifiers, MouseButton, MouseEvent, NodeId, Options,
    PanelFrame, Rect, Router, Slider, Symbol, TooltipStyle, sym,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

const TIMED: ModalTransition = ModalTransition::Timed {
    open_secs: 0.2,
    close_secs: 0.1,
};

const VIEWPORT: Rect = Rect::new(0.0, 0.0, 800.0, 600.0);
const FRAME: Rect = Rect::new(200.0, 150.0, 400.0, 300.0);
const CONTENT: Rect = Rect::new(220.0, 170.0, 360.0, 260.0);
const FRAME_COLOR: [f32; 4] = [0.2, 0.3, 0.4, 1.0];
const BUTTON_BG: [f32; 4] = [0.0, 0.6, 0.0, 1.0];
const TIP_BG: [f32; 4] = [0.6, 0.0, 0.6, 1.0];

fn timed() -> ModalState {
    ModalState::new().with_transition(TIMED)
}

fn instant() -> ModalState {
    ModalState::new().with_transition(ModalTransition::Instant)
}

fn driven() -> ModalState {
    ModalState::new().with_transition(ModalTransition::Driven)
}

/// The left half of a rect: a `LayerShape` of the test's own.
struct LeftHalf(Rect);

impl LayerShape for LeftHalf {
    fn contains(&self, pos: [f32; 2]) -> bool {
        self.0.contains(pos) && pos[0] < self.0.x + self.0.w / 2.0
    }
}

/// A plain modal: a recorder with options, optionally tappable only on
/// the left half of its rect.
struct Sheet {
    rec: Recorder,
    options: ModalOptions,
    left_half_only: bool,
}

impl Sheet {
    fn new(rect: Rect) -> Self {
        Self {
            rec: Recorder {
                rect,
                ..Recorder::at(0.0)
            },
            options: ModalOptions::default(),
            left_half_only: false,
        }
    }

    fn with_options(mut self, options: ModalOptions) -> Self {
        self.options = options;
        self
    }
}

impl DrawControl for Sheet {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.rec.draw(list, ctx);
    }
}

impl HostedControl for Sheet {
    fn bounds(&self) -> Rect {
        self.rec.bounds()
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.rec.hit_test(pos)
    }

    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.rec.handle_mouse(event, ctx)
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.rec.handle_keyboard(event, ctx)
    }

    fn handle_gesture(&self, event: &GestureEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.rec.handle_gesture(event, ctx)
    }

    fn set_hover(&self, hover: Option<ControlPart>) {
        self.rec.set_hover(hover);
    }

    fn handle_focus(&self, event: FocusEvent) {
        self.rec.handle_focus(event);
    }
}

impl Modal for Sheet {
    fn options(&self) -> ModalOptions {
        self.options
    }

    fn filter_layer(&self, layer: &LayerContext) -> Box<dyn LayerFilter> {
        if self.left_half_only {
            Box::new(ModalLayerFilter::new(
                LeftHalf(self.rec.rect),
                layer.clone(),
            ))
        } else {
            Box::new(ModalLayerFilter::new(self.bounds(), layer.clone()))
        }
    }
}

fn sheet() -> Sheet {
    Sheet::new(FRAME)
}

/// A root router with a modal stack installed over `VIEWPORT`.
struct Env {
    router: Router,
    stack: NodeId,
    state: Arc<ModalState>,
}

fn env(state: ModalState) -> Env {
    let state = Arc::new(state);
    let mut router = Router::new();
    let stack = ModalStack::install(&mut router, Arc::clone(&state), VIEWPORT).unwrap();
    Env {
        router,
        stack,
        state,
    }
}

impl Env {
    /// Push a plain `Sheet` over `FRAME`.
    fn push(&mut self, id: impl Into<Symbol>) -> Result<(), ModalError> {
        self.router.push_modal(id, sheet())
    }

    fn push_with(
        &mut self,
        id: impl Into<Symbol>,
        transition: ModalTransition,
    ) -> Result<(), ModalError> {
        self.router.push_modal_with(id, sheet(), transition)
    }

    fn replace_top(&mut self, id: impl Into<Symbol>) -> Result<(), ModalError> {
        self.router.replace_top_modal(id, sheet())
    }

    fn send(&self, event: InputEvent) -> EventResponse {
        self.router.deliver_event(event)
    }

    fn render(&self) -> DrawList {
        let mut list = DrawList::new();
        self.router.render(&mut list, &DrawContext::default());
        list
    }

    fn modals(&self) -> &ModalStack {
        self.router.get::<ModalStack>(self.stack).unwrap()
    }

    /// An app node that is a `Recorder`.
    fn app(&self, id: NodeId) -> &Recorder {
        self.router.get::<Recorder>(id).unwrap()
    }

    /// The recorder of the `Sheet` on layer `id`.
    fn sheet(&self, id: &str) -> Ref<'_, Recorder> {
        Ref::map(self.modals().modal::<Sheet>(id).expect("a sheet"), |s| {
            &s.rec
        })
    }

    /// The content router of the `ModalDialog` on layer `id`.
    fn content<R>(&self, id: &str, read: impl FnOnce(&Router) -> R) -> R {
        let dialog = self
            .modals()
            .modal::<ModalDialog>(id)
            .expect("a dialog on that layer");
        read(dialog.content())
    }

    /// Run `read` on the `Recorder` node `node` of dialog `id`'s content.
    fn content_rec<R>(&self, id: &str, node: NodeId, read: impl FnOnce(&Recorder) -> R) -> R {
        self.content(id, |c| read(c.get::<Recorder>(node).expect("a recorder")))
    }
}

fn ids(state: &ModalState) -> Vec<(Symbol, ModalPhase, bool)> {
    state
        .stack()
        .iter()
        .map(|e| (e.id, e.phase, e.is_top))
        .collect()
}

#[test]
fn a_push_opens_over_the_top_and_leaves_the_lower_one_alone() {
    let mut env = env(timed());
    assert_eq!(env.push("settings"), Ok(()));
    env.state.advance(1.0);
    env.state.take_events();
    assert_eq!(
        env.push("confirm"),
        Ok(()),
        "a second modal goes over the first"
    );
    assert_eq!(
        ids(&env.state),
        [
            (sym!("settings"), ModalPhase::Open, false),
            (sym!("confirm"), ModalPhase::Opening, true),
        ]
    );
    assert_eq!(env.state.progress("confirm"), 0.0);
    assert!(
        env.state.take_events().is_empty(),
        "the lower one emits nothing"
    );
    assert_eq!(env.modals().len(), 2, "one layer per entry");
}

#[test]
fn a_duplicate_push_is_refused() {
    let mut env = env(timed());
    assert_eq!(env.push("settings"), Ok(()));
    assert_eq!(
        env.push(sym!("settings")),
        Err(ModalError::AlreadyOpen),
        "an id already open is refused"
    );
    assert_eq!(env.state.depth(), 1);
    assert_eq!(env.modals().len(), 1, "the refused modal makes no layer");
}

/// Plan 05 removed `MAX_MODAL_DEPTH`: the stack has no depth limit.
#[test]
fn a_fifth_modal_is_refused() {
    let mut env = env(instant());
    for name in ["a", "b", "c", "d", "e", "f"] {
        assert_eq!(env.push(name), Ok(()), "{name}");
    }
    assert_eq!(env.state.depth(), 6, "a fifth and a sixth are accepted");
    assert_eq!(env.modals().len(), 6);
    assert_eq!(env.state.top(), Some(sym!("f")));
}

#[test]
fn dismiss_closes_only_the_top_and_the_one_below_becomes_top() {
    let mut env = env(timed());
    env.push("settings").unwrap();
    env.push("confirm").unwrap();
    let state = Arc::clone(&env.state);
    state.advance(1.0);
    state.take_events();
    state.dismiss(DismissReason::Escape);
    assert_eq!(
        state.take_events(),
        [ModalEvent::Dismissed {
            id: sym!("confirm"),
            by: DismissReason::Escape
        }]
    );
    assert_eq!(state.top(), Some(sym!("settings")));
    assert_eq!(state.phase("confirm"), ModalPhase::Closing);
    assert_eq!(state.phase("settings"), ModalPhase::Open, "never closed");
    state.advance(0.05);
    assert!(
        state.progress("confirm") > 0.0,
        "progress runs down from where it was"
    );
    state.advance(1.0);
    assert_eq!(
        state.take_events(),
        [ModalEvent::Closed {
            id: sym!("confirm"),
            by: DismissReason::Escape
        }]
    );
    assert_eq!(state.depth(), 1);
    assert_eq!(env.modals().len(), 1, "its layer is dropped");
}

#[test]
fn replace_top_reports_replaced_and_opens_at_once() {
    let mut env = env(timed());
    env.push("pause").unwrap();
    env.state.advance(1.0);
    env.state.take_events();
    assert_eq!(env.replace_top("quit"), Ok(()));
    assert_eq!(
        env.state.take_events(),
        [ModalEvent::Dismissed {
            id: sym!("pause"),
            by: DismissReason::Replaced
        }]
    );
    assert_eq!(
        ids(&env.state),
        [
            (sym!("pause"), ModalPhase::Closing, false),
            (sym!("quit"), ModalPhase::Opening, true),
        ],
        "both animate together"
    );
    assert_eq!(env.modals().len(), 2, "both have a layer");
    let mut empty = self::env(timed());
    assert_eq!(
        empty.replace_top("only"),
        Ok(()),
        "with no top it is a push"
    );
    assert_eq!(empty.state.top(), Some(sym!("only")));
}

#[test]
fn replaced_is_reported_only_by_replace_top() {
    let mut env = env(instant());
    env.push("a").unwrap();
    env.state.pop();
    env.push("b").unwrap();
    env.state.close("b");
    env.push("c").unwrap();
    env.state.dismiss_all(DismissReason::App);
    let replaced = env.state.take_events().into_iter().any(|e| {
        matches!(
            e,
            ModalEvent::Dismissed {
                by: DismissReason::Replaced,
                ..
            } | ModalEvent::Closed {
                by: DismissReason::Replaced,
                ..
            }
        )
    });
    assert!(!replaced, "pop, close and dismiss_all never say Replaced");
}

#[test]
fn dismiss_all_closes_top_first() {
    let mut env = env(timed());
    for name in ["a", "b", "c"] {
        env.push(name).unwrap();
    }
    env.state.take_events();
    env.state.dismiss_all(DismissReason::Back);
    let order: Vec<Symbol> = env
        .state
        .take_events()
        .into_iter()
        .map(|e| match e {
            ModalEvent::Dismissed { id, .. } => id,
            other => panic!("only Dismissed expected, got {other:?}"),
        })
        .collect();
    assert_eq!(order, [sym!("c"), sym!("b"), sym!("a")]);
    assert_eq!(env.state.top(), None);
}

#[test]
fn pushing_an_id_that_is_closing_waits_for_its_close() {
    let mut env = env(timed());
    env.push("settings").unwrap();
    env.state.advance(1.0);
    env.state.pop();
    env.state.take_events();
    let again = Sheet::new(Rect::new(10.0, 10.0, 50.0, 50.0));
    assert_eq!(
        env.router.push_modal("settings", again),
        Ok(()),
        "queued, so accepted"
    );
    assert_eq!(env.state.depth(), 1, "not pushed yet");
    assert_eq!(env.modals().len(), 1, "nor its layer");
    assert_eq!(
        env.sheet("settings").rect,
        FRAME,
        "the closing layer is still the first one"
    );
    env.state.advance(1.0);
    assert_eq!(
        env.state.take_events(),
        [ModalEvent::Closed {
            id: sym!("settings"),
            by: DismissReason::App
        }]
    );
    assert_eq!(
        env.state.phase("settings"),
        ModalPhase::Opening,
        "pushed after the close"
    );
    assert_eq!(env.modals().len(), 1);
    assert_eq!(
        env.sheet("settings").rect,
        Rect::new(10.0, 10.0, 50.0, 50.0),
        "the queued modal's layer appears with its entry"
    );
}

#[test]
fn timed_and_driven_entries_move_independently() {
    let mut env = env(driven());
    env.push("book").unwrap();
    env.push_with("confirm", TIMED).unwrap();
    let state = &env.state;
    state.advance(0.1);
    assert_eq!(
        state.progress("book"),
        0.0,
        "advance does not move a Driven entry"
    );
    assert!((state.progress("confirm") - 0.5).abs() < 1e-5);
    state.set_progress("book", 0.4);
    assert!((state.progress("book") - 0.4).abs() < 1e-5);
    assert!((state.progress("confirm") - 0.5).abs() < 1e-5);
    state.set_progress("book", 1.5);
    assert_eq!(state.progress("book"), 1.0, "progress is clamped");
    assert_eq!(state.phase("book"), ModalPhase::Open);
    state.close("book");
    state.advance(10.0);
    assert_eq!(
        state.phase("book"),
        ModalPhase::Closing,
        "a Driven entry stays until driven to 0"
    );
    state.set_progress("book", 0.0);
    assert_eq!(state.phase("book"), ModalPhase::Closed);
}

#[test]
fn an_instant_modal_is_open_at_once() {
    let mut env = env(instant());
    env.push("a").unwrap();
    assert_eq!(env.state.phase("a"), ModalPhase::Open);
    assert_eq!(env.state.progress("a"), 1.0);
    assert_eq!(
        env.state.take_events(),
        [ModalEvent::Opened { id: sym!("a") }]
    );
    env.state.pop();
    assert_eq!(
        env.state.take_events(),
        [
            ModalEvent::Dismissed {
                id: sym!("a"),
                by: DismissReason::App
            },
            ModalEvent::Closed {
                id: sym!("a"),
                by: DismissReason::App
            }
        ],
        "an instant close finishes in the same call"
    );
    let mut zero = self::env(ModalState::new().with_transition(ModalTransition::Timed {
        open_secs: 0.0,
        close_secs: 0.0,
    }));
    zero.push("z").unwrap();
    assert_eq!(
        zero.state.phase("z"),
        ModalPhase::Open,
        "a zero duration is instant"
    );
}

#[test]
fn every_dismissed_is_followed_by_one_closed_with_the_same_id() {
    let mut env = env(timed());
    for name in ["a", "b", "c"] {
        env.push(name).unwrap();
    }
    env.state.advance(1.0);
    env.state.dismiss(DismissReason::Escape);
    env.replace_top("d").unwrap();
    env.state.close("a");
    env.state.dismiss_all(DismissReason::OutsideTap);
    for _ in 0..20 {
        env.state.advance(0.05);
    }
    let events = env.state.take_events();
    let dismissed: Vec<(Symbol, DismissReason)> = events
        .iter()
        .filter_map(|e| match *e {
            ModalEvent::Dismissed { id, by } => Some((id, by)),
            _ => None,
        })
        .collect();
    let closed: Vec<(Symbol, DismissReason)> = events
        .iter()
        .filter_map(|e| match *e {
            ModalEvent::Closed { id, by } => Some((id, by)),
            _ => None,
        })
        .collect();
    assert_eq!(dismissed.len(), closed.len());
    for d in &dismissed {
        assert_eq!(closed.iter().filter(|c| *c == d).count(), 1, "{d:?}");
    }
    assert_eq!(env.state.depth(), 0);
    assert!(env.modals().is_empty(), "every layer is dropped");
}

#[test]
fn dismissing_with_no_top_does_nothing() {
    let mut env = env(timed());
    env.state.dismiss(DismissReason::Escape);
    env.state.pop();
    assert!(env.state.take_events().is_empty());
    env.push("a").unwrap();
    env.state.advance(1.0);
    env.state.pop();
    env.state.take_events();
    env.state.dismiss(DismissReason::Escape);
    assert!(
        env.state.take_events().is_empty(),
        "a second Escape while closing does nothing"
    );
}

#[test]
fn events_are_taken_once() {
    let mut env = env(instant());
    env.push("a").unwrap();
    assert_eq!(env.state.take_events().len(), 1);
    assert!(env.state.take_events().is_empty());
}

#[test]
fn advance_ignores_negative_and_nan_dt() {
    let mut env = env(timed());
    env.push("a").unwrap();
    env.state.advance(-1.0);
    env.state.advance(f32::NAN);
    env.state.advance(f32::INFINITY);
    assert_eq!(env.state.progress("a"), 0.0);
    assert!(env.state.advance(0.1), "still animating");
}

#[test]
fn captures_input_follows_the_entries_without_a_lock() {
    let mut env = env(timed());
    assert!(!env.state.captures_input());
    env.push("a").unwrap();
    assert!(env.state.captures_input(), "from the push");
    env.state.advance(1.0);
    env.state.pop();
    assert!(env.state.captures_input(), "through Closing");
    env.state.advance(0.05);
    assert!(env.state.captures_input());
    env.state.advance(1.0);
    assert!(
        !env.state.captures_input(),
        "false once the last entry is gone"
    );
}

#[test]
fn ids_are_symbols_and_events_are_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<ModalEvent>();
    assert_copy::<DismissReason>();
    let mut env = env(instant());
    env.push(String::from("built")).unwrap();
    assert!(env.state.contains(sym!("built")));
    assert!(env.state.contains("built"));
    let event = env.state.take_events()[0];
    let again = event;
    assert_eq!(event, again);
    // `DismissReason::Control` carries the close control's `NodeId` now.
    let close = env.router.add(None, Recorder::at(0.0));
    let control = DismissReason::Control(close);
    assert_eq!(
        control,
        DismissReason::Control(close.to_string().parse().unwrap())
    );
}

fn every_kind() -> [InputEvent; 4] {
    [
        InputEvent::Mouse(MouseEvent::Down {
            pos: [10.0, 10.0],
            button: MouseButton::Left,
        }),
        InputEvent::Mouse(MouseEvent::Up {
            pos: [10.0, 10.0],
            button: MouseButton::Left,
        }),
        InputEvent::Keyboard(KeyboardEvent::KeyDown { key: Key::Tab }),
        InputEvent::Gesture(GestureEvent::DoubleTap),
    ]
}

fn handler_calls(calls: Vec<Call>) -> usize {
    calls
        .iter()
        .filter(|c| matches!(c, Call::Mouse(..) | Call::Key(..) | Call::Gesture(..)))
        .count()
}

// `ModalFilter` is gone: the installed stack's layer chain, run by the root
// router after the global filters, takes its place.

#[test]
fn the_filter_passes_while_the_state_is_empty() {
    let mut env = env(timed());
    let base = env.router.add(None, Recorder::at(0.0));
    let resp = env.send(InputEvent::Mouse(MouseEvent::Down {
        pos: [10.0, 10.0],
        button: MouseButton::Left,
    }));
    assert_eq!(resp.target, Some(base), "routing as usual");
    assert_eq!(handler_calls(env.app(base).take()), 1);
}

/// The layer chain takes every event while any entry exists: the top modal
/// receives them while it is open, and nothing does while it opens or
/// closes. (Plan 03's filter redirected all of them to the stack's node.)
#[test]
fn the_filter_redirects_to_its_target_while_any_modal_exists() {
    let mut env = env(driven());
    let base = env.router.add(None, Recorder::at(0.0));
    env.router
        .push_modal("settings", Sheet::new(Rect::new(0.0, 0.0, 100.0, 100.0)))
        .unwrap();
    for phase in [ModalPhase::Opening, ModalPhase::Open, ModalPhase::Closing] {
        match phase {
            ModalPhase::Open => env.state.set_progress("settings", 1.0),
            ModalPhase::Closing => env.state.pop(),
            _ => {}
        }
        assert_eq!(env.state.phase("settings"), phase);
        for event in every_kind() {
            let resp = env.send(event);
            assert!(resp.handled, "{phase:?}: {event:?}");
            let expected = (phase == ModalPhase::Open).then_some(env.stack);
            assert_eq!(resp.target, expected, "{phase:?}: {event:?}");
        }
        let handled = handler_calls(env.sheet("settings").take());
        let expected = if phase == ModalPhase::Open { 4 } else { 0 };
        assert_eq!(handled, expected, "{phase:?}: the top modal's share");
        assert_eq!(
            handler_calls(env.app(base).take()),
            0,
            "{phase:?}: the base gets nothing"
        );
    }
}

#[test]
fn a_modifier_change_is_consumed_while_a_modal_exists() {
    let mut env = env(instant());
    env.push("a").unwrap();
    let resp = env.send(InputEvent::Modifiers(Modifiers {
        shift: true,
        ..Modifiers::default()
    }));
    assert!(resp.handled && resp.target.is_none());
    assert!(env.sheet("a").take().is_empty());
}

/// Plan 03 trapped input even when the filter's target was not hosted. Now:
/// pointer events that miss the top modal are still trapped, and keys and
/// gestures still go to it.
#[test]
fn a_filter_whose_target_is_not_hosted_still_traps_input() {
    let mut env = env(instant());
    let base = env.router.add(None, Recorder::at(0.0));
    let far = Sheet::new(Rect::new(500.0, 500.0, 60.0, 60.0)).with_options(
        ModalOptions::default().with_dismiss(DismissOn {
            outside_tap: false,
            ..DismissOn::default()
        }),
    );
    env.router.push_modal("a", far).unwrap();
    for event in every_kind() {
        let resp = env.send(event);
        assert!(resp.handled, "{event:?}");
        if let InputEvent::Mouse(_) = event {
            assert!(resp.target.is_none(), "{event:?} is blocked");
        }
    }
    assert_eq!(
        handler_calls(env.app(base).take()),
        0,
        "nothing reaches the base"
    );
    let calls = env.sheet("a").take();
    assert!(
        calls
            .iter()
            .all(|c| matches!(c, Call::Key(..) | Call::Gesture(..) | Call::Hover(None))),
        "only the key and the gesture reach the modal, which is told the pointer is outside it: {calls:?}"
    );
    assert_eq!(handler_calls(calls), 2);
}

/// Plan 03 rebuilt the host each frame around one filter; the router is
/// built once and its stack serves every frame.
#[test]
fn one_filter_serves_many_builds() {
    let mut env = env(instant());
    let mut seen = Vec::new();
    for round in 0..3 {
        if round == 1 {
            env.push("a").unwrap();
        }
        env.render();
        let resp = env.send(InputEvent::Keyboard(KeyboardEvent::KeyDown {
            key: Key::Enter,
        }));
        seen.push(resp.target);
    }
    assert_eq!(seen, [None, Some(env.stack), Some(env.stack)]);
}

/// A frame painter that draws one box and remembers every view it is given.
#[derive(Clone, Default)]
struct RecordingFrame {
    views: Rc<RefCell<Vec<ModalView>>>,
}

impl ModalFrame for RecordingFrame {
    fn draw_frame(&self, list: &mut DrawList, _ctx: &DrawContext, view: &ModalView) {
        self.views.borrow_mut().push(*view);
        list.push(SdfInstance {
            kind: SdfKind::Box,
            position: view.frame.pos(),
            size: view.frame.size(),
            color: FRAME_COLOR,
            anim: 0,
        });
    }
}

fn content_button() -> Button {
    Button::new("OK", Rect::new(240.0, 200.0, 80.0, 30.0)).with_bg(BUTTON_BG)
}

fn position_of(list: &DrawList, color: [f32; 4]) -> Option<usize> {
    list.instances.iter().position(|i| i.color == color)
}

fn dialog() -> ModalDialog {
    ModalDialog::new(VIEWPORT, FRAME, CONTENT)
}

fn tip_grid(rect: Rect) -> Options {
    let grid = Options::new(&["SV - Save"], rect)
        .with_tooltip_style(TooltipStyle::default().with_bg(TIP_BG));
    grid.set_hover(Some(ControlPart::Custom(0)));
    grid
}

/// A stack with no entry draws nothing, and a dialog whose entry has closed
/// is no longer drawn (its layer is dropped).
#[test]
fn a_modal_with_no_entry_draws_nothing() {
    let mut env = env(driven());
    assert!(env.render().instances.is_empty());
    let mut modal = dialog();
    modal.content_mut().add(None, content_button());
    env.router.push_modal("settings", modal).unwrap();
    env.state.set_progress("settings", 1.0);
    assert!(!env.render().instances.is_empty());
    env.state.close("settings");
    env.state.set_progress("settings", 0.0);
    assert!(env.render().instances.is_empty());
}

#[test]
fn the_backdrop_covers_the_viewport_and_fades_with_progress() {
    for (fade, alpha) in [(true, 0.4), (false, 0.8)] {
        let mut env = env(driven());
        let options = ModalOptions::default().with_backdrop(Some(Backdrop {
            color: [0.0, 0.0, 0.0, 0.8],
            fade_with_progress: fade,
        }));
        env.router
            .push_modal("settings", dialog().with_options(options))
            .unwrap();
        env.state.set_progress("settings", 0.5);
        let list = env.render();
        let backdrop = list.instances[0];
        assert_eq!(backdrop.position, VIEWPORT.pos());
        assert_eq!(backdrop.size, VIEWPORT.size());
        assert!(
            (backdrop.color[3] - alpha).abs() < 1e-5,
            "fade {fade}: {alpha} at half progress"
        );
    }
}

#[test]
fn no_backdrop_means_no_backdrop_instance() {
    let mut env = env(driven());
    let modal = dialog()
        .with_options(ModalOptions::default().with_backdrop(None))
        .with_frame(RecordingFrame::default());
    env.router.push_modal("settings", modal).unwrap();
    let list = env.render();
    assert_eq!(list.instances.len(), 1, "only the frame");
    assert_eq!(list.instances[0].color, FRAME_COLOR);
}

#[test]
fn the_frame_painter_is_called_with_the_view() {
    let mut env = env(driven());
    let frame = RecordingFrame::default();
    env.router
        .push_modal("settings", dialog().with_frame(frame.clone()))
        .unwrap();
    env.router
        .push_modal("confirm", dialog().with_frame(frame.clone()))
        .unwrap();
    env.state.set_progress("settings", 1.0);
    env.state.set_progress("confirm", 0.25);
    env.render();
    let views = frame.views.borrow();
    assert_eq!(views.len(), 2);
    let (l, u) = (views[0], views[1]);
    assert_eq!(
        (l.id, l.phase, l.depth, l.is_top),
        (sym!("settings"), ModalPhase::Open, 0, false)
    );
    assert_eq!(
        (u.id, u.phase, u.depth, u.is_top),
        (sym!("confirm"), ModalPhase::Opening, 1, true)
    );
    assert!((u.progress - 0.25).abs() < 1e-5);
    assert_eq!((u.viewport, u.frame, u.content), (VIEWPORT, FRAME, CONTENT));
}

#[test]
fn when_open_content_is_hidden_while_opening() {
    let mut env = env(driven());
    let mut modal = dialog().with_reveal(ContentReveal::WhenOpen);
    modal.content_mut().add(None, content_button());
    env.router.push_modal("book", modal).unwrap();
    env.state.set_progress("book", 0.9);
    assert!(
        position_of(&env.render(), BUTTON_BG).is_none(),
        "hidden while opening"
    );
    env.state.set_progress("book", 1.0);
    assert!(
        position_of(&env.render(), BUTTON_BG).is_some(),
        "shown once open"
    );
}

#[test]
fn fade_scales_content_alpha_by_progress() {
    let mut env = env(driven());
    let mut modal = dialog().with_frame(RecordingFrame::default());
    modal.content_mut().add(None, content_button());
    env.router.push_modal("settings", modal).unwrap();
    env.state.set_progress("settings", 0.5);
    let list = env.render();
    let faded_bg = [BUTTON_BG[0], BUTTON_BG[1], BUTTON_BG[2], 0.5];
    let content = position_of(&list, faded_bg).expect("content alpha is halved");
    let frame = position_of(&list, FRAME_COLOR).expect("the frame is drawn");
    assert_eq!(list.instances[0].size, VIEWPORT.size(), "backdrop first");
    assert!(
        frame > 0 && content > frame,
        "backdrop, frame, then content"
    );
}

#[test]
fn content_is_clipped_and_its_tooltips_are_not() {
    let mut env = env(driven());
    let mut modal = dialog();
    modal.content_mut().add(None, content_button());
    modal
        .content_mut()
        .add(None, tip_grid(Rect::new(240.0, 200.0, 100.0, 20.0)));
    env.router.push_modal("settings", modal).unwrap();
    env.state.set_progress("settings", 1.0);
    let list = env.render();
    let content = position_of(&list, BUTTON_BG).expect("content drawn");
    let tip = position_of(&list, TIP_BG).expect("the hovered cell's tip is drawn");
    assert!(list.instance_clip(content).is_some(), "content is clipped");
    assert!(list.instance_clip(tip).is_none(), "its tooltip is not");
}

#[test]
fn only_the_top_draws_content_tooltips() {
    let mut env = env(driven());
    let mut lower = dialog().with_reveal(ContentReveal::Always);
    lower
        .content_mut()
        .add(None, tip_grid(Rect::new(240.0, 200.0, 100.0, 20.0)));
    env.router.push_modal("settings", lower).unwrap();
    env.router
        .push_modal(
            "confirm",
            dialog().with_options(ModalOptions::default().with_backdrop(None)),
        )
        .unwrap();
    assert!(
        position_of(&env.render(), TIP_BG).is_none(),
        "not the top: no tip"
    );
    env.state.close("confirm");
    env.state.set_progress("confirm", 0.0);
    assert!(
        position_of(&env.render(), TIP_BG).is_some(),
        "the top again: tip"
    );
}

/// `Router::render` is its `draw` pass then its `draw_overlay` pass.
#[test]
fn split_draw_passes_match_draw() {
    let mut router = Router::new();
    router.add(None, tip_grid(Rect::new(0.0, 0.0, 100.0, 20.0)));
    router.add(None, content_button());
    let ctx = DrawContext::default();
    let mut whole = DrawList::new();
    router.render(&mut whole, &ctx);
    let mut split = DrawList::new();
    router.draw(&mut split, &ctx);
    router.draw_overlay(&mut split, &ctx);
    assert!(position_of(&whole, TIP_BG).is_some());
    assert_eq!(whole.instances, split.instances, "instance for instance");
}

/// A recorder at `(x, y)`, 60 pixels square, inside the content rect.
fn rec(x: f32, y: f32) -> Recorder {
    Recorder {
        rect: Rect::new(x, y, 60.0, 60.0),
        ..Recorder::at(0.0)
    }
}

fn m_down(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Down {
        pos: [x, y],
        button: MouseButton::Left,
    })
}

fn m_up(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Up {
        pos: [x, y],
        button: MouseButton::Left,
    })
}

fn m_click(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Click {
        pos: [x, y],
        button: MouseButton::Left,
    })
}

fn m_move(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Move { pos: [x, y] })
}

fn m_drag(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Drag {
        start_pos: [250.0, 210.0],
        current_pos: [x, y],
        delta: [x - 250.0, y - 210.0],
        button: MouseButton::Left,
    })
}

fn k(key: Key) -> InputEvent {
    InputEvent::Keyboard(KeyboardEvent::KeyDown { key })
}

/// An instant stack with dialog "m" (over `FRAME`) open, built by `build`,
/// and its events taken.
fn open_with(build: impl FnOnce(&mut ModalDialog)) -> Env {
    open_with_on(instant(), build)
}

fn open_with_on(state: ModalState, build: impl FnOnce(&mut ModalDialog)) -> Env {
    let mut env = env(state);
    let mut modal = dialog();
    build(&mut modal);
    env.router.push_modal("m", modal).unwrap();
    env.state.take_events();
    env
}

/// An open dialog "m" with no content.
fn open_state() -> Env {
    open_with(|_| {})
}

fn dismissed(state: &ModalState) -> Vec<DismissReason> {
    state
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            ModalEvent::Dismissed { by, .. } => Some(by),
            _ => None,
        })
        .collect()
}

fn refused(state: &ModalState) -> Vec<DismissReason> {
    state
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            ModalEvent::DismissRefused { by, .. } => Some(by),
            _ => None,
        })
        .collect()
}

/// Replace the dialog's options in `build`.
fn with_options(modal: &mut ModalDialog, options: ModalOptions) {
    let taken = std::mem::replace(modal, dialog());
    *modal = taken.with_options(options);
}

fn dismiss_on(dismiss: DismissOn) -> ModalOptions {
    ModalOptions::default().with_dismiss(dismiss)
}

#[test]
fn escape_dismisses_and_a_second_escape_does_nothing() {
    let env = open_with_on(timed(), |_| {});
    env.state.advance(1.0);
    env.state.take_events();
    env.send(k(Key::Escape));
    assert_eq!(dismissed(&env.state), [DismissReason::Escape]);
    env.send(k(Key::Escape));
    assert!(env.state.take_events().is_empty(), "already closing");
}

#[test]
fn back_dismisses_with_back() {
    let env = open_state();
    env.send(k(Key::Back));
    assert_eq!(dismissed(&env.state), [DismissReason::Back]);
}

#[test]
fn escape_and_back_dismiss_with_an_edit_focused() {
    for key in [Key::Escape, Key::Back] {
        let env = open_with(|m| {
            let edit = m.content_mut().add(
                None,
                Edit::new("name", Rect::new(240.0, 200.0, 200.0, 30.0)),
            );
            let taken = std::mem::replace(m, dialog());
            *m = taken.with_initial_focus(edit);
        });
        env.send(k(Key::Char('a')));
        env.send(k(key));
        assert_eq!(
            dismissed(&env.state).len(),
            1,
            "{key:?} dismisses even then"
        );
    }
}

#[test]
fn escape_reaches_the_content_when_disabled() {
    let mut r = None;
    let env = open_with(|m| {
        let id = m.content_mut().add(None, rec(240.0, 200.0));
        let taken = std::mem::replace(m, dialog());
        *m = taken
            .with_options(dismiss_on(DismissOn {
                escape: false,
                ..DismissOn::default()
            }))
            .with_initial_focus(id);
        r = Some(id);
    });
    env.send(k(Key::Escape));
    assert!(env.state.take_events().is_empty());
    assert_eq!(
        env.content_rec("m", r.unwrap(), Recorder::focus),
        [FocusEvent::Gained, FocusEvent::Lost],
        "the content router clears its focus"
    );
}

#[test]
fn back_can_be_turned_off() {
    let mut r = None;
    let env = open_with(|m| {
        let id = m.content_mut().add(None, rec(240.0, 200.0));
        let taken = std::mem::replace(m, dialog());
        *m = taken
            .with_options(dismiss_on(DismissOn {
                back: false,
                ..DismissOn::default()
            }))
            .with_initial_focus(id);
        r = Some(id);
    });
    env.send(k(Key::Back));
    assert!(env.state.take_events().is_empty());
    assert!(
        env.content_rec("m", r.unwrap(), Recorder::take)
            .iter()
            .any(|c| matches!(c, Call::Key(..))),
        "Back goes to the focused content control"
    );
}

#[test]
fn a_press_and_release_outside_dismisses() {
    let env = open_state();
    env.send(m_down(50.0, 50.0));
    assert!(env.state.take_events().is_empty(), "not on the press");
    env.send(m_up(60.0, 50.0));
    assert_eq!(dismissed(&env.state), [DismissReason::OutsideTap]);
}

#[test]
fn a_release_outside_without_a_press_outside_does_not_dismiss() {
    let env = open_state();
    let resp = env.send(m_up(50.0, 50.0));
    assert!(resp.handled, "consumed");
    assert!(
        env.state.take_events().is_empty(),
        "the tail of the opening tap"
    );
}

#[test]
fn a_drag_from_a_slider_to_outside_does_not_dismiss() {
    let env = open_with(|m| {
        m.content_mut().add(
            None,
            Slider::new(0.5, 0.0, 1.0, Rect::new(240.0, 200.0, 200.0, 20.0)),
        );
    });
    env.send(m_down(250.0, 210.0));
    env.send(m_drag(50.0, 50.0));
    env.send(m_up(50.0, 50.0));
    assert!(env.state.take_events().is_empty());
}

#[test]
fn a_click_outside_dismisses_for_click_only_shells() {
    let env = open_state();
    env.send(m_click(50.0, 50.0));
    assert_eq!(dismissed(&env.state), [DismissReason::OutsideTap]);
}

#[test]
fn outside_tap_can_be_turned_off() {
    let env = open_with(|m| {
        with_options(
            m,
            dismiss_on(DismissOn {
                outside_tap: false,
                ..DismissOn::default()
            }),
        );
    });
    env.send(m_down(50.0, 50.0));
    env.send(m_up(50.0, 50.0));
    env.send(m_click(50.0, 50.0));
    assert!(env.state.take_events().is_empty());
}

/// The close control is named by its `NodeId` in the content router now,
/// and the response names the stack, carrying the dismissal.
#[test]
fn a_close_control_dismisses_with_its_symbol() {
    let mut close = None;
    let env = open_with(|m| {
        let id = m.content_mut().add(
            None,
            Button::new("Close", Rect::new(240.0, 200.0, 80.0, 30.0)),
        );
        let taken = std::mem::replace(m, dialog());
        *m = taken.with_close_control(id);
        close = Some(id);
    });
    let close = close.unwrap();
    let resp = env.send(m_click(250.0, 210.0));
    assert_eq!(resp.target, Some(env.stack), "the response is the stack's");
    assert_eq!(
        resp.action,
        Some(ControlAction::Dismiss {
            control: Some(close)
        })
    );
    assert_eq!(dismissed(&env.state), [DismissReason::Control(close)]);
}

#[test]
fn hold_open_refuses_user_dismissals_and_reports_them() {
    let mut close = None;
    let env = open_with(|m| {
        let id = m.content_mut().add(
            None,
            Button::new("Close", Rect::new(240.0, 200.0, 80.0, 30.0)),
        );
        let taken = std::mem::replace(m, dialog());
        *m = taken
            .with_options(ModalOptions::default().with_hold_open(true))
            .with_close_control(id);
        close = Some(id);
    });
    env.send(k(Key::Escape));
    env.send(k(Key::Back));
    env.send(m_down(50.0, 50.0));
    env.send(m_up(50.0, 50.0));
    env.send(m_click(250.0, 210.0));
    assert_eq!(
        refused(&env.state),
        [
            DismissReason::Escape,
            DismissReason::Back,
            DismissReason::OutsideTap,
            DismissReason::Control(close.unwrap()),
        ]
    );
    assert_eq!(env.state.phase("m"), ModalPhase::Open, "it stays open");
}

#[test]
fn hold_open_does_not_stop_app_dismissals() {
    let env = open_with(|m| with_options(m, ModalOptions::default().with_hold_open(true)));
    env.state.pop();
    assert_eq!(dismissed(&env.state), [DismissReason::App]);
}

#[test]
fn a_disabled_action_is_not_reported_as_refused() {
    let env = open_with(|m| {
        with_options(
            m,
            dismiss_on(DismissOn {
                escape: false,
                back: true,
                outside_tap: false,
            })
            .with_hold_open(true),
        );
    });
    env.send(k(Key::Escape));
    env.send(m_click(50.0, 50.0));
    assert!(env.state.take_events().is_empty());
}

#[test]
fn content_receives_nothing_while_opening_or_closing() {
    let mut r = None;
    let env = open_with_on(timed(), |m| {
        r = Some(m.content_mut().add(None, rec(240.0, 200.0)));
    });
    let r = r.unwrap();
    env.send(m_down(250.0, 210.0));
    env.send(k(Key::Enter));
    assert!(
        env.content_rec("m", r, Recorder::take).is_empty(),
        "nothing while opening"
    );
    env.state.advance(1.0);
    env.state.pop();
    env.send(m_down(250.0, 210.0));
    assert!(
        env.content_rec("m", r, Recorder::take).is_empty(),
        "nothing while closing"
    );
}

/// Plan 03 rebuilt the modal each frame and carried its focus, grab and
/// hover in the entry; now the dialog is built once and its content router
/// keeps them across frames.
#[test]
fn focus_drag_and_hover_inside_the_modal_survive_a_rebuild() {
    let (mut a, mut b) = (None, None);
    let env = open_with(|m| {
        a = Some(m.content_mut().add(None, rec(240.0, 200.0)));
        b = Some(m.content_mut().add(None, rec(340.0, 200.0)));
    });
    let (a, b) = (a.unwrap(), b.unwrap());
    let frame = |event: InputEvent| {
        env.render();
        env.send(event);
    };
    frame(m_move(250.0, 210.0));
    frame(m_down(250.0, 210.0));
    env.content_rec("m", a, Recorder::take);
    frame(m_drag(500.0, 400.0));
    let mouse: Vec<Call> = env
        .content_rec("m", a, Recorder::take)
        .into_iter()
        .filter(|c| matches!(c, Call::Mouse(..)))
        .collect();
    assert!(
        matches!(mouse.as_slice(), [Call::Mouse(MouseEvent::Drag { .. }, _)]),
        "the grab is kept: {mouse:?}"
    );
    frame(m_up(500.0, 400.0));
    env.content_rec("m", a, Recorder::take);
    frame(m_move(500.0, 400.0));
    assert!(
        env.content_rec("m", a, Recorder::hovers).contains(&None),
        "the hover is kept, so leaving clears it"
    );
    env.content_rec("m", b, Recorder::take);
    frame(k(Key::Tab));
    assert!(
        env.content_rec("m", a, Recorder::focus)
            .contains(&FocusEvent::Lost),
        "the focus is kept"
    );
    assert!(
        env.content_rec("m", b, Recorder::focus)
            .contains(&FocusEvent::Gained)
    );
}

#[test]
fn initial_focus_is_applied_once_per_push() {
    fn build(ids: &mut Option<(NodeId, NodeId)>) -> ModalDialog {
        let mut m = dialog();
        let a = m.content_mut().add(None, rec(240.0, 200.0));
        let b = m.content_mut().add(None, rec(340.0, 200.0));
        *ids = Some((a, b));
        m.with_initial_focus(b)
    }
    let keys = |calls: Vec<Call>| calls.iter().any(|c| matches!(c, Call::Key(..)));
    let mut env = env(instant());
    let mut ids = None;
    env.router.push_modal("m", build(&mut ids)).unwrap();
    let (a, b) = ids.unwrap();
    env.send(k(Key::Enter));
    assert!(
        keys(env.content_rec("m", b, Recorder::take)),
        "b starts focused"
    );
    env.send(k(Key::Tab));
    env.send(k(Key::Enter));
    assert!(
        keys(env.content_rec("m", a, Recorder::take)),
        "a later event keeps the moved focus instead of the initial one"
    );
    env.state.pop();
    env.router.push_modal("m", build(&mut ids)).unwrap();
    let (_, b) = ids.unwrap();
    env.send(k(Key::Enter));
    assert!(
        keys(env.content_rec("m", b, Recorder::take)),
        "a new push seeds it again"
    );
}

#[test]
fn the_content_sees_the_modifiers_the_outer_host_saw() {
    let mut r = None;
    let env = open_with(|m| r = Some(m.content_mut().add(None, rec(240.0, 200.0))));
    let shift = Modifiers {
        shift: true,
        ..Modifiers::default()
    };
    env.send(InputEvent::Modifiers(shift));
    env.send(m_down(250.0, 210.0));
    assert_eq!(
        env.content_rec("m", r.unwrap(), Recorder::mouse_contexts)[0].modifiers,
        shift
    );
}

#[test]
fn tab_cycles_the_content_only() {
    let mut env = open_with(|m| {
        m.content_mut().add(None, rec(240.0, 200.0));
        m.content_mut().add(None, rec(340.0, 200.0));
    });
    env.router.add(None, Recorder::at(0.0));
    common::take_focus_journal();
    for _ in 0..3 {
        env.send(k(Key::Tab));
    }
    let order: Vec<(f32, FocusEvent)> = common::take_focus_journal();
    assert_eq!(
        order,
        [
            (240.0, FocusEvent::Gained),
            (240.0, FocusEvent::Lost),
            (340.0, FocusEvent::Gained),
            (340.0, FocusEvent::Lost),
            (240.0, FocusEvent::Gained),
        ],
        "Tab wraps within the content; the app node at 0 is never focused"
    );
}

/// `ModalFrame::contains` is no longer asked: a modal's shape is its
/// layer filter's. A `ModalLayerFilter` over the test's own `LayerShape`.
#[test]
fn a_frame_shape_decides_what_is_outside() {
    let mut env = env(instant());
    let mut half = sheet();
    half.left_half_only = true;
    env.router.push_modal("m", half).unwrap();
    env.state.take_events();
    env.send(m_down(300.0, 300.0));
    env.send(m_up(300.0, 300.0));
    assert!(
        env.state.take_events().is_empty(),
        "the left half is on the modal"
    );
    env.send(m_down(500.0, 300.0));
    env.send(m_up(500.0, 300.0));
    assert_eq!(
        dismissed(&env.state),
        [DismissReason::OutsideTap],
        "the right half is not"
    );
}

const LOWER_BG: [f32; 4] = [0.9, 0.1, 0.1, 1.0];
const UPPER_BG: [f32; 4] = [0.1, 0.1, 0.9, 1.0];
const UPPER_FRAME: Rect = Rect::new(300.0, 250.0, 200.0, 100.0);

fn lower() -> ModalDialog {
    dialog().with_frame(PanelFrame {
        bg_color: Some(LOWER_BG),
        ..PanelFrame::default()
    })
}

fn upper() -> ModalDialog {
    ModalDialog::new(VIEWPORT, UPPER_FRAME, UPPER_FRAME.pad(-10.0)).with_frame(PanelFrame {
        bg_color: Some(UPPER_BG),
        ..PanelFrame::default()
    })
}

fn backdrops(list: &DrawList) -> Vec<usize> {
    list.instances
        .iter()
        .enumerate()
        .filter(|(_, i)| i.size == VIEWPORT.size())
        .map(|(n, _)| n)
        .collect()
}

#[test]
fn an_empty_stack_is_transparent_and_draws_nothing() {
    let mut env = env(instant());
    let base = env.router.add(None, Recorder::at(0.0));
    assert_eq!(env.modals().hit_test([10.0, 10.0]), None);
    let resp = env.send(m_down(10.0, 10.0));
    assert_eq!(resp.target, Some(base), "the base gets it");
    assert!(env.render().instances.is_empty());
}

#[test]
fn tab_in_the_base_skips_the_stack() {
    let mut env = env(instant());
    let a = env.router.add(None, Recorder::at(0.0));
    let b = env.router.add(None, Recorder::at(200.0));
    let mut seen = Vec::new();
    for _ in 0..3 {
        env.send(k(Key::Tab));
        seen.push(env.router.focused());
    }
    assert_eq!(seen, [Some(a), Some(b), Some(a)], "never the stack");
}

#[test]
fn base_controls_receive_nothing_while_any_modal_exists() {
    let mut env = env(driven());
    let base = env.router.add(None, rec(240.0, 200.0));
    env.router.push_modal("lower", lower()).unwrap();
    for phase in [ModalPhase::Opening, ModalPhase::Open, ModalPhase::Closing] {
        match phase {
            ModalPhase::Open => env.state.set_progress("lower", 1.0),
            ModalPhase::Closing => env.state.pop(),
            _ => {}
        }
        env.render();
        for event in [
            m_down(250.0, 210.0),
            m_up(250.0, 210.0),
            k(Key::Enter),
            InputEvent::Gesture(GestureEvent::DoubleTap),
        ] {
            env.send(event);
        }
        assert_eq!(handler_calls(env.app(base).take()), 0, "{phase:?}");
    }
}

/// Plan 03 let the base hover and grab go on the first redirected event;
/// now they go when the layer is pushed.
#[test]
fn base_hover_is_left_and_a_base_grab_dropped_on_the_first_redirected_event() {
    let mut env = env(instant());
    let base = env.router.add(None, rec(240.0, 200.0));
    env.send(m_move(250.0, 210.0));
    env.send(m_down(250.0, 210.0));
    assert!(env.router.active_drag().is_some());
    env.app(base).take();
    env.router.push_modal("lower", lower()).unwrap();
    assert_eq!(env.router.active_drag(), None, "the base grab is dropped");
    env.send(m_drag(260.0, 210.0));
    assert_eq!(
        env.app(base).take(),
        [Call::Hover(None)],
        "left, and nothing else"
    );
    assert_eq!(env.router.active_drag(), None);
}

#[test]
fn base_focus_is_unchanged_after_a_modal_closes() {
    let mut env = env(instant());
    let a = env.router.add(None, Recorder::at(0.0));
    let b = env.router.add(None, Recorder::at(200.0));
    env.router.set_focus(Some(b));
    env.app(b).take();
    env.push("lower").unwrap();
    env.send(k(Key::Tab));
    env.send(k(Key::Escape));
    env.state.dismiss_all(DismissReason::App);
    assert_eq!(env.router.focused(), Some(b));
    assert!(
        env.app(a).focus().is_empty() && env.app(b).focus().is_empty(),
        "no focus event at all"
    );
}

#[test]
fn lower_modals_receive_nothing_and_lose_their_hover() {
    let mut env = env(instant());
    let mut first = lower();
    let content = first.content_mut().add(None, rec(220.0, 180.0));
    env.router.push_modal("lower", first).unwrap();
    env.send(m_move(230.0, 190.0));
    assert_eq!(
        env.content_rec("lower", content, Recorder::hovers),
        [Some(ControlPart::Body)]
    );
    env.router.push_modal("upper", upper()).unwrap();
    env.send(m_move(235.0, 190.0));
    env.send(k(Key::Enter));
    assert_eq!(
        env.content_rec("lower", content, Recorder::take),
        [Call::Hover(None)],
        "loses its hover, receives nothing"
    );
}

#[test]
fn escape_and_back_close_only_the_top() {
    for key in [Key::Escape, Key::Back] {
        let mut env = env(instant());
        env.router.push_modal("lower", lower()).unwrap();
        env.router.push_modal("upper", upper()).unwrap();
        env.state.take_events();
        env.send(k(key));
        assert!(!env.state.contains("upper"), "{key:?} closes the top");
        assert!(env.state.contains("lower"), "{key:?} leaves the one below");
    }
}

#[test]
fn a_tap_on_a_lower_modal_is_outside_the_top() {
    let mut env = env(instant());
    env.router.push_modal("lower", lower()).unwrap();
    env.router.push_modal("upper", upper()).unwrap();
    env.send(m_down(220.0, 170.0));
    env.send(m_up(220.0, 170.0));
    assert!(
        !env.state.contains("upper"),
        "a tap on the lower frame closes the top"
    );
    assert!(env.state.contains("lower"));
}

/// Plan 03 kept each entry's focus in the entry across rebuilds; now each
/// dialog's content router keeps its own.
#[test]
fn focus_is_kept_per_entry() {
    let mut env = env(instant());
    let mut first = lower();
    let a = first.content_mut().add(None, rec(220.0, 180.0));
    let b = first.content_mut().add(None, rec(300.0, 180.0));
    env.router.push_modal("lower", first).unwrap();
    env.send(k(Key::Tab));
    env.send(k(Key::Tab));
    env.router.push_modal("upper", upper()).unwrap();
    env.send(k(Key::Tab));
    env.state.pop();
    env.content_rec("lower", a, Recorder::take);
    env.content_rec("lower", b, Recorder::take);
    env.send(k(Key::Enter));
    assert!(
        env.content_rec("lower", b, Recorder::take)
            .iter()
            .any(|c| matches!(c, Call::Key(..))),
        "the lower modal's focus is where it was: on b"
    );
}

/// There is no build order any more: layers draw in the order of their
/// entries, even when a queued push lands its layer later.
#[test]
fn modals_are_ordered_by_the_stack_not_by_build_order() {
    let mut env = env(instant());
    env.router.push_modal("lower", lower()).unwrap();
    env.router.push_modal("upper", upper()).unwrap();
    let list = env.render();
    let l = position_of(&list, LOWER_BG).expect("lower drawn");
    let u = position_of(&list, UPPER_BG).expect("upper drawn");
    assert!(l < u, "the lower entry draws first");
    assert_eq!(env.state.top(), Some(sym!("upper")));

    let mut env = self::env(timed());
    env.router.push_modal("lower", lower()).unwrap();
    env.router.push_modal("upper", upper()).unwrap();
    env.state.advance(1.0);
    env.state.close("lower");
    env.router.push_modal("lower", lower()).unwrap();
    env.state.advance(1.0);
    assert_eq!(
        env.state.phase("lower"),
        ModalPhase::Opening,
        "queued, then pushed"
    );
    env.state.advance(1.0);
    let list = env.render();
    let l = position_of(&list, LOWER_BG).expect("lower drawn");
    let u = position_of(&list, UPPER_BG).expect("upper drawn");
    assert!(u < l, "the re-pushed lower entry is now above upper");
    assert_eq!(env.state.top(), Some(sym!("lower")));
}

// `input_is_consumed_when_the_top_has_no_modal_built` is dropped: pushing
// goes through `Router::push_modal`, so every entry has a layer and there is
// no unbuilt top to test.

#[test]
fn the_response_names_the_stack_and_carries_the_inner_action() {
    let mut env = env(instant());
    let ok = Button::new("OK", Rect::new(240.0, 200.0, 80.0, 30.0));
    let clicks = Arc::clone(&ok.state);
    let mut first = lower();
    first.content_mut().add(None, ok);
    env.router.push_modal("lower", first).unwrap();
    let resp = env.send(m_click(250.0, 210.0));
    assert_eq!(resp.target, Some(env.stack));
    assert!(matches!(resp.action, Some(ControlAction::Activated { .. })));
    assert!(clicks.take_click(), "the button inside was pressed");
}

#[test]
fn each_level_draws_its_own_backdrop_bottom_to_top() {
    let mut env = env(instant());
    env.router.push_modal("lower", lower()).unwrap();
    env.router.push_modal("upper", upper()).unwrap();
    let list = env.render();
    let backdrops = backdrops(&list);
    assert_eq!(backdrops.len(), 2, "one backdrop per level");
    let l = position_of(&list, LOWER_BG).unwrap();
    let u = position_of(&list, UPPER_BG).unwrap();
    assert!(backdrops[0] < l && l < backdrops[1] && backdrops[1] < u);
}

#[test]
fn the_stack_draws_after_every_base_overlay() {
    let mut env = env(instant());
    let grid = Options::new(&["SV - Save"], Rect::new(10.0, 10.0, 100.0, 20.0))
        .with_tooltip_style(TooltipStyle::default().with_bg(TIP_BG));
    let hovered = Arc::clone(&grid.state);
    env.router.add(None, grid);
    let mut first = lower();
    first.content_mut().add(None, content_button());
    env.router.push_modal("lower", first).unwrap();
    env.router.push_modal("upper", upper()).unwrap();
    // Pushing let the base hover go; hover the cell by hand to draw its tip.
    hovered.set_hovered_cell(Some(0));
    let list = env.render();
    let tip = position_of(&list, TIP_BG).expect("the base overlay is drawn");
    let backdrops = backdrops(&list);
    let l = position_of(&list, LOWER_BG).unwrap();
    let content = position_of(&list, BUTTON_BG).unwrap();
    let u = position_of(&list, UPPER_BG).unwrap();
    assert!(tip > 0, "the base control comes before its overlay");
    assert!(
        tip < backdrops[0]
            && backdrops[0] < l
            && l < content
            && content < backdrops[1]
            && backdrops[1] < u,
        "base, its overlay, lower backdrop, frame, content, upper backdrop, frame"
    );

    // A hovered base recorder under the same two modals.
    let mut env = self::env(instant());
    let watcher = env.router.add(None, rec(500.0, 500.0));
    env.send(m_move(505.0, 505.0));
    env.app(watcher).take();
    env.router.push_modal("lower", lower()).unwrap();
    env.router.push_modal("upper", upper()).unwrap();
    for event in [m_move(505.0, 505.0), m_down(505.0, 505.0), k(Key::Enter)] {
        env.send(event);
    }
    assert_eq!(
        env.app(watcher).take(),
        [Call::Hover(None)],
        "one clear and no handler call"
    );
}

/// Plan 03 needed a host holding the stack and its filter; a router with
/// only the installed stack works.
#[test]
fn a_host_with_only_a_stack_and_its_filter_works() {
    let mut env = env(instant());
    let mut first = lower();
    let close = first.content_mut().add(
        None,
        Button::new("Close", Rect::new(240.0, 200.0, 80.0, 30.0)),
    );
    env.router
        .push_modal("lower", first.with_close_control(close))
        .unwrap();
    env.state.take_events();
    assert_eq!(env.router.len(), 1, "only the stack");
    env.send(m_click(250.0, 210.0));
    assert_eq!(dismissed(&env.state), [DismissReason::Control(close)]);
    assert!(!env.state.captures_input());
    assert!(env.modals().is_empty());
}

/// The README's two-level example, in the router form (plan 05, 8.6).
#[test]
fn the_readme_two_level_example_runs() {
    let modals = Arc::new(ModalState::new());
    let mut router = Router::new();
    ModalStack::install(&mut router, modals.clone(), VIEWPORT).unwrap();
    let unsaved_changes = true;

    let mut settings = ModalDialog::centered(VIEWPORT, [400.0, 300.0], 16.0)
        .with_options(ModalOptions::default().with_hold_open(unsaved_changes));
    let quit = Button::new("Quit", Rect::new(220.0, 170.0, 80.0, 30.0));
    let quit_clicks = Arc::clone(&quit.state);
    settings.content_mut().add(None, quit);
    router.push_modal(sym!("settings"), settings).unwrap();
    modals.advance(1.0);
    router.deliver_event(m_click(230.0, 180.0));
    assert!(quit_clicks.take_click(), "Quit inside settings is pressed");

    let mut confirm = ModalDialog::centered(VIEWPORT, [240.0, 120.0], 12.0);
    let cancel = confirm.content_mut().add(
        None,
        Button::new("Cancel", Rect::new(290.0, 260.0, 80.0, 30.0)),
    );
    router
        .push_modal(sym!("confirm"), confirm.with_close_control(cancel))
        .unwrap();
    modals.advance(1.0);
    modals.take_events();

    router.deliver_event(m_click(300.0, 270.0));
    assert_eq!(
        modals.take_events(),
        [ModalEvent::Dismissed {
            id: sym!("confirm"),
            by: DismissReason::Control(cancel)
        }],
        "Cancel closes the confirm"
    );
    modals.advance(1.0);
    modals.take_events();
    assert_eq!(modals.top(), Some(sym!("settings")));

    router.deliver_event(k(Key::Escape));
    assert_eq!(
        modals.take_events(),
        [ModalEvent::DismissRefused {
            id: sym!("settings"),
            by: DismissReason::Escape
        }],
        "unsaved settings hold open"
    );
}
