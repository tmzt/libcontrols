//! The persistent router: its tree, names, flags, drawing and routing.

mod common;

use common::{Painter, Recorder};
use libcontrols::{
    Button, DrawContext, NameError, NodeFlags, NodeId, NodeState, Rect, Router, Slider, sym,
};
use libmsdf::drawlist::DrawList;

fn render(router: &Router) -> DrawList {
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
    list
}

fn colors(list: &DrawList) -> Vec<[f32; 4]> {
    list.instances.iter().map(|i| i.color).collect()
}

#[test]
fn a_removed_id_never_names_a_later_node() {
    let mut router = Router::new();
    let first = router.add(None, Recorder::at(0.0));
    router.remove(first);
    let second = router.add(None, Recorder::at(0.0));
    assert_ne!(
        first, second,
        "ids are never reused, even for a reused slot"
    );
    assert!(!router.contains(first));
    assert!(router.get::<Recorder>(first).is_none());
    assert_eq!(router.find(&first.to_string()), None);
    router.remove(first);
    assert!(router.contains(second), "removing a stale id does nothing");
}

#[test]
fn an_alias_is_unique_within_a_router() {
    let mut router = Router::new();
    let ok = router.add_named(None, "ok", Recorder::at(0.0)).unwrap();
    let again = router.add_named(None, sym!("ok"), Recorder::at(100.0));
    assert_eq!(again, Err(NameError::Taken(ok)));
    assert_eq!(router.len(), 1, "a refused add adds nothing");
    assert_eq!(router.find("ok"), Some(ok));
    assert_eq!(router.find_alias(sym!("ok")), Some(ok));
}

#[test]
fn separate_routers_may_reuse_an_alias() {
    let mut a = Router::new();
    let mut b = Router::new();
    let in_a = a.add_named(None, "ok", Recorder::at(0.0)).unwrap();
    let in_b = b.add_named(None, "ok", Recorder::at(0.0)).unwrap();
    assert_ne!(in_a, in_b, "the ids still differ");
}

#[test]
fn every_node_is_found_by_its_id_name() {
    let mut router = Router::new();
    let id = router.add(None, Recorder::at(0.0));
    let name = id.to_string();
    assert!(name.starts_with('@'));
    assert_eq!(format!("{id:?}"), name);
    assert_eq!(name.parse::<NodeId>(), Ok(id));
    assert_eq!(router.find(&name), Some(id));
    for bad in ["@", "@x", "17", "@-1", "@ 3", ""] {
        assert!(bad.parse::<NodeId>().is_err(), "{bad:?} is not an id");
    }
}

#[test]
fn ids_are_never_reused_even_when_slots_are() {
    let mut router = Router::new();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..50 {
        let id = router.add(None, Recorder::at(0.0));
        assert!(seen.insert(id), "{id} was handed out twice");
        router.remove(id);
    }
}

#[test]
fn an_id_from_another_router_finds_nothing() {
    let mut a = Router::new();
    let b = Router::new();
    let id = a.add(None, Recorder::at(0.0));
    assert_eq!(b.find(&id.to_string()), None);
    assert!(b.get::<Recorder>(id).is_none());
}

#[test]
fn an_alias_may_not_start_with_at() {
    let mut router = Router::new();
    assert_eq!(
        router.add_named(None, "@7", Recorder::at(0.0)),
        Err(NameError::Reserved)
    );
}

#[test]
fn the_libcontrols_namespace_is_reserved() {
    let mut router = Router::new();
    assert_eq!(
        router.add_named(None, "libcontrols/ModalStack", Recorder::at(0.0)),
        Err(NameError::Reserved)
    );
    assert!(
        router
            .add_named(None, "mygame/ModalStack", Recorder::at(0.0))
            .is_ok(),
        "other namespaces are the app's"
    );
}

#[test]
fn a_removed_nodes_alias_is_free_again() {
    let mut router = Router::new();
    let ok = router.add_named(None, "ok", Recorder::at(0.0)).unwrap();
    router.remove(ok);
    assert_eq!(router.find("ok"), None);
    assert!(router.add_named(None, "ok", Recorder::at(0.0)).is_ok());
}

#[test]
fn get_mut_reaches_the_concrete_control() {
    let mut router = Router::new();
    let slider = router.add(
        None,
        Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 100.0, 20.0)),
    );
    assert!(
        router.get::<Button>(slider).is_none(),
        "the wrong type gives None"
    );
    router.get_mut::<Slider>(slider).unwrap().rect = Rect::new(5.0, 5.0, 50.0, 10.0);
    assert_eq!(
        router.get::<Slider>(slider).unwrap().rect,
        Rect::new(5.0, 5.0, 50.0, 10.0)
    );
}

#[test]
fn removing_a_node_removes_its_subtree() {
    let mut router = Router::new();
    let panel = router.add(None, Recorder::at(0.0));
    let child = router
        .add_named(Some(panel), "child", Recorder::at(0.0))
        .unwrap();
    let grandchild = router.add(Some(child), Recorder::at(0.0));
    let other = router.add(None, Recorder::at(100.0));
    assert_eq!(router.parent(grandchild), Some(child));
    router.remove(panel);
    assert!(!router.contains(child) && !router.contains(grandchild));
    assert_eq!(router.find("child"), None);
    assert_eq!(router.children(None).collect::<Vec<_>>(), [other]);
    assert_eq!(router.len(), 1);
}

#[test]
fn the_tree_draws_in_pre_order_and_overlays_in_a_second_pass() {
    let mut router = Router::new();
    let a = router.add(None, Painter::new(0.1));
    router.add(Some(a), Painter::new(0.2));
    router.add(None, Painter::new(0.3));
    let list = render(&router);
    assert_eq!(
        colors(&list),
        [
            Painter::color(0.1),
            Painter::color(0.2),
            Painter::color(0.3),
            Painter::overlay_color(0.1),
            Painter::overlay_color(0.2),
            Painter::overlay_color(0.3),
        ]
    );
}

#[test]
fn a_node_starts_with_its_controls_default_flags() {
    let mut router = Router::new();
    let mut quiet = Painter::new(0.1);
    quiet.flags = NodeFlags::NONE;
    let quiet = router.add(None, quiet);
    let normal = router.add(None, Painter::new(0.2));
    assert_eq!(router.flags(quiet), NodeFlags::NONE);
    assert_eq!(router.flags(normal), NodeFlags::FOCUSABLE);
    router.set_flags(normal, NodeFlags::DISABLED, true);
    assert_eq!(
        router.flags(normal),
        NodeFlags::FOCUSABLE | NodeFlags::DISABLED
    );
    router.set_flags(normal, NodeFlags::FOCUSABLE, false);
    assert_eq!(router.flags(normal), NodeFlags::DISABLED);
}

#[test]
fn a_hidden_subtree_is_not_drawn() {
    let mut router = Router::new();
    let a = router.add(None, Painter::new(0.1));
    router.add(Some(a), Painter::new(0.2));
    router.add(None, Painter::new(0.3));
    router.set_flags(a, NodeFlags::HIDDEN, true);
    let list = render(&router);
    assert_eq!(
        colors(&list),
        [Painter::color(0.3), Painter::overlay_color(0.3)],
        "the hidden node and its child are skipped in both passes"
    );
}

#[test]
fn each_node_draws_with_its_own_flags_and_state() {
    let mut router = Router::new();
    let a = router.add(None, Painter::new(0.1));
    let b = router.add(None, Painter::new(0.2));
    router.set_flags(b, NodeFlags::DISABLED, true);
    render(&router);
    let pa = router.get::<Painter>(a).unwrap();
    let pb = router.get::<Painter>(b).unwrap();
    assert_eq!(
        pa.drawn_with.borrow().as_slice(),
        &[(NodeFlags::FOCUSABLE, NodeState::NONE)]
    );
    assert_eq!(
        pb.drawn_with.borrow().as_slice(),
        &[(NodeFlags::FOCUSABLE | NodeFlags::DISABLED, NodeState::NONE)]
    );
}

// ---- Routing (step 2) ----

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::Call;
use libcontrols::filter::{FilterContext, FilterStack, Filtered, InputFilter};
use libcontrols::{
    ControlPart, FocusEvent, InputEvent, Key, KeyboardEvent, Label, MouseButton, MouseEvent,
};

/// A recorder covering the given rectangle.
fn rec(x: f32, y: f32, w: f32, h: f32) -> Recorder {
    Recorder {
        rect: Rect::new(x, y, w, h),
        ..Recorder::at(0.0)
    }
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

fn mv(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Move { pos: [x, y] }
}

fn drag(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Drag {
        start_pos: [25.0, 25.0],
        current_pos: [x, y],
        delta: [x - 25.0, y - 25.0],
        button: MouseButton::Left,
    }
}

fn key(k: Key) -> KeyboardEvent {
    KeyboardEvent::KeyDown { key: k }
}

/// How many handler calls (mouse, key, gesture) a node received.
fn handler_calls(router: &Router, id: NodeId) -> usize {
    router
        .get::<Recorder>(id)
        .unwrap()
        .take()
        .iter()
        .filter(|c| matches!(c, Call::Mouse(..) | Call::Key(..) | Call::Gesture(..)))
        .count()
}

/// outer (0,0,300,300) > child (10,10,100,100) > grandchild (20,20,30,30).
fn three_levels() -> (Router, [NodeId; 3]) {
    let mut router = Router::new();
    let outer = router.add(None, rec(0.0, 0.0, 300.0, 300.0));
    let child = router.add(Some(outer), rec(10.0, 10.0, 100.0, 100.0));
    let grandchild = router.add(Some(child), rec(20.0, 20.0, 30.0, 30.0));
    (router, [outer, child, grandchild])
}

#[test]
fn a_press_on_a_nested_child_reaches_only_the_child() {
    let (router, [outer, child, grandchild]) = three_levels();
    let resp = router.deliver_mouse(down(25.0, 25.0));
    assert_eq!(resp.target, Some(grandchild));
    assert_eq!(handler_calls(&router, grandchild), 1);
    let total = handler_calls(&router, outer) + handler_calls(&router, child);
    assert_eq!(total, 0, "exactly one handler call in the whole tree");
}

#[test]
fn a_grab_follows_a_nested_control_off_its_area() {
    let (router, [_, _, grandchild]) = three_levels();
    router.deliver_mouse(down(25.0, 25.0));
    router.deliver_mouse(drag(500.0, 500.0));
    let up_resp = router.deliver_mouse(up(500.0, 500.0));
    assert_eq!(up_resp.target, Some(grandchild));
    let calls = router.get::<Recorder>(grandchild).unwrap().take();
    let mouse: Vec<&MouseEvent> = calls
        .iter()
        .filter_map(|c| match c {
            Call::Mouse(m, _) => Some(m),
            _ => None,
        })
        .collect();
    assert!(matches!(
        mouse.as_slice(),
        [
            MouseEvent::Down { .. },
            MouseEvent::Drag { .. },
            MouseEvent::Up { .. }
        ]
    ));
    assert_eq!(router.active_drag(), None, "the release ends the grab");
}

struct Named<'o>(&'static str, &'o RefCell<Vec<&'static str>>);

impl InputFilter for Named<'static> {
    fn filter(&self, _: &InputEvent, _: &FilterContext) -> Filtered {
        self.1.borrow_mut().push(self.0);
        Filtered::Pass
    }
}

#[test]
fn the_last_pushed_filter_is_asked_first() {
    let order: &'static RefCell<Vec<&'static str>> = Box::leak(Box::new(RefCell::new(Vec::new())));
    let filters = FilterStack::new();
    filters.add(Named("first", order));
    let second: Rc<dyn InputFilter> = Rc::new(Named("second", order));
    filters.push(&second);
    filters.add(Named("third", order));
    let router = Router::new().with_filter_stack(filters);
    router.deliver_keyboard(key(Key::Enter));
    assert_eq!(*order.borrow(), ["third", "second", "first"]);
}

struct Eat;

impl InputFilter for Eat {
    fn filter(&self, _: &InputEvent, _: &FilterContext) -> Filtered {
        Filtered::Consumed(None)
    }
}

#[test]
fn a_pushed_filter_leaves_when_its_arc_is_dropped() {
    let filters = FilterStack::new();
    let mut router = Router::new().with_filter_stack(filters.clone());
    let target = router.add(None, rec(0.0, 0.0, 100.0, 100.0));
    let eat: Rc<dyn InputFilter> = Rc::new(Eat);
    filters.push(&eat);
    assert_eq!(router.deliver_mouse(down(5.0, 5.0)).target, None, "eaten");
    drop(eat);
    assert_eq!(router.deliver_mouse(down(5.0, 5.0)).target, Some(target));
    assert!(filters.is_empty(), "the dead rule was pruned");
}

// ---- Focus, Tab order, hover, state (step 3) ----

fn focus_order(router: &Router, ids: &[NodeId]) -> Vec<(usize, FocusEvent)> {
    let _ = router;
    common::take_focus_journal()
        .into_iter()
        .map(|(x, f)| {
            let i = ids
                .iter()
                .position(|&id| router.get::<Recorder>(id).unwrap().rect.x == x)
                .expect("a known node");
            (i, f)
        })
        .collect()
}

#[test]
fn focus_events_fire_only_on_real_changes() {
    common::take_focus_journal();
    let mut router = Router::new();
    let a = router.add(None, rec(0.0, 0.0, 50.0, 50.0));
    router.set_focus(Some(a));
    router.set_focus(Some(a));
    router.deliver_mouse(down(10.0, 10.0));
    router.deliver_mouse(up(10.0, 10.0));
    router.deliver_mouse(down(10.0, 10.0));
    assert_eq!(focus_order(&router, &[a]), [(0, FocusEvent::Gained)]);
}

/// [A, P{c0, c1}, B], each a recorder with its own left edge.
fn tab_tree() -> (Router, [NodeId; 5]) {
    common::take_focus_journal();
    let mut router = Router::new();
    let a = router.add(None, rec(0.0, 0.0, 40.0, 40.0));
    let p = router.add(None, rec(100.0, 0.0, 200.0, 100.0));
    let c0 = router.add(Some(p), rec(110.0, 10.0, 40.0, 40.0));
    let c1 = router.add(Some(p), rec(160.0, 10.0, 40.0, 40.0));
    let b = router.add(None, rec(400.0, 0.0, 40.0, 40.0));
    (router, [a, p, c0, c1, b])
}

#[test]
fn tab_walks_into_a_panel_and_out_again() {
    let (router, ids) = tab_tree();
    let [a, p, c0, c1, b] = ids;
    let mut visited = Vec::new();
    for _ in 0..8 {
        router.deliver_keyboard(key(Key::Tab));
        visited.push(router.focused().unwrap());
        let events = focus_order(&router, &ids);
        let gained = events
            .iter()
            .filter(|(_, f)| *f == FocusEvent::Gained)
            .count();
        let lost = events
            .iter()
            .filter(|(_, f)| *f == FocusEvent::Lost)
            .count();
        assert_eq!(gained, 1, "one Gained per Tab");
        assert!(lost <= 1, "at most one Lost per Tab");
    }
    assert_eq!(visited, [a, p, c0, c1, b, a, p, c0]);
    router.deliver_keyboard(key(Key::BackTab));
    assert_eq!(router.focused(), Some(p), "BackTab goes the other way");
}

#[test]
fn the_tab_order_is_rebuilt_after_an_edit_or_set_flags() {
    let (mut router, [a, p, c0, _c1, _b]) = tab_tree();
    router.set_focus(Some(a));
    router.set_flags(p, NodeFlags::FOCUSABLE, false);
    router.deliver_keyboard(key(Key::Tab));
    assert_eq!(router.focused(), Some(c0), "the panel is no stop any more");
    let late = router.add(Some(p), rec(210.0, 10.0, 40.0, 40.0));
    router.set_focus(Some(_c1));
    router.deliver_keyboard(key(Key::Tab));
    assert_eq!(router.focused(), Some(late), "a node added later is a stop");
}

#[test]
fn tab_skips_disabled_and_hidden_subtrees() {
    let (mut router, [a, p, _c0, _c1, b]) = tab_tree();
    router.set_flags(p, NodeFlags::DISABLED, true);
    router.set_focus(Some(a));
    router.deliver_keyboard(key(Key::Tab));
    assert_eq!(
        router.focused(),
        Some(b),
        "the disabled panel and children skipped"
    );
    router.set_flags(p, NodeFlags::DISABLED, false);
    router.set_flags(p, NodeFlags::HIDDEN, true);
    router.set_focus(Some(a));
    router.deliver_keyboard(key(Key::Tab));
    assert_eq!(router.focused(), Some(b), "and hidden ones");
}

#[test]
fn a_disabled_node_consumes_events_and_blocks_what_is_beneath() {
    let mut router = Router::new();
    let base = router.add(None, rec(0.0, 0.0, 100.0, 100.0));
    let top = router.add(None, rec(0.0, 0.0, 100.0, 100.0));
    router.set_flags(top, NodeFlags::DISABLED, true);
    let resp = router.deliver_mouse(down(10.0, 10.0));
    assert!(resp.handled && resp.action.is_none(), "consumed");
    assert_eq!(
        resp.target,
        Some(top),
        "naming the disabled node that took it"
    );
    assert_eq!(handler_calls(&router, base), 0, "nothing beneath it is hit");
    assert_eq!(handler_calls(&router, top), 0, "and it takes no input");
    assert_eq!(router.focused(), None);
}

#[test]
fn a_disabled_node_is_drawn_at_half_alpha() {
    let mut router = Router::new();
    let a = router.add(None, Painter::new(0.4));
    router.add(Some(a), Painter::new(0.6));
    router.set_flags(a, NodeFlags::DISABLED, true);
    let list = render(&router);
    assert!(
        list.instances
            .iter()
            .all(|i| (i.color[3] - 0.5).abs() < 1e-6)
    );
}

#[test]
fn focus_within_and_hover_within_follow_the_chains() {
    let (router, [outer, child, grandchild]) = three_levels();
    router.set_focus(Some(grandchild));
    assert_eq!(router.state(grandchild), NodeState::FOCUSED);
    assert_eq!(router.state(child), NodeState::FOCUS_WITHIN);
    assert_eq!(router.state(outer), NodeState::FOCUS_WITHIN);
    router.set_focus(Some(child));
    assert_eq!(router.state(grandchild), NodeState::NONE);
    assert_eq!(router.state(child), NodeState::FOCUSED);
    router.set_focus(None);
    router.deliver_mouse(mv(25.0, 25.0));
    assert_eq!(router.state(grandchild), NodeState::HOVERED);
    assert_eq!(router.state(child), NodeState::HOVER_WITHIN);
    assert_eq!(router.state(outer), NodeState::HOVER_WITHIN);
    router.deliver_mouse(mv(200.0, 200.0));
    assert_eq!(router.state(outer), NodeState::HOVERED);
    assert_eq!(router.state(child), NodeState::NONE);
    assert_eq!(router.state(grandchild), NodeState::NONE);
}

#[test]
fn the_grab_sets_grabbed() {
    let (router, [_, _, grandchild]) = three_levels();
    router.deliver_mouse(down(25.0, 25.0));
    assert!(router.state(grandchild).contains(NodeState::GRABBED));
    router.deliver_mouse(up(25.0, 25.0));
    assert!(!router.state(grandchild).contains(NodeState::GRABBED));
}

#[test]
fn a_focused_button_draws_its_ring() {
    let mut router = Router::new();
    let b = router.add(None, Button::new("OK", Rect::new(0.0, 0.0, 80.0, 30.0)));
    let plain = render(&router).instances.len();
    router.set_focus(Some(b));
    let focused = render(&router).instances.len();
    assert!(
        focused > plain,
        "the focus ring adds instances ({plain} -> {focused})"
    );
}

#[test]
fn a_label_is_no_tab_stop() {
    let mut router = Router::new();
    router.add(None, Label::new("Volume", Rect::new(0.0, 0.0, 80.0, 20.0)));
    let b = router.add(None, Button::new("OK", Rect::new(0.0, 40.0, 80.0, 20.0)));
    router.deliver_keyboard(key(Key::Tab));
    assert_eq!(router.focused(), Some(b));
    router.deliver_keyboard(key(Key::Tab));
    assert_eq!(router.focused(), Some(b), "the label is skipped both ways");
}

#[test]
fn removing_the_focused_node_sends_lost_first() {
    common::take_focus_journal();
    let lost_seen = Cell::new(false);
    let mut router = Router::new();
    let a = router.add(None, rec(0.0, 0.0, 40.0, 40.0));
    router.set_focus(Some(a));
    common::take_focus_journal();
    router.remove(a);
    let journal = common::take_focus_journal();
    lost_seen.set(journal == [(0.0, FocusEvent::Lost)]);
    assert!(lost_seen.get(), "the removed node heard Lost: {journal:?}");
    assert_eq!(router.focused(), None);
}

fn hovers(router: &Router, ids: &[NodeId]) -> Vec<(usize, Option<ControlPart>)> {
    common::take_hover_journal()
        .into_iter()
        .map(|(x, h)| {
            let i = ids
                .iter()
                .position(|&id| router.get::<Recorder>(id).unwrap().rect.x == x)
                .expect("a known node");
            (i, h)
        })
        .collect()
}

const BODY: Option<ControlPart> = Some(ControlPart::Body);

#[test]
fn a_plain_move_hovers_a_panel_child() {
    let (router, ids) = tab_tree();
    common::take_hover_journal();
    router.deliver_mouse(mv(120.0, 20.0));
    assert_eq!(router.hovered(), Some(ids[2]));
    assert_eq!(hovers(&router, &ids), [(1, BODY), (2, BODY)]);
}

#[test]
fn entering_a_child_tells_the_panel_then_the_child() {
    let (router, ids) = tab_tree();
    router.deliver_mouse(mv(105.0, 90.0));
    common::take_hover_journal();
    router.deliver_mouse(mv(120.0, 20.0));
    assert_eq!(
        hovers(&router, &ids),
        [(2, BODY)],
        "the panel was already hovered"
    );
    router.deliver_mouse(mv(500.0, 500.0));
    common::take_hover_journal();
    router.deliver_mouse(mv(120.0, 20.0));
    assert_eq!(
        hovers(&router, &ids),
        [(1, BODY), (2, BODY)],
        "outermost first"
    );
}

#[test]
fn moving_between_siblings_tells_only_the_two() {
    let (router, ids) = tab_tree();
    router.deliver_mouse(mv(120.0, 20.0));
    common::take_hover_journal();
    router.deliver_mouse(mv(170.0, 20.0));
    assert_eq!(hovers(&router, &ids), [(2, None), (3, BODY)]);
}

#[test]
fn leaving_the_panel_tells_the_child_then_the_panel() {
    let (router, ids) = tab_tree();
    router.deliver_mouse(mv(120.0, 20.0));
    common::take_hover_journal();
    router.deliver_mouse(mv(350.0, 300.0));
    assert_eq!(
        hovers(&router, &ids),
        [(2, None), (1, None)],
        "innermost first"
    );
}

// ---- A router is a layer (step 4) ----

/// parent: [A (0..40), R{x (110..150), y (160..200)}, B (400..440)].
fn layered(tab_through: bool) -> (Router, NodeId, NodeId, NodeId, NodeId, NodeId) {
    common::take_focus_journal();
    let mut inner = Router::new();
    let x = inner.add(None, rec(110.0, 10.0, 40.0, 40.0));
    let y = inner.add(None, rec(160.0, 10.0, 40.0, 40.0));
    let mut parent = Router::new();
    let a = parent.add(None, rec(0.0, 0.0, 40.0, 40.0));
    let r = parent.add(None, inner);
    let b = parent.add(None, rec(400.0, 0.0, 40.0, 40.0));
    if tab_through {
        parent.set_flags(r, NodeFlags::TAB_THROUGH, true);
    }
    (parent, a, r, b, x, y)
}

fn inner(parent: &Router, r: NodeId) -> &Router {
    parent.get::<Router>(r).expect("the nested router")
}

#[test]
fn a_nested_router_is_one_tab_stop_and_traps_tab_inside() {
    let (parent, a, r, _b, x, y) = layered(false);
    let tab = || parent.deliver_keyboard(key(Key::Tab));
    tab();
    assert_eq!(parent.focused(), Some(a));
    tab();
    assert_eq!(parent.focused(), Some(r), "the router is one stop");
    assert_eq!(inner(&parent, r).focused(), None);
    let mut inside = Vec::new();
    for _ in 0..3 {
        tab();
        assert_eq!(parent.focused(), Some(r), "Tab stays in the router");
        inside.push(inner(&parent, r).focused().unwrap());
    }
    assert_eq!(inside, [x, y, x], "and cycles its own stops");
    parent.deliver_keyboard(key(Key::Escape));
    assert_eq!(parent.focused(), None);
    assert_eq!(
        inner(&parent, r).focused(),
        None,
        "losing the focus clears inside"
    );
}

#[test]
fn a_nested_router_routes_its_own_tree() {
    let (parent, _a, r, _b, x, _y) = layered(false);
    let resp = parent.deliver_mouse(down(120.0, 20.0));
    assert_eq!(resp.target, Some(r), "the parent sees one leaf");
    assert_eq!(parent.focused(), Some(r));
    let nested = inner(&parent, r);
    assert_eq!(
        nested.focused(),
        Some(x),
        "the nested router focused its node"
    );
    assert_eq!(handler_calls(nested, x), 1);
}

#[test]
fn the_parent_never_routes_inside_a_nested_router() {
    let (parent, _a, r, _b, x, _y) = layered(false);
    parent.deliver_mouse(mv(120.0, 20.0));
    assert_eq!(
        parent.hovered(),
        Some(r),
        "the parent's hover stops at the router"
    );
    let nested = inner(&parent, r);
    assert_eq!(nested.hovered(), Some(x), "the nested router hovers inside");
    assert_eq!(nested.state(x), NodeState::HOVERED);
    assert!(
        parent.find(&x.to_string()).is_none(),
        "x is not the parent's node"
    );
}

#[test]
fn leaving_a_nested_router_clears_its_hover() {
    let (parent, _a, r, _b, x, _y) = layered(false);
    parent.deliver_mouse(mv(120.0, 20.0));
    inner(&parent, r).get::<Recorder>(x).unwrap().take();
    parent.deliver_mouse(mv(300.0, 300.0));
    let nested = inner(&parent, r);
    assert_eq!(nested.hovered(), None);
    assert_eq!(nested.get::<Recorder>(x).unwrap().hovers(), [None]);
}

#[test]
fn a_nested_router_sees_the_parents_modifiers() {
    let (parent, _a, r, _b, x, _y) = layered(false);
    let shift = libcontrols::Modifiers {
        shift: true,
        ..Default::default()
    };
    parent.deliver_modifiers(shift);
    parent.deliver_mouse(down(120.0, 20.0));
    let nested = inner(&parent, r);
    let contexts = nested.get::<Recorder>(x).unwrap().mouse_contexts();
    assert_eq!(contexts[0].modifiers, shift);
}

#[test]
fn tab_through_walks_into_a_nested_router_and_out_again() {
    let (parent, a, r, b, x, y) = layered(true);
    let mut visited = Vec::new();
    for _ in 0..5 {
        parent.deliver_keyboard(key(Key::Tab));
        let outer = parent.focused().unwrap();
        visited.push(if outer == r {
            inner(&parent, r).focused().expect("a stop inside")
        } else {
            outer
        });
    }
    assert_eq!(visited, [a, x, y, b, a]);
    assert_eq!(
        inner(&parent, r).focused(),
        None,
        "leaving it cleared its focus"
    );
    let mut back = Vec::new();
    for _ in 0..3 {
        parent.deliver_keyboard(key(Key::BackTab));
        let outer = parent.focused().unwrap();
        back.push(if outer == r {
            inner(&parent, r).focused().unwrap()
        } else {
            outer
        });
    }
    assert_eq!(back, [b, y, x], "and backwards");
}

#[test]
fn an_empty_tab_through_router_is_skipped() {
    let mut parent = Router::new();
    let a = parent.add(None, rec(0.0, 0.0, 40.0, 40.0));
    let r = parent.add(None, Router::new());
    parent.set_flags(r, NodeFlags::TAB_THROUGH, true);
    let b = parent.add(None, rec(400.0, 0.0, 40.0, 40.0));
    parent.deliver_keyboard(key(Key::Tab));
    parent.deliver_keyboard(key(Key::Tab));
    assert_eq!(
        parent.focused(),
        Some(b),
        "a router with no stops is no stop"
    );
    let _ = a;
}

struct Count(Rc<Cell<u32>>);

impl InputFilter for Count {
    fn filter(&self, _: &InputEvent, _: &FilterContext) -> Filtered {
        self.0.set(self.0.get() + 1);
        Filtered::Pass
    }
}

#[test]
fn a_nested_router_runs_no_filter_stack() {
    let (parent, _a, r, _b, _x, _y) = layered(false);
    let asked = Rc::new(Cell::new(0));
    let filters = FilterStack::new();
    filters.add(Count(asked.clone()));
    let parent = parent.with_filter_stack(filters);
    parent.deliver_mouse(down(120.0, 20.0));
    assert_eq!(asked.get(), 1, "only the root's chain runs, once per event");
    assert!(inner(&parent, r).filter_stack().is_none());
}

// ---- Panel without a host (step 5) ----

use libcontrols::{ControlAction, HostedControl, Options, Panel, TooltipStyle};

#[test]
fn a_panels_children_are_router_nodes() {
    let mut router = Router::new();
    let panel = router.add(None, Panel::new(Rect::new(0.0, 0.0, 300.0, 200.0)));
    let child = router.add(Some(panel), rec(10.0, 10.0, 50.0, 50.0));
    assert_eq!(router.children(Some(panel)).collect::<Vec<_>>(), [child]);
    let resp = router.deliver_mouse(down(20.0, 20.0));
    assert_eq!(resp.target, Some(child));
    assert_eq!(router.focused(), Some(child));
    assert!(router.state(panel).contains(NodeState::FOCUS_WITHIN));
}

#[test]
fn a_panel_answers_only_for_its_body() {
    let mut router = Router::new();
    let panel = router.add(None, Panel::new(Rect::new(0.0, 0.0, 300.0, 200.0)));
    router.add(Some(panel), rec(10.0, 10.0, 50.0, 50.0));
    let on_body = router.deliver_mouse(down(250.0, 150.0));
    assert_eq!(on_body.target, Some(panel));
    assert!(matches!(
        on_body.action,
        Some(ControlAction::Activated { .. })
    ));
    router.deliver_mouse(up(250.0, 150.0));
    let p = router.get::<Panel>(panel).unwrap();
    let ctx = libcontrols::EventContext::new(ControlPart::Body);
    assert_eq!(
        p.handle_keyboard(&key(Key::Char('x')), &ctx),
        None,
        "a panel takes no keys but Enter and Space"
    );
}

#[test]
fn a_panel_childs_tooltip_draws_above_a_later_node() {
    const TIP_BG: [f32; 4] = [0.6, 0.0, 0.6, 1.0];
    let mut router = Router::new();
    let panel = router.add(None, Panel::new(Rect::new(0.0, 0.0, 300.0, 200.0)));
    let grid = router.add(
        Some(panel),
        Options::new(&["SV - Save"], Rect::new(10.0, 10.0, 100.0, 20.0))
            .with_tooltip_style(TooltipStyle::default().with_bg(TIP_BG)),
    );
    router.add(None, Painter::new(0.9));
    router.deliver_mouse(mv(15.0, 15.0));
    assert_eq!(router.hovered(), Some(grid));
    let list = render(&router);
    let tip = list
        .instances
        .iter()
        .position(|i| i.color == TIP_BG)
        .expect("the tip");
    let later = list
        .instances
        .iter()
        .position(|i| i.color == Painter::color(0.9))
        .expect("the later node");
    assert!(
        tip > later,
        "the child's tooltip draws after the later node's first pass"
    );
}

// ---- README ----

/// The example in README.md's "Usage" section.
#[test]
fn the_readme_usage_example_runs() {
    let mut router = Router::new();
    let panel = router.add(
        None,
        Panel::new(Rect::new(0.0, 0.0, 300.0, 200.0)).with_title("AUDIO"),
    );
    let volume = router.add(
        Some(panel),
        Slider::new(0.5, 0.0, 1.0, Rect::new(10.0, 40.0, 280.0, 20.0)),
    );
    let mute = Button::new("Mute", Rect::new(10.0, 80.0, 80.0, 30.0));
    let mute_state = mute.state.clone();
    let mute = router.add(Some(panel), mute);

    // Buttons activate on release: the Click of a tap.
    let response = router.deliver_event(InputEvent::Mouse(MouseEvent::Click {
        pos: [20.0, 90.0],
        button: MouseButton::Left,
    }));
    assert!(response.target == Some(mute) && mute_state.take_click());

    router.get_mut::<Slider>(volume).unwrap().rect = Rect::new(10.0, 40.0, 200.0, 20.0);
    assert_eq!(
        router.get::<Slider>(volume).unwrap().rect,
        Rect::new(10.0, 40.0, 200.0, 20.0)
    );
    assert!(!render(&router).instances.is_empty());
}
