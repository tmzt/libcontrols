//! Where flings end: boundaries, counted flicks and their events
//! (docs/design/20261119_01_RIBBON_TWO_FINGER_AND_THUMB_LABEL_PLAN.md,
//! the 18:31 EST amendment, items 3 to 5; step 5).

use std::sync::Arc;

use libcontrols::{
    BookmarkText, Boundaries, DrawContext, FlingEvent, FlingOptions, FlingProfile, FlingStop,
    MouseButton, MouseEvent, NodeId, Rect, Router, ScrollAxis, ScrollPane, Section, SectionId,
    SectionedContent, SkipMapping,
};
use libmsdf::drawlist::DrawList;

const VP: Rect = Rect::new(0.0, 0.0, 400.0, 1000.0);
const FRAME: f32 = 1.0 / 60.0;

struct Pane {
    router: Router,
    pane: NodeId,
    content: Arc<SectionedContent>,
}

impl Pane {
    fn new(sections: Vec<Section>, stop: FlingStop) -> Self {
        let content = SectionedContent::new();
        content.set_sections(sections);
        let mut router = Router::new();
        let pane = router.add(
            None,
            ScrollPane::new(VP, ScrollAxis::Vertical)
                .with_indicator(false)
                .with_sections(content.clone())
                .with_fling(FlingOptions::new().stop(stop)),
        );
        let mut list = DrawList::new();
        router.render(&mut list, &DrawContext::default());
        Self {
            router,
            pane,
            content,
        }
    }

    fn scroll_pane(&self) -> &ScrollPane {
        self.router.get::<ScrollPane>(self.pane).unwrap()
    }

    fn offset(&self) -> f32 {
        self.scroll_pane().state().offset()[1]
    }

    fn top(&self, id: u64) -> f32 {
        self.content.offset_of(SectionId(id)).unwrap()
    }

    fn index_at_top(&self) -> u64 {
        self.content.section_at(self.offset()).unwrap().0
    }

    fn events(&self) -> Vec<FlingEvent> {
        self.scroll_pane().state().take_fling_events()
    }

    /// A gesture at a steady `v` px/s (positive: the finger moving up, the
    /// content scrolling on), `frames` moves of 16 ms, lifted at once;
    /// with `two`, two fingers lifted together.
    fn gesture(&self, v: f32, frames: usize, two: bool) {
        let r = &self.router;
        r.set_fingers(if two { 2 } else { 1 });
        let mut y = 500.0;
        r.deliver_mouse(MouseEvent::Down {
            pos: [100.0, y],
            button: MouseButton::Left,
        });
        for _ in 0..frames {
            r.advance(0.016);
            y -= v * 0.016;
            r.deliver_mouse(MouseEvent::Drag {
                start_pos: [100.0, 500.0],
                current_pos: [100.0, y],
                delta: [0.0, 0.0],
                button: MouseButton::Left,
            });
        }
        if two {
            r.set_fingers(1);
        }
        r.set_fingers(0);
        r.deliver_mouse(MouseEvent::Up {
            pos: [100.0, y],
            button: MouseButton::Left,
        });
    }

    /// A flick: two moves.
    fn flick(&self, v: f32, two: bool) {
        self.gesture(v, 2, two);
    }

    /// Advance until it stops; returns the frames taken.
    fn settle(&self) -> usize {
        let mut frames = 0;
        while self.scroll_pane().state().is_moving() {
            self.router.advance(FRAME);
            frames += 1;
            assert!(frames < 20_000, "never settled");
        }
        frames
    }
}

fn uniform(n: u64, h: f32) -> Vec<Section> {
    (0..n).map(|i| Section::new(SectionId(i), h)).collect()
}

fn count() -> FlingStop {
    FlingStop::count(Boundaries::Sections, SkipMapping::default())
}

/// An event as (kind, section, count), as the variants are
/// `#[non_exhaustive]`.
fn ev(e: &FlingEvent) -> (&'static str, u64, u32) {
    match *e {
        FlingEvent::Counted { target, n, .. } => ("counted", target.0, n),
        FlingEvent::Passed { id, remaining, .. } => ("passed", id.0, remaining),
        FlingEvent::Retargeted { target, .. } => ("retargeted", target.0, 0),
        FlingEvent::Landed { id, .. } => ("landed", id.0, 0),
        _ => ("other", 0, 0),
    }
}

fn evs(p: &Pane) -> Vec<(&'static str, u64, u32)> {
    p.events().iter().map(ev).collect()
}

/// The first frame's speed after a lift.
fn first_speed(p: &Pane) -> f32 {
    let before = p.offset();
    p.router.advance(FRAME);
    (p.offset() - before) / FRAME
}

// ---- Boundary ----

#[test]
fn a_boundary_fling_lands_on_the_first_top_past_its_natural_landing() {
    for v in [900.0f32, -900.0] {
        // The fling's speed and natural landing, from a free twin.
        let free = Pane::new(uniform(1000, 100.0), FlingStop::Free);
        free.router.scroll_to(free.pane, [0.0, 50_050.0]);
        free.gesture(v, 8, false);
        let lifted = free.offset();
        let speed = first_speed(&free);
        let natural = lifted + speed / FlingProfile::flankr().decay;

        let p = Pane::new(
            uniform(1000, 100.0),
            FlingStop::boundary(Boundaries::Sections),
        );
        p.router.scroll_to(p.pane, [0.0, 50_050.0]);
        p.gesture(v, 8, false);
        assert_eq!(p.offset(), lifted);
        p.settle();
        let expected = if v > 0.0 {
            (natural / 100.0).ceil() * 100.0
        } else {
            (natural / 100.0).floor() * 100.0
        };
        assert_eq!(p.offset(), expected, "v {v}: natural {natural}");
        assert!(matches!(p.events().last(), Some(FlingEvent::Landed { .. })));
    }
}

#[test]
fn a_capped_boundary_fling_lands_on_the_last_top_within_reach() {
    // From 500 a fast flick's natural landing is past 10 000, which is out
    // of reach at 16 000 px/s; 5000 is the last top within it.
    let p = Pane::new(
        uniform(20, 5000.0),
        FlingStop::boundary(Boundaries::Sections),
    );
    p.router.scroll_to(p.pane, [0.0, 0.0]);
    p.gesture(8000.0, 3, false);
    assert!(p.offset() < 5000.0, "{}", p.offset());
    p.settle();
    assert_eq!(p.offset(), 5000.0);
}

#[test]
fn a_boundary_fling_past_the_last_section_lands_at_the_end() {
    let p = Pane::new(
        uniform(30, 100.0),
        FlingStop::boundary(Boundaries::Sections),
    );
    p.router.scroll_to(p.pane, [0.0, 1500.0]);
    p.gesture(3000.0, 4, false);
    p.settle();
    assert_eq!(p.offset(), 2000.0, "the end of the range");
}

#[test]
fn a_height_change_above_the_window_mid_fling_still_lands_on_its_top() {
    let p = Pane::new(
        uniform(1000, 100.0),
        FlingStop::boundary(Boundaries::Sections),
    );
    p.router.scroll_to(p.pane, [0.0, 50_050.0]);
    p.gesture(900.0, 8, false);
    for _ in 0..5 {
        p.router.advance(FRAME);
    }
    p.content.set_height(SectionId(3), 340.0);
    p.settle();
    let landed = match p.events().last() {
        Some(FlingEvent::Landed { id, .. }) => *id,
        other => panic!("{other:?}"),
    };
    assert_eq!(p.offset(), p.top(landed.0));
    assert_eq!(p.offset() % 100.0, 40.0, "on a top, 240 further down");
}

// ---- Counted flicks ----

/// A counted flick at `v` on sections of `h`: the boundaries it passed.
fn counted(v: f32, two: bool, h: f32) -> u64 {
    let p = Pane::new(uniform(4000, h), count());
    let start = 1000.0 * h + h * 0.5;
    p.router.scroll_to(p.pane, [0.0, start]);
    p.flick(v, two);
    let from = p.index_at_top();
    p.settle();
    let to = p.index_at_top();
    assert_eq!(p.offset(), p.top(to), "on a top");
    to - from
}

#[test]
fn counted_flicks_pass_the_tables_counts() {
    let table = [
        // Just past each step, as a measured speed has rounding.
        (601.0, 1),
        (1499.0, 1),
        (1501.0, 2),
        (2122.0, 3),
        (3001.0, 4),
        (4244.0, 5),
        (6001.0, 6),
        (8000.0, 6),
    ];
    for (v, n) in table {
        assert_eq!(counted(v, false, 100.0), n, "one finger at {v}");
        assert_eq!(counted(v, true, 100.0), n * 4, "two at {v}");
    }
}

#[test]
fn the_same_flick_passes_the_same_count_over_any_heights() {
    for v in [1000.0, 3000.0] {
        let small = counted(v, false, 10.0);
        let large = counted(v, false, 10_000.0);
        assert_eq!(small, large, "{v}");
        assert_eq!(counted(v, true, 10.0), counted(v, true, 10_000.0));
    }
}

#[test]
fn a_counted_flick_goes_up_too() {
    let p = Pane::new(uniform(200, 100.0), count());
    p.router.scroll_to(p.pane, [0.0, 10_050.0]);
    p.flick(-3000.0, false);
    // Passing up from inside section 100: 100, 99, 98, 97.
    p.settle();
    assert_eq!(p.offset(), p.top(97));
}

#[test]
fn a_scroll_under_count_is_a_distance_fling() {
    let p = Pane::new(uniform(1000, 100.0), count());
    p.router.scroll_to(p.pane, [0.0, 50_050.0]);
    // 300 ms: longer than a flick.
    p.gesture(900.0, 19, false);
    p.settle();
    assert!(
        !p.events()
            .iter()
            .any(|e| matches!(e, FlingEvent::Counted { .. })),
        "no count"
    );
    assert_ne!(p.offset() % 100.0, 0.0, "free: {}", p.offset());
}

#[test]
fn placeholders_count_as_one_each() {
    let mut list = uniform(100, 100.0);
    for s in &mut list[50..60] {
        *s = Section::placeholder(s.id);
    }
    let p = Pane::new(list, count());
    p.content.set_placeholder_height(500.0);
    p.router.scroll_to(p.pane, [0.0, p.top(49) + 10.0]);
    p.flick(3000.0, false);
    let from = p.index_at_top();
    p.settle();
    assert_eq!(p.index_at_top() - from, 4);
}

#[test]
fn a_flight_past_the_last_section_lands_at_the_end() {
    let p = Pane::new(uniform(15, 100.0), count());
    p.router.scroll_to(p.pane, [0.0, 50.0]);
    p.flick(6000.0, true);
    p.settle();
    assert_eq!(p.offset(), 500.0);
}

#[test]
fn the_flights_length_is_its_duration() {
    let p = Pane::new(uniform(1000, 100.0), count());
    p.router.scroll_to(p.pane, [0.0, 50_050.0]);
    p.flick(1000.0, false);
    let frames = p.settle();
    assert!(
        (frames as f32 * FRAME - 0.30).abs() <= FRAME,
        "{frames} frames"
    );
    let p = Pane::new(uniform(1000, 100.0), count());
    p.router.scroll_to(p.pane, [0.0, 50_050.0]);
    p.flick(3000.0, false);
    let frames = p.settle();
    assert!((frames as f32 * FRAME - 0.45).abs() <= FRAME, "4: {frames}");
}

#[test]
fn fling_events_come_in_order() {
    let p = Pane::new(uniform(1000, 100.0), count());
    p.router.scroll_to(p.pane, [0.0, 50_050.0]);
    p.flick(3000.0, false);
    let from = p.index_at_top();
    p.settle();
    let f = from;
    assert_eq!(
        evs(&p),
        [
            ("counted", f + 4, 4),
            ("passed", f + 1, 3),
            ("passed", f + 2, 2),
            ("passed", f + 3, 1),
            ("passed", f + 4, 0),
            ("landed", f + 4, 0),
        ]
    );
}

/// A pane mid-flight towards the 4th boundary from 50 050 (section 504).
fn mid_flight() -> Pane {
    let p = Pane::new(uniform(1000, 100.0), count());
    p.router.scroll_to(p.pane, [0.0, 50_050.0]);
    p.flick(3000.0, false);
    assert_eq!(evs(&p), [("counted", 504, 4)]);
    for _ in 0..3 {
        p.router.advance(FRAME);
    }
    p
}

#[test]
fn a_target_growing_removed_split_or_merged_mid_flight_still_lands() {
    // Growing above it: on its new top.
    let p = mid_flight();
    p.content.set_height(SectionId(10), 400.0);
    p.content.set_height(SectionId(504), 900.0);
    p.settle();
    assert_eq!(p.offset(), p.top(504));
    // Removed: the next section that way.
    let p = mid_flight();
    p.content.remove(SectionId(504));
    p.settle();
    assert_eq!(p.offset(), p.top(505));
    assert!(evs(&p).contains(&("retargeted", 505, 0)));
    // Split: its first piece.
    let p = mid_flight();
    p.content.edit(|e| {
        e.split(
            SectionId(504),
            vec![
                Section::new(SectionId(5040), 30.0),
                Section::new(SectionId(5041), 70.0),
            ],
        )
    });
    p.settle();
    assert_eq!(p.offset(), p.top(5040));
    // Merged: the merged section's top.
    let p = mid_flight();
    p.content.edit(|e| {
        e.merge(
            SectionId(503),
            SectionId(505),
            Section::new(SectionId(9000), 300.0),
        )
    });
    p.settle();
    assert_eq!(p.offset(), p.top(9000));
    assert!(evs(&p).contains(&("retargeted", 9000, 0)));
}

#[test]
fn flicks_add_up_within_the_boost_window_up_to_48() {
    let p = Pane::new(uniform(4000, 100.0), count());
    p.router.scroll_to(p.pane, [0.0, 100_050.0]);
    let from = p.index_at_top();
    p.flick(3000.0, false);
    p.router.advance(FRAME);
    p.flick(3000.0, false);
    assert!(
        p.events()
            .iter()
            .any(|e| matches!(e, FlingEvent::Retargeted { .. }))
    );
    p.settle();
    assert_eq!(p.index_at_top() - from, 8, "4 and 4");

    let p = Pane::new(uniform(4000, 100.0), count());
    p.router.scroll_to(p.pane, [0.0, 100_050.0]);
    let mut most = 0;
    for _ in 0..5 {
        p.flick(6000.0, true);
        // 24 each, added to what is still to pass, at most 48.
        let (_, remaining) = p.scroll_pane().state().flight().expect("in flight");
        assert!(remaining <= 48, "{remaining}");
        most = most.max(remaining);
        p.router.advance(FRAME);
    }
    assert_eq!(most, 48);
    let (target, _) = p.scroll_pane().state().flight().expect("in flight");
    p.settle();
    assert_eq!(p.offset(), p.top(target.0), "capped");

    // The other way counts afresh.
    let p = Pane::new(uniform(4000, 100.0), count());
    p.router.scroll_to(p.pane, [0.0, 100_050.0]);
    p.flick(3000.0, false);
    p.router.advance(FRAME);
    p.router.advance(FRAME);
    let at = p.offset();
    p.flick(-1000.0, false);
    let from = p.content.section_at(at).unwrap().0;
    p.settle();
    assert!(
        p.index_at_top() < from + 1,
        "{} from {from}",
        p.index_at_top()
    );
}

#[test]
fn at_most_counts_the_same_tops_whether_or_not_others_are_split() {
    // Months of 1000 at level 1, every 12th a year at level 0.
    let months = || -> Vec<Section> {
        (0..200)
            .map(|i| Section::new(SectionId(i), 1000.0).level(if i % 12 == 0 { 0 } else { 1 }))
            .collect()
    };
    let flick = |p: &Pane| {
        p.router.scroll_to(p.pane, [0.0, p.top(100) + 10.0]);
        p.flick(3000.0, false);
        p.settle();
    };
    let plain = Pane::new(
        months(),
        FlingStop::count(Boundaries::AtMost(1), SkipMapping::default()),
    );
    flick(&plain);
    assert_eq!(plain.offset(), plain.top(104));
    let split = Pane::new(
        months(),
        FlingStop::count(Boundaries::AtMost(1), SkipMapping::default()),
    );
    split.content.edit(|e| {
        for m in [101, 102] {
            let weeks = (0..4)
                .map(|w| {
                    Section::new(SectionId(1000 + m * 10 + w), 250.0).level(if w == 0 {
                        1
                    } else {
                        2
                    })
                })
                .collect();
            e.split(SectionId(m), weeks);
        }
    });
    flick(&split);
    assert_eq!(split.offset(), split.top(104), "the same months");
    // Sections counts every top: weeks in the split months.
    let weeks = Pane::new(months(), count());
    weeks.content.edit(|e| {
        let pieces = (0..4)
            .map(|w| Section::new(SectionId(2000 + w), 250.0).level(if w == 0 { 1 } else { 2 }))
            .collect();
        e.split(SectionId(101), pieces);
    });
    flick(&weeks);
    assert_eq!(
        weeks.offset(),
        weeks.top(2003),
        "101's weeks 1, 2, 3, then 2003"
    );
}

#[test]
fn bookmarked_boundaries_count_only_bookmarked_tops() {
    let p = Pane::new(
        uniform(300, 100.0),
        FlingStop::count(Boundaries::Bookmarked, SkipMapping::default()),
    );
    for i in (0..300).step_by(10) {
        p.content
            .add_bookmark(SectionId(i), BookmarkText::Text(format!("{i}")));
    }
    p.router.scroll_to(p.pane, [0.0, 10_050.0]);
    p.flick(1600.0, false);
    p.settle();
    assert_eq!(p.offset(), p.top(120), "110, 120");
}
