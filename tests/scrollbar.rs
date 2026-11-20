//! Scrollbar through the router: standalone over content, its bookmarks
//! from an index of sections, and on a scroll pane
//! (docs/design/20261028_08_SCROLLBAR_PLAN.md, steps 3-5).

use std::sync::Arc;
use std::sync::atomic::Ordering;

use libcontrols::{
    BookmarkText, Button, ButtonState, ControlTheme, DrawContext, MouseButton, MouseEvent, Rect,
    Router, ScrollAxis, ScrollPane, Scrollbar, ScrollbarEvent, ScrollbarOptions, ScrollbarState,
    ScrollbarVisibility, Section, SectionId, SectionedContent,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

const TRACK: Rect = Rect::new(0.0, 0.0, 400.0, 1000.0);
const LEFT: MouseButton = MouseButton::Left;
/// On the thumb at offset 0 (x 392..396, y 0..250).
const ON_THUMB: [f32; 2] = [394.0, 100.0];

fn down(router: &Router, p: [f32; 2]) {
    router.deliver_mouse(MouseEvent::Down {
        pos: p,
        button: LEFT,
    });
}

fn drag(router: &Router, from: [f32; 2], to: [f32; 2]) {
    router.deliver_mouse(MouseEvent::Drag {
        start_pos: from,
        current_pos: to,
        delta: [to[0] - from[0], to[1] - from[1]],
        button: LEFT,
    });
}

fn up(router: &Router, p: [f32; 2]) {
    router.deliver_mouse(MouseEvent::Up {
        pos: p,
        button: LEFT,
    });
    router.deliver_mouse(MouseEvent::Click {
        pos: p,
        button: LEFT,
    });
}

fn tap(router: &Router, p: [f32; 2]) {
    down(router, p);
    up(router, p);
}

fn render(router: &Router) -> DrawList {
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
    list
}

fn rounded(list: &DrawList) -> Vec<SdfInstance> {
    list.instances
        .iter()
        .filter(|i| matches!(i.kind, SdfKind::RoundedBox { .. }))
        .copied()
        .collect()
}

/// The thumbs drawn: rounded boxes 4 wide.
fn thumbs(list: &DrawList) -> Vec<SdfInstance> {
    rounded(list)
        .into_iter()
        .filter(|i| i.size[0] == 4.0)
        .collect()
}

/// The bookmark pills drawn: rounded boxes 25.55 tall.
fn pills(list: &DrawList) -> Vec<SdfInstance> {
    rounded(list)
        .into_iter()
        .filter(|i| (i.size[1] - 25.55).abs() < 1e-3)
        .collect()
}

fn clicked(b: &ButtonState) -> bool {
    b.was_clicked.swap(false, Ordering::SeqCst)
}

struct Standalone {
    router: Router,
    bar: Arc<ScrollbarState>,
    button: Arc<ButtonState>,
    /// The scrollbar node, for its `bookmark_layout`.
    node: libcontrols::NodeId,
}

fn standalone_with(
    options: ScrollbarOptions,
    sections: Option<Arc<SectionedContent>>,
) -> Standalone {
    let mut router = Router::new();
    let button = Button::new("Content", TRACK);
    let button_state = button.state.clone();
    router.add(None, button);
    let bar = match sections {
        Some(s) => ScrollbarState::with_sections(s),
        None => ScrollbarState::new(),
    };
    bar.set_metrics(0.0, 4000.0, 1000.0);
    let node = router.add(None, Scrollbar::new(TRACK, options).with_state(bar.clone()));
    Standalone {
        router,
        bar,
        button: button_state,
        node,
    }
}

fn standalone() -> Standalone {
    standalone_with(ScrollbarOptions::default(), None)
}

impl Standalone {
    fn layout(&self) -> Vec<libcontrols::BookmarkPill> {
        self.router
            .get::<Scrollbar>(self.node)
            .unwrap()
            .bookmark_layout(&DrawContext::default())
    }
}

// ---- step 3: the node ----

#[test]
fn a_tap_in_the_band_grabs_and_does_not_press_the_content() {
    let f = standalone();
    tap(&f.router, ON_THUMB);
    assert!(!clicked(&f.button));
    let events = f.bar.take_events();
    assert!(
        matches!(events[0], ScrollbarEvent::Grabbed { .. }),
        "{events:?}"
    );
    assert!(matches!(
        events.last(),
        Some(ScrollbarEvent::Released { .. })
    ));
}

#[test]
fn a_tap_left_of_the_band_presses_the_content() {
    let f = standalone();
    tap(&f.router, [387.0, 100.0]);
    assert!(clicked(&f.button));
    assert!(f.bar.take_events().is_empty());
}

#[test]
fn a_drag_in_the_band_scrubs_by_the_thumbs_travel() {
    let f = standalone();
    down(&f.router, ON_THUMB);
    // A quarter of the 750 of travel: a quarter of the 3000 range.
    drag(&f.router, ON_THUMB, [394.0, 100.0 + 187.5]);
    drag(&f.router, ON_THUMB, [394.0, 100.0 + 375.0]);
    let scrubs: Vec<f32> = f
        .bar
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            ScrollbarEvent::Scrub { offset, .. } => Some(offset),
            _ => None,
        })
        .collect();
    assert_eq!(scrubs, [750.0, 1500.0]);
    up(&f.router, [394.0, 475.0]);
    assert!(!clicked(&f.button));
}

#[test]
fn the_thumb_is_drawn_while_held_and_gone_after_the_hold_and_fade() {
    let f = standalone();
    assert!(thumbs(&render(&f.router)).is_empty(), "never scrolled");
    down(&f.router, ON_THUMB);
    let list = render(&f.router);
    let t = thumbs(&list);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].size, [4.0, 250.0]);
    assert_eq!(t[0].position, [392.0, 0.0]);
    assert!(matches!(t[0].kind, SdfKind::RoundedBox { radius } if radius == 2.0));
    let text = ControlTheme::default().text_color;
    assert!((t[0].color[3] - 0.35 * text[3]).abs() < 1e-6);
    assert_eq!(&t[0].color[..3], &text[..3]);
    up(&f.router, ON_THUMB);
    assert!(f.router.advance(1.0));
    assert!(f.router.advance(1.0));
    assert_eq!(thumbs(&render(&f.router)).len(), 1, "held 2 s");
    assert!(f.router.advance(0.15), "fading");
    assert!(!f.router.advance(0.15), "idle after 2.3 s");
    assert!(thumbs(&render(&f.router)).is_empty());
}

#[test]
fn standalone_feed_shows_the_thumb_while_scrolling() {
    let f = standalone();
    f.bar.set_scrolling(true);
    f.router.advance(5.0);
    assert_eq!(f.bar.alpha(), 0.35);
    f.bar.set_scrolling(false);
    f.router.advance(1.9);
    assert_eq!(f.bar.alpha(), 0.35);
    f.router.advance(0.5);
    assert_eq!(f.bar.alpha(), 0.0);
    // Always: shown at rest, and not busy for it.
    let a = standalone_with(
        ScrollbarOptions::default().visibility(ScrollbarVisibility::Always),
        None,
    );
    assert_eq!(thumbs(&render(&a.router)).len(), 1);
    assert!(!a.router.advance(0.1));
}

// ---- step 4: bookmarks ----

fn ten() -> Arc<SectionedContent> {
    let c = SectionedContent::new();
    c.set_sections((0..10).map(|i| Section::new(SectionId(i), 400.0)).collect());
    for i in [0, 4, 5] {
        c.add_bookmark(SectionId(i), BookmarkText::Text(format!("Section {i}")));
    }
    c
}

#[test]
fn bookmarks_show_only_while_held_at_their_share_of_the_track() {
    let sections = ten();
    let f = standalone_with(ScrollbarOptions::default(), Some(sections));
    assert!(f.layout().is_empty());
    assert!(pills(&render(&f.router)).is_empty());
    down(&f.router, ON_THUMB);
    let layout = f.layout();
    let centres: Vec<f32> = layout.iter().map(|p| p.rect.y + p.rect.h / 2.0).collect();
    assert_eq!(centres, [0.0, 400.0, 500.0]);
    for p in &layout {
        assert!((p.rect.x + p.rect.w / 2.0 - 200.0).abs() < 1e-3, "{p:?}");
        assert!((p.rect.h - 25.55).abs() < 1e-3);
    }
    let drawn = pills(&render(&f.router));
    assert_eq!(drawn.len(), 3);
    for d in &drawn {
        assert!(matches!(d.kind, SdfKind::RoundedBox { radius } if (radius - 12.775).abs() < 1e-3));
    }
}

#[test]
fn the_pill_nearest_the_thumb_is_brighter() {
    let f = standalone_with(ScrollbarOptions::default(), Some(ten()));
    down(&f.router, ON_THUMB);
    let layout = f.layout();
    let nearest: Vec<SectionId> = layout.iter().filter(|p| p.nearest).map(|p| p.id).collect();
    assert_eq!(nearest, [SectionId(0)]);
    let text = ControlTheme::default().text_color;
    let alphas: Vec<f32> = pills(&render(&f.router))
        .iter()
        .map(|p| p.color[3])
        .collect();
    assert_eq!(alphas, [0.20 * text[3], 0.10 * text[3], 0.10 * text[3]]);
    assert!(
        pills(&render(&f.router))
            .iter()
            .all(|p| p.color[..3] == text[..3])
    );
}

#[test]
fn crowded_bookmarks_give_way_to_the_nearest_and_show_again() {
    // Sections 4 and 5 are 40 apart in the content: 10 on the track.
    let c = SectionedContent::new();
    let mut list: Vec<Section> = (0..4).map(|i| Section::new(SectionId(i), 400.0)).collect();
    list.push(Section::new(SectionId(4), 40.0));
    list.push(Section::new(SectionId(5), 760.0));
    list.extend((6..10).map(|i| Section::new(SectionId(i), 400.0)));
    c.set_sections(list);
    c.add_bookmark(SectionId(4), BookmarkText::Text("four".into()));
    c.add_bookmark(SectionId(5), BookmarkText::Text("five".into()));
    let f = standalone_with(ScrollbarOptions::default(), Some(c));
    down(&f.router, ON_THUMB);
    let ids: Vec<SectionId> = f.layout().iter().map(|p| p.id).collect();
    assert_eq!(ids, [SectionId(4)], "the farther of the two waits");
    // The thumb's centre past both: 5 is nearer now.
    drag(&f.router, ON_THUMB, [394.0, 700.0]);
    let ids: Vec<SectionId> = f.layout().iter().map(|p| p.id).collect();
    assert_eq!(ids, [SectionId(5)]);
}

#[test]
fn a_long_bookmark_is_cut_to_the_width_less_72() {
    let c = ten();
    c.add_bookmark(SectionId(4), BookmarkText::Text("x".repeat(200)));
    let f = standalone_with(ScrollbarOptions::default(), Some(c));
    down(&f.router, ON_THUMB);
    let pill = f
        .layout()
        .into_iter()
        .find(|p| p.id == SectionId(4))
        .unwrap();
    assert!(pill.text.ends_with('\u{2026}'), "{}", pill.text);
    let ctx = DrawContext::default();
    assert!(ctx.text_advance(&pill.text, 13.0) <= 400.0 - 72.0);
    assert!(pill.rect.w <= 400.0 - 48.0);
}

/// Without the `markdown` feature (with it, tests/markdown.rs).
#[cfg(not(feature = "markdown"))]
#[test]
fn a_markdown_bookmark_is_drawn_as_written() {
    let c = ten();
    c.add_bookmark(SectionId(4), BookmarkText::Markdown("**Landed**".into()));
    c.add_bookmark(SectionId(5), BookmarkText::Text("**Landed**".into()));
    let f = standalone_with(ScrollbarOptions::default(), Some(c));
    down(&f.router, ON_THUMB);
    let layout = f.layout();
    let md = layout.iter().find(|p| p.id == SectionId(4)).unwrap();
    let text = layout.iter().find(|p| p.id == SectionId(5)).unwrap();
    assert_eq!(md.text, "**Landed**");
    assert_eq!(
        (md.text.as_str(), md.rect.w),
        (text.text.as_str(), text.rect.w)
    );
}

#[test]
fn bookmarks_replace_go_and_wait_for_their_section() {
    let c = ten();
    let f = standalone_with(ScrollbarOptions::default(), Some(c.clone()));
    down(&f.router, ON_THUMB);
    c.add_bookmark(SectionId(4), BookmarkText::Text("again".into()));
    let fours: Vec<_> = f
        .layout()
        .into_iter()
        .filter(|p| p.id == SectionId(4))
        .collect();
    assert_eq!(fours.len(), 1);
    assert_eq!(fours[0].text, "again");
    assert_eq!(
        c.remove_bookmark(SectionId(4)),
        Some(BookmarkText::Text("again".into()))
    );
    assert!(f.layout().iter().all(|p| p.id != SectionId(4)));
    c.add_bookmark(SectionId(10), BookmarkText::Text("later".into()));
    assert!(f.layout().iter().all(|p| p.id != SectionId(10)));
    c.push(Section::new(SectionId(10), 0.0));
    assert!(f.layout().iter().any(|p| p.id == SectionId(10)));
}

#[test]
fn scrubs_and_releases_name_the_section_and_the_nearest_bookmark() {
    let f = standalone_with(ScrollbarOptions::default(), Some(ten()));
    down(&f.router, ON_THUMB);
    // Thumb top to 400 of 750: offset 1600 (to rounding), section 4; its
    // centre at 525 is
    // nearest the bookmark of section 5 (at 500).
    drag(&f.router, ON_THUMB, [394.0, 500.0]);
    up(&f.router, [394.0, 500.0]);
    let events = f.bar.take_events();
    let want = (Some(SectionId(4)), Some(SectionId(5)));
    assert!(events.iter().any(|e| matches!(
        e,
        ScrollbarEvent::Scrub { offset, section, bookmark, .. } if (*offset - 1600.0).abs() < 0.01 && (*section, *bookmark) == want
    )), "{events:?}");
    assert!(events.iter().any(|e| matches!(
        e,
        ScrollbarEvent::Released { offset, section, bookmark, .. } if (*offset - 1600.0).abs() < 0.01 && (*section, *bookmark) == want
    )), "{events:?}");
}

#[test]
fn pills_fade_with_the_bookmarks_and_can_be_turned_off() {
    let f = standalone_with(ScrollbarOptions::default(), Some(ten()));
    down(&f.router, ON_THUMB);
    up(&f.router, ON_THUMB);
    f.router.advance(2.15);
    let alphas: Vec<f32> = pills(&render(&f.router))
        .iter()
        .map(|p| p.color[3])
        .collect();
    assert_eq!(alphas.len(), 3);
    assert!((alphas[0] - 0.10).abs() < 1e-3, "{alphas:?}");
    assert!((alphas[1] - 0.05).abs() < 1e-3, "{alphas:?}");
    f.router.advance(0.2);
    assert!(pills(&render(&f.router)).is_empty());

    let off = standalone_with(ScrollbarOptions::default().bookmarks(false), Some(ten()));
    down(&off.router, ON_THUMB);
    assert!(off.layout().is_empty());
    assert!(pills(&render(&off.router)).is_empty());
}

// ---- step 5: on a pane ----

const VIEWPORT: Rect = Rect::new(0.0, 0.0, 400.0, 600.0);

struct OnPane {
    router: Router,
    pane: libcontrols::NodeId,
    buttons: Vec<Arc<ButtonState>>,
}

fn pane_with(options: Option<ScrollbarOptions>) -> OnPane {
    let mut router = Router::new();
    let mut pane = ScrollPane::new(VIEWPORT, ScrollAxis::Vertical);
    if let Some(o) = options {
        pane = pane.with_scrollbar(o);
    }
    let pane = router.add(None, pane);
    let mut buttons = Vec::new();
    for i in 0..50 {
        let b = Button::new(
            format!("Row {i}"),
            Rect::new(0.0, i as f32 * 48.0, 400.0, 48.0),
        );
        buttons.push(b.state.clone());
        router.add(Some(pane), b);
    }
    render(&router);
    OnPane {
        router,
        pane,
        buttons,
    }
}

impl OnPane {
    fn offset(&self) -> f32 {
        self.scroll().state().offset()[1]
    }

    fn scroll(&self) -> &ScrollPane {
        self.router.get::<ScrollPane>(self.pane).unwrap()
    }

    fn bar(&self) -> &Arc<ScrollbarState> {
        self.scroll().scrollbar().unwrap()
    }

    fn any_clicked(&self) -> bool {
        self.buttons.iter().any(|b| clicked(b))
    }
}

#[test]
fn a_pane_with_a_scrollbar_draws_it_instead_of_the_indicator() {
    let f = pane_with(Some(ScrollbarOptions::default()));
    assert_eq!(f.scroll().state().content_length(), 2400.0);
    f.router.scroll_to(f.pane, [0.0, 300.0]);
    let list = render(&f.router);
    let t = thumbs(&list);
    assert_eq!(t.len(), 1, "only the scrollbar's thumb: {t:?}");
    // The thumb: 600 / 2400 of 600, 150 long, 392..396; not the indicator's
    // x of 394.
    assert_eq!(t[0].position[0], 392.0);
    assert_eq!(t[0].size, [4.0, 150.0]);
}

#[test]
fn a_press_in_the_band_scrubs_the_pane_and_presses_no_button() {
    let f = pane_with(Some(ScrollbarOptions::default()));
    let at = [394.0, 75.0];
    down(&f.router, at);
    assert!(f.bar().is_held());
    // Half the 450 of travel: half the 1800 range.
    drag(&f.router, at, [394.0, 75.0 + 225.0]);
    assert_eq!(f.offset(), 900.0);
    up(&f.router, [394.0, 300.0]);
    assert!(!f.any_clicked());
    // No fling after the scrub.
    f.router.advance(0.5);
    assert_eq!(f.offset(), 900.0);
    assert!(!f.scroll().state().is_moving());
    // Off the thumb: it jumps, centred under the press.
    down(&f.router, [390.0, 590.0]);
    assert_eq!(f.offset(), 1800.0);
    up(&f.router, [390.0, 590.0]);
    assert!(!f.any_clicked());
}

#[test]
fn a_finger_drag_elsewhere_scrolls_and_shows_the_thumb_for_the_hold() {
    let f = pane_with(Some(ScrollbarOptions::default()));
    assert!(thumbs(&render(&f.router)).is_empty());
    down(&f.router, [100.0, 300.0]);
    // Past the touch slop the pane takes the press, then follows it.
    drag(&f.router, [100.0, 300.0], [100.0, 290.0]);
    drag(&f.router, [100.0, 300.0], [100.0, 190.0]);
    assert_eq!(f.offset(), 100.0);
    // Rest before lifting: no fling.
    f.router.advance(0.2);
    up(&f.router, [100.0, 190.0]);
    assert!(!f.any_clicked());
    assert_eq!(f.bar().alpha(), 0.35);
    f.router.advance(1.7);
    assert_eq!(
        thumbs(&render(&f.router)).len(),
        1,
        "within 2 s of the stop"
    );
    // 2.4 s after the stop: past the hold and the 0.3 s fade.
    f.router.advance(0.7);
    assert!(thumbs(&render(&f.router)).is_empty());
}

#[test]
fn a_fling_keeps_the_thumb_shown_while_it_runs() {
    let f = pane_with(Some(ScrollbarOptions::default()));
    down(&f.router, [100.0, 500.0]);
    let mut y = 500.0;
    for _ in 0..5 {
        drag(&f.router, [100.0, 500.0], [100.0, y - 40.0]);
        y -= 40.0;
        f.router.advance(0.016);
    }
    up(&f.router, [100.0, y]);
    assert!(f.scroll().state().is_moving(), "a fling");
    let mut steps = 0;
    while f.scroll().state().is_moving() {
        assert_eq!(f.bar().alpha(), 0.35);
        assert!(f.router.advance(0.05));
        steps += 1;
        assert!(steps < 1000);
    }
    assert!(steps > 2);
}

/// FNV-1a, for a recorded draw list.
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[test]
fn a_pane_without_a_scrollbar_draws_exactly_as_at_0_10_3() {
    // The same scenario recorded at e7a345c (0.10.3, before this plan).
    let mut router = Router::new();
    let pane = router.add(None, ScrollPane::new(VIEWPORT, ScrollAxis::Vertical));
    for i in 0..50 {
        router.add(
            Some(pane),
            Button::new(
                format!("Row {i}"),
                Rect::new(0.0, i as f32 * 48.0, 400.0, 48.0),
            ),
        );
    }
    let first = format!("{:?}", render(&router));
    down(&router, [100.0, 300.0]);
    drag(&router, [100.0, 300.0], [100.0, 200.0]);
    router.advance(0.016);
    router.deliver_mouse(MouseEvent::Up {
        pos: [100.0, 200.0],
        button: LEFT,
    });
    router.advance(0.2);
    let second = format!("{:?}", render(&router));
    assert_eq!(
        (first.len(), fnv(&first)),
        (7807, 7_688_348_455_939_836_137)
    );
    assert_eq!(
        (second.len(), fnv(&second)),
        (8669, 18_132_709_946_540_164_852)
    );
}

#[test]
fn with_none_the_bands_press_reaches_the_button_beneath() {
    let f = pane_with(Some(
        ScrollbarOptions::default().visibility(ScrollbarVisibility::None),
    ));
    tap(&f.router, [394.0, 100.0]);
    assert!(clicked(&f.buttons[2]));
    assert!(thumbs(&render(&f.router)).is_empty());
}

#[test]
fn a_panes_scrollbar_draws_bookmarks_from_an_index() {
    let f = pane_with(Some(ScrollbarOptions::default()));
    let c = SectionedContent::new();
    c.set_sections((0..5).map(|i| Section::new(SectionId(i), 480.0)).collect());
    c.add_bookmark(SectionId(2), BookmarkText::Text("Middle".into()));
    f.bar().set_sections(Some(c));
    down(&f.router, [394.0, 75.0]);
    let p = pills(&render(&f.router));
    assert_eq!(p.len(), 1);
    // At 960 / 2400 of the 600 track: 240.
    assert!((p[0].position[1] + p[0].size[1] / 2.0 - 240.0).abs() < 1e-3);
    up(&f.router, [394.0, 75.0]);
}

#[test]
fn a_horizontal_pane_keeps_its_indicator() {
    let pane = ScrollPane::new(VIEWPORT, ScrollAxis::Horizontal)
        .with_scrollbar(ScrollbarOptions::default());
    assert!(pane.scrollbar().is_none());
}

// ---- Plan 20261119_01, step 6: the thumb label and bookmark sizes ----

use libcontrols::{
    Bookmark, BookmarkScale, BookmarkSize, Boundaries, FlingOptions, FlingStop, LabelPurpose,
    LabelRequest, SkipMapping,
};

struct Labelled {
    router: Router,
    pane: libcontrols::NodeId,
    content: Arc<SectionedContent>,
}

impl Labelled {
    fn new(options: ScrollbarOptions, content: Arc<SectionedContent>, fling: FlingOptions) -> Self {
        let mut router = Router::new();
        let pane = router.add(
            None,
            ScrollPane::new(TRACK, ScrollAxis::Vertical)
                .with_scrollbar(options.thumb_label(true))
                .with_sections(content.clone())
                .with_fling(fling),
        );
        render(&router);
        Self {
            router,
            pane,
            content,
        }
    }

    fn bar(&self) -> &Arc<ScrollbarState> {
        self.router
            .get::<ScrollPane>(self.pane)
            .unwrap()
            .scrollbar()
            .unwrap()
    }

    fn label(&self) -> Option<String> {
        self.bar().thumb_label().map(|(_, t)| match t {
            BookmarkText::Text(s) | BookmarkText::Markdown(s) => s,
            _ => String::new(),
        })
    }

    fn at(&self, offset: f32) -> Option<String> {
        self.router.scroll_to(self.pane, [0.0, offset]);
        render(&self.router);
        self.label()
    }

    fn offset(&self) -> f32 {
        self.router
            .get::<ScrollPane>(self.pane)
            .unwrap()
            .state()
            .offset()[1]
    }
}

fn labelled(options: ScrollbarOptions) -> Labelled {
    Labelled::new(options, ten(), FlingOptions::default())
}

#[test]
fn the_label_is_the_last_bookmark_at_or_above_the_windows_top() {
    let l = labelled(ScrollbarOptions::default());
    assert_eq!(l.label(), None, "not scrolled yet");
    assert_eq!(l.at(1599.0).as_deref(), Some("Section 0"));
    assert_eq!(
        l.at(1600.0).as_deref(),
        Some("Section 4"),
        "at the boundary"
    );
    assert_eq!(l.at(1601.0).as_deref(), Some("Section 4"));
    assert_eq!(l.at(1999.0).as_deref(), Some("Section 4"));
    assert_eq!(l.at(2000.0).as_deref(), Some("Section 5"));
    assert_eq!(l.at(1500.0).as_deref(), Some("Section 0"), "and back up");
    // Above the first bookmarked section there is none.
    let c = ten();
    c.remove_bookmark(SectionId(0));
    let l = Labelled::new(ScrollbarOptions::default(), c, FlingOptions::default());
    assert_eq!(l.at(100.0), None);
    assert_eq!(l.at(1700.0).as_deref(), Some("Section 4"));
}

#[test]
fn the_label_shows_while_scrolling_holds_two_seconds_and_fades() {
    let l = labelled(ScrollbarOptions::default());
    l.at(1700.0);
    assert_eq!(l.bar().thumb_label_alpha(), 1.0);
    l.router.advance(1.9);
    assert_eq!(l.bar().thumb_label_alpha(), 1.0);
    l.router.advance(0.25);
    let a = l.bar().thumb_label_alpha();
    assert!((a - 0.5).abs() < 0.02, "{a}");
    l.router.advance(0.2);
    assert_eq!(l.bar().thumb_label_alpha(), 0.0);
    assert_eq!(l.label(), None);
}

#[test]
fn the_label_is_not_drawn_while_the_thumb_is_held() {
    let l = labelled(ScrollbarOptions::default());
    l.at(0.0);
    l.router.scroll_to(l.pane, [0.0, 10.0]);
    render(&l.router);
    assert!(l.label().is_some());
    down(&l.router, [394.0, 20.0]);
    assert!(l.bar().is_held());
    assert_eq!(l.label(), None);
    let list = render(&l.router);
    assert_eq!(pills(&list).len(), 3, "the bookmarks as before");
    l.router.deliver_mouse(MouseEvent::Up {
        pos: [394.0, 20.0],
        button: LEFT,
    });
    assert!(l.label().is_some(), "shown again after release");
}

#[test]
fn under_always_the_label_shows_only_while_scrolling_and_hold_after() {
    let l = labelled(ScrollbarOptions::default().visibility(ScrollbarVisibility::Always));
    assert_eq!(l.label(), None, "at rest");
    l.at(1700.0);
    assert!(l.label().is_some());
    l.router.advance(2.5);
    assert_eq!(l.label(), None);
    assert_eq!(thumbs(&render(&l.router)).len(), 1, "the thumb stays");
}

#[test]
fn a_formatter_names_the_top_and_the_target() {
    let c = ten();
    c.set_label_format(Some(Arc::new(|r: &LabelRequest| {
        let p = match r.purpose {
            LabelPurpose::Top => "top",
            LabelPurpose::Target => "target",
            _ => "?",
        };
        BookmarkText::Text(format!("{p} {} at {}", r.section.0, r.index))
    })));
    let l = Labelled::new(
        ScrollbarOptions::default().skip_count(true),
        c,
        FlingOptions::new().stop(FlingStop::count(
            Boundaries::Sections,
            SkipMapping::default(),
        )),
    );
    assert_eq!(l.at(2450.0).as_deref(), Some("top 6 at 6"));
    // A counted flick from 6: 3000 px/s passes 4; the label names the
    // target from release to landing, with the boundaries still to pass.
    let r = &l.router;
    r.deliver_mouse(MouseEvent::Down {
        pos: [100.0, 500.0],
        button: LEFT,
    });
    let mut y = 500.0;
    for _ in 0..2 {
        r.advance(0.016);
        y += 48.0;
        drag(r, [100.0, 500.0], [100.0, y]);
    }
    r.deliver_mouse(MouseEvent::Up {
        pos: [100.0, y],
        button: LEFT,
    });
    render(r);
    let first = l.label().expect("a target label");
    assert!(first.starts_with("target "), "{first}");
    let target: u64 = first.split(' ').nth(1).unwrap().parse().unwrap();
    let mut seen = vec![first.clone()];
    while l
        .router
        .get::<ScrollPane>(l.pane)
        .unwrap()
        .state()
        .flight()
        .is_some()
    {
        r.advance(1.0 / 60.0);
        render(r);
        let now = l.label().unwrap();
        if seen.last() != Some(&now) {
            seen.push(now);
        }
    }
    // The frame it lands on names the top section again.
    assert_eq!(seen.pop(), Some(format!("top {target} at {target}")));
    let name = format!("target {target} at {target}");
    let arrows: Vec<&str> = seen.iter().map(|s| &s[name.len()..]).collect();
    assert_eq!(
        arrows,
        [
            " \u{2191} 4",
            " \u{2191} 3",
            " \u{2191} 2",
            " \u{2191} 1",
            ""
        ],
        "counting down"
    );
    // Landed: the section at the top again, held then faded.
    assert_eq!(l.label(), Some(format!("top {target} at {target}")));
    assert_eq!(l.offset(), l.content.offset_of(SectionId(target)).unwrap());
}

/// A standalone bar over `marks` (section, top, size) in 10 000 of content.
fn sized(marks: &[(u64, f32, BookmarkSize)]) -> Standalone {
    let c = SectionedContent::new();
    let mut tops: Vec<f32> = marks.iter().map(|m| m.1).collect();
    tops.sort_by(f32::total_cmp);
    let mut list = Vec::new();
    let mut at = 0.0;
    for (i, &(id, top, _)) in marks.iter().enumerate() {
        let _ = i;
        if top > at {
            list.push(Section::new(SectionId(10_000 + id), top - at));
        }
        let next = tops.iter().find(|&&t| t > top).copied().unwrap_or(10_000.0);
        list.push(Section::new(SectionId(id), next - top));
        at = next;
    }
    c.set_sections(list);
    for &(id, _, size) in marks {
        c.add_bookmark_with(
            SectionId(id),
            Bookmark::new(BookmarkText::Text(format!("b{id}"))).size(size),
        );
    }
    let f = standalone_with(ScrollbarOptions::default(), Some(c));
    f.bar.set_metrics(0.0, 10_000.0, 1000.0);
    down(&f.router, [394.0, 50.0]);
    f
}

#[test]
fn bookmark_sizes_draw_at_their_scale_with_padding_to_match() {
    let small = BookmarkSize::Themed(BookmarkScale::Small);
    let large = BookmarkSize::Themed(BookmarkScale::Large);
    let medium = BookmarkSize::default();
    let f = sized(&[
        (0, 1000.0, small),
        (1, 3000.0, medium),
        (2, 5000.0, large),
        (3, 7000.0, BookmarkSize::Px(20.0)),
    ]);
    let layout = f.layout();
    assert_eq!(layout.len(), 4);
    for (p, fs) in layout.iter().zip([13.0 * 0.85, 13.0, 13.0 * 1.3, 20.0]) {
        assert!((p.font_size - fs).abs() < 1e-4, "{p:?}");
        let k = fs / 13.0;
        assert!((p.rect.h - (fs * 1.35 + 8.0 * k)).abs() < 1e-3, "{p:?}");
        let text = DrawContext::default().text_advance(&p.text, fs);
        assert!((p.rect.w - (text + 24.0 * k)).abs() < 1e-3, "{p:?}");
    }
    // Medium is 0.11.0's pill exactly.
    assert!((layout[1].rect.h - 25.55).abs() < 1e-4);
    assert_eq!(
        Bookmark::new(BookmarkText::Text("x".into())).size,
        BookmarkSize::Themed(BookmarkScale::Medium)
    );
}

#[test]
fn crowded_pills_give_way_smaller_first_then_farther_first() {
    let small = BookmarkSize::Themed(BookmarkScale::Small);
    let large = BookmarkSize::Themed(BookmarkScale::Large);
    // 2000 and 2100 are 10 px apart on the track: one survives.
    let f = sized(&[(0, 2000.0, small), (1, 2100.0, large)]);
    let ids: Vec<SectionId> = f.layout().iter().map(|p| p.id).collect();
    assert_eq!(ids, [SectionId(1)], "the larger, though farther");
    let f = sized(&[(0, 2000.0, large), (1, 2100.0, small)]);
    let ids: Vec<SectionId> = f.layout().iter().map(|p| p.id).collect();
    assert_eq!(ids, [SectionId(0)]);
    // Equal sizes: the nearest the thumb (centre near 50 on the track).
    let f = sized(&[(0, 2000.0, large), (1, 2100.0, large)]);
    let layout = f.layout();
    assert_eq!(
        layout.iter().map(|p| p.id).collect::<Vec<_>>(),
        [SectionId(0)]
    );
    assert!(layout[0].nearest);
}

// ---- Plan 20261119_01, step 7: snapping while scrubbing ----

/// The scrubs' offsets and bookmarks, and the snaps, in order.
fn scrub_events(bar: &ScrollbarState) -> Vec<(char, f32, Option<SectionId>)> {
    bar.take_events()
        .into_iter()
        .filter_map(|e| match e {
            ScrollbarEvent::Scrub {
                offset, bookmark, ..
            } => Some(('s', offset, bookmark)),
            ScrollbarEvent::Snapped { id, offset, .. } => Some(('n', offset, Some(id))),
            ScrollbarEvent::Released {
                offset, bookmark, ..
            } => Some(('r', offset, bookmark)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_scrub_near_a_bookmark_snaps_to_its_top_once_per_snap() {
    // Bookmarks at tops 0, 1600 and 2000 of 4000: pills at 0, 400, 500 on
    // the track. The thumb is 250 long, grabbed 100 below its top.
    let f = standalone_with(
        ScrollbarOptions::default().snap_to_bookmarks(true),
        Some(ten()),
    );
    down(&f.router, ON_THUMB);
    f.bar.take_events();
    let at_centre = |c: f32| [394.0, c - 125.0 + 100.0];
    drag(&f.router, ON_THUMB, at_centre(410.0));
    assert_eq!(
        scrub_events(&f.bar),
        [
            ('s', 1600.0, Some(SectionId(4))),
            ('n', 1600.0, Some(SectionId(4)))
        ]
    );
    drag(&f.router, ON_THUMB, at_centre(395.0));
    assert!(scrub_events(&f.bar).is_empty(), "still snapped: no event");
    // 50 from either pill: no snap, the offset the thumb's share.
    drag(&f.router, ON_THUMB, at_centre(450.0));
    let e = scrub_events(&f.bar);
    assert_eq!(e.len(), 1);
    assert!((e[0].1 - 325.0 / 750.0 * 3000.0).abs() < 1e-2, "{e:?}");
    // Back in: a new snap, sent again.
    drag(&f.router, ON_THUMB, at_centre(500.0 + 12.0));
    assert_eq!(
        scrub_events(&f.bar),
        [
            ('s', 2000.0, Some(SectionId(5))),
            ('n', 2000.0, Some(SectionId(5)))
        ]
    );
    f.router.deliver_mouse(MouseEvent::Up {
        pos: at_centre(512.0),
        button: LEFT,
    });
    assert_eq!(scrub_events(&f.bar), [('r', 2000.0, Some(SectionId(5)))]);
}

#[test]
fn without_snap_to_bookmarks_a_scrub_is_as_in_0_11_0() {
    let f = standalone_with(ScrollbarOptions::default(), Some(ten()));
    down(&f.router, ON_THUMB);
    f.bar.take_events();
    drag(&f.router, ON_THUMB, [394.0, 410.0 - 125.0 + 100.0]);
    let e = scrub_events(&f.bar);
    assert_eq!(e.len(), 1, "{e:?}");
    assert!((e[0].1 - 285.0 / 750.0 * 3000.0).abs() < 1e-2, "{e:?}");
    assert_eq!(
        e[0].2,
        Some(SectionId(4)),
        "the nearest bookmark, as before"
    );
    assert_eq!(libcontrols::ScrollbarTheme::default().snap_distance, 16.0);
}
