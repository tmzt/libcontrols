//! Area and focus routing: modifiers on every event, focus events, hover.

mod common;

use common::{Call, Recorder};
use libcontrols::{
    ControlHandle, ControlHost, ControlKey, ControlPart, EventContext, FocusEvent, GestureEvent,
    InputEvent, Key, KeyboardEvent, Modifiers, MouseButton, MouseEvent, Panel, Rect,
};

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

#[test]
fn every_pointer_event_carries_the_modifiers_held() {
    let r = Recorder::at(0.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .build();
    host.deliver_event(InputEvent::Modifiers(SHIFT));
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
        host.deliver_mouse(event);
    }
    let contexts = r.mouse_contexts();
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
    let r = Recorder::at(0.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .build();
    host.deliver_event(InputEvent::Modifiers(SHIFT));
    host.deliver_mouse(down(10.0));
    let both = Modifiers {
        ctrl: true,
        ..SHIFT
    };
    host.deliver_event(InputEvent::Modifiers(both));
    host.deliver_mouse(drag(40.0));
    let contexts = r.mouse_contexts();
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
    let r = Recorder::at(0.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .build();
    host.deliver_event(InputEvent::Modifiers(SHIFT));
    host.deliver_mouse(down(10.0));
    host.deliver_event(InputEvent::Modifiers(NONE));
    host.deliver_mouse(drag(200.0));
    host.deliver_mouse(up(200.0));
    let contexts = r.mouse_contexts();
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
    let r = Recorder::at(0.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .build();
    host.deliver_event(InputEvent::Modifiers(SHIFT));
    host.deliver_mouse(MouseEvent::Click {
        pos: [10.0, 10.0],
        button: MouseButton::Left,
    });
    host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Enter });
    let calls = r.take();
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
    let r = Recorder::at(0.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .with_initial_focus(0)
        .build();
    host.deliver_mouse(MouseEvent::Move { pos: [10.0, 10.0] });
    host.deliver_event(InputEvent::Modifiers(SHIFT));
    host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Left });
    host.deliver_gesture(GestureEvent::Pinch { delta: 0.5 });
    let mut seen = 0;
    for call in r.take() {
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
    let r = Recorder::at(0.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .build();
    r.take();
    host.deliver_event(InputEvent::Modifiers(SHIFT));
    assert!(r.take().is_empty(), "no handler hears of a modifier change");
    assert_eq!(host.modifiers(), SHIFT, "the host records it");
}

#[test]
fn a_panel_passes_the_modifiers_it_saw_to_its_children() {
    let child = Recorder::at(10.0);
    let panel =
        Panel::new(Rect::new(0.0, 0.0, 300.0, 200.0)).with_control(ControlHandle::new(&child));
    let mut host = ControlHost::builder().with_control(panel.handle()).build();
    let ctrl = Modifiers { ctrl: true, ..NONE };
    host.deliver_event(InputEvent::Modifiers(ctrl));
    host.deliver_mouse(down(20.0));
    let contexts = child.mouse_contexts();
    assert_eq!(contexts.len(), 1, "the press reaches the child");
    assert_eq!(
        contexts[0].modifiers, ctrl,
        "with the modifiers the panel saw"
    );
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

#[test]
fn a_rebuilt_host_is_seeded_with_the_last_modifiers() {
    let r = Recorder::at(0.0);
    let alt = Modifiers { alt: true, ..NONE };
    let carried = {
        let mut first = ControlHost::builder()
            .with_control(ControlHandle::new(&r))
            .build();
        first.deliver_event(InputEvent::Modifiers(alt));
        first.modifiers()
    };
    let mut next = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .build();
    next.set_modifiers(carried);
    next.deliver_mouse(MouseEvent::Click {
        pos: [10.0, 10.0],
        button: MouseButton::Left,
    });
    let contexts = r.mouse_contexts();
    assert_eq!(contexts.last().map(|c| c.modifiers), Some(alt));
}

fn tab() -> KeyboardEvent {
    KeyboardEvent::KeyDown { key: Key::Tab }
}

fn three() -> [Recorder; 3] {
    common::take_focus_journal();
    [Recorder::at(0.0), Recorder::at(200.0), Recorder::at(400.0)]
}

fn host_of(rs: &[Recorder]) -> ControlHost<'_> {
    let mut builder = ControlHost::builder();
    for r in rs {
        builder = builder.with_control(ControlHandle::new(r));
    }
    builder.build()
}

fn focus_log(rs: &[Recorder]) -> Vec<(usize, FocusEvent)> {
    for r in rs {
        r.take();
    }
    common::take_focus_journal()
        .into_iter()
        .map(|(x, f)| {
            let i = rs
                .iter()
                .position(|r| r.rect.x == x)
                .expect("a known recorder");
            (i, f)
        })
        .collect()
}

#[test]
fn tab_sends_lost_then_gained() {
    let rs = three();
    let mut host = host_of(&rs);
    host.deliver_keyboard(tab());
    assert_eq!(focus_log(&rs), [(0, FocusEvent::Gained)]);
    host.deliver_keyboard(tab());
    assert_eq!(
        focus_log(&rs),
        [(0, FocusEvent::Lost), (1, FocusEvent::Gained)]
    );
    host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Escape });
    assert_eq!(focus_log(&rs), [(1, FocusEvent::Lost)]);
}

#[test]
fn backtab_sends_them_the_other_way() {
    let rs = three();
    let mut host = host_of(&rs);
    host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::BackTab });
    assert_eq!(focus_log(&rs), [(2, FocusEvent::Gained)]);
    host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::BackTab });
    assert_eq!(
        focus_log(&rs),
        [(2, FocusEvent::Lost), (1, FocusEvent::Gained)]
    );
}

#[test]
fn a_press_on_a_control_moves_the_focus_with_events() {
    let rs = three();
    let mut host = host_of(&rs);
    host.deliver_mouse(down(10.0));
    host.deliver_mouse(up(10.0));
    host.deliver_mouse(down(210.0));
    let log = focus_log(&rs);
    assert_eq!(
        log,
        [
            (0, FocusEvent::Gained),
            (0, FocusEvent::Lost),
            (1, FocusEvent::Gained)
        ]
    );
    assert_eq!(host.focused_index(), Some(1));
}

#[test]
fn escape_and_a_press_on_nothing_send_lost() {
    let rs = three();
    let mut host = host_of(&rs);
    host.set_focused_index(Some(0));
    focus_log(&rs);
    host.deliver_mouse(down(150.0));
    assert_eq!(
        focus_log(&rs),
        [(0, FocusEvent::Lost)],
        "a press on nothing"
    );
    host.set_focused_index(Some(2));
    focus_log(&rs);
    host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Escape });
    assert_eq!(focus_log(&rs), [(2, FocusEvent::Lost)], "Escape");
}

#[test]
fn restoring_focus_on_a_rebuilt_host_sends_gained_only() {
    let rs = three();
    let carried = {
        let mut first = host_of(&rs);
        first.set_focused_index(Some(1));
        first.focused_key()
    };
    focus_log(&rs);
    let mut next = host_of(&rs);
    next.set_focused_key(carried);
    assert_eq!(focus_log(&rs), [(1, FocusEvent::Gained)]);
}

#[test]
fn setting_the_focus_it_already_has_sends_nothing() {
    let rs = three();
    let mut host = host_of(&rs);
    host.set_focused_index(Some(1));
    focus_log(&rs);
    host.set_focused_index(Some(1));
    host.set_focused_key(host.focused_key());
    assert!(focus_log(&rs).is_empty());
}

#[test]
fn a_carried_key_that_no_longer_resolves_sends_nothing() {
    let rs = three();
    let mut host = host_of(&rs);
    host.set_focused_key(Some(ControlKey::named("gone")));
    assert_eq!(host.focused_index(), None);
    assert!(focus_log(&rs).is_empty());
}

#[test]
fn initial_focus_sends_gained_at_build() {
    let rs = three();
    let host = ControlHost::builder()
        .with_control(ControlHandle::new(&rs[0]))
        .with_control(ControlHandle::new(&rs[1]))
        .with_initial_focus(1)
        .build();
    assert_eq!(host.focused_index(), Some(1));
    assert_eq!(focus_log(&rs), [(1, FocusEvent::Gained)]);
}

#[test]
fn tab_skips_a_control_that_refuses_focus() {
    let rs = three();
    rs[1].refuses_focus.set(true);
    let mut host = host_of(&rs);
    host.set_focused_index(Some(0));
    host.deliver_keyboard(tab());
    assert_eq!(host.focused_index(), Some(2), "Tab from 0 lands on 2");
    host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::BackTab });
    assert_eq!(host.focused_index(), Some(0), "and BackTab skips it too");
}

#[test]
fn a_press_on_a_control_that_refuses_focus_leaves_focus_alone() {
    let rs = three();
    rs[1].refuses_focus.set(true);
    let mut host = host_of(&rs);
    host.set_focused_index(Some(0));
    focus_log(&rs);
    let resp = host.deliver_mouse(down(210.0));
    assert_eq!(resp.target_index, Some(1), "the press still reaches it");
    assert_eq!(host.focused_index(), Some(0), "but the focus stays");
    assert!(focus_log(&rs).is_empty());
}

#[test]
fn when_every_control_refuses_tab_clears_the_focus() {
    let rs = three();
    let mut host = host_of(&rs);
    host.set_focused_index(Some(0));
    for r in &rs {
        r.refuses_focus.set(true);
    }
    host.deliver_keyboard(tab());
    assert_eq!(host.focused_index(), None);
}

#[test]
fn tab_and_escape_releases_reach_no_control() {
    let rs = three();
    let mut host = host_of(&rs);
    host.set_focused_index(Some(0));
    rs[0].take();
    for key in [Key::Tab, Key::BackTab, Key::Escape] {
        host.deliver_keyboard(KeyboardEvent::KeyUp { key });
    }
    assert!(
        rs[0].take().is_empty(),
        "no release of a host key reaches the control"
    );
}

fn mv(x: f32) -> MouseEvent {
    MouseEvent::Move { pos: [x, 10.0] }
}

fn hover_logs(rs: &[Recorder]) -> Vec<Vec<Option<ControlPart>>> {
    rs.iter().map(|r| r.hovers()).collect()
}

const BODY: Option<ControlPart> = Some(ControlPart::Body);

#[test]
fn only_the_control_entered_and_the_one_left_are_told() {
    let rs = three();
    let mut host = host_of(&rs);
    host.deliver_mouse(mv(10.0));
    assert_eq!(hover_logs(&rs), [vec![BODY], vec![], vec![]]);
    host.deliver_mouse(mv(210.0));
    assert_eq!(hover_logs(&rs), [vec![None], vec![BODY], vec![]]);
}

#[test]
fn a_part_change_tells_the_same_control_once() {
    let r = Recorder::halved(0.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .build();
    host.deliver_mouse(mv(10.0));
    host.deliver_mouse(mv(80.0));
    host.deliver_mouse(mv(90.0));
    assert_eq!(
        r.hovers(),
        [Some(ControlPart::Custom(0)), Some(ControlPart::Custom(1))]
    );
}

#[test]
fn staying_put_tells_nobody() {
    let rs = three();
    let mut host = host_of(&rs);
    host.deliver_mouse(mv(10.0));
    hover_logs(&rs);
    host.deliver_mouse(mv(20.0));
    host.deliver_mouse(mv(30.0));
    assert_eq!(hover_logs(&rs), [vec![], vec![], vec![]]);
}

#[test]
fn leaving_everything_tells_only_the_one_left() {
    let rs = three();
    let mut host = host_of(&rs);
    host.deliver_mouse(mv(210.0));
    hover_logs(&rs);
    host.deliver_mouse(mv(150.0));
    assert_eq!(hover_logs(&rs), [vec![], vec![None], vec![]]);
    host.deliver_mouse(mv(160.0));
    assert_eq!(hover_logs(&rs), [vec![], vec![], vec![]]);
}

#[test]
fn a_grab_freezes_hover_until_the_release() {
    let rs = three();
    let mut host = host_of(&rs);
    host.deliver_mouse(down(10.0));
    hover_logs(&rs);
    host.deliver_mouse(drag(210.0));
    host.deliver_mouse(up(210.0));
    assert_eq!(
        hover_logs(&rs),
        [vec![], vec![], vec![]],
        "frozen while grabbed"
    );
    host.deliver_mouse(mv(210.0));
    assert_eq!(hover_logs(&rs), [vec![None], vec![BODY], vec![]]);
}

#[test]
fn hover_is_carried_across_a_rebuild_so_leaving_still_clears() {
    let rs = three();
    let carried = {
        let mut first = host_of(&rs);
        first.deliver_mouse(mv(10.0));
        first.hovered_key()
    };
    hover_logs(&rs);
    let mut next = host_of(&rs);
    next.set_hovered_key(carried);
    next.deliver_mouse(mv(150.0));
    assert_eq!(hover_logs(&rs), [vec![None], vec![], vec![]]);
}

#[test]
fn the_part_is_resent_once_after_a_rebuild() {
    let rs = three();
    let mut host = host_of(&rs);
    host.set_hovered_key(Some(ControlKey::at(0)));
    host.deliver_mouse(mv(10.0));
    host.deliver_mouse(mv(20.0));
    assert_eq!(hover_logs(&rs), [vec![BODY], vec![], vec![]]);
}

#[test]
fn clear_hover_tells_only_the_hovered_control() {
    let rs = three();
    let mut host = host_of(&rs);
    host.deliver_mouse(mv(410.0));
    hover_logs(&rs);
    host.clear_hover();
    assert_eq!(hover_logs(&rs), [vec![], vec![], vec![None]]);
    assert_eq!(host.hovered_key(), None);
    host.clear_hover();
    assert_eq!(
        hover_logs(&rs),
        [vec![], vec![], vec![]],
        "nothing left to clear"
    );
}

#[test]
fn a_hovered_key_that_no_longer_resolves_is_dropped() {
    let rs = three();
    let mut host = host_of(&rs);
    host.set_hovered_key(Some(ControlKey::named("gone")));
    assert_eq!(host.hovered_key(), None);
}

#[test]
fn a_panel_carries_its_childrens_hover() {
    let child = Recorder::at(10.0);
    let carried = {
        let panel = Panel::new(Rect::new(0.0, 0.0, 300.0, 200.0))
            .with_named_control("child", ControlHandle::new(&child));
        let mut host = ControlHost::builder().with_control(panel.handle()).build();
        host.deliver_mouse(down(20.0));
        host.deliver_mouse(up(20.0));
        panel.hovered_key()
    };
    assert_eq!(carried, Some(ControlKey::named("child")));
    child.take();
    let panel = Panel::new(Rect::new(0.0, 0.0, 300.0, 200.0))
        .with_named_control("child", ControlHandle::new(&child))
        .with_hovered_key(carried);
    let mut host = ControlHost::builder().with_control(panel.handle()).build();
    host.deliver_mouse(MouseEvent::Click {
        pos: [250.0, 150.0],
        button: MouseButton::Left,
    });
    assert_eq!(child.hovers(), [None], "leaving the child still clears it");
}

#[test]
fn at_most_two_set_hover_calls_per_pointer_event() {
    let rs: Vec<Recorder> = (0..8).map(|i| Recorder::halved(i as f32 * 120.0)).collect();
    let mut host = host_of(&rs);
    let xs = [
        5.0, 70.0, 130.0, 110.0, 950.0, 860.0, 15.0, 400.0, 300.0, 610.0,
    ];
    for x in xs {
        host.deliver_mouse(mv(x));
        let calls: usize = rs.iter().map(|r| r.hovers().len()).sum();
        assert!(calls <= 2, "a move to {x} made {calls} set_hover calls");
    }
}
