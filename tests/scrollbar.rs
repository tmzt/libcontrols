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
