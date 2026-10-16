//! Adversarial tests: invariants under arbitrary operation sequences, removal
//! and flag changes during routing, filter-chain re-entrancy, modal-stack
//! phases and degenerate inputs. Each test that first exposed a defect now
//! guards its fix.

mod common;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use libcontrols::{
    Button, ButtonState, ControlAction, ControlPart, DismissReason, DrawContext, DrawControl,
    EventContext, FilterContext, FilterStack, Filtered, FocusEvent, GestureEvent, HostedControl,
    InputEvent, InputFilter, Key, KeyboardEvent, LayerContext, LayerFilter, LayerMask, LayerShape,
    Modal, ModalDialog, ModalEvent, ModalLayerFilter, ModalPhase, ModalStack, ModalState,
    ModalTransition, Modifiers, MouseButton, MouseEvent, NodeFlags, NodeId, NodeState, Rect,
    Router, ShapedLayerFilter,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Something a `Probe` was told.
#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Mouse(MouseEvent),
    Key(KeyboardEvent),
    Gesture(GestureEvent),
    Hover(Option<ControlPart>),
    Focus(FocusEvent),
}

/// A log shared outside the router, so it survives a node's removal.
type Log = Rc<RefCell<Vec<(u32, Ev)>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

/// What the probe tagged `tag` was told, in order.
fn told(log: &Log, tag: u32) -> Vec<Ev> {
    log.borrow()
        .iter()
        .filter(|(t, _)| *t == tag)
        .map(|(_, e)| e.clone())
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
            color: [1.0, 1.0, 1.0, 1.0],
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

const F: NodeFlags = NodeFlags::FOCUSABLE;
const VIEWPORT: Rect = Rect::new(0.0, 0.0, 800.0, 600.0);

fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, w, h)
}

fn down(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Down {
        pos: [x, y],
        button: MouseButton::Left,
    }
}

fn up(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Up {
        pos: [x, y],
        button: MouseButton::Left,
    }
}

fn click(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Click {
        pos: [x, y],
        button: MouseButton::Left,
    }
}

fn mv(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Move { pos: [x, y] }
}

fn key(k: Key) -> KeyboardEvent {
    KeyboardEvent::KeyDown { key: k }
}

fn render(router: &Router) -> DrawList {
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
    list
}

/// A node and every ancestor above it, innermost first (the node excluded).
fn ancestors(router: &Router, id: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut at = router.parent(id);
    while let Some(p) = at {
        out.push(p);
        at = router.parent(p);
    }
    out
}

/// Whether the node or any ancestor declares any of `mask`.
fn blocked(router: &Router, id: NodeId, mask: NodeFlags) -> bool {
    router.flags(id).intersects(mask)
        || ancestors(router, id)
            .iter()
            .any(|&a| router.flags(a).intersects(mask))
}

/// Every node in pre-order.
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
    for root in router.children(None) {
        assert_eq!(router.parent(root), None, "{what}: a root has no parent");
    }
    for &n in &nodes {
        for c in router.children(Some(n)) {
            assert_eq!(router.parent(c), Some(n), "{what}: parent links agree");
        }
    }
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
        assert_eq!(
            s.contains(NodeState::FOCUSED),
            focused == Some(n),
            "{what}: FOCUSED on {n} only if focused ({s:?})"
        );
        assert_eq!(
            s.contains(NodeState::FOCUS_WITHIN),
            fa.contains(&n),
            "{what}: FOCUS_WITHIN on {n} exactly on the focused node's ancestors ({s:?})"
        );
        assert_eq!(
            s.contains(NodeState::HOVERED),
            hovered == Some(n),
            "{what}: HOVERED on {n} only if hovered ({s:?})"
        );
        assert_eq!(
            s.contains(NodeState::HOVER_WITHIN),
            ha.contains(&n),
            "{what}: HOVER_WITHIN on {n} exactly on the hovered node's ancestors ({s:?})"
        );
        assert_eq!(
            s.contains(NodeState::GRABBED),
            grab == Some(n),
            "{what}: GRABBED on {n} only if it holds the grab ({s:?})"
        );
        if let Some(inner) = router.get::<Router>(n) {
            check_router(inner, &format!("{what} > {n}"));
        }
    }
}

/// The id is gone for good: nothing about it is answered.
fn check_gone(router: &Router, id: NodeId) {
    assert!(!router.contains(id), "{id} is gone");
    assert_eq!(router.state(id), NodeState::NONE, "{id} has no state");
    assert_eq!(router.flags(id), NodeFlags::NONE, "{id} has no flags");
    assert_eq!(router.find(&id.to_string()), None, "{id} is not found");
    assert_eq!(router.parent(id), None);
    assert_eq!(router.children(Some(id)).count(), 0);
    assert_ne!(router.focused(), Some(id));
    assert_ne!(router.hovered(), Some(id));
    assert_ne!(router.active_drag().map(|d| d.node), Some(id));
}

/// A deterministic pseudo-random sequence (Knuth's MMIX LCG).
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

    fn coord(&mut self) -> f32 {
        match self.below(40) {
            0 => f32::NAN,
            1 => f32::INFINITY,
            2 => -1.0e30,
            _ => (self.below(42) * 10) as f32,
        }
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        (!items.is_empty()).then(|| items[self.below(items.len())])
    }
}

fn random_flags(rng: &mut Lcg) -> NodeFlags {
    [
        NodeFlags::FOCUSABLE,
        NodeFlags::DISABLED,
        NodeFlags::HIDDEN,
        NodeFlags::TAB_THROUGH,
    ][rng.below(4)]
}

fn random_probe(rng: &mut Lcg, tag: u32, log: &Log) -> Probe {
    let x = (rng.below(30) * 10) as f32;
    let y = (rng.below(30) * 10) as f32;
    let w = (3 + rng.below(15)) as f32 * 10.0;
    let h = (3 + rng.below(15)) as f32 * 10.0;
    let flags = if rng.coin(4) { NodeFlags::NONE } else { F };
    probe(r(x, y, w, h), flags, tag, log)
}

fn random_event(rng: &mut Lcg) -> InputEvent {
    let pos = [rng.coord(), rng.coord()];
    let button = if rng.coin(5) {
        MouseButton::Right
    } else {
        MouseButton::Left
    };
    match rng.below(14) {
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
        8 => InputEvent::Mouse(MouseEvent::Scroll {
            pos,
            delta: [0.0, 1.0],
        }),
        9 => InputEvent::Keyboard(key(if rng.coin(2) { Key::Tab } else { Key::BackTab })),
        10 => InputEvent::Keyboard(key(
            [Key::Escape, Key::Enter, Key::Char('a'), Key::Back][rng.below(4)]
        )),
        11 => InputEvent::Keyboard(KeyboardEvent::KeyUp { key: Key::Tab }),
        12 => InputEvent::Gesture(GestureEvent::Pinch { delta: 0.5 }),
        _ => InputEvent::Modifiers(Modifiers {
            shift: rng.coin(2),
            ..Modifiers::default()
        }),
    }
}

// ---------------------------------------------------------------------------
// Router: invariants under arbitrary sequences
// ---------------------------------------------------------------------------

/// How often the fuzzer saw focus, hover, a grab, and a focused node inside
/// a nested router, so a passing run is known to have exercised them.
#[derive(Default)]
struct Coverage {
    focus: u32,
    hover: u32,
    grab: u32,
    nested_focus: u32,
}

fn fuzz_router(seed: u64, steps: usize, cov: &mut Coverage) {
    let mut rng = Lcg(seed);
    let log = new_log();
    let mut router = Router::new();
    let mut live: Vec<NodeId> = Vec::new();
    let mut nested: Vec<NodeId> = Vec::new();
    let mut removed: Vec<NodeId> = Vec::new();
    let mut tag = 0u32;
    for step in 0..steps {
        let what = format!("seed {seed} step {step}");
        match rng.below(13) {
            0 | 1 => {
                let parent = if rng.coin(3) { None } else { rng.pick(&live) };
                tag += 1;
                let p = random_probe(&mut rng, tag, &log);
                live.push(router.add(parent, p));
            }
            2 => {
                // A nested router with a few nodes of its own.
                let mut inner = Router::new();
                for _ in 0..rng.below(4) {
                    tag += 1;
                    let p = random_probe(&mut rng, tag, &log);
                    let parent = {
                        let ids: Vec<NodeId> = walk(&inner);
                        if rng.coin(2) { None } else { rng.pick(&ids) }
                    };
                    inner.add(parent, p);
                }
                let parent = if rng.coin(2) { None } else { rng.pick(&live) };
                let id = router.add(parent, inner);
                if rng.coin(2) {
                    router.set_flags(id, NodeFlags::TAB_THROUGH, true);
                }
                live.push(id);
                nested.push(id);
            }
            3 => {
                // Edit inside a nested router through get_mut.
                if let Some(id) = rng.pick(&nested) {
                    tag += 1;
                    let p = random_probe(&mut rng, tag, &log);
                    let remove = rng.coin(3);
                    let flag = random_flags(&mut rng);
                    let on = rng.coin(2);
                    if let Some(inner) = router.get_mut::<Router>(id) {
                        let ids = walk(inner);
                        let pick = rng.pick(&ids);
                        if remove {
                            if let Some(victim) = pick {
                                inner.remove(victim);
                            }
                        } else if rng.coin(2) {
                            if let Some(n) = pick {
                                inner.set_flags(n, flag, on);
                            }
                        } else {
                            inner.add(pick, p);
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
            7 => router.clear_hover(),
            8 => {
                render(&router);
            }
            _ => {
                router.deliver_event(random_event(&mut rng));
            }
        }
        check_router(&router, &what);
        cov.focus += u32::from(router.focused().is_some());
        cov.hover += u32::from(router.hovered().is_some());
        cov.grab += u32::from(router.active_drag().is_some());
        cov.nested_focus += u32::from(nested.iter().any(|&n| {
            router
                .get::<Router>(n)
                .is_some_and(|i| i.focused().is_some())
        }));
        for &id in removed.iter().rev().take(8) {
            check_gone(&router, id);
        }
    }
    // Every id handed out was distinct.
    let mut all: Vec<NodeId> = live.iter().chain(removed.iter()).copied().collect();
    all.sort();
    let n = all.len();
    all.dedup();
    assert_eq!(all.len(), n, "ids are never reused");
}

#[test]
fn router_state_invariants_hold_after_every_random_operation() {
    let mut cov = Coverage::default();
    for seed in 1..=40 {
        fuzz_router(seed, 400, &mut cov);
    }
    assert!(
        cov.focus > 100 && cov.hover > 100 && cov.grab > 100 && cov.nested_focus > 10,
        "the fuzzer exercised focus {}, hover {}, grab {}, nested focus {}",
        cov.focus,
        cov.hover,
        cov.grab,
        cov.nested_focus
    );
}

#[test]
fn a_reused_slot_carries_no_stale_state_flags_alias_or_grab() {
    let log = new_log();
    let mut router = Router::new();
    let panel = router
        .add_named(None, "panel", probe(r(0.0, 0.0, 200.0, 200.0), F, 1, &log))
        .unwrap();
    let child = router
        .add_named(
            Some(panel),
            "child",
            probe(r(10.0, 10.0, 50.0, 50.0), F, 2, &log),
        )
        .unwrap();
    router.set_flags(child, NodeFlags::TAB_THROUGH, true);
    router.deliver_mouse(down(20.0, 20.0));
    assert_eq!(router.focused(), Some(child));
    assert_eq!(router.hovered(), Some(child));
    assert_eq!(router.active_drag().map(|d| d.node), Some(child));
    log.borrow_mut().clear();

    router.remove(panel);
    let child_told = told(&log, 2);
    assert!(
        child_told.contains(&Ev::Focus(FocusEvent::Lost)),
        "the focused node heard Lost before leaving: {child_told:?}"
    );
    assert!(
        child_told.contains(&Ev::Hover(None)),
        "the hovered node heard the leave before leaving: {child_told:?}"
    );
    assert!(told(&log, 1).contains(&Ev::Hover(None)), "and its ancestor");
    check_gone(&router, panel);
    check_gone(&router, child);

    // New nodes take the freed slots.
    let fresh = router.add(
        None,
        probe(r(0.0, 0.0, 200.0, 200.0), NodeFlags::NONE, 3, &log),
    );
    let fresh_child = router.add(
        Some(fresh),
        probe(r(10.0, 10.0, 50.0, 50.0), NodeFlags::NONE, 4, &log),
    );
    assert!(fresh > child && fresh_child > fresh, "ids only grow");
    assert_eq!(router.state(fresh), NodeState::NONE);
    assert_eq!(router.state(fresh_child), NodeState::NONE);
    assert_eq!(
        router.flags(fresh_child),
        NodeFlags::NONE,
        "no TAB_THROUGH left"
    );
    assert_eq!(router.find("panel"), None);
    assert_eq!(router.find("child"), None);
    assert!(router.add_named(None, "child", Router::new()).is_ok());

    // The old grab does not follow into the slot: a move reaches no handler.
    log.borrow_mut().clear();
    router.deliver_mouse(mv(500.0, 500.0));
    router.deliver_mouse(MouseEvent::Drag {
        start_pos: [20.0, 20.0],
        current_pos: [500.0, 500.0],
        delta: [1.0, 1.0],
        button: MouseButton::Left,
    });
    assert!(
        told(&log, 3).is_empty() && told(&log, 4).is_empty(),
        "nothing reaches the new nodes in the old slots"
    );
    check_router(&router, "after reuse");
}

#[test]
fn removing_the_grabbed_node_mid_drag_drops_the_grab_cleanly() {
    let log = new_log();
    let mut router = Router::new();
    let a = router.add(None, probe(r(0.0, 0.0, 100.0, 100.0), F, 1, &log));
    router.deliver_mouse(down(10.0, 10.0));
    router.deliver_mouse(MouseEvent::Drag {
        start_pos: [10.0, 10.0],
        current_pos: [300.0, 10.0],
        delta: [290.0, 0.0],
        button: MouseButton::Left,
    });
    router.remove(a);
    assert_eq!(router.active_drag(), None);
    let b = router.add(None, probe(r(0.0, 0.0, 100.0, 100.0), F, 2, &log));
    router.deliver_mouse(mv(50.0, 50.0));
    assert!(
        !told(&log, 2).iter().any(|e| matches!(e, Ev::Mouse(_))),
        "a plain move outside any grab reaches no handler"
    );
    router.deliver_mouse(up(50.0, 50.0));
    assert_eq!(router.state(b) & NodeState::GRABBED, NodeState::NONE);
    check_router(&router, "after removing the grab");
}

#[test]
fn disabling_or_hiding_an_ancestor_drops_focus_grab_and_hover_beneath_it() {
    let log = new_log();
    let mut router = Router::new();
    let panel = router.add(None, probe(r(0.0, 0.0, 200.0, 200.0), F, 1, &log));
    let child = router.add(Some(panel), probe(r(10.0, 10.0, 50.0, 50.0), F, 2, &log));
    router.deliver_mouse(down(20.0, 20.0));
    assert_eq!(router.focused(), Some(child));
    router.set_flags(panel, NodeFlags::DISABLED, true);
    assert_eq!(
        router.focused(),
        None,
        "focus cannot stay in a disabled subtree"
    );
    assert_eq!(router.active_drag(), None, "nor the grab");
    assert!(told(&log, 2).contains(&Ev::Focus(FocusEvent::Lost)));
    router.set_flags(panel, NodeFlags::DISABLED, false);
    router.deliver_mouse(mv(20.0, 20.0));
    assert_eq!(router.hovered(), Some(child));
    router.set_flags(panel, NodeFlags::HIDDEN, true);
    assert_eq!(
        router.hovered(),
        None,
        "hover cannot stay in a hidden subtree"
    );
    check_router(&router, "after hiding");
}

#[test]
fn disabling_an_ancestor_moves_hover_off_its_descendants() {
    let log = new_log();
    let mut router = Router::new();
    let panel = router.add(None, probe(r(0.0, 0.0, 200.0, 200.0), F, 1, &log));
    let child = router.add(Some(panel), probe(r(10.0, 10.0, 50.0, 50.0), F, 2, &log));
    router.deliver_mouse(mv(20.0, 20.0));
    assert_eq!(router.hovered(), Some(child));
    router.set_flags(panel, NodeFlags::DISABLED, true);
    // The descent now stops at the disabled panel: the child is not under
    // the pointer as far as routing is concerned.
    assert_ne!(router.hovered(), Some(child));
    assert!(!router.state(child).contains(NodeState::HOVERED));
}

#[test]
fn an_unrelated_set_flags_does_not_change_the_focus() {
    let log = new_log();
    let mut router = Router::new();
    let label = router.add(
        None,
        probe(r(0.0, 0.0, 50.0, 50.0), NodeFlags::NONE, 1, &log),
    );
    let other = router.add(None, probe(r(100.0, 0.0, 50.0, 50.0), F, 2, &log));
    router.set_focus(Some(label));
    let before = router.focused();
    router.set_flags(other, NodeFlags::TAB_THROUGH, true);
    assert_eq!(
        router.focused(),
        before,
        "set_focus should either refuse a non-focusable node or keep it"
    );
}

#[test]
fn tab_with_no_stops_left_clears_the_focus_and_never_panics() {
    let log = new_log();
    let mut router = Router::new();
    for k in [Key::Tab, Key::BackTab] {
        router.deliver_keyboard(key(k));
    }
    assert_eq!(router.focused(), None, "an empty router has no stop");
    let a = router.add(None, probe(r(0.0, 0.0, 10.0, 10.0), F, 1, &log));
    let b = router.add(None, probe(r(20.0, 0.0, 10.0, 10.0), F, 2, &log));
    router.deliver_keyboard(key(Key::Tab));
    assert_eq!(router.focused(), Some(a));
    router.set_flags(a, NodeFlags::DISABLED, true);
    router.set_flags(b, NodeFlags::HIDDEN, true);
    assert_eq!(router.focused(), None);
    for k in [Key::Tab, Key::BackTab, Key::Tab] {
        router.deliver_keyboard(key(k));
        assert_eq!(router.focused(), None);
    }
    router.remove(a);
    router.remove(b);
    router.deliver_keyboard(key(Key::BackTab));
    assert_eq!(router.focused(), None);
    check_router(&router, "empty");
}

/// The tag of the innermost focused probe, descending through routers.
fn focus_leaf(router: &Router) -> Option<u32> {
    let f = router.focused()?;
    if let Some(inner) = router.get::<Router>(f) {
        return focus_leaf(inner);
    }
    router.get::<Probe>(f).map(|p| p.tag)
}

/// `[a, R1(TT): [b, R2(TT): [c, d], e], f]`, tags 1..=6.
fn two_deep(log: &Log) -> (Router, NodeId, NodeId) {
    let sq = |x: f32, tag: u32| probe(r(x, 0.0, 10.0, 10.0), F, tag, log);
    let mut r2 = Router::new();
    r2.add(None, sq(30.0, 3));
    r2.add(None, sq(40.0, 4));
    let mut r1 = Router::new();
    r1.add(None, sq(20.0, 2));
    let r2_id = r1.add(None, r2);
    r1.set_flags(r2_id, NodeFlags::TAB_THROUGH, true);
    r1.add(None, sq(50.0, 5));
    let mut p = Router::new();
    p.add(None, sq(10.0, 1));
    let r1_id = p.add(None, r1);
    p.set_flags(r1_id, NodeFlags::TAB_THROUGH, true);
    p.add(None, sq(60.0, 6));
    (p, r1_id, r2_id)
}

#[test]
fn tab_through_nested_two_deep_walks_every_stop_in_both_directions() {
    let log = new_log();
    let (p, r1, r2) = two_deep(&log);
    let mut seen = Vec::new();
    for _ in 0..8 {
        p.deliver_keyboard(key(Key::Tab));
        seen.push(focus_leaf(&p));
    }
    let expect: Vec<Option<u32>> = [1, 2, 3, 4, 5, 6, 1, 2].iter().map(|&t| Some(t)).collect();
    assert_eq!(seen, expect, "forward");
    // After leaving R1 at the wrap, nothing stays focused inside R2.
    let inner1 = p.get::<Router>(r1).unwrap();
    let inner2 = inner1.get::<Router>(r2).unwrap();
    assert_eq!(inner2.focused(), None, "R2 forgot its focus when left");

    p.set_focus(None);
    assert_eq!(inner1.focused(), None, "Lost reaches the nested router");
    let mut back = Vec::new();
    for _ in 0..7 {
        p.deliver_keyboard(key(Key::BackTab));
        back.push(focus_leaf(&p));
    }
    let expect: Vec<Option<u32>> = [6, 5, 4, 3, 2, 1, 6].iter().map(|&t| Some(t)).collect();
    assert_eq!(back, expect, "backward");
    check_router(&p, "two deep");
}

#[test]
fn tab_leaves_an_empty_trapping_nested_router() {
    let log = new_log();
    let mut p = Router::new();
    p.add(None, probe(r(0.0, 0.0, 10.0, 10.0), F, 1, &log));
    p.add(None, Router::new());
    p.add(None, probe(r(20.0, 0.0, 10.0, 10.0), F, 2, &log));
    let mut reached = false;
    for _ in 0..4 {
        p.deliver_keyboard(key(Key::Tab));
        reached |= focus_leaf(&p) == Some(2);
    }
    assert!(reached, "Tab is stuck on a router with nothing to focus");
}

#[test]
fn removing_a_focused_nested_router_tells_its_focused_node_lost() {
    let log = new_log();
    let mut inner = Router::new();
    inner.add(None, probe(r(0.0, 0.0, 10.0, 10.0), F, 1, &log));
    let mut p = Router::new();
    let rid = p.add(None, inner);
    p.deliver_keyboard(key(Key::Tab));
    p.deliver_keyboard(key(Key::Tab));
    assert_eq!(focus_leaf(&p), Some(1));
    p.remove(rid);
    assert_eq!(
        told(&log, 1).last(),
        Some(&Ev::Focus(FocusEvent::Lost)),
        "the inner focused node heard Lost before its router left"
    );
    assert_eq!(p.focused(), None);
}

#[test]
fn a_press_inside_a_nested_router_never_leaves_two_focused_nodes() {
    let log = new_log();
    let mut inner = Router::new();
    inner.add(None, probe(r(0.0, 0.0, 50.0, 50.0), F, 1, &log));
    let mut p = Router::new();
    let rid = p.add(None, inner);
    let b = p.add(None, probe(r(100.0, 0.0, 50.0, 50.0), F, 2, &log));
    p.set_flags(rid, NodeFlags::FOCUSABLE, false);
    p.deliver_mouse(down(110.0, 10.0));
    p.deliver_mouse(up(110.0, 10.0));
    assert_eq!(p.focused(), Some(b));
    p.deliver_mouse(down(10.0, 10.0));
    let inner_focus = p.get::<Router>(rid).unwrap().focused();
    // Keys go to `b`, yet the inner node is FOCUSED and draws its ring.
    assert!(
        !(p.focused() == Some(b) && inner_focus.is_some()),
        "parent focus {:?} and inner focus {:?} at once",
        p.focused(),
        inner_focus
    );
}

#[test]
fn tab_through_works_on_a_router_node_that_is_not_itself_a_stop() {
    let log = new_log();
    let mut inner = Router::new();
    inner.add(None, probe(r(10.0, 0.0, 10.0, 10.0), F, 2, &log));
    let mut p = Router::new();
    p.add(None, probe(r(0.0, 0.0, 10.0, 10.0), F, 1, &log));
    let rid = p.add(None, inner);
    p.set_flags(rid, NodeFlags::TAB_THROUGH, true);
    p.set_flags(rid, NodeFlags::FOCUSABLE, false);
    p.add(None, probe(r(20.0, 0.0, 10.0, 10.0), F, 3, &log));
    p.deliver_keyboard(key(Key::Tab));
    p.deliver_keyboard(key(Key::Tab));
    assert_eq!(focus_leaf(&p), Some(2), "its stops join the parent's order");
}

#[test]
fn non_finite_pointer_positions_hit_nothing_and_never_panic() {
    let log = new_log();
    let mut router = Router::new();
    let a = router.add(None, probe(r(0.0, 0.0, 100.0, 100.0), F, 1, &log));
    let mut inner = Router::new();
    inner.add(None, probe(r(200.0, 0.0, 100.0, 100.0), F, 2, &log));
    router.add(None, inner);
    router.deliver_mouse(down(10.0, 10.0));
    router.deliver_mouse(up(10.0, 10.0));
    assert_eq!(router.focused(), Some(a));
    for v in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
        -f32::MAX,
    ] {
        for e in [mv(v, v), down(v, 10.0), up(10.0, v), click(v, v)] {
            router.deliver_mouse(e);
            check_router(&router, "non-finite");
        }
        router.deliver_gesture(GestureEvent::Rotate { delta: v });
        router.deliver_mouse(MouseEvent::Scroll {
            pos: [v, v],
            delta: [v, v],
        });
    }
    router.deliver_mouse(mv(f32::NAN, f32::NAN));
    assert_eq!(router.hovered(), None, "NaN is over nothing");
    assert_eq!(
        router.focused(),
        None,
        "a press on nothing cleared the focus"
    );
    render(&router);
}

#[test]
fn malformed_id_names_find_nothing() {
    let mut router = Router::new();
    let a = router.add(None, Router::new());
    for name in [
        "@",
        "@-1",
        "@+1",
        "@1x",
        "@ 1",
        "@0",
        "@99999999999999999999999",
        "",
        "@@1",
    ] {
        assert_eq!(router.find(name), None, "{name:?}");
    }
    assert_eq!(router.find(&a.to_string()), Some(a));
    // An id has one spelling: a leading zero names nothing.
    assert_eq!(router.find(&format!("@0{}", a.get())), None);
}

#[test]
fn adding_under_a_removed_parent_silently_makes_a_root() {
    // An API trap rather than a crash: the stale parent is not reported.
    let mut router = Router::new();
    let gone = router.add(None, Router::new());
    router.remove(gone);
    let orphan = router.add(Some(gone), Router::new());
    assert_eq!(router.parent(orphan), None);
    assert!(router.children(None).any(|c| c == orphan));
}

// ---------------------------------------------------------------------------
// Filters
// ---------------------------------------------------------------------------

/// Counts its calls and answers `answer`.
struct Counter {
    calls: Rc<Cell<u32>>,
    answer: Filtered,
}

impl InputFilter for Counter {
    fn filter(&self, _event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        self.calls.set(self.calls.get() + 1);
        self.answer.clone()
    }
}

/// On its first call, adds an owned rule and pushes one onto its own chain.
struct Grower {
    stack: FilterStack,
    pushed: Rc<dyn InputFilter>,
    added: Rc<Cell<u32>>,
    done: Cell<bool>,
}

impl InputFilter for Grower {
    fn filter(&self, _event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        if !self.done.replace(true) {
            self.stack.add(Counter {
                calls: Rc::clone(&self.added),
                answer: Filtered::Pass,
            });
            self.stack.push(&self.pushed);
        }
        Filtered::Pass
    }
}

#[test]
fn rules_added_or_pushed_by_a_running_rule_are_asked_from_the_next_event() {
    let stack = FilterStack::new();
    let added = Rc::new(Cell::new(0));
    let pushed_calls = Rc::new(Cell::new(0));
    let pushed: Rc<dyn InputFilter> = Rc::new(Counter {
        calls: Rc::clone(&pushed_calls),
        answer: Filtered::Pass,
    });
    stack.add(Grower {
        stack: stack.clone(),
        pushed: Rc::clone(&pushed),
        added: Rc::clone(&added),
        done: Cell::new(false),
    });
    let log = new_log();
    let mut router = Router::new().with_filter_stack(stack.clone());
    router.add(None, probe(r(0.0, 0.0, 10.0, 10.0), F, 1, &log));
    router.deliver_mouse(down(5.0, 5.0));
    assert_eq!((added.get(), pushed_calls.get()), (0, 0), "not this event");
    assert!(router.focused().is_some(), "the event still routed");
    router.deliver_mouse(up(5.0, 5.0));
    assert_eq!((added.get(), pushed_calls.get()), (1, 1), "from the next");
    assert_eq!(stack.len(), 3);
}

/// Drops the last `Rc` of another rule when asked.
struct Dropper(RefCell<Option<Rc<dyn InputFilter>>>);

impl InputFilter for Dropper {
    fn filter(&self, _event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        self.0.borrow_mut().take();
        Filtered::Pass
    }
}

#[test]
fn a_pushed_rule_dropped_by_a_rule_above_it_mid_run_is_skipped_and_pruned() {
    let stack = FilterStack::new();
    let calls = Rc::new(Cell::new(0));
    let victim: Rc<dyn InputFilter> = Rc::new(Counter {
        calls: Rc::clone(&calls),
        answer: Filtered::Consumed(None),
    });
    stack.push(&victim);
    stack.add(Dropper(RefCell::new(Some(victim))));
    let resp = stack.run(
        &InputEvent::Keyboard(key(Key::Enter)),
        &FilterContext::default(),
    );
    assert_eq!(resp, Filtered::Pass, "the dropped rule was not asked");
    assert_eq!(calls.get(), 0);
    assert_eq!(stack.len(), 1);
}

/// Runs its own chain once from inside its filter (a rule that asks "the
/// rest of the chain" for a synthesized event), counting its outer calls.
struct Reenter {
    stack: RefCell<Option<FilterStack>>,
    inside: Cell<bool>,
    outer_calls: Rc<Cell<u32>>,
}

impl InputFilter for Reenter {
    fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered {
        if self.inside.get() {
            return Filtered::Pass;
        }
        self.outer_calls.set(self.outer_calls.get() + 1);
        self.inside.set(true);
        let stack = self.stack.borrow().clone();
        if let Some(stack) = stack {
            stack.run(event, ctx);
        }
        self.inside.set(false);
        Filtered::Pass
    }
}

#[test]
fn a_reentrant_run_never_asks_a_rule_twice_in_one_pass() {
    let stack = FilterStack::new();
    let dead: Rc<dyn InputFilter> = Rc::new(Counter {
        calls: Rc::new(Cell::new(0)),
        answer: Filtered::Pass,
    });
    stack.push(&dead);
    let below = Rc::new(Cell::new(0));
    stack.add(Counter {
        calls: Rc::clone(&below),
        answer: Filtered::Pass,
    });
    let outer_calls = Rc::new(Cell::new(0));
    stack.add(Reenter {
        stack: RefCell::new(Some(stack.clone())),
        inside: Cell::new(false),
        outer_calls: Rc::clone(&outer_calls),
    });
    drop(dead);
    stack.run(
        &InputEvent::Keyboard(key(Key::Enter)),
        &FilterContext::default(),
    );
    assert_eq!(outer_calls.get(), 1, "the top rule was asked once per pass");
}

/// Redirects mouse events to a node while `to` is set; `only_up` limits it
/// to releases.
struct Redirector {
    to: Rc<Cell<Option<NodeId>>>,
    only_up: bool,
    keys: bool,
}

impl InputFilter for Redirector {
    fn filter(&self, event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        let Some(id) = self.to.get() else {
            return Filtered::Pass;
        };
        match event {
            InputEvent::Mouse(MouseEvent::Up { .. }) => Filtered::Redirect(id),
            InputEvent::Mouse(_) if !self.only_up => Filtered::Redirect(id),
            InputEvent::Keyboard(_) if self.keys => Filtered::Redirect(id),
            _ => Filtered::Pass,
        }
    }
}

fn redirecting(only_up: bool, keys: bool) -> (Router, Rc<Cell<Option<NodeId>>>) {
    let to = Rc::new(Cell::new(None));
    let stack = FilterStack::new();
    stack.add(Redirector {
        to: Rc::clone(&to),
        only_up,
        keys,
    });
    (Router::new().with_filter_stack(stack), to)
}

#[test]
fn a_redirected_hover_keeps_hover_within_on_the_targets_ancestors() {
    let log = new_log();
    let (mut router, to) = redirecting(false, false);
    let panel = router.add(None, probe(r(0.0, 0.0, 200.0, 200.0), F, 1, &log));
    let sibling = router.add(Some(panel), probe(r(100.0, 10.0, 50.0, 50.0), F, 2, &log));
    let child = router.add(Some(panel), probe(r(10.0, 10.0, 50.0, 50.0), F, 3, &log));
    router.deliver_mouse(mv(110.0, 20.0));
    assert_eq!(router.hovered(), Some(sibling));
    to.set(Some(child));
    router.deliver_mouse(mv(20.0, 20.0));
    to.set(None);
    // A plain move over the same child: routing agrees it is hovered.
    router.deliver_mouse(mv(20.0, 20.0));
    assert_eq!(router.hovered(), Some(child));
    assert!(
        router.state(panel).contains(NodeState::HOVER_WITHIN),
        "the panel's child is hovered, so the panel has HOVER_WITHIN: {:?}",
        router.state(panel)
    );
}

#[test]
fn a_redirected_release_ends_the_targets_grab() {
    let log = new_log();
    let (mut router, to) = redirecting(true, false);
    let a = router.add(None, probe(r(0.0, 0.0, 100.0, 100.0), F, 1, &log));
    router.deliver_mouse(down(10.0, 10.0));
    assert_eq!(router.active_drag().map(|d| d.node), Some(a));
    to.set(Some(a));
    router.deliver_mouse(up(10.0, 10.0));
    to.set(None);
    assert_eq!(router.active_drag(), None, "the press is over");
    log.borrow_mut().clear();
    router.deliver_mouse(mv(500.0, 500.0));
    assert!(
        !told(&log, 1).iter().any(|e| matches!(e, Ev::Mouse(_))),
        "a move after the release is not a drag (only the hover leaves)"
    );
}

#[test]
fn a_redirect_to_a_disabled_or_hidden_node_reaches_no_handler() {
    let log = new_log();
    let (mut router, to) = redirecting(false, true);
    let a = router.add(None, probe(r(0.0, 0.0, 100.0, 100.0), F, 1, &log));
    router.set_flags(a, NodeFlags::DISABLED, true);
    to.set(Some(a));
    router.deliver_keyboard(key(Key::Enter));
    router.deliver_mouse(down(10.0, 10.0));
    router.set_flags(a, NodeFlags::DISABLED, false);
    router.set_flags(a, NodeFlags::HIDDEN, true);
    router.deliver_mouse(mv(10.0, 10.0));
    let calls = told(&log, 1);
    assert!(calls.is_empty(), "it takes no input: {calls:?}");
    assert_ne!(router.hovered(), Some(a), "a hidden node is never hovered");
}

#[test]
fn redirects_to_a_nested_router_and_to_the_modal_stack_do_not_panic() {
    let log = new_log();
    let (mut router, to) = redirecting(false, true);
    let mut inner = Router::new();
    inner.add(None, probe(r(0.0, 0.0, 100.0, 100.0), F, 1, &log));
    let rid = router.add(None, inner);
    let state = Arc::new(ModalState::new().with_transition(ModalTransition::Instant));
    let stack = ModalStack::install(&mut router, state, VIEWPORT).unwrap();
    to.set(Some(rid));
    let resp = router.deliver_mouse(down(10.0, 10.0));
    assert_eq!(resp.target, Some(rid));
    router.deliver_keyboard(key(Key::Tab));
    to.set(Some(stack));
    let resp = router.deliver_mouse(down(10.0, 10.0));
    assert_eq!(resp.target, Some(stack));
    router.deliver_keyboard(key(Key::Escape));
    router.deliver_gesture(GestureEvent::DoubleTap);
    check_router(&router, "after redirects");
}

/// A rule that re-enters the router it filters for.
struct Reentrant {
    router: RefCell<Weak<Router>>,
    depth: Cell<u32>,
}

impl InputFilter for Reentrant {
    fn filter(&self, _event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        if self.depth.get() > 0 {
            return Filtered::Pass;
        }
        let Some(router) = self.router.borrow().upgrade() else {
            return Filtered::Pass;
        };
        self.depth.set(1);
        router.deliver_keyboard(key(Key::Tab));
        router.deliver_mouse(down(10.0, 10.0));
        router.deliver_mouse(mv(150.0, 10.0));
        router.set_focus(None);
        router.clear_hover();
        render(&router);
        self.depth.set(0);
        Filtered::Pass
    }
}

#[test]
fn a_filter_that_reenters_its_router_does_not_panic() {
    let log = new_log();
    let stack = FilterStack::new();
    let mut router = Router::new().with_filter_stack(stack.clone());
    router.add(None, probe(r(0.0, 0.0, 100.0, 100.0), F, 1, &log));
    let mut inner = Router::new();
    inner.add(None, probe(r(120.0, 0.0, 100.0, 100.0), F, 2, &log));
    router.add(None, inner);
    let router = Rc::new(router);
    let rule = Rc::new(Reentrant {
        router: RefCell::new(Rc::downgrade(&router)),
        depth: Cell::new(0),
    });
    let as_dyn: Rc<dyn InputFilter> = rule.clone();
    stack.push(&as_dyn);
    for e in [
        InputEvent::Mouse(down(150.0, 10.0)),
        InputEvent::Keyboard(key(Key::Tab)),
        InputEvent::Mouse(up(150.0, 10.0)),
        InputEvent::Mouse(mv(10.0, 10.0)),
    ] {
        router.deliver_event(e);
        check_router(&router, "re-entered");
    }
}

// ---------------------------------------------------------------------------
// Modals
// ---------------------------------------------------------------------------

/// A plain self-drawn modal, optionally tracking its own lifetime.
struct Blob {
    rect: Rect,
    alive: Option<Rc<()>>,
    block_all: bool,
}

fn blob(rect: Rect) -> Blob {
    Blob {
        rect,
        alive: None,
        block_all: false,
    }
}

impl DrawControl for Blob {
    fn draw(&self, list: &mut DrawList, _ctx: &DrawContext) {
        list.push(SdfInstance {
            kind: SdfKind::Box,
            position: self.rect.pos(),
            size: self.rect.size(),
            color: [0.5, 0.5, 0.5, 1.0],
            anim: 0,
        });
    }
}

impl HostedControl for Blob {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.rect.contains(pos).then_some((ControlPart::Body, 0.0))
    }

    fn handle_mouse(&self, _event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        None
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, _ctx: &EventContext) -> Option<ControlAction> {
        (event.key() == Some(Key::Enter)).then_some(ControlAction::Dismiss { control: None })
    }
}

/// Spends every event.
struct Wall;

impl InputFilter for Wall {
    fn filter(&self, _event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        Filtered::Consumed(None)
    }
}

impl LayerFilter for Wall {}

impl Modal for Blob {
    fn filter_layer(&self, layer: &LayerContext) -> Box<dyn LayerFilter> {
        if self.block_all {
            Box::new(Wall)
        } else {
            Box::new(ModalLayerFilter::new(self.bounds(), layer.clone()))
        }
    }
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

const TIMED: ModalTransition = ModalTransition::Timed {
    open_secs: 0.1,
    close_secs: 0.1,
};

#[test]
fn a_queued_push_cancelled_by_dismiss_all_drops_its_modal() {
    let (mut router, state, stack) = modal_setup(TIMED);
    router
        .push_modal("x", blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    state.advance(1.0);
    state.pop();
    assert_eq!(state.phase("x"), ModalPhase::Closing);
    let alive = Rc::new(());
    let mut queued = blob(r(300.0, 300.0, 100.0, 100.0));
    queued.alive = Some(Rc::clone(&alive));
    router.push_modal("x", queued).unwrap();
    assert_eq!(Rc::strong_count(&alive), 2, "held while queued");
    state.dismiss_all(DismissReason::App);
    state.advance(1.0);
    render(&router);
    assert_eq!(state.depth(), 0, "the queued push was cancelled");
    assert_eq!(layers(&router, stack), 0);
    assert_eq!(
        Rc::strong_count(&alive),
        1,
        "the cancelled modal was dropped (RAII)"
    );
}

#[test]
fn advance_with_a_huge_finite_dt_finishes_both_transitions() {
    let (mut router, state, stack) = modal_setup(ModalTransition::default());
    router
        .push_modal("x", blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    state.advance(f32::MAX);
    assert_eq!(state.phase("x"), ModalPhase::Open, "fully open");
    state.pop();
    state.advance(f32::MAX);
    render(&router);
    assert_eq!(state.depth(), 0, "fully closed");
    assert_eq!(layers(&router, stack), 0);
}

/// A dialog centred in the viewport, frame (250, 200, 300, 200), with a probe
/// at (270, 220, 50, 50) and an optional close button.
fn dialog_with_probe(log: &Log, tag: u32) -> (ModalDialog, NodeId) {
    let mut dialog = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
    let p = dialog
        .content_mut()
        .add(None, probe(r(270.0, 220.0, 50.0, 50.0), F, tag, log));
    (dialog, p)
}

#[test]
fn opening_a_modal_drops_the_grab_inside_the_one_beneath() {
    let log = new_log();
    let (mut router, state, stack) = modal_setup(ModalTransition::Instant);
    let (dialog, _p) = dialog_with_probe(&log, 1);
    router.push_modal("a", dialog).unwrap();
    router.deliver_mouse(down(290.0, 240.0));
    {
        let s = router.get::<ModalStack>(stack).unwrap();
        let a = s.modal::<ModalDialog>("a").unwrap();
        assert!(a.content().active_drag().is_some(), "pressed inside a");
    }
    router
        .push_modal("b", blob(r(600.0, 50.0, 100.0, 100.0)))
        .unwrap();
    router.deliver_mouse(up(290.0, 240.0));
    state.close("b");
    render(&router);
    let s = router.get::<ModalStack>(stack).unwrap();
    let a = s.modal::<ModalDialog>("a").unwrap();
    assert_eq!(
        a.content().active_drag(),
        None,
        "the release went to b's layer; a's slider must not stay grabbed"
    );
}

#[test]
fn an_outside_tap_still_dismisses_after_a_modal_above_closed_mid_press() {
    let log = new_log();
    let (mut router, state, _stack) = modal_setup(ModalTransition::Instant);
    let (dialog, _p) = dialog_with_probe(&log, 1);
    router.push_modal("a", dialog).unwrap();
    router.deliver_mouse(down(290.0, 240.0));
    router
        .push_modal("b", blob(r(600.0, 50.0, 100.0, 100.0)))
        .unwrap();
    router.deliver_mouse(up(290.0, 240.0));
    state.close("b");
    render(&router);
    // A clean tap outside a, which dismisses on outside taps.
    router.deliver_mouse(down(20.0, 20.0));
    router.deliver_mouse(up(20.0, 20.0));
    assert!(!state.contains("a"), "the outside tap dismissed a");
}

#[test]
fn leaving_a_modals_shape_clears_the_hover_inside_it() {
    let log = new_log();
    let (mut router, _state, stack) = modal_setup(ModalTransition::Instant);
    let (dialog, p) = dialog_with_probe(&log, 1);
    router.push_modal("a", dialog).unwrap();
    router.deliver_mouse(mv(290.0, 240.0));
    {
        let s = router.get::<ModalStack>(stack).unwrap();
        let a = s.modal::<ModalDialog>("a").unwrap();
        assert_eq!(a.content().hovered(), Some(p));
    }
    router.deliver_mouse(mv(10.0, 10.0));
    let s = router.get::<ModalStack>(stack).unwrap();
    let a = s.modal::<ModalDialog>("a").unwrap();
    assert_eq!(a.content().hovered(), None, "the pointer left the dialog");
    assert_eq!(told(&log, 1).last(), Some(&Ev::Hover(None)));
}

#[test]
fn closing_a_dialog_on_a_press_does_not_click_through_to_the_base() {
    let (mut router, state, _stack) = modal_setup(ModalTransition::Instant);
    let rect = r(300.0, 250.0, 100.0, 40.0);
    let base_state = Arc::new(ButtonState::new(false));
    router.add(
        None,
        Button::new("Base", rect).with_state(Arc::clone(&base_state)),
    );
    let mut dialog = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
    let close = dialog.content_mut().add(None, Button::new("Close", rect));
    router
        .push_modal("d", dialog.with_close_control(close))
        .unwrap();
    // A shell that sends Down, Up and Click for one tap.
    router.deliver_mouse(down(320.0, 260.0));
    assert!(
        !state.contains("d"),
        "the close button dismissed on the press"
    );
    router.deliver_mouse(up(320.0, 260.0));
    router.deliver_mouse(click(320.0, 260.0));
    assert!(
        !base_state.take_click(),
        "the tap that closed the dialog also pressed the base button"
    );
}

#[test]
fn a_release_after_its_layer_closed_reaches_no_base_node() {
    let log = new_log();
    let (mut router, state, _stack) = modal_setup(ModalTransition::Instant);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    router
        .push_modal("m", blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    router.deliver_mouse(down(350.0, 350.0));
    state.pop();
    router.deliver_mouse(up(350.0, 350.0));
    let calls = told(&log, 1);
    assert!(
        !calls.iter().any(|e| matches!(e, Ev::Mouse(_))),
        "the base node got a release it never had a press for: {calls:?}"
    );
}

#[test]
fn a_layer_filter_that_blocks_everything_still_yields_to_the_app() {
    let log = new_log();
    let (mut router, state, stack) = modal_setup(ModalTransition::Instant);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    let mut b = blob(r(300.0, 300.0, 100.0, 100.0));
    b.block_all = true;
    router.push_modal("m", b).unwrap();
    for e in [
        InputEvent::Keyboard(key(Key::Escape)),
        InputEvent::Keyboard(key(Key::Back)),
        InputEvent::Keyboard(key(Key::Enter)),
        InputEvent::Mouse(down(10.0, 10.0)),
        InputEvent::Mouse(up(10.0, 10.0)),
    ] {
        let resp = router.deliver_event(e);
        assert!(resp.handled && resp.target.is_none());
    }
    assert!(state.contains("m"), "nothing the user does closes it");
    assert!(told(&log, 1).is_empty(), "and nothing leaks beneath");
    state.pop();
    router.deliver_mouse(down(10.0, 10.0));
    assert_eq!(layers(&router, stack), 0);
    assert!(!told(&log, 1).is_empty(), "the base gets input again");
}

#[test]
fn a_driven_transition_left_half_way_keeps_capturing_until_driven_closed() {
    let log = new_log();
    let (mut router, state, stack) = modal_setup(ModalTransition::Driven);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    router
        .push_modal("m", blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    state.set_progress("m", 0.5);
    state.advance(10.0);
    assert_eq!(
        state.phase("m"),
        ModalPhase::Opening,
        "advance does nothing"
    );
    router.deliver_keyboard(key(Key::Enter));
    state.dismiss(DismissReason::App);
    assert_eq!(state.phase("m"), ModalPhase::Closing);
    assert_eq!(state.progress("m"), 0.5, "closing starts where it was");
    router.deliver_mouse(down(10.0, 10.0));
    assert!(told(&log, 1).is_empty(), "a closing layer still captures");
    state.set_progress("m", f32::NAN);
    state.set_progress("m", 1.0);
    assert_eq!(state.phase("m"), ModalPhase::Closing, "never reopens");
    state.set_progress("m", -3.0);
    assert_eq!(state.phase("m"), ModalPhase::Closed);
    router.deliver_mouse(down(10.0, 10.0));
    assert_eq!(layers(&router, stack), 0);
    assert!(!told(&log, 1).is_empty());
}

/// Every `Dismissed` is followed by exactly one `Closed` with the same id
/// and reason, and nothing closes that was not dismissed.
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

fn fuzz_modals(seed: u64, steps: usize) {
    let mut rng = Lcg(seed);
    let (mut router, state, stack) = modal_setup(TIMED);
    let log = new_log();
    router.add(None, probe(VIEWPORT, F, 1, &log));
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
    for step in 0..steps {
        let what = format!("seed {seed} step {step}");
        let id = ids[rng.below(3)];
        let rect = rects[rng.below(3)];
        match rng.below(14) {
            0 | 1 => {
                let t = transitions[rng.below(4)];
                let mut b = blob(rect);
                b.block_all = rng.coin(8);
                let was_open = state.contains(id);
                let res = router.push_modal_with(id, b, t);
                assert_eq!(res.is_err(), was_open, "{what}: refused only when open");
            }
            2 => {
                if rng.coin(2) {
                    let mut dialog = ModalDialog::new(VIEWPORT, rect, rect.pad(-5.0));
                    let p = dialog
                        .content_mut()
                        .add(None, probe(rect.pad(-10.0), F, 2, &log));
                    let _ = router.replace_top_modal(id, dialog.with_initial_focus(p));
                } else {
                    let _ = router.replace_top_modal(id, blob(rect));
                }
            }
            3 => state.pop(),
            4 => state.close(id),
            5 => state.dismiss_all(DismissReason::App),
            6 => {
                let dt = [0.0, 0.03, 1.0, f32::NAN, -1.0, f32::INFINITY][rng.below(6)];
                state.advance(dt);
            }
            7 => {
                let p = [0.0, 0.5, 1.0, f32::NAN, 2.0, -1.0][rng.below(6)];
                if rng.coin(2) {
                    state.set_progress(id, p);
                } else {
                    state.set_top_progress(p);
                }
            }
            8 => {
                render(&router);
            }
            9 => state.dismiss(DismissReason::Escape),
            _ => {
                router.deliver_event(random_event(&mut rng));
            }
        }
        events.extend(state.take_events());
        assert_eq!(
            layers(&router, stack),
            state.depth(),
            "{what}: one layer per entry"
        );
        let entries = state.stack();
        assert!(
            entries.iter().filter(|e| e.is_top).count() <= 1,
            "{what}: one top"
        );
        for e in &entries {
            assert!((0.0..=1.0).contains(&e.progress), "{what}: progress");
            assert_ne!(e.phase, ModalPhase::Closed, "{what}: no closed entry");
            if e.is_top {
                assert_ne!(e.phase, ModalPhase::Closing, "{what}: closing top");
            }
            assert!(
                router
                    .get::<ModalStack>(stack)
                    .unwrap()
                    .modal::<Blob>(e.id)
                    .is_some()
                    || router
                        .get::<ModalStack>(stack)
                        .unwrap()
                        .modal::<ModalDialog>(e.id)
                        .is_some(),
                "{what}: each entry has its modal"
            );
        }
        check_router(&router, &what);
    }
    // Close everything and drive every transition home.
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
fn modal_stack_invariants_hold_after_every_random_operation() {
    for seed in 1..=40 {
        fuzz_modals(seed, 300);
    }
}

#[test]
fn layer_masks_survive_degenerate_inputs() {
    let alpha: Arc<[u8]> = Arc::from(vec![255u8; 16]);
    let short: Arc<[u8]> = Arc::from(vec![255u8; 3]);
    let empty: Arc<[u8]> = Arc::from(Vec::<u8>::new());
    let masks = [
        LayerMask::texture(r(0.0, 0.0, 0.0, 0.0), 4, 4, alpha.clone(), 0),
        LayerMask::texture(r(0.0, 0.0, 10.0, 10.0), 0, 0, alpha.clone(), 0),
        LayerMask::texture(r(0.0, 0.0, 10.0, 10.0), 4, 4, empty, 0),
        LayerMask::texture(r(0.0, 0.0, 10.0, 10.0), 4, 4, short, 0),
        LayerMask::texture(r(0.0, 0.0, -10.0, 10.0), 4, 4, alpha.clone(), 0),
        LayerMask::texture(
            r(0.0, 0.0, 10.0, 10.0),
            u32::MAX,
            u32::MAX,
            alpha.clone(),
            0,
        ),
        LayerMask::texture(
            r(f32::NAN, 0.0, f32::INFINITY, f32::MAX),
            4,
            4,
            alpha.clone(),
            0,
        ),
        LayerMask::texture(r(0.0, 0.0, f32::INFINITY, 10.0), 4, 4, alpha, 0),
        LayerMask::drawn(f32::NAN),
        LayerMask::drawn(f32::INFINITY),
        LayerMask::drawn(0.0),
    ];
    let mut list = DrawList::new();
    for (pos, size) in [
        ([0.0, 0.0], [10.0, 10.0]),
        ([f32::NAN, 0.0], [10.0, 10.0]),
        ([0.0, 0.0], [0.0, 0.0]),
        ([1.0e30, -1.0e30], [f32::INFINITY, 1.0]),
    ] {
        list.push(SdfInstance {
            kind: SdfKind::Circle,
            position: pos,
            size,
            color: [1.0; 4],
            anim: 0,
        });
    }
    let points = [
        [5.0, 5.0],
        [0.0, 0.0],
        [10.0, 10.0],
        [f32::NAN, 5.0],
        [f32::INFINITY, f32::INFINITY],
        [f32::NEG_INFINITY, 5.0],
        [1.0e38, 1.0e38],
        [-0.0, -0.0],
    ];
    for mask in &masks {
        for p in points {
            let _ = mask.contains(p);
        }
        mask.drawn(&list, 0..list.instances.len());
        #[allow(clippy::reversed_empty_ranges)]
        mask.drawn(&list, 3..1);
        mask.drawn(&list, 0..usize::MAX);
        mask.drawn(&list, 2..2);
        for p in points {
            let _ = mask.contains(p);
        }
    }
    // An empty shape is outside everywhere, and a NaN threshold too.
    let empty_drawn = LayerMask::drawn(0.0);
    empty_drawn.drawn(&DrawList::new(), 0..0);
    assert!(!empty_drawn.contains([5.0, 5.0]));
    assert!(
        !masks[8].contains([5.0, 5.0]),
        "a NaN inside is never inside"
    );
    assert!(
        !masks[0].contains([0.0, 0.0]),
        "a zero-size rect has no inside"
    );
}

/// A shaped modal used with non-finite pointer input.
struct Shaped(Rect);

impl DrawControl for Shaped {
    fn draw(&self, list: &mut DrawList, _ctx: &DrawContext) {
        list.push(SdfInstance {
            kind: SdfKind::Circle,
            position: self.0.pos(),
            size: self.0.size(),
            color: [1.0; 4],
            anim: 0,
        });
    }
}

impl HostedControl for Shaped {
    fn bounds(&self) -> Rect {
        self.0
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.0.contains(pos).then_some((ControlPart::Body, 0.0))
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
}

impl Modal for Shaped {
    fn filter_layer(&self, layer: &LayerContext) -> Box<dyn LayerFilter> {
        Box::new(ShapedLayerFilter::new(LayerMask::drawn(0.0), layer.clone()))
    }
}

#[test]
fn modal_routing_survives_non_finite_pointers_and_dt() {
    let log = new_log();
    let (mut router, state, _stack) = modal_setup(TIMED);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    router
        .push_modal("s", Shaped(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    let (dialog, _) = dialog_with_probe(&log, 2);
    router.push_modal("d", dialog).unwrap();
    for dt in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 1.0] {
        state.advance(dt);
        render(&router);
        for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
            router.deliver_mouse(down(v, v));
            router.deliver_mouse(up(v, 300.0));
            router.deliver_mouse(click(v, v));
            router.deliver_mouse(mv(v, v));
            router.deliver_gesture(GestureEvent::Pinch { delta: v });
        }
    }
    state.close("d");
    state.advance(1.0);
    render(&router);
    for v in [f32::NAN, f32::INFINITY] {
        router.deliver_mouse(down(v, v));
        router.deliver_mouse(up(v, v));
    }
    assert!(
        told(&log, 1).is_empty(),
        "the base saw nothing while a layer is up"
    );
}

/// A layer filter and modal that reach back into the router while it routes
/// and draws.
struct Prober {
    router: Weak<RefCell<Router>>,
    stack: NodeId,
    depth: Cell<u32>,
    count: Rc<Cell<u32>>,
}

impl Prober {
    fn poke(&self) {
        if self.depth.get() > 0 {
            return;
        }
        self.depth.set(1);
        if let Some(router) = self.router.upgrade()
            && let Ok(router) = router.try_borrow()
        {
            let s = router.get::<ModalStack>(self.stack).unwrap();
            let _ = s.len();
            let _ = s.modal::<Poker>("p").is_some();
            let _ = s.state().top();
            let _ = s.state().stack();
            render(&router);
            router.deliver_keyboard(key(Key::Char('z')));
            router.deliver_mouse(mv(350.0, 350.0));
            self.count.set(self.count.get() + 1);
        }
        self.depth.set(0);
    }
}

impl InputFilter for Prober {
    fn filter(&self, _event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        self.poke();
        Filtered::Pass
    }
}

impl LayerFilter for Prober {
    fn drawn(&self, _list: &DrawList, _range: std::ops::Range<usize>) {
        self.poke();
    }
}

/// A modal whose filter and handlers re-enter the router.
struct Poker {
    rect: Rect,
    router: Weak<RefCell<Router>>,
    stack: NodeId,
    count: Rc<Cell<u32>>,
    depth: Cell<u32>,
}

impl Poker {
    fn prober(&self) -> Prober {
        Prober {
            router: self.router.clone(),
            stack: self.stack,
            depth: Cell::new(0),
            count: Rc::clone(&self.count),
        }
    }
}

impl DrawControl for Poker {
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

impl HostedControl for Poker {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.rect.contains(pos).then_some((ControlPart::Body, 0.0))
    }

    fn handle_mouse(&self, _event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        if self.depth.replace(1) == 0 {
            self.prober().poke();
            self.depth.set(0);
        }
        None
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, _ctx: &EventContext) -> Option<ControlAction> {
        if self.depth.replace(1) == 0 {
            self.prober().poke();
            self.depth.set(0);
        }
        (event.key() == Some(Key::Enter)).then_some(ControlAction::Dismiss { control: None })
    }
}

impl Modal for Poker {
    fn filter_layer(&self, _layer: &LayerContext) -> Box<dyn LayerFilter> {
        Box::new(self.prober())
    }
}

#[test]
fn layer_filters_and_modals_that_reenter_the_router_do_not_panic() {
    let state = Arc::new(ModalState::new().with_transition(ModalTransition::Instant));
    let router = Rc::new(RefCell::new(Router::new()));
    let stack =
        ModalStack::install(&mut router.borrow_mut(), Arc::clone(&state), VIEWPORT).unwrap();
    let count = Rc::new(Cell::new(0));
    let poker = Poker {
        rect: r(300.0, 300.0, 100.0, 100.0),
        router: Rc::downgrade(&router),
        stack,
        count: Rc::clone(&count),
        depth: Cell::new(0),
    };
    router.borrow_mut().push_modal("p", poker).unwrap();
    for e in [
        InputEvent::Mouse(down(350.0, 350.0)),
        InputEvent::Mouse(up(350.0, 350.0)),
        InputEvent::Keyboard(key(Key::Char('a'))),
        InputEvent::Mouse(mv(10.0, 10.0)),
    ] {
        router.borrow().deliver_event(e);
        render(&router.borrow());
    }
    assert!(count.get() > 0, "the re-entrant paths ran");
    router.borrow().deliver_keyboard(key(Key::Enter));
    render(&router.borrow());
    assert_eq!(state.depth(), 0, "the modal dismissed itself");
}

/// A modal whose drop asks the stack how many layers it has.
struct Nosy {
    rect: Rect,
    router: Weak<RefCell<Router>>,
    stack: NodeId,
}

impl DrawControl for Nosy {
    fn draw(&self, _list: &mut DrawList, _ctx: &DrawContext) {}
}

impl HostedControl for Nosy {
    fn bounds(&self) -> Rect {
        self.rect
    }

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
}

impl Modal for Nosy {}

impl Drop for Nosy {
    fn drop(&mut self) {
        if let Some(router) = self.router.upgrade()
            && let Ok(router) = router.try_borrow()
            && let Some(stack) = router.get::<ModalStack>(self.stack)
        {
            let _ = stack.len();
        }
    }
}

#[test]
fn a_modal_whose_drop_queries_the_stack_does_not_panic() {
    let state = Arc::new(ModalState::new().with_transition(ModalTransition::Instant));
    let router = Rc::new(RefCell::new(Router::new()));
    let stack =
        ModalStack::install(&mut router.borrow_mut(), Arc::clone(&state), VIEWPORT).unwrap();
    let nosy = Nosy {
        rect: r(300.0, 300.0, 100.0, 100.0),
        router: Rc::downgrade(&router),
        stack,
    };
    router.borrow_mut().push_modal("n", nosy).unwrap();
    state.pop();
    render(&router.borrow());
    assert_eq!(state.depth(), 0);
}

#[test]
fn a_hidden_modal_stack_does_not_trap_input_invisibly() {
    let log = new_log();
    let (mut router, _state, stack) = modal_setup(ModalTransition::Instant);
    router.add(None, probe(VIEWPORT, F, 1, &log));
    router
        .push_modal("m", blob(r(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    router.set_flags(stack, NodeFlags::HIDDEN, true);
    let list = render(&router);
    let drawn_by_stack = list.instances.len() > 1;
    router.deliver_mouse(down(10.0, 10.0));
    let base_got_it = !told(&log, 1).is_empty();
    assert!(
        drawn_by_stack || base_got_it,
        "nothing of the modal is drawn, yet the base receives nothing"
    );
}

#[test]
fn dialog_initial_focus_and_close_controls_tolerate_bad_ids() {
    let log = new_log();
    let (mut router, state, stack) = modal_setup(ModalTransition::Instant);
    let mut stranger = Router::new();
    let foreign = stranger.add(None, Router::new());
    let (mut dialog, p) = dialog_with_probe(&log, 1);
    let gone = dialog
        .content_mut()
        .add(None, probe(r(400.0, 220.0, 50.0, 50.0), F, 2, &log));
    dialog.content_mut().remove(gone);
    dialog.content_mut().set_flags(p, NodeFlags::DISABLED, true);
    let dialog = dialog
        .with_initial_focus(gone)
        .with_close_control(foreign)
        .with_close_control(gone);
    router.push_modal("d", dialog).unwrap();
    router.deliver_keyboard(key(Key::Tab));
    router.deliver_keyboard(key(Key::Enter));
    router.deliver_mouse(down(290.0, 240.0));
    router.deliver_mouse(up(290.0, 240.0));
    assert!(state.contains("d"), "no stale id closes the dialog");
    let s = router.get::<ModalStack>(stack).unwrap();
    let d = s.modal::<ModalDialog>("d").unwrap();
    assert_eq!(d.content().focused(), None, "nothing focusable remains");
    check_router(d.content(), "dialog content");
}
