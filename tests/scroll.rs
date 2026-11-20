//! Scroll panes (docs/design/20261023_02_SCROLL_PANE_PLAN.md, section 5).

mod common;

use std::sync::Arc;

use common::{Call, Recorder};
use libcontrols::{
    Button, ButtonState, ControlAction, ControlPart, ControlTheme, DrawContext, DrawControl,
    EventContext, FlingOptions, FlingProfile, HostedControl, InputEvent, Key, KeyboardEvent, Label,
    MouseButton, MouseEvent, NodeId, Overscroll, Rect, Router, ScrollAxis, ScrollEdge, ScrollPane,
    ScrollState, ScrollTheme, Slider,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

const VIEWPORT: Rect = Rect::new(0.0, 0.0, 200.0, 100.0);
const ROW: f32 = 40.0;

fn down(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Down {
        pos: [x, y],
        button: MouseButton::Left,
    }
}

fn up(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Up {
        pos: [x, y],
        button: MouseButton::Left,
    }
}

fn click(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Click {
        pos: [x, y],
        button: MouseButton::Left,
    }
}

fn drag(from: [f32; 2], x: f32, y: f32) -> MouseEvent {
    MouseEvent::Drag {
        start_pos: from,
        current_pos: [x, y],
        delta: [0.0, 0.0],
        button: MouseButton::Left,
    }
}

fn key(k: Key) -> KeyboardEvent {
    KeyboardEvent::KeyDown { key: k }
}

fn render(router: &Router) -> DrawList {
    let mut list = DrawList::default();
    router.render(&mut list, &DrawContext::default());
    list
}

fn row(i: usize) -> Rect {
    Rect::new(0.0, i as f32 * ROW, 200.0, ROW)
}

/// A vertical pane of `n` buttons, 40 px each, in a 100 px viewport.
struct List {
    router: Router,
    pane: NodeId,
    rows: Vec<NodeId>,
    clicks: Vec<Arc<ButtonState>>,
}

fn list_with(n: usize, pane: ScrollPane) -> List {
    let mut router = Router::new();
    let pane = router.add(None, pane);
    let mut rows = Vec::new();
    let mut clicks = Vec::new();
    for i in 0..n {
        let b = Button::new(format!("row {i}"), row(i));
        clicks.push(Arc::clone(&b.state));
        rows.push(router.add(Some(pane), b));
    }
    render(&router);
    List {
        router,
        pane,
        rows,
        clicks,
    }
}

fn list(n: usize) -> List {
    list_with(n, ScrollPane::new(VIEWPORT, ScrollAxis::Vertical))
}

impl List {
    fn state(&self) -> &Arc<ScrollState> {
        self.router.get::<ScrollPane>(self.pane).unwrap().state()
    }

    fn offset(&self) -> f32 {
        self.state().offset()[1]
    }

    fn clicked(&self) -> Vec<usize> {
        (0..self.clicks.len())
            .filter(|&i| self.clicks[i].take_click())
            .collect()
    }
}

// ---- The range ----

#[test]
fn the_range_comes_from_the_children_or_the_app() {
    let l = list(10);
    assert_eq!(l.state().max(), [0.0, 300.0]);
    let l = list_with(
        10,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical).with_content_size([200.0, 1000.0]),
    );
    assert_eq!(l.state().max(), [0.0, 900.0]);
    let l = list(2);
    assert_eq!(l.state().max(), [0.0, 0.0], "the content fits");
}

#[test]
fn scroll_to_clamps_and_works_before_the_first_frame() {
    let mut router = Router::new();
    let pane = router.add(None, ScrollPane::new(VIEWPORT, ScrollAxis::Vertical));
    for i in 0..10 {
        router.add(Some(pane), Button::new("r", row(i)));
    }
    router.scroll_to(pane, [0.0, 120.0]);
    let state = Arc::clone(router.get::<ScrollPane>(pane).unwrap().state());
    assert_eq!(state.offset(), [0.0, 120.0]);
    router.scroll_to(pane, [0.0, 5000.0]);
    assert_eq!(state.offset(), [0.0, 300.0]);
    router.scroll_by(pane, [0.0, -100.0]);
    assert_eq!(state.offset(), [0.0, 200.0]);
}

#[test]
fn a_state_kept_by_the_app_brings_a_rebuilt_pane_back_to_its_place() {
    let state = ScrollState::new();
    let l = list_with(
        10,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical).with_state(Arc::clone(&state)),
    );
    l.router.scroll_to(l.pane, [0.0, 150.0]);
    drop(l);
    let l = list_with(
        10,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical).with_state(Arc::clone(&state)),
    );
    assert_eq!(l.offset(), 150.0);
}

#[test]
fn a_sticky_pane_follows_the_end_as_content_arrives() {
    let state = ScrollState::sticky(ScrollEdge::End);
    let mut l = list_with(
        2,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical).with_state(Arc::clone(&state)),
    );
    assert_eq!(l.offset(), 0.0);
    for i in 2..10 {
        l.router.add(Some(l.pane), Button::new("r", row(i)));
    }
    render(&l.router);
    assert_eq!(l.offset(), 300.0);
    assert!(state.is_following());
    l.router.scroll_to(l.pane, [0.0, 100.0]);
    l.router.add(Some(l.pane), Button::new("r", row(10)));
    render(&l.router);
    assert_eq!(l.offset(), 100.0, "scrolled back, it holds still");
}

// ---- Drawing ----

#[test]
fn the_children_are_clipped_translated_and_culled() {
    let l = list_with(
        10,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical).with_indicator(false),
    );
    l.router.scroll_to(l.pane, [0.0, 50.0]);
    let drawn = render(&l.router);

    // By hand: the viewport's clip, the offset's translate, and only rows
    // 1 to 3 (40..160 against the 50..150 in view).
    let mut want = DrawList::default();
    let ctx = DrawContext::default();
    want.push_clip([0.0, 0.0], [200.0, 100.0]);
    want.push_translate(0.0, -50.0);
    for i in 1..=3 {
        let mut c = ctx.clone();
        c.flags = l.router.flags(l.rows[i]);
        l.router
            .get::<Button>(l.rows[i])
            .unwrap()
            .draw(&mut want, &c);
    }
    want.push_translate_end();
    want.push_clip_end();
    // The overlay pass: translated, not clipped (the buttons draw nothing).
    want.push_translate(0.0, -50.0);
    want.push_translate_end();
    let (a, b) = (drawn.lower(), want.lower());
    assert_eq!(drawn.instances.len(), want.instances.len());
    assert_eq!(a.draws, b.draws);
    assert_eq!(a.param_bank, b.param_bank);
}

/// Draws a box in each pass, so a test can tell the passes apart.
struct Marker(Rect);

impl DrawControl for Marker {
    fn draw(&self, list: &mut DrawList, _ctx: &DrawContext) {
        list.push(SdfInstance {
            kind: SdfKind::Box,
            position: self.0.pos(),
            size: self.0.size(),
            color: [1.0, 0.0, 0.0, 1.0],
            anim: 0,
        });
    }

    fn draw_overlay(&self, list: &mut DrawList, _ctx: &DrawContext) {
        list.push(SdfInstance {
            kind: SdfKind::Box,
            position: self.0.pos(),
            size: self.0.size(),
            color: [0.0, 1.0, 0.0, 1.0],
            anim: 0,
        });
    }
}

impl HostedControl for Marker {
    fn bounds(&self) -> Rect {
        self.0
    }
    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.0.contains(pos).then_some((ControlPart::Body, 0.0))
    }
    fn handle_mouse(&self, _e: &MouseEvent, _c: &EventContext) -> Option<ControlAction> {
        None
    }
    fn handle_keyboard(&self, _e: &KeyboardEvent, _c: &EventContext) -> Option<ControlAction> {
        None
    }
}

#[test]
fn overlays_are_moved_with_the_content_but_not_clipped() {
    let mut router = Router::new();
    let pane = router.add(
        None,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical).with_indicator(false),
    );
    router.add(Some(pane), Marker(Rect::new(0.0, 80.0, 50.0, 40.0)));
    let drawn = render(&router);
    assert_eq!(drawn.instances.len(), 2);
    assert!(
        drawn.instance_clip(0).is_some(),
        "the first pass is clipped"
    );
    assert!(drawn.instance_clip(1).is_none(), "the overlay is not");
}

#[test]
fn nested_panes_intersect_their_clips() {
    let mut router = Router::new();
    let outer = router.add(
        None,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical).with_indicator(false),
    );
    let inner = router.add(
        Some(outer),
        ScrollPane::new(Rect::new(50.0, 60.0, 100.0, 300.0), ScrollAxis::Horizontal)
            .with_indicator(false),
    );
    router.add(Some(inner), Marker(Rect::new(50.0, 60.0, 400.0, 40.0)));
    router.add(Some(outer), Marker(Rect::new(0.0, 400.0, 10.0, 10.0)));
    router.scroll_to(outer, [0.0, 30.0]);
    let drawn = render(&router);
    // The inner pane's viewport is at 30..330 on the screen, cut by the
    // outer's 0..100.
    let clip = drawn.instance_clip(0).expect("clipped");
    assert_eq!(clip.min, [50.0, 30.0]);
    assert_eq!(clip.max, [150.0, 100.0]);
    assert_eq!(
        drawn.instances.len(),
        2,
        "the near marker's two passes; the far one is culled"
    );
}

// ---- Hits ----

#[test]
fn a_point_in_the_viewport_reaches_the_child_in_its_own_coordinates() {
    let mut router = Router::new();
    let pane = router.add(None, ScrollPane::new(VIEWPORT, ScrollAxis::Vertical));
    let mut r = Recorder::at(0.0);
    r.rect = Rect::new(0.0, 200.0, 100.0, 100.0);
    let rec = router.add(Some(pane), r);
    render(&router);
    router.scroll_to(pane, [0.0, 150.0]);
    // Screen y 60 is content y 210.
    let resp = router.deliver_mouse(down(10.0, 60.0));
    assert_eq!(resp.target, Some(rec));
    let calls = router.get::<Recorder>(rec).unwrap().take();
    assert!(
        matches!(calls.last(), Some(Call::Mouse(MouseEvent::Down { pos, .. }, _)) if *pos == [10.0, 210.0]),
        "{calls:?}"
    );
    router.deliver_mouse(up(10.0, 60.0));
    // Screen y 120 is content y 270, inside the child but outside the
    // viewport.
    let resp = router.deliver_mouse(down(10.0, 120.0));
    assert_ne!(resp.target, Some(rec));
    assert_eq!(router.to_content(rec, [10.0, 60.0]), [10.0, 210.0]);
    assert_eq!(
        router.visible_bounds(rec),
        Some(Rect::new(0.0, 50.0, 100.0, 50.0))
    );
}

// ---- Tap or scroll ----

#[test]
fn a_tap_inside_a_pane_activates_once_on_release() {
    let l = list(10);
    l.router.deliver_mouse(down(100.0, 50.0));
    assert!(l.clicked().is_empty(), "not on the press");
    l.router.deliver_mouse(drag([100.0, 50.0], 103.0, 45.0));
    l.router.deliver_mouse(up(103.0, 45.0));
    l.router.deliver_mouse(click(103.0, 45.0));
    assert_eq!(l.clicked(), [1]);
    assert_eq!(l.offset(), 0.0);
}

#[test]
fn a_drag_past_the_slop_scrolls_and_the_button_does_not_activate() {
    let l = list(10);
    l.router.deliver_mouse(down(100.0, 50.0));
    let resp = l.router.deliver_mouse(drag([100.0, 50.0], 100.0, 30.0));
    assert_eq!(resp.target, Some(l.pane));
    assert_eq!(l.offset(), 0.0, "it follows from where the slop was passed");
    l.router.deliver_mouse(drag([100.0, 50.0], 100.0, 10.0));
    assert_eq!(l.offset(), 20.0);
    l.router.deliver_mouse(up(100.0, 10.0));
    l.router.deliver_mouse(click(100.0, 10.0));
    assert!(l.clicked().is_empty(), "a scroll does not press");
}

#[test]
fn the_child_is_told_its_press_is_up_when_the_pane_takes_it() {
    let mut router = Router::new();
    let pane = router.add(None, ScrollPane::new(VIEWPORT, ScrollAxis::Vertical));
    let mut r = Recorder::at(0.0);
    r.rect = Rect::new(0.0, 0.0, 200.0, 400.0);
    let rec = router.add(Some(pane), r);
    render(&router);
    router.deliver_mouse(down(50.0, 80.0));
    router.deliver_mouse(drag([50.0, 80.0], 50.0, 60.0));
    let calls = router.get::<Recorder>(rec).unwrap().take();
    assert!(
        matches!(calls.last(), Some(Call::Mouse(MouseEvent::Up { .. }, _))),
        "{calls:?}"
    );
}

#[test]
fn a_drag_across_a_vertical_pane_stays_with_the_child() {
    let mut router = Router::new();
    let pane = router.add(None, ScrollPane::new(VIEWPORT, ScrollAxis::Vertical));
    let mut r = Recorder::at(0.0);
    r.rect = Rect::new(0.0, 0.0, 200.0, 400.0);
    let rec = router.add(Some(pane), r);
    render(&router);
    router.deliver_mouse(down(50.0, 50.0));
    router.deliver_mouse(drag([50.0, 50.0], 90.0, 52.0));
    router.deliver_mouse(drag([50.0, 50.0], 90.0, 20.0));
    let offset = router.get::<ScrollPane>(pane).unwrap().state().offset();
    assert_eq!(offset, [0.0, 0.0], "decided at the slop, as horizontal");
    let drags = router
        .get::<Recorder>(rec)
        .unwrap()
        .take()
        .into_iter()
        .filter(|c| matches!(c, Call::Mouse(MouseEvent::Drag { .. }, _)))
        .count();
    assert_eq!(drags, 2);
}

#[test]
fn a_slider_keeps_its_drag_inside_a_pane() {
    let mut router = Router::new();
    let pane = router.add(None, ScrollPane::new(VIEWPORT, ScrollAxis::Vertical));
    let slider = router.add(
        Some(pane),
        Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 10.0, 200.0, 20.0)),
    );
    for i in 1..10 {
        router.add(Some(pane), Button::new("r", row(i)));
    }
    render(&router);
    router.deliver_mouse(down(100.0, 20.0));
    let resp = router.deliver_mouse(drag([100.0, 20.0], 100.0, 80.0));
    assert_eq!(resp.target, Some(slider));
    let offset = router.get::<ScrollPane>(pane).unwrap().state().offset();
    assert_eq!(offset, [0.0, 0.0]);
}

#[test]
fn nested_panes_split_drags_by_axis() {
    let mut router = Router::new();
    let outer = router.add(None, ScrollPane::new(VIEWPORT, ScrollAxis::Vertical));
    let inner = router.add(
        Some(outer),
        ScrollPane::new(Rect::new(0.0, 0.0, 200.0, 60.0), ScrollAxis::Horizontal),
    );
    for i in 0..5 {
        router.add(
            Some(inner),
            Button::new("c", Rect::new(i as f32 * 100.0, 0.0, 100.0, 60.0)),
        );
    }
    for i in 2..10 {
        router.add(Some(outer), Button::new("r", row(i)));
    }
    render(&router);
    let off = |id| router.get::<ScrollPane>(id).unwrap().state().offset();
    router.deliver_mouse(down(150.0, 30.0));
    router.deliver_mouse(drag([150.0, 30.0], 130.0, 30.0));
    router.deliver_mouse(drag([150.0, 30.0], 100.0, 30.0));
    router.deliver_mouse(up(100.0, 30.0));
    assert_eq!(off(inner), [30.0, 0.0]);
    assert_eq!(off(outer), [0.0, 0.0]);
    router.deliver_mouse(down(150.0, 30.0));
    router.deliver_mouse(drag([150.0, 30.0], 150.0, 10.0));
    router.deliver_mouse(drag([150.0, 30.0], 150.0, 0.0));
    router.deliver_mouse(up(150.0, 0.0));
    assert_eq!(off(outer), [0.0, 10.0]);
}

// ---- Fling ----

/// A fast drag up from y 90 to 30, one event a frame.
fn flick(l: &List) {
    l.router.deliver_mouse(down(100.0, 90.0));
    let mut y = 90.0;
    for _ in 0..6 {
        l.router.advance(0.016);
        y -= 10.0;
        l.router.deliver_mouse(drag([100.0, 90.0], 100.0, y));
    }
}

#[test]
fn a_fling_carries_on_and_stops_within_the_range() {
    let l = list(100);
    flick(&l);
    l.router.deliver_mouse(up(100.0, 30.0));
    let lifted = l.offset();
    let mut frames = 0;
    while l.router.advance(0.016) {
        frames += 1;
        assert!(frames < 10_000, "it never settled");
    }
    assert!(l.offset() > lifted + 50.0, "{} after {lifted}", l.offset());
    assert!(l.offset() <= l.state().max()[1]);
    assert!(!l.state().is_moving());
}

#[test]
fn a_finger_that_rests_before_lifting_does_not_fling() {
    let l = list(100);
    flick(&l);
    l.router.advance(0.2);
    l.router.deliver_mouse(up(100.0, 30.0));
    let lifted = l.offset();
    for _ in 0..30 {
        l.router.advance(0.016);
    }
    assert_eq!(l.offset(), lifted);
}

#[test]
fn a_touch_during_a_fling_stops_it_and_is_not_a_tap() {
    let l = list(100);
    flick(&l);
    l.router.deliver_mouse(up(100.0, 30.0));
    l.router.deliver_mouse(click(100.0, 30.0));
    l.router.advance(0.016);
    let resp = l.router.deliver_mouse(down(100.0, 50.0));
    assert_eq!(resp.target, Some(l.pane));
    let held = l.offset();
    l.router.advance(0.1);
    assert_eq!(l.offset(), held, "the fling stopped");
    l.router.deliver_mouse(up(100.0, 50.0));
    l.router.deliver_mouse(click(100.0, 50.0));
    assert!(l.clicked().is_empty());
}

// ---- Overscroll ----

#[test]
fn an_overscrolling_pane_resists_past_the_end_and_springs_back() {
    let l = list_with(
        10,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical).with_overscroll(Overscroll::Px(50.0)),
    );
    l.router.advance(0.0);
    l.router.deliver_mouse(down(100.0, 10.0));
    l.router.deliver_mouse(drag([100.0, 10.0], 100.0, 20.0));
    l.router.deliver_mouse(drag([100.0, 10.0], 100.0, 120.0));
    let over = l.state().overscroll();
    assert!(over < -10.0 && over > -50.0, "{over}");
    assert_eq!(l.offset(), 0.0);
    // Hits follow the drawn content: row 0 is now lower on the screen.
    l.router.deliver_mouse(up(100.0, 120.0));
    let mut frames = 0;
    while l.state().overscroll() != 0.0 {
        l.router.advance(0.016);
        frames += 1;
        assert!(frames < 120, "it never sprang back");
    }
    assert_eq!(l.offset(), 0.0);
}

#[test]
fn a_fraction_overscroll_is_a_share_of_the_viewport() {
    let l = list_with(
        10,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical).with_overscroll(Overscroll::Fraction(0.25)),
    );
    l.router.deliver_mouse(down(100.0, 10.0));
    l.router.deliver_mouse(drag([100.0, 10.0], 100.0, 20.0));
    l.router.deliver_mouse(drag([100.0, 10.0], 100.0, 2000.0));
    let over = l.state().overscroll();
    assert!(over < -20.0 && over > -25.0, "{over}");
}

#[test]
fn without_overscroll_a_pull_at_the_start_moves_nothing() {
    let l = list(10);
    l.router.deliver_mouse(down(100.0, 10.0));
    l.router.deliver_mouse(drag([100.0, 10.0], 100.0, 60.0));
    l.router.deliver_mouse(drag([100.0, 10.0], 100.0, 90.0));
    assert_eq!((l.offset(), l.state().overscroll()), (0.0, 0.0));
}

// ---- Wheel and keys ----

#[test]
fn a_wheel_and_a_two_finger_drag_scroll_the_same_way() {
    let l = list(10);
    let resp = l.router.deliver_mouse(MouseEvent::Scroll {
        pos: [100.0, 50.0],
        delta: [0.0, -1.0],
    });
    assert_eq!(resp.target, Some(l.pane));
    assert_eq!(l.offset(), 48.0, "one notch, towards the end");
    l.router.deliver_mouse(MouseEvent::ScrollPixels {
        pos: [100.0, 50.0],
        delta: [0.0, -30.0],
    });
    assert_eq!(l.offset(), 78.0);
    l.router.deliver_mouse(MouseEvent::ScrollPixels {
        pos: [100.0, 50.0],
        delta: [0.0, 30.0],
    });
    assert_eq!(l.offset(), 48.0, "and back");
}

#[test]
fn a_wheel_over_a_slider_moves_the_slider_not_the_pane() {
    let mut router = Router::new();
    let pane = router.add(None, ScrollPane::new(VIEWPORT, ScrollAxis::Vertical));
    let slider = router.add(
        Some(pane),
        Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 10.0, 200.0, 20.0)),
    );
    for i in 1..10 {
        router.add(Some(pane), Button::new("r", row(i)));
    }
    render(&router);
    let resp = router.deliver_mouse(MouseEvent::Scroll {
        pos: [100.0, 20.0],
        delta: [0.0, -1.0],
    });
    assert_eq!(resp.target, Some(slider));
    let offset = router.get::<ScrollPane>(pane).unwrap().state().offset();
    assert_eq!(offset, [0.0, 0.0]);
}

#[test]
fn page_keys_move_the_pane_around_the_focus() {
    let l = list(10);
    l.router.set_focus(Some(l.rows[0]));
    l.router.deliver_keyboard(key(Key::PageDown));
    assert_eq!(l.offset(), 52.0, "a viewport less one line");
    l.router.deliver_keyboard(key(Key::Down));
    assert_eq!(l.offset(), 100.0);
    l.router.deliver_keyboard(key(Key::End));
    assert_eq!(l.offset(), 300.0);
    l.router.deliver_keyboard(key(Key::Home));
    assert_eq!(l.offset(), 0.0);
}

#[test]
fn tab_brings_the_focus_into_view() {
    let l = list(10);
    l.router.set_focus(Some(l.rows[2]));
    assert_eq!(l.offset(), 20.0, "row 2 ends at 120");
    l.router.deliver_keyboard(key(Key::Tab));
    assert_eq!(l.router.focused(), Some(l.rows[3]));
    assert_eq!(l.offset(), 60.0);
    l.router.set_focus(Some(l.rows[0]));
    assert_eq!(l.offset(), 0.0);
}

// ---- Anchoring ----

/// Move every row from `from` on down by `by` (negative: up).
fn shift_rows(l: &mut List, from: usize, by: f32) {
    for &id in &l.rows[from..] {
        let b = l.router.get_mut::<Button>(id).unwrap();
        b.rect.y += by;
    }
}

#[test]
fn content_added_above_the_view_does_not_move_what_is_seen() {
    let mut l = list(10);
    l.router.scroll_to(l.pane, [0.0, 100.0]);
    render(&l.router);
    shift_rows(&mut l, 0, ROW);
    let new = l.router.add(Some(l.pane), Button::new("new", row(0)));
    render(&l.router);
    assert_eq!(l.offset(), 140.0);
    assert_eq!(
        l.router.visible_bounds(l.rows[2]).map(|r| r.y),
        Some(0.0),
        "row 2 is where it was, its top 20 px above the viewport"
    );
    assert!(l.router.visible_bounds(new).is_none());
}

#[test]
fn removing_the_top_child_anchors_to_the_next() {
    let mut l = list(10);
    l.router.scroll_to(l.pane, [0.0, 90.0]);
    render(&l.router);
    // Row 2 (80..120) is at the top; it leaves and the rest move up.
    l.router.remove(l.rows[2]);
    shift_rows(&mut l, 3, -ROW);
    render(&l.router);
    assert_eq!(l.offset(), 50.0);
    assert_eq!(
        l.router.visible_bounds(l.rows[3]).map(|r| r.y),
        Some(30.0),
        "row 3 stays at 30 on the screen"
    );
}

#[test]
fn anchoring_can_be_turned_off() {
    let mut l = list_with(
        10,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical).with_anchoring(false),
    );
    l.router.scroll_to(l.pane, [0.0, 100.0]);
    render(&l.router);
    shift_rows(&mut l, 0, ROW);
    render(&l.router);
    assert_eq!(l.offset(), 100.0);
}

// ---- The indicator ----

fn indicator_count(list: &DrawList) -> usize {
    list.instances
        .iter()
        .filter(|i| {
            matches!(i.kind, SdfKind::RoundedBox { radius } if radius == 2.0) && i.size[0] == 4.0
        })
        .count()
}

#[test]
fn the_indicator_shows_while_moving_then_fades() {
    let l = list(10);
    assert_eq!(indicator_count(&render(&l.router)), 0, "never moved");
    l.router.scroll_to(l.pane, [0.0, 100.0]);
    assert_eq!(indicator_count(&render(&l.router)), 1);
    l.router.advance(0.4);
    assert_eq!(indicator_count(&render(&l.router)), 1, "kept while idle");
    l.router.advance(0.3);
    assert_eq!(indicator_count(&render(&l.router)), 0, "faded");
    assert!(!l.router.advance(0.016));
}

#[test]
fn no_indicator_when_the_content_fits() {
    let l = list(2);
    l.router.scroll_to(l.pane, [0.0, 10.0]);
    assert_eq!(indicator_count(&render(&l.router)), 0);
}

// ---- Activation on release ----

#[test]
fn buttons_and_labels_activate_on_the_click_not_the_press() {
    let mut router = Router::new();
    let b = Button::new("b", Rect::new(0.0, 0.0, 50.0, 50.0));
    let clicks = Arc::clone(&b.state);
    router.add(None, b);
    router.add(None, Label::new("l", Rect::new(100.0, 0.0, 50.0, 50.0)));
    let resp = router.deliver_mouse(down(10.0, 10.0));
    assert_eq!(resp.action, None);
    assert!(!clicks.take_click());
    router.deliver_mouse(up(10.0, 10.0));
    let resp = router.deliver_mouse(click(10.0, 10.0));
    assert!(matches!(resp.action, Some(ControlAction::Activated { .. })));
    assert!(clicks.take_click());
    let resp = router.deliver_event(InputEvent::Mouse(down(110.0, 10.0)));
    assert_eq!(resp.action, None);
    let resp = router.deliver_event(InputEvent::Mouse(click(110.0, 10.0)));
    assert!(matches!(resp.action, Some(ControlAction::Activated { .. })));
}

// ---- Plan 20261119_01, step 4: the fling profile and two fingers ----

/// A context drawing with `fling` as the theme's profile.
fn themed(fling: FlingProfile) -> DrawContext<'static> {
    // ScrollTheme is #[non_exhaustive]: set the field on a default.
    let mut theme = ControlTheme::default();
    theme.scroll.fling = fling;
    DrawContext::default().with_theme(theme)
}

fn render_with(router: &Router, ctx: &DrawContext) {
    let mut list = DrawList::default();
    router.render(&mut list, ctx);
}

/// 0.11.0's fling, for the same flick: its release speed from the first
/// and last sample, then Euler steps of `dt` at decay 3.2 until under 30
/// px/s or at the end. Returns the distance it travels.
fn old_fling_distance(v: f32, dt: f32, room: f32) -> f32 {
    let mut fling = v.clamp(-8000.0, 8000.0);
    let mut d = 0.0f32;
    while fling.abs() >= 30.0 {
        let before = d;
        d = (d + fling * dt).min(room);
        fling *= (-3.2 * dt).exp();
        if d == before {
            break;
        }
    }
    d
}

#[test]
fn classic_flings_exactly_as_0_11_0() {
    let l = list(1000);
    render_with(&l.router, &themed(FlingProfile::classic()));
    flick(&l);
    l.router.deliver_mouse(up(100.0, 30.0));
    let lifted = l.offset();
    while l.router.advance(0.016) {}
    // The flick: 60 px over 6 frames of 16 ms.
    let expected = old_fling_distance(60.0 / 0.096, 0.016, 1e9);
    assert!(
        (l.offset() - lifted - expected).abs() < 0.01,
        "{} vs {expected}",
        l.offset() - lifted
    );
    // A rest before lifting does not fling, and a touch stops it, as in
    // a_finger_that_rests_before_lifting_does_not_fling and
    // a_touch_during_a_fling_stops_it_and_is_not_a_tap.
    let l = list(100);
    render_with(&l.router, &themed(FlingProfile::classic()));
    flick(&l);
    l.router.advance(0.2);
    l.router.deliver_mouse(up(100.0, 30.0));
    let lifted = l.offset();
    l.router.advance(0.1);
    assert_eq!(l.offset(), lifted);
    let l = list(100);
    render_with(&l.router, &themed(FlingProfile::classic()));
    flick(&l);
    l.router.deliver_mouse(up(100.0, 30.0));
    l.router.advance(0.016);
    l.router.deliver_mouse(down(100.0, 50.0));
    let held = l.offset();
    l.router.advance(0.1);
    assert_eq!(l.offset(), held);
}

/// Fling a fresh 1000-row list at `fling` with a steady flick of 10 px a
/// 16 ms frame, then advance by `dt` until it stops. Returns the
/// distance travelled after the lift.
fn fling_distance(fling: FlingProfile, dt: f32) -> f32 {
    let l = list(1000);
    render_with(&l.router, &themed(fling));
    l.router.scroll_to(l.pane, [0.0, 10_000.0]);
    flick(&l);
    l.router.deliver_mouse(up(100.0, 30.0));
    let lifted = l.offset();
    let mut frames = 0;
    while l.router.advance(dt) {
        frames += 1;
        assert!(frames < 100_000);
    }
    l.offset() - lifted
}

#[test]
fn the_default_fling_covers_the_same_distance_at_60_and_10_frames_a_second() {
    let fast = fling_distance(FlingProfile::flankr(), 1.0 / 60.0);
    let slow = fling_distance(FlingProfile::flankr(), 0.1);
    assert!(fast > 100.0, "{fast}");
    // Plain steps of at most dt_max (1/30 s): within 2% (CHANGES.md).
    assert!((fast - slow).abs() / fast < 0.02, "{fast} vs {slow}");
}

/// A steady drag on `l` with `fingers` down: `steps` moves of `dy` px,
/// a 16 ms frame each, from y 90; returns the last y.
fn steady(l: &List, fingers: u8, steps: usize, dy: f32) -> f32 {
    l.router.set_fingers(fingers);
    l.router.deliver_mouse(down(100.0, 90.0));
    let mut y = 90.0;
    for _ in 0..steps {
        l.router.advance(0.016);
        y += dy;
        l.router.deliver_mouse(drag([100.0, 90.0], 100.0, y));
    }
    y
}

#[test]
fn two_fingers_drag_five_times_and_one_again_once() {
    let l = list(1000);
    render(&l.router);
    l.router.scroll_to(l.pane, [0.0, 10_000.0]);
    l.router.set_fingers(1);
    l.router.deliver_mouse(down(100.0, 90.0));
    l.router.set_fingers(2);
    // Past the slop: the pane takes the press at y 70.
    l.router.deliver_mouse(drag([100.0, 90.0], 100.0, 70.0));
    let taken = l.offset();
    l.router.deliver_mouse(drag([100.0, 90.0], 100.0, 60.0));
    assert_eq!(l.offset(), taken + 50.0, "10 px at 5x");
    l.router.set_fingers(1);
    l.router.deliver_mouse(drag([100.0, 90.0], 100.0, 50.0));
    assert_eq!(l.offset(), taken + 60.0, "1x from where it is");
    l.router.advance(0.5);
    l.router.set_fingers(0);
    l.router.deliver_mouse(up(100.0, 50.0));

    // An override of 4.0.
    let l = list_with(
        1000,
        ScrollPane::new(VIEWPORT, ScrollAxis::Vertical)
            .with_fling(FlingOptions::new().two_finger_factor(4.0)),
    );
    l.router.scroll_to(l.pane, [0.0, 10_000.0]);
    l.router.set_fingers(2);
    l.router.deliver_mouse(down(100.0, 90.0));
    l.router.deliver_mouse(drag([100.0, 90.0], 100.0, 70.0));
    let taken = l.offset();
    l.router.deliver_mouse(drag([100.0, 90.0], 100.0, 60.0));
    assert_eq!(l.offset(), taken + 40.0, "10 px at 4x");
    let profile = l
        .router
        .get::<ScrollPane>(l.pane)
        .unwrap()
        .fling_profile(&ScrollTheme::default());
    assert_eq!(profile.two_finger_factor, 4.0);
    assert_eq!(
        profile.decay,
        FlingProfile::flankr().decay,
        "the rest the theme's"
    );
}

/// The first frame's movement after a lift: the fling's speed times dt.
fn first_frame_after(l: &List) -> f32 {
    let lifted = l.offset();
    l.router.advance(1.0 / 60.0);
    (l.offset() - lifted) * 60.0
}

#[test]
fn a_pair_lifted_together_flings_five_times_and_apart_once() {
    // 10 px a frame up: 625 px/s, under the knee.
    let one = {
        let l = list(1000);
        render(&l.router);
        l.router.scroll_to(l.pane, [0.0, 10_000.0]);
        steady(&l, 1, 6, -10.0);
        l.router.set_fingers(0);
        l.router.deliver_mouse(up(100.0, 30.0));
        first_frame_after(&l)
    };
    assert!((one - 625.0).abs() < 2.0, "{one}");
    let pair = |apart: f32| {
        let l = list(1000);
        render(&l.router);
        l.router.scroll_to(l.pane, [0.0, 10_000.0]);
        l.router.set_fingers(1);
        l.router.deliver_mouse(down(100.0, 90.0));
        l.router.set_fingers(2);
        let mut y = 90.0;
        for _ in 0..6 {
            l.router.advance(0.016);
            y -= 10.0;
            l.router.deliver_mouse(drag([100.0, 90.0], 100.0, y));
        }
        l.router.set_fingers(1);
        if apart > 0.0 {
            // The other finger keeps moving at the same speed meanwhile.
            let frames = (apart / 0.016).round() as usize;
            for _ in 0..frames {
                l.router.advance(0.016);
                y -= 10.0;
                l.router.deliver_mouse(drag([100.0, 90.0], 100.0, y));
            }
        } else {
            l.router.advance(0.001);
        }
        l.router.set_fingers(0);
        l.router.deliver_mouse(up(100.0, y));
        first_frame_after(&l)
    };
    let together = pair(0.0);
    assert!(
        (together - 5.0 * one).abs() < 10.0,
        "{together} vs 5 x {one}"
    );
    let apart = pair(0.192);
    assert!((apart - one).abs() < 2.0, "{apart} vs {one}");
}

#[test]
fn a_two_finger_fling_is_capped_at_max_speed() {
    let l = list(2000);
    render(&l.router);
    l.router.scroll_to(l.pane, [0.0, 10_000.0]);
    l.router.set_fingers(2);
    l.router.deliver_mouse(down(100.0, 90.0));
    let mut y = 90.0;
    for _ in 0..6 {
        l.router.advance(0.016);
        y -= 60.0;
        l.router.deliver_mouse(drag([100.0, 90.0], 100.0, y));
    }
    l.router.set_fingers(0);
    l.router.deliver_mouse(up(100.0, y));
    let v = first_frame_after(&l);
    assert!((v - 16_000.0).abs() < 1.0, "{v}");
}

#[test]
fn a_two_finger_press_on_a_row_is_no_tap() {
    let l = list(10);
    l.router.set_fingers(1);
    l.router.deliver_mouse(down(100.0, 10.0));
    l.router.set_fingers(2);
    l.router.set_fingers(0);
    l.router.deliver_mouse(up(100.0, 10.0));
    l.router.deliver_mouse(click(100.0, 10.0));
    assert!(l.clicked().is_empty());
    l.router.set_fingers(1);
    l.router.deliver_mouse(down(100.0, 10.0));
    l.router.set_fingers(0);
    l.router.deliver_mouse(up(100.0, 10.0));
    l.router.deliver_mouse(click(100.0, 10.0));
    assert_eq!(l.clicked(), [0]);
}
