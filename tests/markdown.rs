//! MarkdownView and Markdown bookmarks through the router
//! (docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md, step 4): a `Router` with
//! a view node, a `DrawContext` without a shaper (so runs measure at 0.55 a
//! character and draw no glyphs) and counters over the `DrawList`.

#![cfg(feature = "markdown")]

use libcontrols::{
    BookmarkText, ControlAction, ControlPart, ControlTheme, DrawContext, MarkdownOptions,
    MarkdownView, MouseButton, MouseEvent, NodeId, Rect, Router, Scrollbar, ScrollbarOptions,
    ScrollbarState, Section, SectionId, SectionedContent,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

const LEFT: MouseButton = MouseButton::Left;
/// 14 px type (the default theme), 1.35 lines.
const LH: f32 = 14.0 * 1.35;
const FIXTURE: &str = "Some **bold**, *italic*, `code` and https://example.com.";

fn tap(router: &Router, p: [f32; 2]) -> Option<ControlAction> {
    router.deliver_mouse(MouseEvent::Down {
        pos: p,
        button: LEFT,
    });
    router.deliver_mouse(MouseEvent::Up {
        pos: p,
        button: LEFT,
    });
    router
        .deliver_mouse(MouseEvent::Click {
            pos: p,
            button: LEFT,
        })
        .action
}

fn render(router: &Router) -> DrawList {
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
    list
}

/// The underlines: plain boxes.
fn underlines(list: &DrawList) -> Vec<SdfInstance> {
    list.instances
        .iter()
        .filter(|i| matches!(i.kind, SdfKind::Box))
        .copied()
        .collect()
}

fn view_router(md: &str, options: MarkdownOptions) -> (Router, NodeId) {
    let mut router = Router::new();
    let node = router.add(None, MarkdownView::new(md, [10.0, 20.0], 1000.0, options));
    (router, node)
}

fn view(router: &Router, node: NodeId) -> &MarkdownView {
    router.get::<MarkdownView>(node).unwrap()
}

#[test]
fn a_views_text_and_height() {
    let (router, node) = view_router(FIXTURE, MarkdownOptions::default());
    let v = view(&router, node);
    assert_eq!(v.text(), "Some bold, italic, code and https://example.com.");
    assert_eq!(v.links(), ["https://example.com"]);
    assert_eq!(v.px(), 14.0);
    assert!((v.height() - LH).abs() < 1e-4);
    assert_eq!(v.source(), FIXTURE);
    let (router, node) = view_router("one\n\ntwo", MarkdownOptions::default());
    let v = view(&router, node);
    assert_eq!(v.text(), "one\ntwo");
    assert!(
        (v.height() - 2.5 * LH).abs() < 1e-4,
        "two lines and a block gap"
    );
}

#[test]
fn a_click_on_a_link_answers_its_index_and_elsewhere_nothing() {
    let (router, node) = view_router(FIXTURE, MarkdownOptions::default());
    let r = view(&router, node)
        .link_rect(0)
        .expect("the link is placed");
    let on = [r.x + r.w * 0.5, r.y + r.h * 0.5];
    assert_eq!(
        tap(&router, on),
        Some(ControlAction::Activated {
            part: ControlPart::Custom(0)
        })
    );
    assert_eq!(tap(&router, [12.0, on[1]]), None, "not on the link");
    // The tap area reaches 15% of a line above it.
    assert_eq!(
        view(&router, node).link_at([on[0], r.y - 0.1 * LH]),
        Some(0)
    );
    assert_eq!(view(&router, node).link_at([on[0], r.y - 0.2 * LH]), None);
}

#[test]
fn links_are_drawn_in_the_accent_colour_with_one_underline_each() {
    let md = "See https://a.example and https://b.example now.";
    let (router, node) = view_router(md, MarkdownOptions::default());
    let lines = underlines(&render(&router));
    assert_eq!(lines.len(), 2);
    let accent = ControlTheme::default().accent_color;
    assert!(lines.iter().all(|i| i.color == accent));
    let r = view(&router, node).link_rect(1).unwrap();
    let thick = (14.0_f32 * 0.07).max(1.0);
    let under = lines[1];
    assert!((under.position[0] - r.x).abs() < 1e-3);
    assert!((under.position[1] - (r.y + LH * 0.92 - thick)).abs() < 1e-3);
    assert_eq!(under.size, [r.w, thick]);
}

#[test]
fn overriding_link_color_reaches_the_instances() {
    let red = [1.0, 0.0, 0.0, 1.0];
    let (router, _) = view_router(FIXTURE, MarkdownOptions::new().link_color(red));
    let lines = underlines(&render(&router));
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].color, red);
    // A theme's link colour reaches a view that does not override it.
    let (router, _) = view_router(FIXTURE, MarkdownOptions::default());
    let mut theme = ControlTheme::default();
    theme.markdown.link_color = Some(red);
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default().with_theme(theme));
    assert_eq!(underlines(&list)[0].color, red);
}

#[test]
fn centered_centres_each_line() {
    let (router, node) = view_router(
        "https://a.example\n\nhttps://longer.example/path",
        MarkdownOptions::new().centered(true),
    );
    let v = view(&router, node);
    for i in 0..2 {
        let r = v.link_rect(i).unwrap();
        assert!((r.x + r.w / 2.0 - (10.0 + 500.0)).abs() < 1e-3, "{r:?}");
    }
    let (router, node) = view_router("https://a.example", MarkdownOptions::default());
    assert_eq!(view(&router, node).link_rect(0).unwrap().x, 10.0);
}

#[test]
fn move_to_moves_every_run() {
    let (mut router, node) = view_router(FIXTURE, MarkdownOptions::default());
    let before = underlines(&render(&router));
    let rect = view(&router, node).rect();
    router
        .get_mut::<MarkdownView>(node)
        .unwrap()
        .move_to(110.0, 70.0);
    let after = underlines(&render(&router));
    assert_eq!(
        view(&router, node).rect(),
        Rect::new(110.0, 70.0, rect.w, rect.h)
    );
    assert_eq!(before.len(), after.len());
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(a.position, [b.position[0] + 100.0, b.position[1] + 50.0]);
    }
}

#[test]
fn set_text_and_set_width_lay_out_again() {
    let (mut router, node) = view_router("short", MarkdownOptions::default());
    let v = router.get_mut::<MarkdownView>(node).unwrap();
    v.set_text("one two three four five six seven");
    assert_eq!(v.text().lines().count(), 1);
    v.set_width(100.0);
    assert!(v.text().lines().count() > 1);
    assert!((v.height() - v.text().lines().count() as f32 * LH).abs() < 1e-3);
}

#[cfg(feature = "cmark")]
#[test]
fn with_cmark_blocks_are_formatted() {
    let (router, node) = view_router(
        "# Title\n\n- one\n- two\n\n> quoted\n\n[docs](https://a.example)",
        MarkdownOptions::default(),
    );
    let v = view(&router, node);
    assert_eq!(v.text(), "Title\n\u{2022} one\n\u{2022} two\nquoted\ndocs");
    assert_eq!(v.links(), ["https://a.example"]);
    assert_eq!(underlines(&render(&router)).len(), 1);
}

#[cfg(not(feature = "cmark"))]
#[test]
fn without_cmark_blocks_are_literal() {
    let (router, node) = view_router("# Title\n\n- one", MarkdownOptions::default());
    assert_eq!(view(&router, node).text(), "# Title\n- one");
}

// ---- Markdown bookmarks ----

const TRACK: Rect = Rect::new(0.0, 0.0, 400.0, 1000.0);

fn bar_with(marks: &[(u64, BookmarkText)]) -> (Router, NodeId) {
    let c = SectionedContent::new();
    c.set_sections((0..10).map(|i| Section::new(SectionId(i), 400.0)).collect());
    for (id, text) in marks {
        c.add_bookmark(SectionId(*id), text.clone());
    }
    let state = ScrollbarState::with_sections(c);
    state.set_metrics(0.0, 4000.0, 1000.0);
    let mut router = Router::new();
    let node = router.add(
        None,
        Scrollbar::new(TRACK, ScrollbarOptions::default()).with_state(state.clone()),
    );
    // Hold the thumb, so the bookmarks show.
    state.press([394.0, 100.0]);
    (router, node)
}

#[test]
fn a_markdown_bookmark_draws_without_its_markers() {
    let (router, node) = bar_with(&[
        (4, BookmarkText::Markdown("**Landed**".into())),
        (5, BookmarkText::Text("**Landed**".into())),
    ]);
    let pills = router
        .get::<Scrollbar>(node)
        .unwrap()
        .bookmark_layout(&DrawContext::default());
    let md = pills.iter().find(|p| p.id == SectionId(4)).unwrap();
    let text = pills.iter().find(|p| p.id == SectionId(5)).unwrap();
    assert_eq!(md.text, "Landed");
    assert_eq!(text.text, "**Landed**");
    let ctx = DrawContext::default();
    assert!((md.rect.w - (ctx.text_advance("Landed", 13.0) + 24.0)).abs() < 1e-3);
    assert!(md.rect.w < text.rect.w);
}

#[test]
fn a_markdown_bookmarks_code_is_drawn_over_a_chip() {
    let (router, node) = bar_with(&[(4, BookmarkText::Markdown("run `cargo test`".into()))]);
    let pills = router
        .get::<Scrollbar>(node)
        .unwrap()
        .bookmark_layout(&DrawContext::default());
    let pill = &pills[0];
    assert_eq!(pill.text, "run cargo test");
    let ctx = DrawContext::default();
    // The code span is padded 2 px either side.
    let w = ctx.text_advance("run ", 13.0) + ctx.text_advance("cargo test", 13.0) + 4.0;
    assert!((pill.rect.w - (w + 24.0)).abs() < 1e-3);
    let list = render(&router);
    let chips: Vec<&SdfInstance> = list
        .instances
        .iter()
        .filter(|i| matches!(i.kind, SdfKind::RoundedBox { radius } if radius == 3.0))
        .collect();
    assert_eq!(chips.len(), 1);
    let chip = chips[0];
    let x = pill.rect.x + 12.0 + ctx.text_advance("run ", 13.0);
    assert!((chip.position[0] - x).abs() < 1e-3);
    assert!((chip.size[0] - (ctx.text_advance("cargo test", 13.0) + 4.0)).abs() < 1e-3);
    // At the pill's own alpha, drawn again over it.
    let pill_box = list
        .instances
        .iter()
        .find(|i| matches!(i.kind, SdfKind::RoundedBox { radius } if (radius - pill.rect.h / 2.0).abs() < 1e-3))
        .unwrap();
    assert_eq!(chip.color, pill_box.color);
}

#[test]
fn a_long_markdown_bookmark_is_cut_counting_its_visible_text() {
    let long = format!("**{}**", "x".repeat(200));
    let (router, node) = bar_with(&[(4, BookmarkText::Markdown(long))]);
    let pill = router
        .get::<Scrollbar>(node)
        .unwrap()
        .bookmark_layout(&DrawContext::default())
        .remove(0);
    assert!(pill.text.ends_with('\u{2026}'), "{}", pill.text);
    assert!(!pill.text.contains('*'));
    let ctx = DrawContext::default();
    assert!(ctx.text_advance(&pill.text, 13.0) <= 400.0 - 72.0);
}
