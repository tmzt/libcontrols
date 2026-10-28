//! EditableLabel through the router: modes, taps, typing, confirm, cancel,
//! clear, placeholder, read-only and theme resolution
//! (docs/design/20261028_05_EDITABLE_LABEL_PLAN.md).

use std::sync::Arc;

use libcontrols::{
    ControlAction, ControlTheme, DrawContext, EditableLabel, EditableLabelEvent,
    EditableLabelOptions, EditableLabelState, EditableLabelTheme, Key, KeyboardEvent, MouseButton,
    MouseEvent, NodeFlags, NodeId, ReadValue, Rect, Router,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

// The control is at (10, 10) 200 x 32. With the default 32 px buttons the
// pencil / check is x 178..210, the X (when shown) 146..178.
const RECT: Rect = Rect {
    x: 10.0,
    y: 10.0,
    w: 200.0,
    h: 32.0,
};
const BUTTON: [f32; 2] = [194.0, 26.0];
const X_BUTTON: [f32; 2] = [162.0, 26.0];
const TEXT: [f32; 2] = [40.0, 26.0];
const OUTSIDE: [f32; 2] = [400.0, 400.0];

fn tap(router: &Router, p: [f32; 2]) -> Option<ControlAction> {
    let button = MouseButton::Left;
    router.deliver_mouse(MouseEvent::Down { pos: p, button });
    router.deliver_mouse(MouseEvent::Up { pos: p, button });
    router
        .deliver_mouse(MouseEvent::Click { pos: p, button })
        .action
}

fn key(router: &Router, k: Key) -> Option<ControlAction> {
    router
        .deliver_keyboard(KeyboardEvent::KeyDown { key: k })
        .action
}

fn type_str(router: &Router, s: &str) {
    for ch in s.chars() {
        router.deliver_keyboard(KeyboardEvent::Char { ch });
    }
}

fn render_with(router: &Router, ctx: &DrawContext) -> DrawList {
    let mut list = DrawList::new();
    router.render(&mut list, ctx);
    list
}

fn render(router: &Router) -> DrawList {
    render_with(router, &DrawContext::default())
}

fn lines(list: &DrawList) -> Vec<&SdfInstance> {
    list.instances
        .iter()
        .filter(|i| matches!(i.kind, SdfKind::Line { .. }))
        .collect()
}

/// A router holding `label`, drawn once so hit testing has its layout.
fn setup(label: EditableLabel) -> (Router, NodeId, Arc<EditableLabelState>) {
    let state = label.state.clone();
    let mut router = Router::new();
    let id = router.add(None, label);
    render(&router);
    (router, id, state)
}

#[test]
fn display_mode_draws_the_pencil_and_no_field() {
    let (router, id, state) = setup(EditableLabel::new("Laptop", RECT));
    let list = render(&router);
    let segs = lines(&list);
    assert_eq!(segs.len(), 6, "the pencil is six segments");
    let muted = ControlTheme::default().muted_text_color;
    assert!(segs.iter().all(|l| l.color == muted), "in the muted colour");
    assert_eq!(
        list.instances.len(),
        6,
        "no field, no ring, no text without a font"
    );
    for l in &segs {
        assert!(
            l.position[0] >= 178.0 && l.position[0] <= 210.0,
            "in the button"
        );
    }
    assert!(!state.is_editing());
    assert!(router.flags(id).contains(NodeFlags::FOCUSABLE));
    let label = router.get::<EditableLabel>(id).unwrap();
    assert_eq!(label.read_value(), "Laptop");
}

#[test]
fn pencil_tap_switches_to_editing_with_a_check() {
    let (router, id, state) = setup(EditableLabel::new("Laptop", RECT));
    let action = tap(&router, BUTTON);
    assert_eq!(
        action,
        Some(ControlAction::Activated {
            part: EditableLabel::PENCIL
        })
    );
    assert!(state.is_editing());
    assert_eq!(state.draft().as_deref(), Some("Laptop"));
    assert_eq!(state.draft_state().cursor(), 6, "cursor at the end");
    assert_eq!(state.take_events(), vec![EditableLabelEvent::Started]);
    assert_eq!(router.focused(), Some(id), "the press focused it");

    let list = render(&router);
    let segs = lines(&list);
    assert_eq!(segs.len(), 2, "the check, no X (clear is off)");
    let accent = ControlTheme::default().accent_color;
    assert!(segs.iter().all(|l| l.color == accent));
    let field_bg = EditableLabelTheme::default().field_bg;
    let field = list
        .instances
        .iter()
        .find(|i| i.color == field_bg)
        .expect("the field's fill");
    assert_eq!(field.position, [10.0, 10.0]);
    assert_eq!(field.size, [200.0 - 32.0 - 4.0, 32.0], "left of the gap");
    let cursor = list
        .instances
        .iter()
        .find(|i| matches!(i.kind, SdfKind::Box) && i.color == accent);
    assert!(cursor.is_some(), "the field's cursor");
}

#[test]
fn typing_then_check_confirms_the_value() {
    let (router, _, state) = setup(EditableLabel::new("Laptop", RECT));
    tap(&router, BUTTON);
    render(&router);
    type_str(&router, " 2");
    key(&router, Key::Backspace);
    type_str(&router, "3");
    assert_eq!(state.draft().as_deref(), Some("Laptop 3"));
    assert_eq!(state.text(), "Laptop", "the text waits for confirm");

    let action = tap(&router, BUTTON);
    assert_eq!(
        action,
        Some(ControlAction::Activated {
            part: EditableLabel::CONFIRM
        })
    );
    assert!(!state.is_editing());
    assert_eq!(state.text(), "Laptop 3");
    assert_eq!(
        state.take_events(),
        vec![
            EditableLabelEvent::Started,
            EditableLabelEvent::Confirmed("Laptop 3".into())
        ]
    );
    assert_eq!(lines(&render(&router)).len(), 6, "back to the pencil");
}

#[test]
fn enter_confirms() {
    let (router, _, state) = setup(EditableLabel::new("a", RECT));
    tap(&router, BUTTON);
    type_str(&router, "b");
    key(&router, Key::Enter);
    assert_eq!(state.text(), "ab");
    assert!(!state.is_editing());
}

#[test]
fn escape_reverts() {
    let (router, _, state) = setup(EditableLabel::new("Laptop", RECT));
    tap(&router, BUTTON);
    type_str(&router, "xyz");
    // The router takes Escape and clears the focus; losing it cancels.
    key(&router, Key::Escape);
    assert_eq!(router.focused(), None);
    assert!(!state.is_editing());
    assert_eq!(state.text(), "Laptop");
    assert_eq!(
        state.take_events(),
        vec![EditableLabelEvent::Started, EditableLabelEvent::Cancelled]
    );
}

#[test]
fn back_and_a_press_elsewhere_cancel() {
    let (router, _, state) = setup(EditableLabel::new("one", RECT));
    tap(&router, BUTTON);
    type_str(&router, "!");
    key(&router, Key::Back);
    assert_eq!(state.text(), "one");
    assert!(!state.is_editing());

    tap(&router, BUTTON);
    type_str(&router, "?");
    tap(&router, OUTSIDE);
    assert!(!state.is_editing());
    assert_eq!(state.text(), "one");
}

#[test]
fn clear_is_shown_only_when_enabled_and_non_empty() {
    // Off: no X, and its place is the field.
    let (router, _, state) = setup(EditableLabel::new("Laptop", RECT));
    tap(&router, BUTTON);
    render(&router);
    assert_eq!(lines(&render(&router)).len(), 2);
    tap(&router, X_BUTTON);
    assert_eq!(state.draft().as_deref(), Some("Laptop"), "nothing cleared");
    assert!(!state.take_events().contains(&EditableLabelEvent::Cleared));

    // On: the X, left of the check.
    let (router, _, state) = setup(EditableLabel::new("Laptop", RECT).with_clear(true));
    tap(&router, BUTTON);
    let list = render(&router);
    let segs = lines(&list);
    assert_eq!(segs.len(), 4, "X and check");
    let muted = ControlTheme::default().muted_text_color;
    let xs: Vec<_> = segs.iter().filter(|l| l.color == muted).collect();
    assert_eq!(xs.len(), 2);
    assert!(
        xs.iter()
            .all(|l| l.position[0] >= 146.0 && l.position[0] <= 178.0)
    );
    let action = tap(&router, X_BUTTON);
    assert_eq!(
        action,
        Some(ControlAction::Activated {
            part: EditableLabel::CLEAR
        })
    );
    assert_eq!(state.draft().as_deref(), Some(""));
    assert!(state.is_editing(), "clearing keeps editing");
    assert_eq!(
        state.take_events(),
        vec![EditableLabelEvent::Started, EditableLabelEvent::Cleared]
    );

    // Empty: no X; a tap there is the field's.
    assert_eq!(lines(&render(&router)).len(), 2);
    assert_eq!(
        tap(&router, X_BUTTON),
        Some(ControlAction::Activated {
            part: EditableLabel::FIELD
        })
    );
    assert!(state.take_events().is_empty());

    // Confirming the empty value: the caller decides what empty means.
    tap(&router, BUTTON);
    assert_eq!(state.text(), "");
    assert_eq!(
        state.take_events(),
        vec![EditableLabelEvent::Confirmed(String::new())]
    );
}

#[test]
fn placeholder_for_empty_text_and_draft() {
    let label = EditableLabel::new("", RECT).with_placeholder("Pixel 9");
    assert!(label.shows_placeholder());
    assert_eq!(label.shown_text(), "Pixel 9");
    let (router, id, state) = setup(label);
    tap(&router, BUTTON);
    let label = router.get::<EditableLabel>(id).unwrap();
    assert!(label.shows_placeholder(), "over the empty field");
    type_str(&router, "M");
    assert!(!label.shows_placeholder());
    assert_eq!(label.shown_text(), "M");
    state.cancel();
    assert!(label.shows_placeholder());

    let plain = EditableLabel::new("", RECT);
    assert!(!plain.shows_placeholder(), "no placeholder set");
}

#[test]
fn read_only_draws_text_only_and_never_edits() {
    let (router, id, state) = setup(EditableLabel::new("Laptop", RECT).with_read_only(true));
    assert!(!router.flags(id).contains(NodeFlags::FOCUSABLE));
    assert!(lines(&render(&router)).is_empty(), "no pencil");
    assert_eq!(tap(&router, BUTTON), None);
    assert!(!state.is_editing());
}

#[test]
fn the_label_edits_only_when_enabled() {
    let (router, _, state) = setup(EditableLabel::new("Laptop", RECT));
    assert_eq!(tap(&router, TEXT), None);
    assert!(!state.is_editing());

    let (router, _, state) = setup(EditableLabel::new("Laptop", RECT).with_label_edits(true));
    tap(&router, TEXT);
    assert!(state.is_editing());
}

#[test]
fn enter_or_space_on_the_focused_label_starts_editing() {
    let (router, id, state) = setup(EditableLabel::new("Laptop", RECT));
    router.set_focus(Some(id));
    key(&router, Key::Enter);
    assert!(state.is_editing());
    key(&router, Key::Enter);
    assert!(!state.is_editing(), "and Enter confirms");
    key(&router, Key::Space);
    assert!(state.is_editing());
}

#[test]
fn theme_defaults_and_option_overrides_resolve_at_draw() {
    let red = [1.0, 0.0, 0.0, 1.0];
    let green = [0.0, 1.0, 0.0, 1.0];
    let mut theme = ControlTheme::default();
    theme.editable_label.confirm_color = Some(red);
    theme.editable_label.button_width = 48.0;

    let options = EditableLabelOptions::new().pencil_color(green).gap(10.0);
    let resolved = options.with_theme(&theme.editable_label);
    assert_eq!(resolved.confirm_color, Some(red), "the theme's");
    assert_eq!(resolved.pencil_color, Some(green), "the override");
    assert_eq!(resolved.button_width, 48.0);
    assert_eq!(resolved.gap, 10.0);
    assert_eq!(resolved.icon_size, 14.0, "the default theme's");

    let label = EditableLabel::new("Laptop", RECT).with_options(options);
    let state = label.state.clone();
    let mut router = Router::new();
    router.add(None, label);
    let ctx = DrawContext::default().with_theme(theme);
    let segs = render_with(&router, &ctx);
    assert!(lines(&segs).iter().all(|l| l.color == green));

    // The 48 px button from the drawn theme: x 162..210.
    tap(&router, [170.0, 26.0]);
    assert!(state.is_editing());
    let list = render_with(&router, &ctx);
    assert!(lines(&list).iter().all(|l| l.color == red), "check in red");
    let field_bg = EditableLabelTheme::default().field_bg;
    let field = list.instances.iter().find(|i| i.color == field_bg).unwrap();
    assert_eq!(field.size[0], 200.0 - 48.0 - 10.0);
}

#[test]
fn state_calls_outside_editing_are_no_ops() {
    let state = EditableLabelState::new("a");
    assert_eq!(state.confirm(), None);
    assert!(!state.cancel());
    assert!(!state.clear());
    state.set_draft("ignored");
    assert_eq!(state.draft(), None);
    assert!(state.take_events().is_empty());

    assert!(state.begin_edit());
    assert!(!state.begin_edit(), "already editing");
    state.set_draft("from the soft keyboard");
    assert_eq!(state.draft_state().cursor(), 22);
    assert_eq!(state.confirm().as_deref(), Some("from the soft keyboard"));
    state.set_text("set by the app");
    assert_eq!(state.text(), "set by the app");
    assert_eq!(
        state.take_events(),
        vec![
            EditableLabelEvent::Started,
            EditableLabelEvent::Confirmed("from the soft keyboard".into())
        ]
    );
}
