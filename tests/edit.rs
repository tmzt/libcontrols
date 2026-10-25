use libcontrols::control::HostedControl;
use libcontrols::event::{
    ControlAction, ControlPart, EventContext, Key, KeyboardEvent, MouseButton, MouseEvent,
};
use libcontrols::flags::NodeState;
use libcontrols::rect::{Rect, TextAlign};
use libcontrols::traits::{DrawContext, DrawControl};
use libcontrols::{Edit, EditState};
use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfKind};

#[test]
fn cursor_bounds_and_utf8_safety() {
    let state = EditState::new("🦀a🚀");
    // "🦀" is 4 bytes (0..4), "a" is 1 byte (4..5), "🚀" is 4 bytes (5..9).
    assert_eq!(state.cursor(), 9);

    // Setting beyond length clamps to text.len()
    state.set_cursor(100);
    assert_eq!(state.cursor(), 9);

    // Setting inside '🦀' (bytes 1, 2, 3) clamps to 0 or 4
    state.set_cursor(1);
    assert_eq!(state.cursor(), 0);
    state.set_cursor(3);
    assert_eq!(state.cursor(), 4);

    // Setting inside '🚀' (bytes 6, 7, 8)
    state.set_cursor(6);
    assert_eq!(state.cursor(), 5);
    state.set_cursor(8);
    assert_eq!(state.cursor(), 9);

    // Setting on valid boundaries
    state.set_cursor(0);
    assert_eq!(state.cursor(), 0);
    state.set_cursor(4);
    assert_eq!(state.cursor(), 4);
    state.set_cursor(5);
    assert_eq!(state.cursor(), 5);
    state.set_cursor(9);
    assert_eq!(state.cursor(), 9);

    // Empty string
    let empty = EditState::new("");
    assert_eq!(empty.cursor(), 0);
    empty.set_cursor(5);
    assert_eq!(empty.cursor(), 0);
}

#[test]
fn cursor_movement_left_right_home_end() {
    let state = EditState::new("🦀a🚀");
    assert_eq!(state.cursor(), 9);

    // Step left
    state.move_cursor_left();
    assert_eq!(state.cursor(), 5); // before '🚀'
    state.move_cursor_left();
    assert_eq!(state.cursor(), 4); // before 'a'
    state.move_cursor_left();
    assert_eq!(state.cursor(), 0); // before '🦀'
    state.move_cursor_left();
    assert_eq!(state.cursor(), 0); // clamped at 0

    // Step right
    state.move_cursor_right();
    assert_eq!(state.cursor(), 4);
    state.move_cursor_right();
    assert_eq!(state.cursor(), 5);
    state.move_cursor_right();
    assert_eq!(state.cursor(), 9);
    state.move_cursor_right();
    assert_eq!(state.cursor(), 9); // clamped at len

    // Home and End
    state.move_cursor_home();
    assert_eq!(state.cursor(), 0);
    state.move_cursor_end();
    assert_eq!(state.cursor(), 9);
}

#[test]
fn editing_insert_and_delete_operations() {
    let state = EditState::new("ac");
    state.set_cursor(1);

    // Insert char in middle
    state.insert_char('b');
    assert_eq!(state.text(), "abc");
    assert_eq!(state.cursor(), 2);

    // Insert text at start
    state.move_cursor_home();
    state.insert_text("12");
    assert_eq!(state.text(), "12abc");
    assert_eq!(state.cursor(), 2);

    // Delete backwards in middle
    let deleted = state.delete_backwards();
    assert!(deleted);
    assert_eq!(state.text(), "1abc");
    assert_eq!(state.cursor(), 1);

    // Delete forwards
    let deleted_fwd = state.delete_forwards();
    assert!(deleted_fwd);
    assert_eq!(state.text(), "1bc");
    assert_eq!(state.cursor(), 1);

    // Delete backwards at 0 returns false
    state.move_cursor_home();
    assert!(!state.delete_backwards());
    assert_eq!(state.text(), "1bc");
    assert_eq!(state.cursor(), 0);

    // Delete forwards at end returns false
    state.move_cursor_end();
    assert!(!state.delete_forwards());
    assert_eq!(state.text(), "1bc");
    assert_eq!(state.cursor(), 3);

    // Multi-byte UTF-8 deletion
    state.set_text("🦀rust");
    state.set_cursor(4); // after '🦀'
    assert!(state.delete_backwards());
    assert_eq!(state.text(), "rust");
    assert_eq!(state.cursor(), 0);

    state.set_text("rust🦀");
    state.set_cursor(4); // before '🦀'
    assert!(state.delete_forwards());
    assert_eq!(state.text(), "rust");
    assert_eq!(state.cursor(), 4);

    // set_text with shorter string clamps cursor
    state.set_cursor(4);
    state.set_text("hi");
    assert_eq!(state.cursor(), 2);
}

#[test]
fn mouse_click_and_drag_hit_testing() {
    let rect = Rect::new(10.0, 20.0, 100.0, 30.0);
    let edit = Edit::new("hello", rect)
        .with_align(TextAlign::Left)
        .with_font_size(14.0);

    // pad_h = 4.0, font_size = 14.0, fallback char_w = 14.0 * 0.55 = 7.7
    // pen_x = 10.0 + 4.0 = 14.0
    // char 0 boundary: 14.0 + 0.0 = 14.0
    // char 1 boundary: 14.0 + 7.7 = 21.7
    // char 2 boundary: 14.0 + 15.4 = 29.4
    // char 3 boundary: 14.0 + 23.1 = 37.1
    // char 4 boundary: 14.0 + 30.8 = 44.8
    // char 5 boundary: 14.0 + 38.5 = 52.5

    // Click to the left of pen_x -> index 0
    assert_eq!(edit.char_index_at_x(5.0), 0);
    assert_eq!(edit.char_index_at_x(14.0), 0);

    // Click near char 1 boundary (21.7): midpoint between 14.0 and 21.7 is 17.85
    assert_eq!(edit.char_index_at_x(16.0), 0);
    assert_eq!(edit.char_index_at_x(19.0), 1);
    assert_eq!(edit.char_index_at_x(21.7), 1);

    // Click far to the right -> index 5 ("hello".len())
    assert_eq!(edit.char_index_at_x(80.0), 5);

    // handle_mouse with Click and Down
    let ctx = EventContext::new(ControlPart::NumericBox);
    let click_ev = MouseEvent::Click {
        pos: [19.0, 25.0],
        button: MouseButton::Left,
    };
    let act = edit.handle_mouse(&click_ev, &ctx);
    assert_eq!(
        act,
        Some(ControlAction::Activated {
            part: ControlPart::NumericBox
        })
    );
    assert_eq!(edit.cursor(), 1);

    // Drag to new pos
    let drag_ev = MouseEvent::Drag {
        start_pos: [19.0, 25.0],
        current_pos: [40.0, 25.0],
        delta: [21.0, 0.0],
        button: MouseButton::Left,
    };
    let drag_act = edit.handle_mouse(&drag_ev, &ctx);
    assert!(drag_act.is_none());
    assert_eq!(edit.cursor(), 3);
}

#[test]
fn keyboard_navigation_and_text_input() {
    let edit = Edit::new("test", Rect::new(0.0, 0.0, 80.0, 24.0)).with_cursor(4);
    let ctx = EventContext::new(ControlPart::Body);

    // Left arrow
    let key_left = KeyboardEvent::KeyDown { key: Key::Left };
    let resp = edit.handle_keyboard(&key_left, &ctx);
    assert_eq!(resp, Some(ControlAction::StepLeft));
    assert_eq!(edit.cursor(), 3);

    // Home
    let key_home = KeyboardEvent::KeyDown { key: Key::Home };
    let resp = edit.handle_keyboard(&key_home, &ctx);
    assert_eq!(resp, Some(ControlAction::StepLeft));
    assert_eq!(edit.cursor(), 0);

    // Right arrow
    let key_right = KeyboardEvent::KeyDown { key: Key::Right };
    let resp = edit.handle_keyboard(&key_right, &ctx);
    assert_eq!(resp, Some(ControlAction::StepRight));
    assert_eq!(edit.cursor(), 1);

    // End
    let key_end = KeyboardEvent::KeyDown { key: Key::End };
    let resp = edit.handle_keyboard(&key_end, &ctx);
    assert_eq!(resp, Some(ControlAction::StepRight));
    assert_eq!(edit.cursor(), 4);

    // Backspace
    let key_bs = KeyboardEvent::KeyDown {
        key: Key::Backspace,
    };
    let resp = edit.handle_keyboard(&key_bs, &ctx);
    assert_eq!(resp, Some(ControlAction::Custom("backspace".into())));
    assert_eq!(edit.text(), "tes");
    assert_eq!(edit.cursor(), 3);

    // Character input
    let key_char = KeyboardEvent::Char { ch: 't' };
    let resp = edit.handle_keyboard(&key_char, &ctx);
    assert_eq!(resp, Some(ControlAction::TextInput { ch: 't' }));
    assert_eq!(edit.text(), "test");
    assert_eq!(edit.cursor(), 4);

    // Space key
    let key_space = KeyboardEvent::KeyDown { key: Key::Space };
    let resp = edit.handle_keyboard(&key_space, &ctx);
    assert_eq!(resp, Some(ControlAction::TextInput { ch: ' ' }));
    assert_eq!(edit.text(), "test ");
    assert_eq!(edit.cursor(), 5);

    // Delete key
    edit.set_cursor(0);
    let key_del = KeyboardEvent::KeyDown { key: Key::Delete };
    let resp = edit.handle_keyboard(&key_del, &ctx);
    assert_eq!(resp, Some(ControlAction::Custom("delete".into())));
    assert_eq!(edit.text(), "est ");
    assert_eq!(edit.cursor(), 0);

    // Enter key
    let key_enter = KeyboardEvent::KeyDown { key: Key::Enter };
    let resp = edit.handle_keyboard(&key_enter, &ctx);
    assert_eq!(
        resp,
        Some(ControlAction::Activated {
            part: ControlPart::NumericBox
        })
    );
}

#[test]
fn cursor_rendering_in_drawlist() {
    let rect = Rect::new(0.0, 0.0, 100.0, 30.0);
    let edit = Edit::new("abc", rect)
        .with_align(TextAlign::Left)
        .with_font_size(14.0);

    let ctx = DrawContext::default();

    // Inactive / unfocused: cursor box is NOT drawn
    let mut list = DrawList::new();
    edit.draw(&mut list, &ctx);

    let has_cursor = list
        .instances
        .iter()
        .any(|inst| inst.size[0] == 1.5 && inst.kind == SdfKind::Box);
    assert!(!has_cursor, "Unfocused edit should not draw cursor bar");

    // Active edit: cursor box IS drawn
    let active_edit = edit.clone().with_active(true).with_cursor(1);
    let mut list_active = DrawList::new();
    active_edit.draw(&mut list_active, &ctx);

    let cursor_inst = list_active
        .instances
        .iter()
        .find(|inst| inst.size[0] == 1.5 && inst.kind == SdfKind::Box);
    assert!(
        cursor_inst.is_some(),
        "Active edit must draw cursor bar SDF box"
    );

    let cursor = cursor_inst.unwrap();
    let pad_h = 4.0;
    let line_box_h = 14.0 * LINE_BOX_RATIO;
    let expected_pen_y = (30.0 - line_box_h) * 0.5;
    assert_eq!(cursor.size[1], line_box_h);
    assert_eq!(cursor.position[1], expected_pen_y);

    // Offset of 1 character: advance of "a"
    let adv_1 = ctx.text_advance("a", 14.0);
    assert!((cursor.position[0] - (pad_h + adv_1)).abs() < 1e-4);

    // Focused via ctx.state (NodeState::FOCUSED)
    let focused_ctx = DrawContext {
        state: NodeState::FOCUSED,
        ..DrawContext::default()
    };
    let mut list_focused = DrawList::new();
    edit.with_cursor(2).draw(&mut list_focused, &focused_ctx);

    let focused_cursor = list_focused
        .instances
        .iter()
        .find(|inst| inst.size[0] == 1.5 && inst.kind == SdfKind::Box);
    assert!(
        focused_cursor.is_some(),
        "NodeState::FOCUSED edit must draw cursor bar"
    );
    let adv_2 = ctx.text_advance("ab", 14.0);
    assert!((focused_cursor.unwrap().position[0] - (pad_h + adv_2)).abs() < 1e-4);
}
