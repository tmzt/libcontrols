//! Sections drawn by a scroll pane, their cache, heights, anchoring and
//! scrolling to a section (docs/design/20261028_08_SCROLLBAR_PLAN.md,
//! steps 6 and 7).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use libcontrols::{
    BookmarkText, Button, ControlTheme, DrawContext, MouseButton, MouseEvent, NodeId, Rect, Router,
    ScrollAxis, ScrollEdge, ScrollPane, ScrollState, ScrollbarOptions, Section, SectionContext,
    SectionId, SectionSource, SectionedContent,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

/// Off the origin, so a section's place shows `vp.x` and `vp.y`.
const VP: Rect = Rect::new(10.0, 20.0, 400.0, 1000.0);
/// The marker a counting source's boxes carry in green.
const MARK: f32 = 0.5;

/// Draws each section as one `Box` its size, red the id, and counts the
/// calls by id. With `report`, it reports that height.
#[derive(Default)]
struct Counting {
    calls: Mutex<HashMap<u64, usize>>,
    report: Mutex<Option<f32>>,
    /// A clip of its own around the box: the top this tall.
    clip_top: Option<f32>,
}

impl Counting {
    fn calls(&self, id: u64) -> usize {
        self.calls.lock().unwrap().get(&id).copied().unwrap_or(0)
    }

    fn total(&self) -> usize {
        self.calls.lock().unwrap().values().sum()
    }

    fn reset(&self) {
        self.calls.lock().unwrap().clear();
    }
}

impl SectionSource for Counting {
    fn render_section(&self, id: SectionId, cx: &SectionContext) -> DrawList {
        *self.calls.lock().unwrap().entry(id.0).or_default() += 1;
        let mut height = cx.height();
        if let Some(h) = *self.report.lock().unwrap() {
            cx.report_height(h);
            height = h;
        }
        let mut list = DrawList::new();
        if let Some(h) = self.clip_top {
            list.push_clip([0.0, 0.0], [cx.width(), h]);
        }
        list.push(section_box(id, [cx.width(), height]));
        if self.clip_top.is_some() {
            list.push_clip_end();
        }
        list
    }
}

fn section_box(id: SectionId, size: [f32; 2]) -> SdfInstance {
    SdfInstance {
        kind: SdfKind::Box,
        position: [0.0, 0.0],
        size,
        color: [id.0 as f32, MARK, 0.25, 1.0],
        anim: 0,
    }
}

/// The section boxes drawn: (id, index in the list, instance).
fn boxes(list: &DrawList) -> Vec<(u64, usize, SdfInstance)> {
    list.instances
        .iter()
        .enumerate()
        .filter(|(_, i)| matches!(i.kind, SdfKind::Box) && i.color[1] == MARK)
        .map(|(n, i)| (i.color[0] as u64, n, *i))
        .collect()
}

fn ids(list: &DrawList) -> Vec<u64> {
    boxes(list).into_iter().map(|(id, _, _)| id).collect()
}

struct Fixture {
    router: Router,
    pane: NodeId,
    source: Arc<Counting>,
    content: Arc<SectionedContent>,
}

impl Fixture {
    fn new(sections: Vec<Section>, source: Counting, pane: ScrollPane) -> Self {
        let source = Arc::new(source);
        let content = SectionedContent::with_source(source.clone());
        content.set_sections(sections);
        let mut router = Router::new();
        let pane = router.add(None, pane.with_sections(content.clone()));
        Self {
            router,
            pane,
            source,
            content,
        }
    }

    /// 2000 sections of 100 (a 20 000-line file in 10-line sections).
    fn file() -> Self {
        Self::new(uniform(2000, 100.0, false), Counting::default(), plain())
    }

    fn render(&self) -> DrawList {
        self.render_with(&DrawContext::default())
    }

    fn render_with(&self, ctx: &DrawContext) -> DrawList {
        let mut list = DrawList::new();
        self.router.render(&mut list, ctx);
        list
    }

    fn pane(&self) -> &ScrollPane {
        self.router.get::<ScrollPane>(self.pane).unwrap()
    }

    fn offset(&self) -> f32 {
        self.pane().state().offset()[1]
    }

    fn top(&self, id: u64) -> f32 {
        self.content.offset_of(SectionId(id)).unwrap()
    }
}

fn uniform(n: u64, height: f32, estimated: bool) -> Vec<Section> {
    (0..n)
        .map(|i| Section::new(SectionId(i), height).estimated(estimated))
        .collect()
}

fn plain() -> ScrollPane {
    ScrollPane::new(VP, ScrollAxis::Vertical).with_indicator(false)
}

// ---- Step 6: drawing, the cache ----

#[test]
fn the_first_frame_draws_the_visible_sections_and_at_most_two_more() {
    let f = Fixture::file();
    let list = f.render();
    for i in 0..10 {
        assert_eq!(f.source.calls(i), 1, "visible section {i}");
    }
    let total = f.source.total();
    assert!((10..=12).contains(&total), "{total} renders");
    // Drawn: the visible ones and the cached neighbours, each once, at its
    // place inside the viewport's clip.
    let drawn = boxes(&list);
    assert_eq!(drawn.len(), total);
    for (id, n, b) in drawn {
        assert_eq!(b.position, [VP.x, VP.y + id as f32 * 100.0], "section {id}");
        assert_eq!(b.size, [400.0, 100.0]);
        let clip = list.instance_clip(n).expect("clipped");
        assert_eq!(clip.min, [VP.x, VP.y]);
        assert_eq!(clip.max, [VP.right(), VP.bottom()]);
    }
}

#[test]
fn a_second_frame_draws_nothing_new_and_a_small_scroll_little() {
    let f = Fixture::file();
    f.render();
    f.source.reset();
    // The second frame draws no visible section again; it goes on
    // prefetching, two a frame, until the margin (one window) is full.
    let list = f.render();
    assert!((0..10).all(|i| f.source.calls(i) == 0), "visible: cached");
    assert!(f.source.total() <= 2, "{}", f.source.total());
    assert!(ids(&list).len() >= 12);
    settle(&f, &DrawContext::default());
    f.source.reset();
    f.render();
    assert_eq!(f.source.total(), 0, "settled: nothing drawn");
    f.router.scroll_by(f.pane, [0.0, 100.0]);
    f.render();
    let total = f.source.total();
    assert!(total <= 3, "{total}: one newly visible plus prefetch");
    for i in 1..=10 {
        assert!(f.content.is_cached(SectionId(i)), "section {i}");
    }
}

#[test]
fn invalidating_a_section_draws_only_it_again() {
    let f = Fixture::file();
    f.render();
    // Let the prefetch settle around the window.
    for _ in 0..5 {
        f.render();
    }
    f.source.reset();
    f.content.invalidate(SectionId(3));
    assert!(!f.content.is_cached(SectionId(3)));
    f.render();
    assert_eq!(f.source.total(), 1);
    assert_eq!(f.source.calls(3), 1);
    assert!(f.content.is_cached(SectionId(3)));
}

/// Render until nothing more is drawn; the calls of each id.
fn settle(f: &Fixture, ctx: &DrawContext) {
    for _ in 0..20 {
        let before = f.source.total();
        f.render_with(ctx);
        if f.source.total() == before {
            return;
        }
    }
    panic!("never settled");
}

#[test]
fn a_width_or_theme_change_draws_every_drawn_section_once_more() {
    let f = Fixture::file();
    let ctx = DrawContext::default();
    settle(&f, &ctx);
    let drawn: Vec<u64> = ids(&f.render_with(&ctx));
    let before: Vec<usize> = drawn.iter().map(|&i| f.source.calls(i)).collect();
    assert!(before.iter().all(|&n| n == 1));

    f.pane().set_viewport(Rect::new(VP.x, VP.y, 300.0, VP.h));
    settle(&f, &ctx);
    for &i in &drawn {
        assert_eq!(f.source.calls(i), 2, "section {i} after the width change");
    }
    let list = f.render_with(&ctx);
    assert!(boxes(&list).iter().all(|(_, _, b)| b.size[0] == 300.0));

    let theme = ControlTheme {
        text_color: [0.2, 0.3, 0.4, 1.0],
        ..ControlTheme::default()
    };
    let themed = DrawContext::default().with_theme(theme);
    settle(&f, &themed);
    for &i in &drawn {
        assert_eq!(f.source.calls(i), 3, "section {i} after the theme change");
    }
}

#[test]
fn far_drawings_are_dropped_and_at_most_max_cached_are_kept() {
    let f = Fixture::file();
    let mut at = 0.0;
    while at < 100_000.0 {
        at += 1000.0;
        f.router.scroll_to(f.pane, [0.0, at]);
        f.render();
    }
    f.render();
    let window = (f.offset(), f.offset() + 1000.0);
    let cached: Vec<u64> = (0..2000)
        .filter(|&i| f.content.is_cached(SectionId(i)))
        .collect();
    assert!(!cached.is_empty());
    assert!(cached.len() <= 128, "{}", cached.len());
    for &i in &cached {
        let (top, bottom) = (f.top(i), f.top(i) + 100.0);
        let d = if bottom <= window.0 {
            window.0 - bottom
        } else {
            (top - window.1).max(0.0)
        };
        assert!(d <= 3000.0, "section {i} is {d} from the window");
    }

    f.content.set_max_cached(12);
    f.render();
    let n = (0..2000)
        .filter(|&i| f.content.is_cached(SectionId(i)))
        .count();
    assert!(n <= 12, "{n}");
}

#[test]
fn the_content_length_is_the_sections_and_children_draw_over_them() {
    let mut f = Fixture::file();
    let button = f.router.add(
        Some(f.pane),
        Button::new("over", Rect::new(VP.x, VP.y, 100.0, 40.0)),
    );
    let list = f.render();
    assert_eq!(f.pane().state().content_length(), 200_000.0);
    assert_eq!(f.pane().state().max()[1], 199_000.0);
    let last_section = boxes(&list).iter().map(|&(_, n, _)| n).max().unwrap();
    assert!(
        list.instances.len() > last_section + 1,
        "the button draws after the sections"
    );
    // A child further than the sections lengthens the content.
    f.router.remove(button);
    f.router.add(
        Some(f.pane),
        Button::new("far", Rect::new(VP.x, VP.y + 250_000.0, 100.0, 40.0)),
    );
    f.render();
    assert_eq!(f.pane().state().content_length(), 250_040.0);
}

#[test]
fn a_closure_works_as_a_source() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let content = SectionedContent::with_source(Arc::new(
        move |id: SectionId, cx: &SectionContext| -> DrawList {
            log.lock()
                .unwrap()
                .push((id, cx.index(), cx.width(), cx.height()));
            let mut list = DrawList::new();
            list.push(section_box(id, [cx.width(), cx.height()]));
            list
        },
    ));
    content.set_sections(uniform(3, 50.0, false));
    let mut router = Router::new();
    router.add(None, plain().with_sections(content));
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
    assert_eq!(ids(&list), [0, 1, 2]);
    assert_eq!(
        seen.lock().unwrap()[1],
        (SectionId(1), 1, 400.0, 50.0),
        "id, index, width, height"
    );
}

#[test]
fn a_sections_ink_and_clips_land_at_its_place_less_the_offset() {
    let f = Fixture::new(
        uniform(2000, 100.0, false),
        Counting {
            clip_top: Some(60.0),
            ..Counting::default()
        },
        plain(),
    );
    f.router.scroll_to(f.pane, [0.0, 1234.0]);
    let drawn = f.render();
    let found = boxes(&drawn);
    assert!(found.iter().any(|&(id, _, _)| id == 12));

    // By hand: the viewport's clip, the offset's translate, and each drawn
    // section recorded where it lands, its own clip moved to the screen.
    let mut want = DrawList::new();
    want.push_clip(VP.pos(), VP.size());
    want.push_translate(0.0, -1234.0);
    for &(id, _, _) in &found {
        let top = VP.y + id as f32 * 100.0;
        want.push_clip([VP.x, top - 1234.0], [400.0, 60.0]);
        let mut b = section_box(SectionId(id), [400.0, 100.0]);
        b.position = [VP.x, top];
        want.push(b);
        want.push_clip_end();
    }
    want.push_translate_end();
    want.push_clip_end();
    want.push_translate(0.0, -1234.0);
    want.push_translate_end();
    assert_eq!(drawn.instances, want.instances);
    assert_eq!(drawn.instance_clips(), want.instance_clips());
    let (a, b) = (drawn.lower(), want.lower());
    assert_eq!(a.draws, b.draws);
    assert_eq!(a.param_bank, b.param_bank);
    // Section 12 (1200..1300) is cut by the viewport's top and its own clip.
    let (_, n, _) = found.iter().find(|&&(id, _, _)| id == 12).unwrap();
    let clip = drawn.instance_clip(*n).unwrap();
    assert_eq!(clip.min, [VP.x, VP.y]);
    assert_eq!(clip.max, [VP.right(), VP.y + 1200.0 - 1234.0 + 60.0]);
}

#[test]
fn an_index_without_a_source_draws_nothing() {
    let content = SectionedContent::new();
    content.set_sections(uniform(20, 100.0, false));
    let mut router = Router::new();
    let pane = router.add(None, plain().with_sections(content.clone()));
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
    assert!(ids(&list).is_empty());
    assert_eq!(
        router
            .get::<ScrollPane>(pane)
            .unwrap()
            .state()
            .content_length(),
        2000.0
    );
    assert!(!content.is_cached(SectionId(0)));
}

#[test]
fn a_section_invalidated_while_it_is_drawn_is_drawn_again_next_frame() {
    struct SelfInvalidating {
        content: Mutex<Option<Arc<SectionedContent>>>,
        calls: Mutex<usize>,
    }
    impl SectionSource for SelfInvalidating {
        fn render_section(&self, id: SectionId, cx: &SectionContext) -> DrawList {
            let mut calls = self.calls.lock().unwrap();
            *calls += 1;
            if id == SectionId(0)
                && *calls == 1
                && let Some(c) = self.content.lock().unwrap().as_ref()
            {
                c.invalidate(id);
            }
            let mut list = DrawList::new();
            list.push(section_box(id, [cx.width(), cx.height()]));
            list
        }
    }
    let source = Arc::new(SelfInvalidating {
        content: Mutex::new(None),
        calls: Mutex::new(0),
    });
    let content = SectionedContent::with_source(source.clone());
    *source.content.lock().unwrap() = Some(content.clone());
    content.set_sections(uniform(1, 100.0, false));
    let mut router = Router::new();
    router.add(None, plain().with_sections(content.clone()));
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
    assert_eq!(ids(&list), [0], "drawn this frame all the same");
    assert!(!content.is_cached(SectionId(0)), "kept stale");
    router.render(&mut DrawList::new(), &DrawContext::default());
    assert_eq!(*source.calls.lock().unwrap(), 2);
    assert!(content.is_cached(SectionId(0)));
}

// ---- Step 7: heights, anchoring, scrolling to a section ----

#[test]
fn reported_heights_correct_estimates_and_keep_the_anchor_in_place() {
    let f = Fixture::new(uniform(200, 100.0, true), Counting::default(), plain());
    *f.source.report.lock().unwrap() = Some(140.0);
    assert!(f.router.scroll_to_section(f.pane, SectionId(50)));
    assert_eq!(f.offset(), 5000.0);
    for _ in 0..5 {
        f.render();
        assert_eq!(f.offset(), f.top(50), "section 50 stays at the top");
    }
    assert!(f.top(50) > 5000.0, "sections above it grew");
    let s = &f.content.sections()[50];
    assert_eq!((s.height, s.estimated), (140.0, false));
}

#[test]
fn a_section_above_growing_shifts_the_offset_and_one_below_or_the_anchor_does_not() {
    let f = Fixture::new(uniform(200, 100.0, false), Counting::default(), plain());
    f.router.scroll_to_section(f.pane, SectionId(50));
    f.render();
    assert_eq!(f.offset(), 5000.0);

    assert!(f.content.set_height(SectionId(10), 140.0));
    f.render();
    assert_eq!(f.offset(), 5040.0);
    assert_eq!(f.offset(), f.top(50));

    f.content.set_height(SectionId(70), 140.0);
    f.render();
    assert_eq!(f.offset(), 5040.0, "below the window");

    f.content.set_height(SectionId(50), 140.0);
    f.render();
    assert_eq!(f.offset(), 5040.0, "the anchor itself");
}

#[test]
fn an_insert_or_remove_above_the_window_shifts_by_its_height() {
    let f = Fixture::new(uniform(200, 100.0, false), Counting::default(), plain());
    f.router.scroll_to_section(f.pane, SectionId(50));
    f.render();
    f.content.insert(3, Section::new(SectionId(1000), 250.0));
    f.render();
    assert_eq!(f.offset(), 5250.0);
    assert_eq!(
        f.content.remove(SectionId(1000)).map(|s| s.height),
        Some(250.0)
    );
    f.render();
    assert_eq!(f.offset(), 5000.0);
    f.content.remove(SectionId(0));
    f.render();
    assert_eq!(f.offset(), 4900.0);
    assert_eq!(f.offset(), f.top(50));
}

#[test]
fn a_sticky_end_pane_keeps_following_as_sections_are_pushed() {
    let f = Fixture::new(
        uniform(20, 100.0, false),
        Counting::default(),
        plain().with_state(ScrollState::sticky(ScrollEdge::End)),
    );
    f.render();
    assert_eq!(f.offset(), 1000.0);
    for i in 20..30 {
        f.content.push(Section::new(SectionId(i), 100.0));
        f.render();
        assert_eq!(f.offset(), (i as f32 + 1.0) * 100.0 - 1000.0);
    }
    assert!(f.pane().state().is_following());
}

#[test]
fn anchoring_waits_while_the_thumb_is_held_and_applies_on_release() {
    // 100 sections of 100: content 10 000 in a 1000 window, the thumb 100
    // long; at 4500 it is midway, 470..570 on the screen.
    let f = Fixture::new(
        uniform(100, 100.0, false),
        Counting::default(),
        plain().with_scrollbar(ScrollbarOptions::default()),
    );
    f.router.scroll_to(f.pane, [0.0, 4500.0]);
    f.render();
    let on_thumb = [VP.right() - 6.0, 520.0];
    f.router.deliver_mouse(MouseEvent::Down {
        pos: on_thumb,
        button: MouseButton::Left,
    });
    assert!(f.pane().scrollbar().unwrap().is_held());
    f.content.set_height(SectionId(10), 140.0);
    f.render();
    assert_eq!(f.offset(), 4500.0, "no shift under the held thumb");
    f.router.deliver_mouse(MouseEvent::Up {
        pos: on_thumb,
        button: MouseButton::Left,
    });
    f.render();
    assert_eq!(f.offset(), 4540.0, "the shift on release");
}

#[test]
fn scroll_to_section_puts_its_top_at_the_windows_top() {
    let f = Fixture::file();
    assert!(f.router.scroll_to_section(f.pane, SectionId(500)));
    assert_eq!(f.offset(), 50_000.0);
    let list = f.render();
    assert!(
        boxes(&list)
            .iter()
            .any(|&(id, _, b)| id == 500 && b.position[1] == VP.y + 50_000.0)
    );
    assert!(f.router.scroll_to_section(f.pane, SectionId(1999)));
    assert_eq!(f.offset(), 199_000.0, "clamped at the end");
    assert!(!f.router.scroll_to_section(f.pane, SectionId(5000)));

    let mut router = Router::new();
    let bare = router.add(None, plain());
    let button = router.add(None, Button::new("b", Rect::new(0.0, 0.0, 10.0, 10.0)));
    assert!(!router.scroll_to_section(bare, SectionId(0)));
    assert!(!router.scroll_to_section(button, SectionId(0)));
}

#[test]
fn section_at_the_offset_of_every_id_is_that_id() {
    let f = Fixture::new(
        (0..2000)
            .map(|i| Section::new(SectionId(i * 7 + 3), 50.0 + (i % 13) as f32 * 10.0))
            .collect(),
        Counting::default(),
        plain(),
    );
    for s in f.content.sections() {
        assert_eq!(
            f.content.section_at(f.content.offset_of(s.id).unwrap()),
            Some(s.id)
        );
    }
}

#[test]
fn a_pane_with_sections_and_a_scrollbar_draws_their_bookmarks_while_held() {
    let f = Fixture::new(
        uniform(100, 100.0, false),
        Counting::default(),
        plain().with_scrollbar(ScrollbarOptions::default()),
    );
    f.content
        .add_bookmark(SectionId(0), BookmarkText::Text("Start".into()));
    f.content
        .add_bookmark(SectionId(60), BookmarkText::Text("Later".into()));
    let pills = |list: &DrawList| -> Vec<SdfInstance> {
        list.instances
            .iter()
            .filter(|i| {
                matches!(i.kind, SdfKind::RoundedBox { .. }) && (i.size[1] - 25.55).abs() < 1e-3
            })
            .copied()
            .collect()
    };
    assert!(pills(&f.render()).is_empty(), "not held");
    let on_thumb = [VP.right() - 6.0, VP.y + 50.0];
    f.router.deliver_mouse(MouseEvent::Down {
        pos: on_thumb,
        button: MouseButton::Left,
    });
    let p = pills(&f.render());
    assert_eq!(p.len(), 2);
    let mut centres: Vec<f32> = p.iter().map(|i| i.position[1] + i.size[1] / 2.0).collect();
    centres.sort_by(f32::total_cmp);
    // At top / 10 000 of the 1000 track.
    assert!((centres[0] - VP.y).abs() < 1e-3, "{centres:?}");
    assert!((centres[1] - (VP.y + 600.0)).abs() < 1e-3, "{centres:?}");
}
