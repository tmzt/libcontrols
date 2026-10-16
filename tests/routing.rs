//! Router routing: modifiers on every event, focus events, hover.
//!
//! Ported from `tests/routing.rs` (the `ControlHost` era). The router is
//! persistent, so the "rebuilt host" tests become "kept across frames" tests:
//! there is nothing to carry.

mod common;

use common::{Call, Recorder};
use libcontrols::{
    ControlPart, DrawContext, EventContext, FocusEvent, GestureEvent, InputEvent, Key,
    KeyboardEvent, Modifiers, MouseButton, MouseEvent, NodeFlags, NodeId, Panel, Rect, Router,
};
use libmsdf::drawlist::DrawList;

const SHIFT: Modifiers = Modifiers {
    shift: true,
    alt: false,
    ctrl: false,
    meta: false,
};

const NONE: Modifiers = Modifiers {
    shift: false,
    alt: false,
    ctrl: false,
    meta: false,
};

fn down(x: f32) -> MouseEvent {
    MouseEvent::Down {
        pos: [x, 10.0],
        button: MouseButton::Left,
    }
}

fn up(x: f32) -> MouseEvent {
    MouseEvent::Up {
        pos: [x, 10.0],
        button: MouseButton::Left,
    }
}

fn drag(x: f32) -> MouseEvent {
    MouseEvent::Drag {
        start_pos: [10.0, 10.0],
        current_pos: [x, 10.0],
        delta: [x - 10.0, 0.0],
        button: MouseButton::Left,
    }
}

/// The recorder a node owns.
fn rec(router: &Router, id: NodeId) -> &Recorder {
    router.get::<Recorder>(id).expect("a recorder node")
}

/// A router holding one recorder at `x`, and its id.
fn one(r: Recorder) -> (Router, NodeId) {
    let mut router = Router::new();
    let id = router.add(None, r);
    (router, id)
}

/// Draw one frame, as an app does between events.
fn frame(router: &Router) {
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
}

#[test]
fn every_pointer_event_carries_the_modifiers_held() {
    let (router, id) = one(Recorder::at(0.0));
    router.deliver_event(InputEvent::Modifiers(SHIFT));
    let events = [
        down(10.0),
        drag(20.0),
        up(20.0),
        MouseEvent::Click {
            pos: [10.0, 10.0],
            button: MouseButton::Left,
        },
        MouseEvent::Scroll {
            pos: [10.0, 10.0],
            delta: [0.0, 1.0],
        },
        MouseEvent::ScrollPixels {
            pos: [10.0, 10.0],
            delta: [0.0, 4.0],
        },
    ];
    for event in events {
        router.deliver_mouse(event);
    }
    let contexts = rec(&router, id).mouse_contexts();
    assert_eq!(
        contexts.len(),
        events.len(),
        "every pointer event reaches it"
    );
    assert!(
        contexts.iter().all(|c| c.modifiers == SHIFT),
        "each must carry the Shift held: {contexts:?}"
    );
}

#[test]
fn a_shift_pressed_mid_drag_shows_on_the_next_drag_event() {
    let (router, id) = one(Recorder::at(0.0));
    router.deliver_event(InputEvent::Modifiers(SHIFT));
    router.deliver_mouse(down(10.0));
    let both = Modifiers {
        ctrl: true,
        ..SHIFT
    };
    router.deliver_event(InputEvent::Modifiers(both));
    router.deliver_mouse(drag(40.0));
    let contexts = rec(&router, id).mouse_contexts();
    let on_drag = contexts.last().expect("the drag reached the control");
    assert_eq!(on_drag.modifiers, both, "the Ctrl added mid-drag must show");
    assert_eq!(
        on_drag.press_modifiers,
        Some(SHIFT),
        "the press held only Shift"
    );
}

#[test]
fn press_modifiers_are_those_held_at_the_press() {
    let (router, id) = one(Recorder::at(0.0));
    router.deliver_event(InputEvent::Modifiers(SHIFT));
    router.deliver_mouse(down(10.0));
    router.deliver_event(InputEvent::Modifiers(NONE));
    router.deliver_mouse(drag(200.0));
    router.deliver_mouse(up(200.0));
    let contexts = rec(&router, id).mouse_contexts();
    assert_eq!(contexts.len(), 3);
    for ctx in &contexts {
        assert_eq!(ctx.press_modifiers, Some(SHIFT), "{ctx:?}");
    }
    assert_eq!(
        contexts[1].modifiers, NONE,
        "Shift was let go before the drag"
    );
}

#[test]
fn press_modifiers_are_none_outside_a_grab() {
    let (router, id) = one(Recorder::at(0.0));
    router.deliver_event(InputEvent::Modifiers(SHIFT));
    router.deliver_mouse(MouseEvent::Click {
        pos: [10.0, 10.0],
        button: MouseButton::Left,
    });
    router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Enter });
    let calls = rec(&router, id).take();
    let contexts: Vec<EventContext> = calls
        .iter()
        .filter_map(|c| match c {
            Call::Mouse(_, ctx) | Call::Key(_, ctx) => Some(*ctx),
            _ => None,
        })
        .collect();
    assert_eq!(contexts.len(), 2);
    assert!(contexts.iter().all(|c| c.press_modifiers.is_none()));
}

#[test]
fn keys_and_gestures_carry_the_modifiers_too() {
    let (router, id) = one(Recorder::at(0.0));
    router.set_focus(Some(id));
    router.deliver_mouse(MouseEvent::Move { pos: [10.0, 10.0] });
    router.deliver_event(InputEvent::Modifiers(SHIFT));
    router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Left });
    router.deliver_gesture(GestureEvent::Pinch { delta: 0.5 });
    let mut seen = 0;
    for call in rec(&router, id).take() {
        match call {
            Call::Key(_, ctx) | Call::Gesture(_, ctx) => {
                assert_eq!(ctx.modifiers, SHIFT);
                assert_eq!(ctx.part, ControlPart::Body);
                seen += 1;
            }
            _ => {}
        }
    }
    assert_eq!(seen, 2, "the key and the gesture both arrive");
}

#[test]
fn a_modifier_change_reaches_no_control() {
    let (router, id) = one(Recorder::at(0.0));
    rec(&router, id).take();
    router.deliver_event(InputEvent::Modifiers(SHIFT));
    assert!(
        rec(&router, id).take().is_empty(),
        "no handler hears of a modifier change"
    );
    assert_eq!(router.modifiers(), SHIFT, "the router records it");
}

#[test]
fn a_panel_passes_the_modifiers_it_saw_to_its_children() {
    // A panel forwards nothing now: its child is a node of the same router,
    // which hands it the modifiers itself.
    let mut router = Router::new();
    let panel = router.add(None, Panel::new(Rect::new(0.0, 0.0, 300.0, 200.0)));
    let child = router.add(Some(panel), Recorder::at(10.0));
    let ctrl = Modifiers { ctrl: true, ..NONE };
    router.deliver_event(InputEvent::Modifiers(ctrl));
    router.deliver_mouse(down(20.0));
    let contexts = rec(&router, child).mouse_contexts();
    assert_eq!(contexts.len(), 1, "the press reaches the child");
    assert_eq!(contexts[0].modifiers, ctrl, "with the modifiers held");
}

#[test]
fn a_context_is_built_outside_the_crate_with_new_and_with_methods() {
    let ctx = EventContext::new(ControlPart::Track)
        .with_modifiers(SHIFT)
        .with_press_modifiers(Some(NONE));
    assert_eq!(ctx.part, ControlPart::Track);
    assert_eq!(ctx.modifiers, SHIFT);
    assert_eq!(ctx.press_modifiers, Some(NONE));
    assert_eq!(ctx.with_part(ControlPart::Thumb).part, ControlPart::Thumb);
    let mut assigned = EventContext::default();
    assigned.modifiers = SHIFT;
    assert_eq!(assigned.part, ControlPart::Body, "the default part is Body");
}

/// Was `a_rebuilt_host_is_seeded_with_the_last_modifiers`: the router is not
/// rebuilt, so it keeps the modifiers across frames; `set_modifiers` still
/// seeds a fresh router (as a parent does for a nested one).
#[test]
fn the_router_keeps_the_last_modifiers_across_frames() {
    let alt = Modifiers { alt: true, ..NONE };
    let (router, id) = one(Recorder::at(0.0));
    router.deliver_event(InputEvent::Modifiers(alt));
    frame(&router);
    router.deliver_mouse(MouseEvent::Click {
        pos: [10.0, 10.0],
        button: MouseButton::Left,
    });
    let contexts = rec(&router, id).mouse_contexts();
    assert_eq!(contexts.last().map(|c| c.modifiers), Some(alt));

    let (fresh, id) = one(Recorder::at(0.0));
    fresh.set_modifiers(router.modifiers());
    fresh.deliver_mouse(MouseEvent::Click {
        pos: [10.0, 10.0],
        button: MouseButton::Left,
    });
    let contexts = rec(&fresh, id).mouse_contexts();
    assert_eq!(contexts.last().map(|c| c.modifiers), Some(alt));
}

fn tab() -> KeyboardEvent {
    KeyboardEvent::KeyDown { key: Key::Tab }
}

/// A router of three recorders at x = 0, 200 and 400, and their ids; empties
/// the focus journal.
fn three() -> (Router, Vec<NodeId>) {
    common::take_focus_journal();
    let mut router = Router::new();
    let ids = [0.0, 200.0, 400.0]
        .into_iter()
        .map(|x| router.add(None, Recorder::at(x)))
        .collect();
    (router, ids)
}

/// The focus events since the last call, as `(index into ids, event)`;
/// empties every recorder's log.
fn focus_log(router: &Router, ids: &[NodeId]) -> Vec<(usize, FocusEvent)> {
    let xs: Vec<f32> = ids
        .iter()
        .map(|&id| {
            let r = rec(router, id);
            r.take();
            r.rect.x
        })
        .collect();
    common::take_focus_journal()
        .into_iter()
        .map(|(x, f)| {
            let i = xs.iter().position(|&rx| rx == x).expect("a known recorder");
            (i, f)
        })
        .collect()
}

#[test]
fn tab_sends_lost_then_gained() {
    let (router, ids) = three();
    router.deliver_keyboard(tab());
    assert_eq!(focus_log(&router, &ids), [(0, FocusEvent::Gained)]);
    router.deliver_keyboard(tab());
    assert_eq!(
        focus_log(&router, &ids),
        [(0, FocusEvent::Lost), (1, FocusEvent::Gained)]
    );
    router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Escape });
    assert_eq!(focus_log(&router, &ids), [(1, FocusEvent::Lost)]);
}

#[test]
fn backtab_sends_them_the_other_way() {
    let (router, ids) = three();
    router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::BackTab });
    assert_eq!(focus_log(&router, &ids), [(2, FocusEvent::Gained)]);
    router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::BackTab });
    assert_eq!(
        focus_log(&router, &ids),
        [(2, FocusEvent::Lost), (1, FocusEvent::Gained)]
    );
}

#[test]
fn a_press_on_a_control_moves_the_focus_with_events() {
    let (router, ids) = three();
    router.deliver_mouse(down(10.0));
    router.deliver_mouse(up(10.0));
    router.deliver_mouse(down(210.0));
    assert_eq!(
        focus_log(&router, &ids),
        [
            (0, FocusEvent::Gained),
            (0, FocusEvent::Lost),
            (1, FocusEvent::Gained)
        ]
    );
    assert_eq!(router.focused(), Some(ids[1]));
}

#[test]
fn escape_and_a_press_on_nothing_send_lost() {
    let (router, ids) = three();
    router.set_focus(Some(ids[0]));
    focus_log(&router, &ids);
    router.deliver_mouse(down(150.0));
    assert_eq!(
        focus_log(&router, &ids),
        [(0, FocusEvent::Lost)],
        "a press on nothing"
    );
    router.set_focus(Some(ids[2]));
    focus_log(&router, &ids);
    router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Escape });
    assert_eq!(focus_log(&router, &ids), [(2, FocusEvent::Lost)], "Escape");
}

/// Was `restoring_focus_on_a_rebuilt_host_sends_gained_only`: nothing is
/// rebuilt, so the focus stays across frames and no repeat `Gained` is sent.
#[test]
fn the_focus_is_kept_across_frames_with_no_repeat_gained() {
    let (router, ids) = three();
    router.set_focus(Some(ids[1]));
    assert_eq!(focus_log(&router, &ids), [(1, FocusEvent::Gained)]);
    for _ in 0..3 {
        frame(&router);
        router.deliver_mouse(MouseEvent::Move { pos: [210.0, 10.0] });
        router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Enter });
    }
    assert_eq!(router.focused(), Some(ids[1]));
    assert!(focus_log(&router, &ids).is_empty());
}

#[test]
fn setting_the_focus_it_already_has_sends_nothing() {
    let (router, ids) = three();
    router.set_focus(Some(ids[1]));
    focus_log(&router, &ids);
    router.set_focus(Some(ids[1]));
    router.set_focus(router.focused());
    assert!(focus_log(&router, &ids).is_empty());
}

/// Was `a_carried_key_that_no_longer_resolves_sends_nothing`: a removed
/// node's id never resolves again.
#[test]
fn a_removed_id_that_no_longer_resolves_sends_nothing() {
    let (mut router, ids) = three();
    let gone = router.add(None, Recorder::at(600.0));
    router.remove(gone);
    router.set_focus(Some(gone));
    assert_eq!(router.focused(), None);
    assert!(focus_log(&router, &ids).is_empty());
}

/// Was `initial_focus_sends_gained_at_build`: there is no build; the first
/// `set_focus` on a new router sends `Gained` once.
#[test]
fn initial_focus_sends_gained_once() {
    let (router, ids) = three();
    router.set_focus(Some(ids[1]));
    assert_eq!(router.focused(), Some(ids[1]));
    assert_eq!(focus_log(&router, &ids), [(1, FocusEvent::Gained)]);
}

#[test]
fn tab_skips_a_control_that_refuses_focus() {
    let (mut router, ids) = three();
    router.set_flags(ids[1], NodeFlags::FOCUSABLE, false);
    router.set_focus(Some(ids[0]));
    router.deliver_keyboard(tab());
    assert_eq!(router.focused(), Some(ids[2]), "Tab from 0 lands on 2");
    router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::BackTab });
    assert_eq!(router.focused(), Some(ids[0]), "and BackTab skips it too");
}

#[test]
fn a_press_on_a_control_that_refuses_focus_leaves_focus_alone() {
    let (mut router, ids) = three();
    router.set_flags(ids[1], NodeFlags::FOCUSABLE, false);
    router.set_focus(Some(ids[0]));
    focus_log(&router, &ids);
    let resp = router.deliver_mouse(down(210.0));
    assert_eq!(resp.target, Some(ids[1]), "the press still reaches it");
    assert_eq!(router.focused(), Some(ids[0]), "but the focus stays");
    assert!(focus_log(&router, &ids).is_empty());
}

#[test]
fn when_every_control_refuses_tab_clears_the_focus() {
    let (mut router, ids) = three();
    router.set_focus(Some(ids[0]));
    for &id in &ids {
        router.set_flags(id, NodeFlags::FOCUSABLE, false);
    }
    router.deliver_keyboard(tab());
    assert_eq!(router.focused(), None);
}

#[test]
fn tab_and_escape_releases_reach_no_control() {
    let (router, ids) = three();
    router.set_focus(Some(ids[0]));
    rec(&router, ids[0]).take();
    for key in [Key::Tab, Key::BackTab, Key::Escape] {
        router.deliver_keyboard(KeyboardEvent::KeyUp { key });
    }
    assert!(
        rec(&router, ids[0]).take().is_empty(),
        "no release of a router key reaches the control"
    );
}

fn mv(x: f32) -> MouseEvent {
    MouseEvent::Move { pos: [x, 10.0] }
}

fn hover_logs(router: &Router, ids: &[NodeId]) -> Vec<Vec<Option<ControlPart>>> {
    ids.iter().map(|&id| rec(router, id).hovers()).collect()
}

const BODY: Option<ControlPart> = Some(ControlPart::Body);

#[test]
fn only_the_control_entered_and_the_one_left_are_told() {
    let (router, ids) = three();
    router.deliver_mouse(mv(10.0));
    assert_eq!(hover_logs(&router, &ids), [vec![BODY], vec![], vec![]]);
    router.deliver_mouse(mv(210.0));
    assert_eq!(hover_logs(&router, &ids), [vec![None], vec![BODY], vec![]]);
}

#[test]
fn a_part_change_tells_the_same_control_once() {
    let (router, id) = one(Recorder::halved(0.0));
    router.deliver_mouse(mv(10.0));
    router.deliver_mouse(mv(80.0));
    router.deliver_mouse(mv(90.0));
    assert_eq!(
        rec(&router, id).hovers(),
        [Some(ControlPart::Custom(0)), Some(ControlPart::Custom(1))]
    );
}

#[test]
fn staying_put_tells_nobody() {
    let (router, ids) = three();
    router.deliver_mouse(mv(10.0));
    hover_logs(&router, &ids);
    router.deliver_mouse(mv(20.0));
    router.deliver_mouse(mv(30.0));
    assert_eq!(hover_logs(&router, &ids), [vec![], vec![], vec![]]);
}

#[test]
fn leaving_everything_tells_only_the_one_left() {
    let (router, ids) = three();
    router.deliver_mouse(mv(210.0));
    hover_logs(&router, &ids);
    router.deliver_mouse(mv(150.0));
    assert_eq!(hover_logs(&router, &ids), [vec![], vec![None], vec![]]);
    router.deliver_mouse(mv(160.0));
    assert_eq!(hover_logs(&router, &ids), [vec![], vec![], vec![]]);
}

#[test]
fn a_grab_freezes_hover_until_the_release() {
    let (router, ids) = three();
    router.deliver_mouse(down(10.0));
    hover_logs(&router, &ids);
    router.deliver_mouse(drag(210.0));
    router.deliver_mouse(up(210.0));
    assert_eq!(
        hover_logs(&router, &ids),
        [vec![], vec![], vec![]],
        "frozen while grabbed"
    );
    router.deliver_mouse(mv(210.0));
    assert_eq!(hover_logs(&router, &ids), [vec![None], vec![BODY], vec![]]);
}

/// Was `hover_is_carried_across_a_rebuild_so_leaving_still_clears`: the
/// router keeps the hover across frames, so leaving still clears.
#[test]
fn hover_is_kept_across_frames_so_leaving_still_clears() {
    let (router, ids) = three();
    router.deliver_mouse(mv(10.0));
    hover_logs(&router, &ids);
    frame(&router);
    assert_eq!(router.hovered(), Some(ids[0]));
    router.deliver_mouse(mv(150.0));
    assert_eq!(hover_logs(&router, &ids), [vec![None], vec![], vec![]]);
}

// the_part_is_resent_once_after_a_rebuild: dropped; there is no rebuild and no
// way to seed a hovered node without its part, so nothing is ever resent.

#[test]
fn clear_hover_tells_only_the_hovered_control() {
    let (router, ids) = three();
    router.deliver_mouse(mv(410.0));
    hover_logs(&router, &ids);
    router.clear_hover();
    assert_eq!(hover_logs(&router, &ids), [vec![], vec![], vec![None]]);
    assert_eq!(router.hovered(), None);
    router.clear_hover();
    assert_eq!(
        hover_logs(&router, &ids),
        [vec![], vec![], vec![]],
        "nothing left to clear"
    );
}

/// Was `a_hovered_key_that_no_longer_resolves_is_dropped`: removing the
/// hovered node drops the hover, and nobody else is told.
#[test]
fn a_removed_hovered_node_is_dropped() {
    let (mut router, ids) = three();
    router.deliver_mouse(mv(10.0));
    hover_logs(&router, &ids);
    router.remove(ids[0]);
    assert_eq!(router.hovered(), None);
    router.deliver_mouse(mv(150.0));
    assert_eq!(hover_logs(&router, &ids[1..]), [vec![], vec![]]);
}

#[test]
fn a_panel_carries_its_childrens_hover() {
    // The router keeps a panel child's hover; leaving the child clears it.
    let mut router = Router::new();
    let panel = router.add(None, Panel::new(Rect::new(0.0, 0.0, 300.0, 200.0)));
    let child = router.add(Some(panel), Recorder::at(10.0));
    router.deliver_mouse(MouseEvent::Move { pos: [20.0, 10.0] });
    assert_eq!(router.hovered(), Some(child));
    rec(&router, child).take();
    router.deliver_mouse(MouseEvent::Click {
        pos: [250.0, 150.0],
        button: MouseButton::Left,
    });
    assert_eq!(
        rec(&router, child).hovers(),
        [None],
        "leaving the child clears it"
    );
    assert_eq!(router.hovered(), Some(panel));
}

#[test]
fn at_most_two_set_hover_calls_per_pointer_event() {
    let mut router = Router::new();
    let ids: Vec<NodeId> = (0..8)
        .map(|i| router.add(None, Recorder::halved(i as f32 * 120.0)))
        .collect();
    let xs = [
        5.0, 70.0, 130.0, 110.0, 950.0, 860.0, 15.0, 400.0, 300.0, 610.0,
    ];
    for x in xs {
        router.deliver_mouse(mv(x));
        let calls: usize = ids.iter().map(|&id| rec(&router, id).hovers().len()).sum();
        assert!(calls <= 2, "a move to {x} made {calls} set_hover calls");
    }
}
