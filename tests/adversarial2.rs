//! Second-round adversarial tests, aimed at the fixes of the first round
//! (commit a998e8c): the layer-press swallow, `covered`/`reset` on layers,
//! `ModalStack::sync` ordering, focus on nested router nodes, Tab walks
//! through trapping and `TAB_THROUGH` routers, redirects into nested routers,
//! and the filter chain's re-entrancy counter. The first round's fuzzers are
//! re-run with redirect filters and nested routers in play.
//!
//! Each test that first exposed a defect now guards its fix.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use libcontrols::{
    Button, ButtonState, ControlAction, ControlPart, DismissReason, DrawContext, DrawControl,
    EventContext, FilterContext, FilterStack, Filtered, FocusEvent, GestureEvent, HostedControl,
    InputEvent, InputFilter, Key, KeyboardEvent, LayerContext, LayerFilter, Modal, ModalDialog,
    ModalEvent, ModalLayerFilter, ModalPhase, ModalStack, ModalState, ModalTransition, Modifiers,
    MouseButton, MouseEvent, NodeFlags, NodeId, NodeState, Rect, Router,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Mouse(MouseEvent),
    Key(KeyboardEvent),
    Gesture(GestureEvent),
    Hover(Option<ControlPart>),
    Focus(FocusEvent),
}

type Log = Rc<RefCell<Vec<(u32, Ev)>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn told(log: &Log, tag: u32) -> Vec<Ev> {
    log.borrow()
        .iter()
        .filter(|(t, _)| *t == tag)
        .map(|(_, e)| e.clone())
        .collect()
}

fn handler_calls(log: &Log, tag: u32) -> Vec<Ev> {
    told(log, tag)
        .into_iter()
        .filter(|e| matches!(e, Ev::Mouse(_) | Ev::Key(_) | Ev::Gesture(_)))
        .collect()
}

/// A rectangle control with declared flags that logs every call.
struct Probe {
    rect: Rect,
    flags: NodeFlags,
    tag: u32,
    log: Log,
}

fn probe(rect: Rect, flags: NodeFlags, tag: u32, log: &Log) -> Probe {
    Probe {
        rect,
        flags,
        tag,
        log: Rc::clone(log),
    }
}

impl Probe {
    fn note(&self, ev: Ev) {
        self.log.borrow_mut().push((self.tag, ev));
    }
}

impl DrawControl for Probe {
    fn draw(&self, list: &mut DrawList, _ctx: &DrawContext) {
        list.push(SdfInstance {
            kind: SdfKind::Box,
            position: self.rect.pos(),
            size: self.rect.size(),
            color: [1.0; 4],
            anim: 0,
        });
    }
}

impl HostedControl for Probe {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.rect.contains(pos).then_some((ControlPart::Body, 0.0))
    }

    fn handle_mouse(&self, event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        self.note(Ev::Mouse(*event));
        None
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, _ctx: &EventContext) -> Option<ControlAction> {
        self.note(Ev::Key(*event));
        None
    }

    fn handle_gesture(&self, event: &GestureEvent, _ctx: &EventContext) -> Option<ControlAction> {
        self.note(Ev::Gesture(*event));
        None
    }

    fn set_hover(&self, hover: Option<ControlPart>) {
        self.note(Ev::Hover(hover));
    }

    fn handle_focus(&self, event: FocusEvent) {
        self.note(Ev::Focus(event));
    }

    fn default_flags(&self) -> NodeFlags {
        self.flags
    }
}

/// A plain self-drawn modal with the standard layer filter.
struct Blob(Rect);

impl DrawControl for Blob {
    fn draw(&self, list: &mut DrawList, _ctx: &DrawContext) {
        list.push(SdfInstance {
            kind: SdfKind::Box,
            position: self.0.pos(),
            size: self.0.size(),
            color: [0.5; 4],
            anim: 0,
        });
    }
}

impl HostedControl for Blob {
    fn bounds(&self) -> Rect {
        self.0
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.0.contains(pos).then_some((ControlPart::Body, 0.0))
    }

    fn handle_mouse(&self, _event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        None
    }

    fn handle_keyboard(&self, _e: &KeyboardEvent, _ctx: &EventContext) -> Option<ControlAction> {
        None
    }
}

impl Modal for Blob {
    fn filter_layer(&self, layer: &LayerContext) -> Box<dyn LayerFilter> {
        Box::new(ModalLayerFilter::new(self.bounds(), layer.clone()))
    }
}

const F: NodeFlags = NodeFlags::FOCUSABLE;
const VIEWPORT: Rect = Rect::new(0.0, 0.0, 800.0, 600.0);
const TIMED: ModalTransition = ModalTransition::Timed {
    open_secs: 0.1,
    close_secs: 0.1,
};

fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, w, h)
}

fn down_b(x: f32, y: f32, button: MouseButton) -> MouseEvent {
    MouseEvent::Down {
        pos: [x, y],
        button,
    }
}

fn up_b(x: f32, y: f32, button: MouseButton) -> MouseEvent {
    MouseEvent::Up {
        pos: [x, y],
        button,
    }
}

fn click_b(x: f32, y: f32, button: MouseButton) -> MouseEvent {
    MouseEvent::Click {
        pos: [x, y],
        button,
    }
}

fn down(x: f32, y: f32) -> MouseEvent {
    down_b(x, y, MouseButton::Left)
}

fn up(x: f32, y: f32) -> MouseEvent {
    up_b(x, y, MouseButton::Left)
}

fn click(x: f32, y: f32) -> MouseEvent {
    click_b(x, y, MouseButton::Left)
}

fn mv(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Move { pos: [x, y] }
}

fn key(k: Key) -> KeyboardEvent {
    KeyboardEvent::KeyDown { key: k }
}

/// Down, Up and Click at one point, as a shell sends one tap.
fn tap(router: &Router, x: f32, y: f32) {
    router.deliver_mouse(down(x, y));
    router.deliver_mouse(up(x, y));
    router.deliver_mouse(click(x, y));
}

fn render(router: &Router) -> DrawList {
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
    list
}

fn modal_setup(t: ModalTransition) -> (Router, Arc<ModalState>, NodeId) {
    let state = Arc::new(ModalState::new().with_transition(t));
    let mut router = Router::new();
    let stack = ModalStack::install(&mut router, Arc::clone(&state), VIEWPORT).unwrap();
    (router, state, stack)
}

fn layers(router: &Router, stack: NodeId) -> usize {
    router.get::<ModalStack>(stack).unwrap().len()
}

/// The tag of the innermost focused probe, descending through routers.
fn focus_leaf(router: &Router) -> Option<u32> {
    let f = router.focused()?;
    if let Some(inner) = router.get::<Router>(f) {
        return focus_leaf(inner);
    }
    router.get::<Probe>(f).map(|p| p.tag)
}

fn ancestors(router: &Router, id: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut at = router.parent(id);
    while let Some(p) = at {
        out.push(p);
        at = router.parent(p);
    }
    out
}

fn blocked(router: &Router, id: NodeId, mask: NodeFlags) -> bool {
    router.flags(id).intersects(mask)
        || ancestors(router, id)
            .iter()
            .any(|&a| router.flags(a).intersects(mask))
}

fn walk(router: &Router) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack: Vec<NodeId> = router.children(None).collect();
    stack.reverse();
    while let Some(n) = stack.pop() {
        out.push(n);
        let mut kids: Vec<NodeId> = router.children(Some(n)).collect();
        kids.reverse();
        stack.extend(kids);
    }
    out
}

/// The router's state invariants, recursively through nested routers.
fn check_router(router: &Router, what: &str) {
    let nodes = walk(router);
    assert_eq!(nodes.len(), router.len(), "{what}: len matches the tree");
    let focused = router.focused();
    let hovered = router.hovered();
    let grab = router.active_drag().map(|d| d.node);
    for (name, id) in [("focus", focused), ("hover", hovered), ("grab", grab)] {
        if let Some(id) = id {
            assert!(router.contains(id), "{what}: {name} names a live node");
        }
    }
    if let Some(f) = focused {
        assert!(
            !blocked(router, f, NodeFlags::DISABLED | NodeFlags::HIDDEN),
            "{what}: the focused node takes input"
        );
    }
    if let Some(h) = hovered {
        assert!(
            !blocked(router, h, NodeFlags::HIDDEN),
            "{what}: the hovered node is visible"
        );
    }
    if let Some(g) = grab {
        assert!(
            !blocked(router, g, NodeFlags::DISABLED | NodeFlags::HIDDEN),
            "{what}: the grabbed node takes input"
        );
    }
    let fa = focused.map(|f| ancestors(router, f)).unwrap_or_default();
    let ha = hovered.map(|h| ancestors(router, h)).unwrap_or_default();
    for &n in &nodes {
        let s = router.state(n);
        assert_eq!(s.contains(NodeState::FOCUSED), focused == Some(n), "{what}");
        assert_eq!(
            s.contains(NodeState::FOCUS_WITHIN),
            fa.contains(&n),
            "{what}"
        );
        assert_eq!(s.contains(NodeState::HOVERED), hovered == Some(n), "{what}");
        assert_eq!(
            s.contains(NodeState::HOVER_WITHIN),
            ha.contains(&n),
            "{what}"
        );
        assert_eq!(s.contains(NodeState::GRABBED), grab == Some(n), "{what}");
        if let Some(inner) = router.get::<Router>(n) {
            check_router(inner, &format!("{what} > {n}"));
        }
    }
}

/// A focused node inside a nested router is the only focused leaf: its
/// router node holds the parent's focus.
fn check_single_focus(router: &Router, what: &str) {
    for n in walk(router) {
        if let Some(inner) = router.get::<Router>(n) {
            if inner.focused().is_some() {
                assert_eq!(
                    router.focused(),
                    Some(n),
                    "{what}: {n} has a focused node inside, so it holds the parent's focus"
                );
            }
            check_single_focus(inner, &format!("{what} > {n}"));
        }
    }
}

/// Redirects events to a node while `to` is set.
struct Redirector {
    to: Rc<Cell<Option<NodeId>>>,
    keys: bool,
}

impl InputFilter for Redirector {
    fn filter(&self, event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        let Some(id) = self.to.get() else {
            return Filtered::Pass;
        };
        match event {
            InputEvent::Mouse(_) => Filtered::Redirect(id),
            InputEvent::Keyboard(_) if self.keys => Filtered::Redirect(id),
            _ => Filtered::Pass,
        }
    }
}

fn redirecting(router: Router, keys: bool) -> (Router, Rc<Cell<Option<NodeId>>>) {
    let to = Rc::new(Cell::new(None));
    let stack = FilterStack::new();
    stack.add(Redirector {
        to: Rc::clone(&to),
        keys,
    });
    (router.with_filter_stack(stack), to)
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn coin(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        (!items.is_empty()).then(|| items[self.below(items.len())])
    }
}

fn random_event(rng: &mut Lcg) -> InputEvent {
    let pos = [(rng.below(42) * 10) as f32, (rng.below(42) * 10) as f32];
    let button = if rng.coin(5) {
        MouseButton::Right
    } else {
        MouseButton::Left
    };
    match rng.below(13) {
        0 | 1 => InputEvent::Mouse(MouseEvent::Move { pos }),
        2 | 3 => InputEvent::Mouse(MouseEvent::Down { pos, button }),
        4 | 5 => InputEvent::Mouse(MouseEvent::Up { pos, button }),
        6 => InputEvent::Mouse(MouseEvent::Click { pos, button }),
        7 => InputEvent::Mouse(MouseEvent::Drag {
            start_pos: [0.0, 0.0],
            current_pos: pos,
            delta: [1.0, 1.0],
            button,
        }),
        8 => InputEvent::Keyboard(key(if rng.coin(2) { Key::Tab } else { Key::BackTab })),
        9 => InputEvent::Keyboard(key(
            [Key::Escape, Key::Enter, Key::Char('a'), Key::Back][rng.below(4)]
        )),
        10 => InputEvent::Gesture(GestureEvent::Pinch { delta: 0.5 }),
        11 => InputEvent::Keyboard(KeyboardEvent::KeyUp { key: Key::Tab }),
        _ => InputEvent::Modifiers(Modifiers {
            shift: rng.coin(2),
            ..Modifiers::default()
        }),
    }
}

fn random_probe(rng: &mut Lcg, tag: u32, log: &Log) -> Probe {
    let x = (rng.below(30) * 10) as f32;
    let y = (rng.below(30) * 10) as f32;
    let w = (3 + rng.below(15)) as f32 * 10.0;
    let h = (3 + rng.below(15)) as f32 * 10.0;
    let flags = if rng.coin(4) { NodeFlags::NONE } else { F };
    probe(r(x, y, w, h), flags, tag, log)
}

fn random_flags(rng: &mut Lcg) -> NodeFlags {
    [
        NodeFlags::FOCUSABLE,
        NodeFlags::DISABLED,
        NodeFlags::HIDDEN,
        NodeFlags::TAB_THROUGH,
    ][rng.below(4)]
}

// ---------------------------------------------------------------------------
// The layer-press swallow
// ---------------------------------------------------------------------------

#[test]
fn a_chorded_press_taken_by_a_layer_never_clicks_through_after_it_closes() {
    let log = new_log();
    let (mut router, state, _stack) = modal_setup(ModalTransition::Instant);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    router
        .push_modal("m", Blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    router.deliver_mouse(down_b(350.0, 350.0, MouseButton::Left));
    router.deliver_mouse(down_b(350.0, 350.0, MouseButton::Right));
    state.pop();
    for e in [
        up_b(350.0, 350.0, MouseButton::Right),
        click_b(350.0, 350.0, MouseButton::Right),
        up_b(350.0, 350.0, MouseButton::Left),
        click_b(350.0, 350.0, MouseButton::Left),
    ] {
        router.deliver_mouse(e);
    }
    let calls = handler_calls(&log, 1);
    assert!(
        calls.is_empty(),
        "both presses began on the layer, yet the base got {calls:?}"
    );
}

#[test]
fn a_press_of_another_button_after_the_layer_closed_keeps_the_first_press_swallowed() {
    let log = new_log();
    let (mut router, state, _stack) = modal_setup(ModalTransition::Instant);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    router
        .push_modal("m", Blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    router.deliver_mouse(down(350.0, 350.0));
    router.deliver_keyboard(key(Key::Escape));
    assert!(!state.contains("m"), "Escape closed the layer mid-press");
    // A right press on the base is its own press: it reaches the base.
    router.deliver_mouse(down_b(350.0, 350.0, MouseButton::Right));
    router.deliver_mouse(up_b(350.0, 350.0, MouseButton::Right));
    log.borrow_mut().clear();
    // The left press the layer took ends.
    router.deliver_mouse(up(350.0, 350.0));
    router.deliver_mouse(click(350.0, 350.0));
    let calls = handler_calls(&log, 1);
    assert!(
        calls.is_empty(),
        "the left press began on the layer, yet the base got {calls:?}"
    );
}

#[test]
fn a_press_taken_by_a_layer_closed_by_keyboard_is_swallowed_and_then_forgotten() {
    let log = new_log();
    let (mut router, state, _stack) = modal_setup(ModalTransition::Instant);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    router
        .push_modal("m", Blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    router.deliver_mouse(down(350.0, 350.0));
    router.deliver_keyboard(key(Key::Escape));
    assert!(!state.contains("m"));
    router.deliver_mouse(up(350.0, 350.0));
    router.deliver_mouse(click(350.0, 350.0));
    assert!(handler_calls(&log, 1).is_empty(), "the rest of it is spent");
    // A fresh tap on the base is not swallowed.
    tap(&router, 20.0, 20.0);
    let calls = handler_calls(&log, 1);
    assert_eq!(calls.len(), 3, "the next tap reaches the base: {calls:?}");
}

#[test]
fn a_release_without_a_click_leaves_no_swallow_behind() {
    let log = new_log();
    let (mut router, state, _stack) = modal_setup(ModalTransition::Instant);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    router
        .push_modal("m", Blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    // The layer takes a press and its release; no Click follows (the
    // pointer moved). Later the layer closes.
    router.deliver_mouse(down(350.0, 350.0));
    router.deliver_mouse(up(360.0, 360.0));
    state.pop();
    router.deliver_mouse(mv(20.0, 20.0));
    // A Click-only tap (a shell or an assistive tool that sends just Click).
    router.deliver_mouse(click(20.0, 20.0));
    let calls = handler_calls(&log, 1);
    assert!(
        calls
            .iter()
            .any(|e| matches!(e, Ev::Mouse(MouseEvent::Click { .. }))),
        "a later Click on the base was swallowed by a press long over: {calls:?}"
    );
}

#[test]
fn a_layer_pushed_mid_press_does_not_get_the_rest_of_that_press() {
    let log = new_log();
    let (mut router, _state, _stack) = modal_setup(ModalTransition::Instant);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    let rect = r(300.0, 250.0, 100.0, 40.0);
    // The press lands on the base; the app answers it by opening a dialog
    // whose OK button happens to lie under the pointer.
    router.deliver_mouse(down(320.0, 260.0));
    let ok = Arc::new(ButtonState::new(false));
    let mut dialog = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
    dialog
        .content_mut()
        .add(None, Button::new("OK", rect).with_state(Arc::clone(&ok)));
    router.push_modal("confirm", dialog).unwrap();
    router.deliver_mouse(up(320.0, 260.0));
    router.deliver_mouse(click(320.0, 260.0));
    assert!(
        !ok.take_click(),
        "the dialog's OK was clicked by a press that began before it opened"
    );
}

// ---------------------------------------------------------------------------
// Two layers: the rest of a press after the top closes
// ---------------------------------------------------------------------------

#[test]
fn one_outside_tap_dismisses_only_the_top_of_two_layers() {
    for t in [ModalTransition::Instant, TIMED] {
        let (mut router, state, _stack) = modal_setup(t);
        router
            .push_modal("a", Blob(r(100.0, 100.0, 600.0, 400.0)))
            .unwrap();
        router
            .push_modal("b", Blob(r(300.0, 250.0, 100.0, 100.0)))
            .unwrap();
        state.advance(1.0);
        tap(&router, 10.0, 10.0);
        assert!(!state.contains("b"), "{t:?}: the tap dismissed b");
        assert!(
            state.contains("a"),
            "{t:?}: one tap dismissed both layers: {:?}",
            state.take_events()
        );
    }
}

#[test]
fn closing_the_top_dialog_on_a_press_does_not_click_the_dialog_beneath() {
    for t in [ModalTransition::Instant, TIMED] {
        let (mut router, state, _stack) = modal_setup(t);
        let rect = r(300.0, 250.0, 100.0, 40.0);
        let lower = Arc::new(ButtonState::new(false));
        let mut a = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
        a.content_mut().add(
            None,
            Button::new("Delete", rect).with_state(Arc::clone(&lower)),
        );
        router.push_modal("a", a).unwrap();
        let mut b = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
        let close = b.content_mut().add(None, Button::new("Close", rect));
        router.push_modal("b", b.with_close_control(close)).unwrap();
        state.advance(1.0);
        tap(&router, 320.0, 260.0);
        assert!(!state.contains("b"), "{t:?}: Close closed b");
        assert!(
            !lower.take_click(),
            "{t:?}: the tap that closed b also pressed a's Delete"
        );
    }
}

#[test]
fn a_layer_pushed_over_a_held_press_resets_the_lower_filter() {
    // Press inside a, push b, release and close b: a's filter forgot the
    // press, so a later Down outside a is blocked (not passed to a) and its
    // tap dismisses a.
    let log = new_log();
    let (mut router, state, stack) = modal_setup(ModalTransition::Instant);
    let mut a = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
    let p = a
        .content_mut()
        .add(None, probe(r(270.0, 220.0, 50.0, 50.0), F, 1, &log));
    router.push_modal("a", a).unwrap();
    router.deliver_mouse(down(290.0, 240.0));
    for round in 0..3 {
        router
            .push_modal("b", Blob(r(600.0, 50.0, 100.0, 100.0)))
            .unwrap();
        router.deliver_mouse(mv(290.0, 240.0));
        state.close("b");
        render(&router);
        let s = router.get::<ModalStack>(stack).unwrap();
        let d = s.modal::<ModalDialog>("a").unwrap();
        assert_eq!(d.content().active_drag(), None, "round {round}");
        assert_ne!(d.content().hovered(), Some(p), "round {round}");
    }
    router.deliver_mouse(up(290.0, 240.0));
    log.borrow_mut().clear();
    router.deliver_mouse(down(10.0, 10.0));
    router.deliver_mouse(up(10.0, 10.0));
    assert!(!state.contains("a"), "the outside tap dismissed a");
    assert!(handler_calls(&log, 1).is_empty());
}

#[test]
fn replace_top_mid_press_neither_clicks_the_new_top_nor_strands_a_grab() {
    let log = new_log();
    let (mut router, state, stack) = modal_setup(ModalTransition::Instant);
    let mut a = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
    a.content_mut()
        .add(None, probe(r(270.0, 220.0, 50.0, 50.0), F, 1, &log));
    router.push_modal("a", a).unwrap();
    router.deliver_mouse(down(290.0, 240.0));
    let mut b = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
    b.content_mut()
        .add(None, probe(r(270.0, 220.0, 50.0, 50.0), F, 2, &log));
    router.replace_top_modal("b", b).unwrap();
    render(&router);
    assert_eq!(layers(&router, stack), 1);
    assert_eq!(state.top(), state.stack().last().map(|e| e.id));
    router.deliver_mouse(up(290.0, 240.0));
    check_router(&router, "after replace");
    let s = router.get::<ModalStack>(stack).unwrap();
    let d = s.modal::<ModalDialog>("b").unwrap();
    assert_eq!(d.content().active_drag(), None, "no grab inside b");
}

// ---------------------------------------------------------------------------
// ModalStack::sync
// ---------------------------------------------------------------------------

/// A modal that counts its drops and, when dropped, asks the shared state
/// to dismiss everything.
struct Dropper {
    rect: Rect,
    drops: Rc<Cell<u32>>,
    state: Option<Arc<ModalState>>,
}

impl DrawControl for Dropper {
    fn draw(&self, _list: &mut DrawList, _ctx: &DrawContext) {}
}

impl HostedControl for Dropper {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.rect.contains(pos).then_some((ControlPart::Body, 0.0))
    }

    fn handle_mouse(&self, _e: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        None
    }

    fn handle_keyboard(&self, _e: &KeyboardEvent, _ctx: &EventContext) -> Option<ControlAction> {
        None
    }
}

impl Modal for Dropper {}

impl Drop for Dropper {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        if let Some(state) = &self.state {
            state.dismiss_all(DismissReason::App);
        }
    }
}

#[test]
fn a_queued_push_cancelled_and_re_pushed_in_one_frame_shows_the_second_modal() {
    let (mut router, state, stack) = modal_setup(TIMED);
    let drops = Rc::new(Cell::new(0));
    let rect = r(300.0, 300.0, 100.0, 100.0);
    let mk = || Dropper {
        rect,
        drops: Rc::clone(&drops),
        state: None,
    };
    router.push_modal("x", mk()).unwrap();
    state.advance(1.0);
    state.pop();
    router.push_modal("x", mk()).unwrap();
    state.dismiss_all(DismissReason::App);
    router.push_modal("x", mk()).unwrap();
    assert_eq!(drops.get(), 1, "the first queued modal was dropped");
    state.advance(1.0);
    render(&router);
    assert_eq!(state.depth(), 1);
    assert_eq!(state.phase("x"), ModalPhase::Opening);
    assert_eq!(layers(&router, stack), 1);
    assert_eq!(drops.get(), 2, "the original layer closed and dropped");
    state.advance(1.0);
    assert_eq!(state.phase("x"), ModalPhase::Open);
    state.dismiss_all(DismissReason::App);
    state.advance(1.0);
    render(&router);
    assert_eq!(layers(&router, stack), 0);
    assert_eq!(drops.get(), 3, "every modal dropped exactly once");
}

#[test]
fn a_closed_layer_whose_drop_dismisses_everything_leaves_the_stack_consistent() {
    let (mut router, state, stack) = modal_setup(ModalTransition::Instant);
    let drops = Rc::new(Cell::new(0));
    let rect = r(300.0, 300.0, 100.0, 100.0);
    router
        .push_modal(
            "a",
            Dropper {
                rect,
                drops: Rc::clone(&drops),
                state: None,
            },
        )
        .unwrap();
    router
        .push_modal(
            "b",
            Dropper {
                rect,
                drops: Rc::clone(&drops),
                state: Some(Arc::clone(&state)),
            },
        )
        .unwrap();
    state.close("b");
    // The sync that drops b runs b's Drop, which closes a too.
    render(&router);
    render(&router);
    assert_eq!(state.depth(), 0);
    assert_eq!(layers(&router, stack), 0);
    assert_eq!(drops.get(), 2);
    let log = new_log();
    router.add(None, probe(VIEWPORT, F, 1, &log));
    tap(&router, 10.0, 10.0);
    assert_eq!(
        handler_calls(&log, 1).len(),
        3,
        "input reaches the app again"
    );
}

// ---------------------------------------------------------------------------
// Focus on nested router nodes
// ---------------------------------------------------------------------------

#[test]
fn a_press_focus_inside_a_non_focusable_router_survives_an_unrelated_set_flags() {
    let log = new_log();
    let mut inner = Router::new();
    inner.add(None, probe(r(0.0, 0.0, 50.0, 50.0), F, 1, &log));
    let mut p = Router::new();
    let rid = p.add(None, inner);
    p.set_flags(rid, NodeFlags::FOCUSABLE, false);
    let other = p.add(None, probe(r(100.0, 0.0, 50.0, 50.0), F, 2, &log));
    p.deliver_mouse(down(10.0, 10.0));
    p.deliver_mouse(up(10.0, 10.0));
    assert_eq!(focus_leaf(&p), Some(1), "the press focused the inner node");
    // Anything at all: a style toggle on an unrelated node.
    p.set_flags(other, NodeFlags::TAB_THROUGH, true);
    assert_eq!(
        focus_leaf(&p),
        Some(1),
        "an unrelated set_flags took the focus away (parent focus {:?})",
        p.focused()
    );
}

#[test]
fn tab_focus_inside_a_non_focusable_tab_through_router_survives_an_unrelated_set_flags() {
    let log = new_log();
    let mut inner = Router::new();
    inner.add(None, probe(r(10.0, 0.0, 10.0, 10.0), F, 2, &log));
    let mut p = Router::new();
    let a = p.add(None, probe(r(0.0, 0.0, 10.0, 10.0), F, 1, &log));
    let rid = p.add(None, inner);
    p.set_flags(rid, NodeFlags::TAB_THROUGH, true);
    p.set_flags(rid, NodeFlags::FOCUSABLE, false);
    p.deliver_keyboard(key(Key::Tab));
    p.deliver_keyboard(key(Key::Tab));
    assert_eq!(focus_leaf(&p), Some(2));
    p.set_flags(a, NodeFlags::TAB_THROUGH, true);
    assert_eq!(
        focus_leaf(&p),
        Some(2),
        "an unrelated set_flags lost the focus"
    );
}

#[test]
fn a_redirected_press_into_a_nested_router_never_leaves_two_focused_nodes() {
    let log = new_log();
    let mut inner = Router::new();
    inner.add(None, probe(r(0.0, 0.0, 50.0, 50.0), F, 1, &log));
    let mut base = Router::new();
    let rid = base.add(None, inner);
    let b = base.add(None, probe(r(100.0, 0.0, 50.0, 50.0), F, 2, &log));
    let (p, to) = redirecting(base, false);
    p.deliver_mouse(down(110.0, 10.0));
    p.deliver_mouse(up(110.0, 10.0));
    assert_eq!(p.focused(), Some(b));
    to.set(Some(rid));
    p.deliver_mouse(down(10.0, 10.0));
    to.set(None);
    let inner_focus = p.get::<Router>(rid).unwrap().focused();
    assert!(
        !(p.focused() == Some(b) && inner_focus.is_some()),
        "parent focus {:?} and inner focus {:?} at once",
        p.focused(),
        inner_focus
    );
}

#[test]
fn set_focus_on_a_non_focusable_node_clears_the_focus_and_tells_lost() {
    let log = new_log();
    let mut p = Router::new();
    let a = p.add(None, probe(r(0.0, 0.0, 10.0, 10.0), F, 1, &log));
    let b = p.add(
        None,
        probe(r(20.0, 0.0, 10.0, 10.0), NodeFlags::NONE, 2, &log),
    );
    p.set_focus(Some(a));
    p.set_focus(Some(b));
    assert_eq!(p.focused(), None);
    assert_eq!(
        told(&log, 1)
            .into_iter()
            .filter(|e| matches!(e, Ev::Focus(_)))
            .collect::<Vec<_>>(),
        vec![Ev::Focus(FocusEvent::Gained), Ev::Focus(FocusEvent::Lost)]
    );
    assert!(told(&log, 2).is_empty());
}

// ---------------------------------------------------------------------------
// Tab walks
// ---------------------------------------------------------------------------

#[test]
fn a_trapping_router_whose_only_entries_have_no_stops_is_skipped() {
    let log = new_log();
    // Case 1: its only node is a TAB_THROUGH router with no stops.
    let mut r1 = Router::new();
    let r2 = r1.add(None, Router::new());
    r1.set_flags(r2, NodeFlags::TAB_THROUGH, true);
    // Case 2: its only node is a plain TAB_THROUGH node that is not focusable.
    let mut r3 = Router::new();
    r3.add(
        None,
        probe(r(0.0, 50.0, 10.0, 10.0), NodeFlags::TAB_THROUGH, 9, &log),
    );
    for (case, trap) in [(1, r1), (2, r3)] {
        let mut p = Router::new();
        p.add(None, probe(r(0.0, 0.0, 10.0, 10.0), F, 1, &log));
        let tid = p.add(None, trap);
        p.add(None, probe(r(20.0, 0.0, 10.0, 10.0), F, 2, &log));
        p.deliver_keyboard(key(Key::Tab));
        assert_eq!(focus_leaf(&p), Some(1));
        p.deliver_keyboard(key(Key::Tab));
        assert_ne!(
            p.focused(),
            Some(tid),
            "case {case}: Tab stopped on a trapping router with nothing to focus"
        );
        assert_eq!(focus_leaf(&p), Some(2), "case {case}");
    }
}

#[test]
fn tab_through_inside_trapping_inside_tab_through_cycles_and_leaves_correctly() {
    let log = new_log();
    let sq = |x: f32, tag: u32| probe(r(x, 0.0, 10.0, 10.0), F, tag, &log);
    // p: [1, R1(TT): [R2(trap): [R3(TT): [3, 4]]], 6]
    let mut r3 = Router::new();
    r3.add(None, sq(30.0, 3));
    r3.add(None, sq(40.0, 4));
    let mut r2 = Router::new();
    let r3_id = r2.add(None, r3);
    r2.set_flags(r3_id, NodeFlags::TAB_THROUGH, true);
    let mut r1 = Router::new();
    let r2_id = r1.add(None, r2);
    let mut p = Router::new();
    p.add(None, sq(10.0, 1));
    let r1_id = p.add(None, r1);
    p.set_flags(r1_id, NodeFlags::TAB_THROUGH, true);
    p.add(None, sq(60.0, 6));
    let mut seen = Vec::new();
    for _ in 0..7 {
        p.deliver_keyboard(key(Key::Tab));
        seen.push(focus_leaf(&p));
        check_router(&p, "walk");
    }
    // 1, then the trap (R2 as a stop), then 3, 4, 3, 4, 3: Tab is trapped.
    assert_eq!(seen[0], Some(1));
    assert_eq!(
        p.get::<Router>(r1_id).unwrap().focused(),
        Some(r2_id),
        "inside the trap"
    );
    assert_eq!(&seen[2..], &[Some(3), Some(4), Some(3), Some(4), Some(3)]);
    // The trap's stops disappear through get_mut on the innermost router.
    {
        let r1 = p.get_mut::<Router>(r1_id).unwrap();
        let r2 = r1.get_mut::<Router>(r2_id).unwrap();
        let r3 = r2.get_mut::<Router>(r3_id).unwrap();
        for id in walk(r3) {
            r3.set_flags(id, NodeFlags::DISABLED, true);
        }
    }
    check_router(&p, "stops gone");
    let mut out = Vec::new();
    for _ in 0..3 {
        p.deliver_keyboard(key(Key::Tab));
        out.push(focus_leaf(&p));
        check_router(&p, "after");
    }
    assert!(
        out.contains(&Some(6)) && out.contains(&Some(1)),
        "Tab leaves the emptied trap and walks the parent: {out:?}"
    );
}

// ---------------------------------------------------------------------------
// Redirects
// ---------------------------------------------------------------------------

#[test]
fn a_redirected_hover_to_a_node_in_a_disabled_subtree_leaves_no_stale_hover() {
    let log = new_log();
    let (mut p, to) = redirecting(Router::new(), false);
    let panel = p.add(None, probe(r(0.0, 0.0, 200.0, 200.0), F, 1, &log));
    let child = p.add(Some(panel), probe(r(10.0, 10.0, 50.0, 50.0), F, 2, &log));
    to.set(Some(child));
    p.deliver_mouse(mv(20.0, 20.0));
    assert_eq!(p.hovered(), Some(child));
    p.set_flags(panel, NodeFlags::DISABLED, true);
    check_router(&p, "after disabling the panel");
    log.borrow_mut().clear();
    p.deliver_mouse(mv(20.0, 20.0));
    assert!(handler_calls(&log, 2).is_empty(), "the child takes nothing");
    assert_ne!(p.hovered(), Some(child), "no hover inside a disabled panel");
    check_router(&p, "after the redirect");
}

#[test]
fn a_redirected_up_with_another_button_keeps_the_targets_grab() {
    let log = new_log();
    let (mut p, to) = redirecting(Router::new(), false);
    let a = p.add(None, probe(r(0.0, 0.0, 100.0, 100.0), F, 1, &log));
    p.deliver_mouse(down(10.0, 10.0));
    to.set(Some(a));
    p.deliver_mouse(up_b(10.0, 10.0, MouseButton::Right));
    to.set(None);
    assert_eq!(
        p.active_drag().map(|d| d.node),
        Some(a),
        "the left press is still held"
    );
    p.deliver_mouse(up(10.0, 10.0));
    assert_eq!(p.active_drag(), None);
}

// ---------------------------------------------------------------------------
// The filter chain's re-entrancy counter
// ---------------------------------------------------------------------------

/// Panics once, on the first event it is asked about.
struct PanicsOnce(Cell<bool>);

impl InputFilter for PanicsOnce {
    fn filter(&self, _event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        if !self.0.replace(true) {
            panic!("a rule failed");
        }
        Filtered::Pass
    }
}

struct PassAll;

impl InputFilter for PassAll {
    fn filter(&self, _event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        Filtered::Pass
    }
}

fn rules_held(stack: &FilterStack) -> usize {
    // Debug reports every entry the chain holds, dead ones included.
    let text = format!("{stack:?}");
    let digits: String = text
        .split("rules: ")
        .nth(1)
        .unwrap()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().unwrap()
}

#[test]
fn a_rule_that_panicked_once_does_not_stop_pruning_forever() {
    let stack = FilterStack::new();
    stack.add(PanicsOnce(Cell::new(false)));
    let ctx = FilterContext::default();
    let event = InputEvent::Keyboard(key(Key::Enter));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        stack.run(&event, &ctx);
    }));
    assert!(caught.is_err(), "the rule panicked");
    for _ in 0..50 {
        let rule: Rc<dyn InputFilter> = Rc::new(PassAll);
        stack.push(&rule);
        drop(rule);
        stack.run(&event, &ctx);
    }
    assert_eq!(stack.len(), 1, "one live rule");
    assert_eq!(
        rules_held(&stack),
        1,
        "dead pushed rules pile up after a panic"
    );
}

#[test]
fn dead_pushed_rules_are_pruned_after_a_nested_run_returns() {
    struct Nest {
        stack: FilterStack,
        depth: Cell<u32>,
    }
    impl InputFilter for Nest {
        fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered {
            if self.depth.replace(1) == 0 {
                self.stack.run(event, ctx);
                self.depth.set(0);
            }
            Filtered::Pass
        }
    }
    let stack = FilterStack::new();
    let dead: Rc<dyn InputFilter> = Rc::new(PassAll);
    stack.push(&dead);
    drop(dead);
    let nest: Rc<dyn InputFilter> = Rc::new(Nest {
        stack: stack.clone(),
        depth: Cell::new(0),
    });
    stack.push(&nest);
    stack.run(
        &InputEvent::Keyboard(key(Key::Enter)),
        &FilterContext::default(),
    );
    assert_eq!(rules_held(&stack), 1, "the outer run pruned once it ended");
    drop(nest);
}

// ---------------------------------------------------------------------------
// Disabled and hidden modal stacks
// ---------------------------------------------------------------------------

#[test]
fn hiding_the_stack_mid_press_still_swallows_the_rest_of_the_press() {
    let log = new_log();
    let (mut router, _state, stack) = modal_setup(ModalTransition::Instant);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    router
        .push_modal("m", Blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    router.deliver_mouse(down(350.0, 350.0));
    router.set_flags(stack, NodeFlags::HIDDEN, true);
    router.deliver_mouse(up(350.0, 350.0));
    router.deliver_mouse(click(350.0, 350.0));
    assert!(handler_calls(&log, 1).is_empty());
    tap(&router, 20.0, 20.0);
    assert_eq!(
        handler_calls(&log, 1).len(),
        3,
        "a hidden stack traps nothing"
    );
}

#[test]
fn a_disabled_stack_with_a_layer_up_lets_nothing_through_and_nothing_close_it() {
    let log = new_log();
    let (mut router, state, stack) = modal_setup(ModalTransition::Instant);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    router
        .push_modal("m", Blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    router.set_flags(stack, NodeFlags::DISABLED, true);
    tap(&router, 10.0, 10.0);
    router.deliver_keyboard(key(Key::Escape));
    router.deliver_keyboard(key(Key::Tab));
    assert!(state.contains("m"));
    assert!(handler_calls(&log, 1).is_empty());
    assert_eq!(router.focused(), None);
    router.set_flags(stack, NodeFlags::DISABLED, false);
    tap(&router, 10.0, 10.0);
    assert!(
        !state.contains("m"),
        "enabled again, an outside tap dismisses"
    );
}

// ---------------------------------------------------------------------------
// Fuzzers, with redirect filters and nested routers in play
// ---------------------------------------------------------------------------

fn fuzz_router_with_redirects(seed: u64, steps: usize, single_focus: Option<bool>) -> u32 {
    let mut rng = Lcg(seed);
    let log = new_log();
    let (mut router, to) = redirecting(Router::new(), true);
    let mut live: Vec<NodeId> = Vec::new();
    let mut nested: Vec<NodeId> = Vec::new();
    let mut removed: Vec<NodeId> = Vec::new();
    let mut tag = 0u32;
    let mut redirected = 0u32;
    for step in 0..steps {
        let op = rng.below(14);
        let what = format!("seed {seed} step {step} op {op} redirect {:?}", to.get());
        match op {
            0 | 1 => {
                let parent = if rng.coin(3) { None } else { rng.pick(&live) };
                tag += 1;
                let p = random_probe(&mut rng, tag, &log);
                live.push(router.add(parent, p));
            }
            2 => {
                let mut inner = Router::new();
                for _ in 0..rng.below(4) {
                    tag += 1;
                    let p = random_probe(&mut rng, tag, &log);
                    let ids = walk(&inner);
                    let parent = if rng.coin(2) { None } else { rng.pick(&ids) };
                    let id = inner.add(parent, p);
                    if rng.coin(4) {
                        // A router nested two deep.
                        let mut deeper = Router::new();
                        tag += 1;
                        deeper.add(None, random_probe(&mut rng, tag, &log));
                        let d = inner.add(Some(id), deeper);
                        if rng.coin(2) {
                            inner.set_flags(d, NodeFlags::TAB_THROUGH, true);
                        }
                    }
                }
                let parent = if rng.coin(2) { None } else { rng.pick(&live) };
                let id = router.add(parent, inner);
                if rng.coin(2) {
                    router.set_flags(id, NodeFlags::TAB_THROUGH, true);
                }
                if rng.coin(3) {
                    router.set_flags(id, NodeFlags::FOCUSABLE, false);
                }
                live.push(id);
                nested.push(id);
            }
            3 => {
                if let Some(id) = rng.pick(&nested) {
                    tag += 1;
                    let p = random_probe(&mut rng, tag, &log);
                    let flag = random_flags(&mut rng);
                    let on = rng.coin(2);
                    let choice = rng.below(3);
                    let pick_seed = rng.next();
                    if let Some(inner) = router.get_mut::<Router>(id) {
                        let ids = walk(inner);
                        let pick =
                            (!ids.is_empty()).then(|| ids[(pick_seed % ids.len() as u64) as usize]);
                        match (choice, pick) {
                            (0, Some(v)) => inner.remove(v),
                            (1, Some(n)) => inner.set_flags(n, flag, on),
                            _ => {
                                inner.add(pick, p);
                            }
                        }
                    }
                }
            }
            4 => {
                if let Some(victim) = rng.pick(&live) {
                    let mut doomed = vec![victim];
                    let mut i = 0;
                    while i < doomed.len() {
                        doomed.extend(router.children(Some(doomed[i])));
                        i += 1;
                    }
                    router.remove(victim);
                    removed.extend(doomed);
                    live.retain(|&id| router.contains(id));
                    nested.retain(|&id| router.contains(id));
                }
            }
            5 => {
                if let Some(id) = rng.pick(&live) {
                    let flag = random_flags(&mut rng);
                    router.set_flags(id, flag, rng.coin(2));
                }
            }
            6 => {
                let target = if rng.coin(4) {
                    rng.pick(&removed)
                } else {
                    rng.pick(&live)
                };
                router.set_focus(target);
            }
            7 => {
                // Point the redirect somewhere: a node, a nested router, a
                // removed id, or nowhere.
                let target = match rng.below(4) {
                    0 => None,
                    1 if single_focus == Some(false) => None,
                    1 => rng.pick(&nested),
                    2 => rng.pick(&removed),
                    _ => rng.pick(&live),
                };
                let into_nested = target.is_some_and(|t| nested.contains(&t));
                to.set(if into_nested && single_focus == Some(false) {
                    None
                } else {
                    target
                });
            }
            8 => {
                render(&router);
            }
            _ => {
                redirected += u32::from(to.get().is_some());
                let event = random_event(&mut rng);
                router.deliver_event(event);
                check_router(&router, &format!("{what} {event:?}"));
                if single_focus.is_some() {
                    check_single_focus(&router, &format!("{what} {event:?}"));
                }
            }
        }
        check_router(&router, &what);
        if single_focus.is_some() {
            check_single_focus(&router, &what);
        }
    }
    redirected
}

#[test]
fn router_invariants_hold_with_redirects_and_nested_routers() {
    let mut redirected = 0;
    for seed in 1..=40 {
        redirected += fuzz_router_with_redirects(seed, 400, None);
    }
    assert!(redirected > 500, "redirects exercised: {redirected}");
}

#[test]
fn only_one_leaf_is_focused_with_redirects_and_nested_routers() {
    for seed in 1..=40 {
        fuzz_router_with_redirects(seed, 400, Some(true));
    }
}

fn check_pairing(events: &[ModalEvent]) {
    let mut open: Vec<(libcontrols::Symbol, DismissReason)> = Vec::new();
    for e in events {
        match *e {
            ModalEvent::Dismissed { id, by } => open.push((id, by)),
            ModalEvent::Closed { id, by } => {
                let at = open
                    .iter()
                    .position(|&(i, b)| i == id && b == by)
                    .unwrap_or_else(|| panic!("Closed {id:?} {by:?} without Dismissed"));
                open.remove(at);
            }
            _ => {}
        }
    }
    assert!(open.is_empty(), "dismissed but never closed: {open:?}");
}

/// Base probes are tags 1 (a full-viewport leaf) and 2..=4 (inside a nested
/// base router); dialog content probes are tags 10 and up.
fn fuzz_modals_with_redirects(seed: u64, steps: usize) {
    let mut rng = Lcg(seed);
    let state = Arc::new(ModalState::new().with_transition(TIMED));
    let (mut router, to) = redirecting(Router::new(), true);
    let stack = ModalStack::install(&mut router, Arc::clone(&state), VIEWPORT).unwrap();
    let log = new_log();
    let base = router.add(None, probe(VIEWPORT, F, 1, &log));
    let mut inner = Router::new();
    inner.add(None, probe(r(0.0, 0.0, 100.0, 100.0), F, 2, &log));
    inner.add(None, probe(r(200.0, 200.0, 100.0, 100.0), F, 3, &log));
    let deep = inner.add(None, Router::new());
    inner
        .get_mut::<Router>(deep)
        .unwrap()
        .add(None, probe(r(400.0, 0.0, 50.0, 50.0), F, 4, &log));
    let nested = router.add(None, inner);
    let targets = [Some(base), Some(nested), Some(stack), None, None];
    let ids = ["a", "b", "c"];
    let transitions = [
        ModalTransition::Instant,
        TIMED,
        ModalTransition::Timed {
            open_secs: 0.0,
            close_secs: 0.05,
        },
        ModalTransition::Driven,
    ];
    let rects = [
        r(100.0, 100.0, 200.0, 200.0),
        r(250.0, 150.0, 200.0, 200.0),
        r(400.0, 300.0, 100.0, 100.0),
    ];
    let mut events = Vec::new();
    let mut tag = 10u32;
    for step in 0..steps {
        let what = format!("seed {seed} step {step}");
        let id = ids[rng.below(3)];
        let rect = rects[rng.below(3)];
        match rng.below(15) {
            0 | 1 => {
                let t = transitions[rng.below(4)];
                let res = if rng.coin(2) {
                    let mut dialog = ModalDialog::new(VIEWPORT, rect, rect.pad(-5.0));
                    let mut content = Router::new();
                    tag += 1;
                    content.add(None, probe(rect.pad(-10.0), F, tag, &log));
                    let c = dialog.content_mut().add(None, content);
                    if rng.coin(2) {
                        dialog
                            .content_mut()
                            .set_flags(c, NodeFlags::TAB_THROUGH, true);
                    }
                    router.push_modal_with(id, dialog, t)
                } else {
                    router.push_modal_with(id, Blob(rect), t)
                };
                let _ = res;
            }
            2 => {
                let _ = router.replace_top_modal(id, Blob(rect));
            }
            3 => state.pop(),
            4 => state.close(id),
            5 => state.dismiss_all(DismissReason::App),
            6 => {
                let dt = [0.0, 0.03, 1.0, f32::NAN][rng.below(4)];
                state.advance(dt);
            }
            7 => state.set_top_progress([0.0, 0.5, 1.0][rng.below(3)]),
            8 => {
                render(&router);
            }
            9 => to.set(targets[rng.below(targets.len())]),
            10 => {
                let flag = [NodeFlags::DISABLED, NodeFlags::HIDDEN][rng.below(2)];
                let on = rng.coin(3);
                router.set_flags(stack, flag, on);
            }
            _ => {
                let event = random_event(&mut rng);
                let capturing = state.captures_input()
                    && !router
                        .flags(stack)
                        .intersects(NodeFlags::HIDDEN | NodeFlags::DISABLED);
                let before = log.borrow().len();
                router.deliver_event(event);
                if capturing && to.get().is_none() {
                    let leaked: Vec<(u32, Ev)> = log.borrow()[before..]
                        .iter()
                        .filter(|(t, e)| {
                            *t < 10 && matches!(e, Ev::Mouse(_) | Ev::Key(_) | Ev::Gesture(_))
                        })
                        .cloned()
                        .collect();
                    assert!(
                        leaked.is_empty(),
                        "{what}: {event:?} reached the app under a layer: {leaked:?}"
                    );
                }
            }
        }
        events.extend(state.take_events());
        assert_eq!(
            layers(&router, stack),
            state.depth(),
            "{what}: one layer per entry"
        );
        check_router(&router, &what);
    }
    router.set_flags(stack, NodeFlags::HIDDEN | NodeFlags::DISABLED, false);
    state.dismiss_all(DismissReason::App);
    for _ in 0..3 {
        for e in state.stack() {
            state.set_progress(e.id, 0.0);
        }
        state.advance(10.0);
    }
    render(&router);
    events.extend(state.take_events());
    assert_eq!(state.depth(), 0, "seed {seed}: everything closed");
    assert_eq!(layers(&router, stack), 0);
    check_pairing(&events);
}

#[test]
fn modal_invariants_hold_with_redirects_nested_routers_and_flagged_stacks() {
    for seed in 1..=40 {
        fuzz_modals_with_redirects(seed, 300);
    }
}

#[test]
fn only_one_leaf_is_focused_when_no_redirect_targets_a_nested_router() {
    for seed in 1..=40 {
        fuzz_router_with_redirects(seed, 400, Some(false));
    }
}
