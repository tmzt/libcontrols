//! The drawer's and the layout's behaviour (plans 01 and 04).

use libmsdf::drawlist::{DrawList, SdfKind};

use super::composite::shade;
use super::*;
use crate::button::Button;
use crate::event::{ControlAction, Key, KeyboardEvent, MouseButton};
use crate::scroll::{ScrollAxis, ScrollPane};

const VP: Rect = Rect::new(0.0, 0.0, 400.0, 800.0);

fn down(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Down {
        pos: [x, y],
        button: MouseButton::Left,
    })
}

fn up(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Up {
        pos: [x, y],
        button: MouseButton::Left,
    })
}

fn click(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Click {
        pos: [x, y],
        button: MouseButton::Left,
    })
}

fn key(k: Key) -> InputEvent {
    InputEvent::Keyboard(KeyboardEvent::KeyDown { key: k })
}

/// Down, Up and Click at a point: the routes, and whether any answer
/// activated a control.
fn tap(l: &DrawerLayout, x: f32, y: f32) -> (Vec<DrawerRoute>, bool) {
    let routes: Vec<_> = [down(x, y), up(x, y), click(x, y)]
        .into_iter()
        .map(|e| l.deliver(e))
        .collect();
    let activated = routes.iter().any(|r| match r {
        DrawerRoute::Main(Some(r)) | DrawerRoute::Drawer(Some(r)) => {
            matches!(r.action, Some(ControlAction::Activated { .. }))
        }
        _ => false,
    });
    (routes, activated)
}

fn is_main(r: &DrawerRoute) -> bool {
    matches!(r, DrawerRoute::Main(Some(_)))
}

fn is_drawer(r: &DrawerRoute) -> bool {
    matches!(r, DrawerRoute::Drawer(Some(_)))
}

/// A layout with a main button at (20, 20) 100 x 40 and a content button at
/// sheet (20, 20) 100 x 40: the same screen point when the Left drawer is
/// open.
fn layout(edge: DrawerEdge) -> (DrawerLayout, NodeId, NodeId) {
    let mut l = DrawerLayout::new(VP, edge, DrawerOptions::default());
    let main_btn = l.main_mut().add(
        None,
        Button::new("main", Rect::new(20.0, 20.0, 100.0, 40.0)),
    );
    let sheet_btn = l.content_mut().add(
        None,
        Button::new("sheet", Rect::new(20.0, 20.0, 100.0, 40.0)),
    );
    (l, main_btn, sheet_btn)
}

// ---- Theme resolution ----

#[test]
fn with_theme_takes_each_some_and_each_none_from_its_side() {
    let theme = DrawerTheme {
        scrim: [0.1, 0.2, 0.3, 0.5],
        corner_radius: 4.0,
        keep_warm: 1.0,
        ..DrawerTheme::default()
    };
    let r = DrawerOptions::new()
        .corner_radius(9.0)
        .sheet_color([1.0, 0.0, 0.0, 1.0])
        .with_theme(&theme);
    assert_eq!(r.corner_radius, 9.0);
    assert_eq!(r.sheet_color, Some([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(r.scrim, [0.1, 0.2, 0.3, 0.5]);
    assert_eq!(r.keep_warm, 1.0);
    assert_eq!(r.open_duration, 0.300);
    assert_eq!(r.open_curve, CubicBezier::EMPHASIZED_DECELERATE);
    assert_eq!(r.size, theme.size);
    assert_eq!(r.size_vertical, theme.size_vertical);
    let r = DrawerOptions::new()
        .size(DrawerSize::Px(200.0))
        .with_theme(&theme);
    assert_eq!(r.size_for(DrawerEdge::Left), DrawerSize::Px(200.0));
    assert_eq!(r.size_for(DrawerEdge::Bottom), DrawerSize::Px(200.0));
    assert_eq!(
        DrawerOptions::default().with_theme(&DrawerTheme::default()),
        DrawerOptions::new().with_theme(&ControlTheme::default().drawer)
    );
}

#[test]
fn a_theme_change_moves_unoverridden_values_only() {
    let d = Drawer::new(
        VP,
        DrawerEdge::Left,
        DrawerOptions::new().corner_radius(3.0),
    );
    let mut theme = ControlTheme::default();
    theme.drawer.corner_radius = 30.0;
    theme.drawer.scrim = [1.0, 1.0, 1.0, 0.5];
    theme.drawer.open_duration = 1.0;
    let rev = d.content_mark().revision();
    d.set_theme(&theme);
    assert!(
        d.content_mark().revision() > rev,
        "a theme change marks the sheet"
    );
    let r = d.resolved();
    assert_eq!(r.corner_radius, 3.0);
    assert_eq!(r.scrim, [1.0, 1.0, 1.0, 0.5]);
    assert_eq!(d.state().motion().open_duration, 1.0);
    let rev = d.content_mark().revision();
    d.set_theme(&theme);
    assert_eq!(
        d.content_mark().revision(),
        rev,
        "an unchanged theme does not"
    );
}

// ---- Geometry ----

#[test]
fn default_sizes() {
    assert_eq!(
        Drawer::new(VP, DrawerEdge::Left, DrawerOptions::default()).extent(),
        340.0
    );
    let wide = Rect::new(0.0, 0.0, 1000.0, 600.0);
    assert_eq!(
        Drawer::new(wide, DrawerEdge::Right, DrawerOptions::default()).extent(),
        360.0
    );
    assert_eq!(
        Drawer::new(wide, DrawerEdge::Top, DrawerOptions::default()).extent(),
        360.0
    );
    assert!(
        (Drawer::new(VP, DrawerEdge::Bottom, DrawerOptions::default()).extent() - 480.0).abs()
            < 1e-3
    );
    assert_eq!(DrawerSize::Px(900.0).extent(400.0), 400.0);
    assert_eq!(DrawerEdge::default(), DrawerEdge::Left);
}

#[test]
fn sheet_for_each_edge_at_0_half_and_1() {
    let vp = Rect::new(10.0, 20.0, 400.0, 800.0);
    let o = DrawerOptions::new().size(DrawerSize::Px(100.0));
    let at = |edge, t| Drawer::new(vp, edge, o).sheet_at(t);
    use DrawerEdge::*;
    assert_eq!(at(Left, 1.0), Rect::new(10.0, 20.0, 100.0, 800.0));
    assert_eq!(at(Left, 0.5), Rect::new(-40.0, 20.0, 100.0, 800.0));
    assert_eq!(at(Left, 0.0), Rect::new(-90.0, 20.0, 100.0, 800.0));
    assert_eq!(at(Right, 1.0), Rect::new(310.0, 20.0, 100.0, 800.0));
    assert_eq!(at(Right, 0.5), Rect::new(360.0, 20.0, 100.0, 800.0));
    assert_eq!(at(Right, 0.0), Rect::new(410.0, 20.0, 100.0, 800.0));
    assert_eq!(at(Top, 1.0), Rect::new(10.0, 20.0, 400.0, 100.0));
    assert_eq!(at(Top, 0.5), Rect::new(10.0, -30.0, 400.0, 100.0));
    assert_eq!(at(Top, 0.0), Rect::new(10.0, -80.0, 400.0, 100.0));
    assert_eq!(at(Bottom, 1.0), Rect::new(10.0, 720.0, 400.0, 100.0));
    assert_eq!(at(Bottom, 0.5), Rect::new(10.0, 770.0, 400.0, 100.0));
    assert_eq!(at(Bottom, 0.0), Rect::new(10.0, 820.0, 400.0, 100.0));
}

#[test]
fn closed_sheets_are_outside_the_layout_and_open_ones_inside() {
    for edge in [
        DrawerEdge::Left,
        DrawerEdge::Right,
        DrawerEdge::Top,
        DrawerEdge::Bottom,
    ] {
        let (l, ..) = layout(edge);
        let s = l.drawer().sheet();
        let r = l.rect();
        let outside =
            s.right() <= r.x || s.x >= r.right() || s.bottom() <= r.y || s.y >= r.bottom();
        assert!(outside, "{edge:?} closed: {s:?}");
        l.set_open_now(true);
        let s = l.drawer().sheet();
        assert!(s.x >= r.x && s.right() <= r.right() && s.y >= r.y && s.bottom() <= r.bottom());
    }
}

#[test]
fn set_rect_moves_the_viewport_keeps_t_and_marks() {
    let (l, ..) = layout(DrawerEdge::Left);
    l.set_progress(0.5);
    let (c, m) = (
        l.drawer().content_mark().revision(),
        l.drawer().main_mark().revision(),
    );
    l.set_rect(Rect::new(0.0, 0.0, 300.0, 600.0));
    assert_eq!(l.drawer().viewport(), Rect::new(0.0, 0.0, 300.0, 600.0));
    assert_eq!(l.state().progress(), 0.5);
    assert!(l.drawer().content_mark().revision() > c);
    assert!(l.drawer().main_mark().revision() > m);
    assert_eq!(l.drawer().extent(), 255.0);
}

// ---- Calls ----

#[test]
fn forwarded_calls_drive_the_shared_state() {
    let state = DrawerState::new();
    let (l, ..) = layout(DrawerEdge::Left);
    let l = l.with_state(state.clone());
    l.open();
    assert_eq!(state.phase(), DrawerPhase::Opening);
    l.advance(0.05);
    l.close();
    assert_eq!(state.phase(), DrawerPhase::Closing);
    l.toggle();
    assert_eq!(state.phase(), DrawerPhase::Opening);
    l.set_open_now(false);
    assert_eq!(state.phase(), DrawerPhase::Closed);
    l.set_progress(0.3);
    assert_eq!(
        (state.phase(), state.progress()),
        (DrawerPhase::Dragging, 0.3)
    );
    l.settle(true, 1.0);
    assert_eq!(state.phase(), DrawerPhase::Opening);
    while l.advance(0.016) {}
    assert_eq!((state.phase(), state.progress()), (DrawerPhase::Open, 1.0));
    l.set_open_now(false);
    assert_eq!(state.phase(), DrawerPhase::Closed);
}

#[test]
fn options_reach_the_states_motion() {
    let state = DrawerState::new();
    let d = Drawer::new(
        VP,
        DrawerEdge::Left,
        DrawerOptions::new().open_duration(1.0),
    )
    .with_state(state.clone());
    d.open();
    let mut n = 0;
    while d.advance(0.02) {
        n += 1;
    }
    assert!((49..=51).contains(&n), "{n} steps of 20 ms");
}

// ---- Routing ----

#[test]
fn closed_taps_reach_the_main_view() {
    let (l, ..) = layout(DrawerEdge::Left);
    let (routes, activated) = tap(&l, 50.0, 40.0);
    assert!(routes.iter().all(is_main));
    assert!(activated);
}

#[test]
fn open_taps_reach_the_content_in_sheet_coordinates_and_not_the_main_view() {
    let (l, main_btn, sheet_btn) = layout(DrawerEdge::Left);
    l.set_open_now(true);
    let (routes, activated) = tap(&l, 50.0, 40.0);
    assert!(routes.iter().all(is_drawer), "{routes:?}");
    assert!(activated);
    let DrawerRoute::Drawer(Some(r)) = &routes[0] else {
        unreachable!()
    };
    assert_eq!(r.target, Some(sheet_btn));
    assert_ne!(l.main().focused(), Some(main_btn));
}

#[test]
fn content_positions_follow_the_sheet() {
    // A Right drawer of 340 px in a 400 px viewport: the sheet starts at
    // x = 60, so the content button (sheet 20..120) is at screen 80..180.
    let (l, _, sheet_btn) = layout(DrawerEdge::Right);
    l.set_open_now(true);
    let (routes, activated) = tap(&l, 85.0, 40.0);
    assert!(activated);
    let DrawerRoute::Drawer(Some(r)) = &routes[0] else {
        panic!("{routes:?}")
    };
    assert_eq!(r.target, Some(sheet_btn));
    // Half open, the sheet is at x = 230: screen 85 is the scrim.
    l.set_progress(0.5);
    assert_eq!(l.deliver(down(85.0, 40.0)), DrawerRoute::Scrim);
    let (routes, activated) = tap(&l, 255.0, 40.0);
    assert!(activated && routes.iter().all(is_drawer));
}

#[test]
fn a_scrim_tap_is_scrim_and_reaches_nothing() {
    let (mut l, ..) = layout(DrawerEdge::Left);
    let far = l
        .main_mut()
        .add(None, Button::new("far", Rect::new(360.0, 20.0, 30.0, 40.0)));
    l.set_open_now(true);
    let (routes, activated) = tap(&l, 370.0, 40.0);
    assert_eq!(routes, vec![DrawerRoute::Scrim; 3]);
    assert!(!activated);
    assert_ne!(l.main().focused(), Some(far));
    // The app closes on it.
    l.close();
    assert_eq!(l.state().phase(), DrawerPhase::Closing);
}

#[test]
fn a_press_ends_where_it_began() {
    // A main press whose Down opened the drawer still ends on the main view.
    let (l, ..) = layout(DrawerEdge::Left);
    assert!(is_main(&l.deliver(down(50.0, 40.0))));
    l.set_open_now(true);
    let r_up = l.deliver(up(50.0, 40.0));
    let r_click = l.deliver(click(50.0, 40.0));
    assert!(is_main(&r_up) && is_main(&r_click));
    let activated = [r_up, r_click].iter().any(|r| match r {
        DrawerRoute::Main(Some(r)) => matches!(r.action, Some(ControlAction::Activated { .. })),
        _ => false,
    });
    assert!(activated, "the menu button activates on release");
    // And a content press that ends on the scrim stays with the content.
    assert!(is_drawer(&l.deliver(down(50.0, 40.0))));
    assert!(is_drawer(&l.deliver(up(370.0, 40.0))));
}

#[test]
fn keys_go_to_the_content_while_open_and_to_the_main_view_otherwise() {
    let (l, ..) = layout(DrawerEdge::Left);
    assert!(is_main(&l.deliver(key(Key::Tab))));
    l.open();
    assert!(is_drawer(&l.deliver(key(Key::Tab))));
    l.set_open_now(true);
    assert!(is_drawer(&l.deliver(key(Key::Tab))));
    l.close();
    assert!(is_main(&l.deliver(key(Key::Tab))));
    l.set_progress(0.0);
    assert!(is_main(&l.deliver(key(Key::Tab))));
    l.set_progress(0.2);
    assert!(is_drawer(&l.deliver(key(Key::Tab))));
}

#[test]
fn a_drawer_alone_says_main_none() {
    let d = Drawer::new(VP, DrawerEdge::Left, DrawerOptions::default());
    assert_eq!(d.deliver(down(10.0, 10.0)), DrawerRoute::Main(None));
    assert_eq!(d.deliver(key(Key::Enter)), DrawerRoute::Main(None));
    d.set_open_now(true);
    assert_eq!(d.deliver(down(380.0, 10.0)), DrawerRoute::Scrim);
    // Outside the viewport is not the drawer's.
    assert_eq!(d.deliver(down(500.0, 10.0)), DrawerRoute::Main(None));
}

#[test]
fn initial_focus_on_reaching_open() {
    let (l, _, sheet_btn) = layout(DrawerEdge::Left);
    let l = l.with_initial_focus(sheet_btn);
    l.open();
    l.advance(0.1);
    assert_eq!(l.content().focused(), None);
    while l.advance(0.1) {}
    assert_eq!(l.content().focused(), Some(sheet_btn));
}

// ---- Marks ----

#[test]
fn marks_follow_input_and_rest() {
    let (l, ..) = layout(DrawerEdge::Left);
    let m = l.drawer().main_mark().revision();
    tap(&l, 50.0, 40.0);
    assert!(
        l.drawer().main_mark().revision() > m,
        "main input marks main"
    );
    l.set_open_now(true);
    let c = l.drawer().content_mark().revision();
    tap(&l, 50.0, 40.0);
    assert!(
        l.drawer().content_mark().revision() > c,
        "content input marks the sheet"
    );
    let (c, m) = (
        l.drawer().content_mark().revision(),
        l.drawer().main_mark().revision(),
    );
    for _ in 0..10 {
        assert!(!l.advance(0.016));
    }
    assert_eq!(
        l.drawer().content_mark().revision(),
        c,
        "at rest nothing marks"
    );
    assert_eq!(l.drawer().main_mark().revision(), m);
    l.mark_main();
    assert!(l.drawer().main_mark().revision() > m);
}

// ---- Drawing ----

fn rendered(f: impl Fn(&mut DrawList)) -> DrawList {
    let mut list = DrawList::new();
    f(&mut list);
    list
}

#[test]
fn closed_render_equals_the_main_view_alone() {
    let (l, ..) = layout(DrawerEdge::Left);
    let ctx = DrawContext::default();
    let a = rendered(|list| l.render(list, &ctx));
    let b = rendered(|list| l.render_main(list, &ctx));
    assert_eq!(a.instances, b.instances);
    assert!(!a.instances.is_empty());
}

#[test]
fn open_render_adds_scrim_sheet_and_clipped_content() {
    let (l, ..) = layout(DrawerEdge::Left);
    l.set_progress(0.5);
    let ctx = DrawContext::default();
    let main_n = rendered(|list| l.render_main(list, &ctx)).instances.len();
    let list = rendered(|list| l.render(list, &ctx));
    let scrim = &list.instances[main_n];
    assert_eq!(scrim.kind, SdfKind::Box);
    assert_eq!(scrim.size, VP.size());
    assert!((scrim.color[3] - 0.32 * 0.5).abs() < 1e-6);
    let bg = &list.instances[main_n + 1];
    assert!(
        matches!(bg.kind, SdfKind::RoundedBoxPerCorner { radii } if radii == [0.0, 16.0, 16.0, 0.0])
    );
    assert_eq!(bg.color, ControlTheme::default().bg_color);
    // Everything of the content is inside one clip: the visible sheet.
    let sheet = l.drawer().sheet();
    assert_eq!(sheet.x, -170.0);
    let clip = list.instance_clip(main_n + 2).expect("content is clipped");
    for i in main_n + 2..list.instances.len() {
        assert_eq!(list.instance_clip(i), Some(clip));
    }
    assert!(list.instances.len() > main_n + 2);
}

#[test]
fn render_sheet_draws_in_sheet_coordinates() {
    let (l, ..) = layout(DrawerEdge::Right);
    l.set_open_now(true);
    let list = rendered(|list| l.render_sheet(list, &DrawContext::default()));
    assert_eq!(list.instances[0].position, [0.0, 0.0]);
    assert_eq!(list.instances[0].size, [340.0, 800.0]);
    assert!(
        matches!(list.instances[0].kind, SdfKind::RoundedBoxPerCorner { radii } if radii == [16.0, 0.0, 0.0, 16.0])
    );
    // The content button's first instance at its own (20, 20).
    assert!(
        list.instances[1..]
            .iter()
            .any(|i| i.position == [20.0, 20.0])
    );
}

#[test]
fn render_at_moves_a_scroll_panes_clip_by_the_origin() {
    let mut r = Router::new();
    let pane = r.add(
        None,
        ScrollPane::new(Rect::new(10.0, 10.0, 100.0, 100.0), ScrollAxis::Vertical)
            .with_content_size([100.0, 400.0]),
    );
    r.add(
        Some(pane),
        Button::new("x", Rect::new(10.0, 10.0, 80.0, 30.0)),
    );
    let ctx = DrawContext::default();
    let plain = rendered(|list| r.render(list, &ctx));
    let at = rendered(|list| r.render_at(list, &ctx, [200.0, 50.0]));
    let c0 = plain.instance_clip(0).unwrap();
    let c1 = at.instance_clip(0).unwrap();
    let moved = rendered(|list| {
        list.push_clip([210.0, 60.0], [100.0, 100.0]);
        list.push(plain.instances[0]);
        list.push_clip_end();
    });
    assert_ne!(c0, c1);
    assert_eq!(Some(c1), moved.instance_clip(0));
    let zero = rendered(|list| r.render_at(list, &ctx, [0.0, 0.0]));
    assert_eq!(zero.instances, plain.instances);
    assert_eq!(zero.instance_clip(0), Some(c0));
}

// ---- Composite parameters and the shader's math (CPU) ----

#[test]
fn composite_params_for_each_edge_at_2_625() {
    let s = 2.625;
    let o = DrawerOptions::new().size(DrawerSize::Px(100.0));
    let vp = Rect::new(0.0, 0.0, 400.0, 800.0);
    let d = Drawer::new(vp, DrawerEdge::Left, o);
    d.set_progress(0.25);
    let p = d.composite_params(s, (1050, 2100));
    assert_eq!(p.frame, [1050.0, 2100.0, 0.25, 0.0]);
    assert_eq!(p.viewport, [0.0, 0.0, 1050.0, 2100.0]);
    assert_eq!(p.sheet_open, [0.0, 0.0, 262.5, 2100.0]);
    assert_eq!(p.scrim, [0.0, 0.0, 0.0, 0.32]);
    assert_eq!(p.shadow, [42.0, 0.25, 0.0, 0.0]);
    let at = |edge| Drawer::new(vp, edge, o).composite_params(s, (1050, 2100));
    assert_eq!(
        at(DrawerEdge::Right).sheet_open,
        [787.5, 0.0, 262.5, 2100.0]
    );
    assert_eq!(at(DrawerEdge::Right).frame[3], 1.0);
    assert_eq!(at(DrawerEdge::Top).sheet_open, [0.0, 0.0, 1050.0, 262.5]);
    assert_eq!(
        at(DrawerEdge::Bottom).sheet_open,
        [0.0, 1837.5, 1050.0, 262.5]
    );
    assert_eq!(at(DrawerEdge::Bottom).frame[3], 3.0);
}

#[test]
fn shader_math_slides_the_sheet_and_scales_the_scrim() {
    let o = DrawerOptions::new()
        .size(DrawerSize::Px(100.0))
        .shadow((0.0, 0.0));
    let d = Drawer::new(Rect::new(0.0, 0.0, 400.0, 200.0), DrawerEdge::Left, o);
    let grey = |_| [0.5, 0.5, 0.5];
    let red = |_| [1.0, 0.0, 0.0, 1.0];
    // t = 0.5: the sheet's leading column is at extent / 2.
    d.set_progress(0.5);
    let u = d.composite_params(1.0, (400, 200));
    assert_eq!(shade(&u, [49.5, 10.5], grey, red), [1.0, 0.0, 0.0]);
    let after = shade(&u, [50.5, 10.5], grey, red);
    let want = 0.5 * (1.0 - 0.32 * 0.5);
    assert!((after[0] - want).abs() < 1e-6, "the scrim darkens by a * t");
    // The sheet is sampled at its own coordinates.
    let at = shade(&u, [10.5, 10.5], grey, |p| [p[0] / 100.0, 0.0, 0.0, 1.0]);
    // Screen 10.5 is sheet 60.5 (the sheet starts at -50).
    assert!((at[0] - 0.605).abs() < 1e-6, "{at:?}");
    // t = 0: the main view unchanged.
    d.set_open_now(false);
    let u = d.composite_params(1.0, (400, 200));
    assert_eq!(shade(&u, [0.5, 0.5], grey, red), [0.5, 0.5, 0.5]);
    // A translucent sheet is laid over by its straight alpha.
    d.set_open_now(true);
    let u = d.composite_params(1.0, (400, 200));
    let c = shade(&u, [0.5, 0.5], grey, |_| [0.25, 0.0, 0.0, 0.5]);
    let base = 0.5 * (1.0 - 0.32);
    assert!((c[0] - (0.25 * 0.5 + base * 0.5)).abs() < 1e-6);
}

#[test]
fn shader_math_shadow_falls_off_beyond_the_open_edge() {
    let o = DrawerOptions::new()
        .size(DrawerSize::Px(100.0))
        .scrim([0.0, 0.0, 0.0, 0.0])
        .shadow((10.0, 0.5));
    let d = Drawer::new(Rect::new(0.0, 0.0, 400.0, 200.0), DrawerEdge::Bottom, o);
    d.set_open_now(true);
    let u = d.composite_params(1.0, (400, 200));
    let white = |_| [1.0, 1.0, 1.0];
    let clear = |_| [0.0, 0.0, 0.0, 0.0];
    // Bottom sheet at y 100..200: its open edge is y = 100, shadow above it.
    let at_edge = shade(&u, [5.0, 100.0 - 0.0001], white, clear)[0];
    assert!((at_edge - 0.5).abs() < 1e-3, "{at_edge}");
    let mid = shade(&u, [5.0, 95.0], white, clear)[0];
    assert!((mid - (1.0 - 0.5 * 0.25)).abs() < 1e-5, "{mid}");
    assert_eq!(shade(&u, [5.0, 89.0], white, clear)[0], 1.0);
}
