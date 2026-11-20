//! CopyLabel through the router and a modal dialog: the glyph's copy
//! request, the inert text, the drawn glyph and the cut text
//! (docs/design/20261120_01_COPY_LABEL_PLAN.md).

use std::sync::Arc;

use libcontrols::{
    ControlAction, ControlTheme, CopyLabel, CopyLabelState, DrawContext, InputEvent, Key,
    KeyboardEvent, ModalDialog, ModalStack, ModalState, ModalTransition, MouseButton, MouseEvent,
    NodeFlags, NodeId, ReadValue, Rect, Router, TextAlign,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

// The control is at (10, 10) 200 x 32. With the default 32 px button the
// glyph is in x 178..210; the text area is x 10..174 (less the 4 px gap).
const RECT: Rect = Rect {
    x: 10.0,
    y: 10.0,
    w: 200.0,
    h: 32.0,
};
const GLYPH: [f32; 2] = [194.0, 26.0];
const TEXT: [f32; 2] = [40.0, 26.0];

fn tap(router: &Router, p: [f32; 2]) -> Option<ControlAction> {
    let button = MouseButton::Left;
    router.deliver_mouse(MouseEvent::Down { pos: p, button });
    router.deliver_mouse(MouseEvent::Up { pos: p, button });
    router
        .deliver_mouse(MouseEvent::Click { pos: p, button })
        .action
}

fn render(router: &Router) -> DrawList {
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
    list
}

fn lines(list: &DrawList) -> Vec<&SdfInstance> {
    list.instances
        .iter()
        .filter(|i| matches!(i.kind, SdfKind::Line { .. }))
        .collect()
}

/// A router holding `label`, drawn once so hit testing has its layout.
fn setup(label: CopyLabel) -> (Router, NodeId, Arc<CopyLabelState>) {
    let state = label.state.clone();
    let mut router = Router::new();
    let id = router.add(None, label);
    render(&router);
    (router, id, state)
}

#[test]
fn a_click_on_the_glyph_asks_for_a_copy_once() {
    let (router, id, state) = setup(CopyLabel::new("IMG_0042.jpg", RECT));
    assert!(!state.take_copy(), "nothing asked yet");
    let action = tap(&router, GLYPH);
    assert_eq!(
        action,
        Some(ControlAction::Activated {
            part: CopyLabel::COPY
        })
    );
    assert!(state.take_copy(), "the click asked for a copy");
    assert!(!state.take_copy(), "seen once");
    assert_eq!(state.text(), "IMG_0042.jpg");
    let label = router.get::<CopyLabel>(id).unwrap();
    assert_eq!(label.read_value(), "IMG_0042.jpg");
}

#[test]
fn a_click_on_the_text_does_nothing() {
    let (router, _, state) = setup(CopyLabel::new("IMG_0042.jpg", RECT));
    assert_eq!(tap(&router, TEXT), None);
    assert!(!state.take_copy());
}

#[test]
fn enter_or_space_on_the_focused_label_asks_for_a_copy() {
    let (router, id, state) = setup(CopyLabel::new("IMG_0042.jpg", RECT));
    assert!(router.flags(id).contains(NodeFlags::FOCUSABLE));
    router.set_focus(Some(id));
    for key in [Key::Enter, Key::Space] {
        let action = router
            .deliver_keyboard(KeyboardEvent::KeyDown { key })
            .action;
        assert_eq!(
            action,
            Some(ControlAction::Activated {
                part: CopyLabel::COPY
            })
        );
        assert!(state.take_copy());
        assert!(!state.take_copy());
    }
}

#[test]
fn it_draws_the_glyph_as_eight_segments_in_the_button() {
    let (router, _, _) = setup(CopyLabel::new("IMG_0042.jpg", RECT));
    let list = render(&router);
    let segs = lines(&list);
    assert_eq!(segs.len(), 8, "two overlapping squares, eight segments");
    let muted = ControlTheme::default().muted_text_color;
    assert!(segs.iter().all(|l| l.color == muted), "in the muted colour");
    for l in &segs {
        let SdfKind::Line { end, .. } = l.kind else {
            unreachable!()
        };
        for p in [l.position, end] {
            assert!(p[0] >= 178.0 && p[0] <= 210.0, "in the button: {p:?}");
            assert!(p[1] >= 10.0 && p[1] <= 42.0, "in the button: {p:?}");
        }
    }
}

#[test]
fn the_glyph_colour_and_alignment_can_be_set() {
    let red = [1.0, 0.0, 0.0, 1.0];
    let label = CopyLabel::new("a", RECT)
        .with_glyph_color(red)
        .with_text_color([0.0, 1.0, 0.0, 1.0])
        .with_font_size(20.0)
        .with_align(TextAlign::Right);
    assert_eq!(label.align, TextAlign::Right);
    assert_eq!(label.font_size, Some(20.0));
    let (router, _, _) = setup(label);
    let list = render(&router);
    assert!(lines(&list).iter().all(|l| l.color == red));
}

#[test]
fn a_long_text_is_cut_with_an_ellipsis() {
    let ctx = DrawContext::default();
    let short = CopyLabel::new("IMG_0042.jpg", RECT);
    assert_eq!(short.shown_text(&ctx), "IMG_0042.jpg");
    let long = CopyLabel::new("a_very_long_file_name_from_a_camera_roll_0042.jpg", RECT);
    let shown = long.shown_text(&ctx);
    assert!(shown.ends_with('\u{2026}'), "cut: {shown}");
    assert!(shown.chars().count() < 49);
    // The text area less its padding: 164 - 8.
    let size = ctx.theme.default_font_size;
    assert!(ctx.text_advance(&shown, size) <= 156.0);
}

#[test]
fn a_shared_state_updates_the_text() {
    let state = Arc::new(CopyLabelState::new("one"));
    let label = CopyLabel::new("ignored", RECT).with_state(state.clone());
    state.set_text("two");
    assert_eq!(label.text(), "two");
}

#[test]
fn it_works_in_a_modal_dialogs_content() {
    let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
    let frame = Rect::new(200.0, 150.0, 400.0, 300.0);
    let content = Rect::new(220.0, 170.0, 360.0, 260.0);
    let modal_state = Arc::new(ModalState::new().with_transition(ModalTransition::Instant));
    let mut router = Router::new();
    ModalStack::install(&mut router, Arc::clone(&modal_state), viewport).unwrap();

    let label = CopyLabel::new("IMG_0042.jpg", Rect::new(240.0, 200.0, 300.0, 32.0));
    let state = label.state.clone();
    let mut dialog = ModalDialog::new(viewport, frame, content);
    dialog.content_mut().add(None, label);
    router.push_modal("details", dialog).unwrap();
    render(&router);

    let click = |x: f32| {
        router.deliver_event(InputEvent::Mouse(MouseEvent::Click {
            pos: [x, 216.0],
            button: MouseButton::Left,
        }))
    };
    // The glyph is in x 508..540; the text from 240.
    click(300.0);
    assert!(!state.take_copy(), "the text does nothing");
    let resp = click(524.0);
    assert!(matches!(resp.action, Some(ControlAction::Activated { .. })));
    assert!(state.take_copy(), "the glyph inside the dialog asked");
}
