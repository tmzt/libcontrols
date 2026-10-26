use std::sync::Arc;

use libcontrols::control::HostedControl;
use libcontrols::event::{
    ControlAction, ControlPart, EventContext, Key, KeyboardEvent, MouseButton, MouseEvent,
};
use libcontrols::flags::NodeState;
use libcontrols::rect::Rect;
use libcontrols::text_area::{cursor_line_and_col, wrap_text};
use libcontrols::traits::{DrawContext, DrawControl};
use libcontrols::{TextArea, TextAreaState};
use libmsdf::drawlist::{DrawList, SdfKind};

#[test]
fn wrap_text_empty_and_single_line() {
    let measure = |s: &str| s.len() as f32 * 10.0;

    let empty = wrap_text("", 100.0, measure);
    assert_eq!(empty.len(), 1);
    assert_eq!(empty[0].start, 0);
    assert_eq!(empty[0].end, 0);
    assert_eq!(empty[0].next_start, 0);

    let single = wrap_text("hello", 100.0, measure);
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].start, 0);
    assert_eq!(single[0].end, 5);
    assert_eq!(single[0].next_start, 5);
    assert_eq!(single[0].width, 50.0);
}

#[test]
fn wrap_text_explicit_newlines() {
    let measure = |s: &str| s.len() as f32 * 10.0;

    // "a\nb"
    let lines = wrap_text("a\nb", 100.0, measure);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].start, 0);
    assert_eq!(lines[0].end, 1);
    assert_eq!(lines[0].next_start, 2);
    assert_eq!(lines[1].start, 2);
    assert_eq!(lines[1].end, 3);
    assert_eq!(lines[1].next_start, 3);

    // Trailing newline "hello\n"
    let trailing = wrap_text("hello\n", 100.0, measure);
    assert_eq!(trailing.len(), 2);
    assert_eq!(trailing[0].start, 0);
    assert_eq!(trailing[0].end, 5);
    assert_eq!(trailing[0].next_start, 6);
    assert_eq!(trailing[1].start, 6);
    assert_eq!(trailing[1].end, 6);
    assert_eq!(trailing[1].next_start, 6);

    // Multiple newlines "\n\n"
    let double_nl = wrap_text("\n\n", 100.0, measure);
    assert_eq!(double_nl.len(), 3);
    assert_eq!(double_nl[0].start, 0);
    assert_eq!(double_nl[0].end, 0);
    assert_eq!(double_nl[0].next_start, 1);
    assert_eq!(double_nl[1].start, 1);
    assert_eq!(double_nl[1].end, 1);
    assert_eq!(double_nl[1].next_start, 2);
    assert_eq!(double_nl[2].start, 2);
    assert_eq!(double_nl[2].end, 2);
    assert_eq!(double_nl[2].next_start, 2);
}

#[test]
fn wrap_text_soft_wrapping() {
    let measure = |s: &str| s.len() as f32 * 10.0;

    // "one two three":
    // "one " is 4 chars = 40.0
    // "one two " is 8 chars = 80.0
    // With max_width = 85.0: "one two " fits, "three" wraps to next line
    let lines = wrap_text("one two three", 85.0, measure);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].start, 0);
    assert_eq!(lines[0].end, 8); // "one two "
    assert_eq!(lines[0].next_start, 8);
    assert_eq!(lines[1].start, 8);
    assert_eq!(lines[1].end, 13); // "three"
    assert_eq!(lines[1].next_start, 13);

    // Mid-word split for words wider than max_width
    let mid_word = wrap_text("abcdefgh", 35.0, measure);
    // 35.0 fits 3 chars (30.0), breaks at 3
    assert_eq!(mid_word.len(), 3);
    assert_eq!(mid_word[0].start, 0);
    assert_eq!(mid_word[0].end, 3);
    assert_eq!(mid_word[1].start, 3);
    assert_eq!(mid_word[1].end, 6);
    assert_eq!(mid_word[2].start, 6);
    assert_eq!(mid_word[2].end, 8);
}

#[test]
fn cursor_line_and_col_mapping() {
    let measure = |s: &str| s.len() as f32 * 10.0;
    let lines = wrap_text("line1\nline2", 100.0, measure);
    // line 0: 0..5, next_start: 6
    // line 1: 6..11, next_start: 11

    assert_eq!(cursor_line_and_col(&lines, 0), (0, 0));
    assert_eq!(cursor_line_and_col(&lines, 3), (0, 3));
    assert_eq!(cursor_line_and_col(&lines, 5), (0, 5));
    assert_eq!(cursor_line_and_col(&lines, 6), (1, 0));
    assert_eq!(cursor_line_and_col(&lines, 8), (1, 2));
    assert_eq!(cursor_line_and_col(&lines, 11), (1, 5));
}

#[test]
fn mouse_click_and_drag_positioning() {
    let rect = Rect::new(10.0, 20.0, 200.0, 100.0);
    let area = TextArea::new("line1\nline2", rect)
        .with_padding([10.0, 10.0])
        .with_font_size(14.0)
        .with_line_height(1.5);

    // pen_origin: [20.0, 30.0]
    // line_step: 14.0 * 1.5 = 21.0
    // line 0 y range: 30.0 .. 51.0
    // line 1 y range: 51.0 .. 72.0
    // char_w: 14.0 * 0.55 = 7.7

    // Click on line 0 at start
    let ctx = EventContext::new(ControlPart::Body);
    let click1 = MouseEvent::Click {
        pos: [20.0, 35.0],
        button: MouseButton::Left,
    };
    let act = area.handle_mouse(&click1, &ctx);
    assert_eq!(
        act,
        Some(ControlAction::Activated {
            part: ControlPart::Body
        })
    );
    assert_eq!(area.cursor(), 0);

    // Click on line 0 after char 2 (approx 20.0 + 2 * 7.7 = 35.4)
    let click2 = MouseEvent::Click {
        pos: [36.0, 35.0],
        button: MouseButton::Left,
    };
    area.handle_mouse(&click2, &ctx);
    assert_eq!(area.cursor(), 2);

    // Click on line 1 at start
    let click3 = MouseEvent::Click {
        pos: [20.0, 55.0],
        button: MouseButton::Left,
    };
    area.handle_mouse(&click3, &ctx);
    assert_eq!(area.cursor(), 6); // "line2" starts at byte 6

    // Drag to line 1 char 3
    let drag = MouseEvent::Drag {
        start_pos: [20.0, 55.0],
        current_pos: [44.0, 55.0],
        delta: [24.0, 0.0],
        button: MouseButton::Left,
    };
    area.handle_mouse(&drag, &ctx);
    assert_eq!(area.cursor(), 9); // 6 + 3 = 9
}

#[test]
fn keyboard_arrow_navigation_up_down_left_right() {
    let rect = Rect::new(0.0, 0.0, 200.0, 100.0);
    let area = TextArea::new("abcd\nefgh", rect)
        .with_padding([0.0, 0.0])
        .with_font_size(14.0)
        .with_cursor(0);

    let ctx = EventContext::new(ControlPart::Body);

    // Step right twice
    let right = KeyboardEvent::KeyDown { key: Key::Right };
    area.handle_keyboard(&right, &ctx);
    assert_eq!(area.cursor(), 1);
    area.handle_keyboard(&right, &ctx);
    assert_eq!(area.cursor(), 2);

    // Step left once
    let left = KeyboardEvent::KeyDown { key: Key::Left };
    area.handle_keyboard(&left, &ctx);
    assert_eq!(area.cursor(), 1);

    // Step down to line 1
    let down = KeyboardEvent::KeyDown { key: Key::Down };
    area.handle_keyboard(&down, &ctx);
    // Line 1 starts at 5 ("abcd\n" is 5 bytes). Col 1 is byte 6 ('f').
    assert_eq!(area.cursor(), 6);

    // Step up back to line 0
    let up = KeyboardEvent::KeyDown { key: Key::Up };
    area.handle_keyboard(&up, &ctx);
    assert_eq!(area.cursor(), 1);

    // Step up when at top line moves to home (0)
    area.handle_keyboard(&up, &ctx);
    assert_eq!(area.cursor(), 0);

    // Home and End
    let end = KeyboardEvent::KeyDown { key: Key::End };
    area.handle_keyboard(&end, &ctx);
    assert_eq!(area.cursor(), 4); // end of line 0 ("abcd")

    let home = KeyboardEvent::KeyDown { key: Key::Home };
    area.handle_keyboard(&home, &ctx);
    assert_eq!(area.cursor(), 0);
}

#[test]
fn keyboard_editing_enter_backspace_delete_insert() {
    let rect = Rect::new(0.0, 0.0, 200.0, 100.0);
    let area = TextArea::new("abc", rect).with_cursor(3);
    let ctx = EventContext::new(ControlPart::Body);

    // Enter inserts newline
    let enter = KeyboardEvent::KeyDown { key: Key::Enter };
    let act_enter = area.handle_keyboard(&enter, &ctx);
    assert_eq!(act_enter, Some(ControlAction::TextInput { ch: '\n' }));
    assert_eq!(area.text(), "abc\n");
    assert_eq!(area.cursor(), 4);

    // Char input
    let char_d = KeyboardEvent::Char { ch: 'd' };
    area.handle_keyboard(&char_d, &ctx);
    assert_eq!(area.text(), "abc\nd");
    assert_eq!(area.cursor(), 5);

    // Backspace
    let bs = KeyboardEvent::KeyDown {
        key: Key::Backspace,
    };
    area.handle_keyboard(&bs, &ctx);
    assert_eq!(area.text(), "abc\n");
    assert_eq!(area.cursor(), 4);

    // Backspace the newline
    area.handle_keyboard(&bs, &ctx);
    assert_eq!(area.text(), "abc");
    assert_eq!(area.cursor(), 3);

    // Move to 0 and Delete
    area.set_cursor(0);
    let del = KeyboardEvent::KeyDown { key: Key::Delete };
    area.handle_keyboard(&del, &ctx);
    assert_eq!(area.text(), "bc");
    assert_eq!(area.cursor(), 0);
}

#[test]
fn rendering_emits_background_border_cursor_and_glow() {
    let mut list = DrawList::new();
    let ctx = DrawContext {
        state: NodeState::FOCUSED,
        ..DrawContext::default()
    };

    let rect = Rect::new(20.0, 20.0, 200.0, 100.0);
    let area = TextArea::new("Hello\nWorld", rect)
        .with_cursor(5) // end of line 0
        .with_active(true);

    area.draw(&mut list, &ctx);

    // Should draw:
    // 1. Background rounded box
    // 2. Border outline
    // 3. Cursor box
    // 4. Glow ring outlines
    let has_bg = list
        .instances
        .iter()
        .any(|inst| matches!(inst.kind, SdfKind::RoundedBox { .. }));
    let has_outline = list
        .instances
        .iter()
        .any(|inst| matches!(inst.kind, SdfKind::Outline { .. }));
    let has_cursor = list
        .instances
        .iter()
        .any(|inst| matches!(inst.kind, SdfKind::Box) && (inst.size[0] - 1.5).abs() < 1e-3);

    assert!(has_bg, "must draw background");
    assert!(has_outline, "must draw outline/glow");
    assert!(has_cursor, "must draw cursor bar");
}

#[test]
fn state_sharing_with_edit_state() {
    let state = Arc::new(TextAreaState::new("shared initial"));
    let area = TextArea::new("", Rect::new(0.0, 0.0, 100.0, 50.0)).with_state(state.clone());

    assert_eq!(area.text(), "shared initial");
    area.set_text("updated via area");
    assert_eq!(state.text(), "updated via area");
}
