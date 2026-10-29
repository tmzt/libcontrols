//! PopupMenu through the router: opened by a call on its state, drawn as a
//! layer of the modal stack, chosen by pointer or keys, dismissed by an
//! outside tap, Escape or Back, placed to fit, themed
//! (docs/design/20261028_06_POPUP_MENU_PLAN.md); toggle items and rows of
//! cells in shared columns
//! (docs/design/20261028_10_POPUP_MENU_TOGGLES_AND_COLUMNS_PLAN.md).

use std::sync::Arc;

use libcontrols::{
    Button, ControlAction, ControlPart, ControlTheme, DismissReason, DrawContext, Key,
    KeyboardEvent, ModalError, ModalStack, ModalState, MouseButton, MouseEvent, PopupCell,
    PopupCellWidth, PopupItem, PopupMenu, PopupMenuEvent, PopupMenuOptions, PopupMenuState,
    PopupMenuTheme, PopupRow, Rect, Router, Symbol, TextAlign,
};
use libmsdf::drawlist::{DrawList, SdfKind};

const VIEWPORT: Rect = Rect::new(0.0, 0.0, 400.0, 600.0);
/// A ⋮ at the top right.
const ANCHOR: Rect = Rect::new(350.0, 8.0, 42.0, 42.0);
const ID: &str = "menu";

// With the default theme and no shaper (0.55 em labels), the menu for
// `items()` is 160 wide (min_width), right-aligned to the anchor at x 232,
// 4 px below it at y 54, 6 px padding: items at x 238..386, y 60..100
// (One), 100..140 (Two, disabled), 140..180 (heading), 180..189
// (separator), 189..229 (Three, checked); the frame is 181 high.
const ONE: [f32; 2] = [300.0, 80.0];
const TWO: [f32; 2] = [300.0, 120.0];
const HEADING: [f32; 2] = [300.0, 160.0];
const SEPARATOR: [f32; 2] = [300.0, 184.0];
const THREE: [f32; 2] = [300.0, 209.0];
const OUTSIDE: [f32; 2] = [60.0, 320.0];
const BUTTON_RECT: Rect = Rect::new(10.0, 300.0, 100.0, 40.0);

fn items() -> Vec<PopupItem> {
    vec![
        PopupItem::action("a/one", "One"),
        PopupItem::action("a/two", "Two").with_enabled(false),
        PopupItem::heading("Plugin"),
        PopupItem::separator(),
        PopupItem::action("a/three", "Three").with_checked(Some(true)),
    ]
}

struct Fixture {
    router: Router,
    modal: Arc<ModalState>,
    state: Arc<PopupMenuState>,
    button: Arc<libcontrols::ButtonState>,
}

fn fixture_with(options: PopupMenuOptions) -> Fixture {
    let mut router = Router::new();
    let button = Button::new("Beneath", BUTTON_RECT);
    let button_state = button.state.clone();
    router.add(None, button);
    let modal = Arc::new(ModalState::new());
    ModalStack::install(&mut router, modal.clone(), VIEWPORT).unwrap();
    let menu = PopupMenu::new(VIEWPORT).with_options(options);
    let state = menu.state.clone();
    router.attach_popup_menu(ID, menu).unwrap();
    Fixture {
        router,
        modal,
        state,
        button: button_state,
    }
}

fn fixture() -> Fixture {
    fixture_with(PopupMenuOptions::default())
}

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

fn render_with(router: &Router, ctx: &DrawContext) -> DrawList {
    let mut list = DrawList::new();
    router.render(&mut list, ctx);
    list
}

fn render(router: &Router) -> DrawList {
    render_with(router, &DrawContext::default())
}

fn has_bg(list: &DrawList, color: [f32; 4]) -> bool {
    list.instances.iter().any(|i| i.color == color)
}

fn lines(list: &DrawList) -> usize {
    list.instances
        .iter()
        .filter(|i| matches!(i.kind, SdfKind::Line { .. }))
        .count()
}

fn chosen(id: &str) -> PopupMenuEvent {
    PopupMenuEvent::Chosen(Symbol::intern(id))
}

#[test]
fn open_by_call_puts_it_on_the_stack_and_draws_it_without_a_backdrop() {
    let f = fixture();
    let bg = PopupMenuTheme::default().bg;
    assert!(!has_bg(&render(&f.router), bg));
    assert!(!f.state.is_open());

    f.state.open(ANCHOR, items());
    assert!(f.state.is_open());
    assert!(f.modal.contains(ID));
    let list = render(&f.router);
    let frame = f.state.layout().expect("laid out by the draw").frame;
    assert_eq!(frame, Rect::new(232.0, 54.0, 160.0, 181.0));
    let bg_box = list
        .instances
        .iter()
        .find(|i| i.color == bg)
        .expect("the menu's background");
    assert_eq!(bg_box.position, frame.pos());
    assert_eq!(bg_box.size, frame.size());
    // No backdrop: nothing covers the viewport.
    assert!(
        !list
            .instances
            .iter()
            .any(|i| i.position == VIEWPORT.pos() && i.size == VIEWPORT.size())
    );
    // The check mark (2) and the separator (1).
    assert_eq!(lines(&list), 3);
    assert!(f.state.take_events().is_empty());
}

#[test]
fn close_by_call_removes_it_and_queues_nothing() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    render(&f.router);
    f.state.close();
    assert!(!f.state.is_open());
    assert!(!f.modal.contains(ID));
    assert!(!has_bg(&render(&f.router), PopupMenuTheme::default().bg));
    assert!(f.state.take_events().is_empty());
}

#[test]
fn tap_on_an_action_chooses_it_and_closes() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    render(&f.router);
    let action = tap(&f.router, ONE);
    assert_eq!(
        action,
        Some(ControlAction::Activated {
            part: ControlPart::Custom(0)
        })
    );
    assert_eq!(f.state.take_events(), vec![chosen("a/one")]);
    assert!(!f.state.is_open());
    assert!(!f.modal.contains(ID));
    assert!(!has_bg(&render(&f.router), PopupMenuTheme::default().bg));
    // The tap ended with the menu: the button beneath saw none of it.
    assert!(!f.button.take_click());
}

#[test]
fn disabled_items_headings_and_separators_take_no_tap() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    render(&f.router);
    for p in [TWO, HEADING, SEPARATOR] {
        assert_eq!(tap(&f.router, p), None);
        assert!(f.state.is_open(), "still open after a tap at {p:?}");
    }
    assert!(f.state.take_events().is_empty());
    tap(&f.router, THREE);
    assert_eq!(f.state.take_events(), vec![chosen("a/three")]);
}

#[test]
fn a_press_that_slides_to_another_item_chooses_nothing() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    render(&f.router);
    let button = MouseButton::Left;
    f.router
        .deliver_mouse(MouseEvent::Down { pos: ONE, button });
    f.router
        .deliver_mouse(MouseEvent::Up { pos: THREE, button });
    f.router
        .deliver_mouse(MouseEvent::Click { pos: THREE, button });
    assert!(f.state.is_open());
    assert!(f.state.take_events().is_empty());
}

#[test]
fn keys_move_the_highlight_over_enabled_actions_and_enter_chooses() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    render(&f.router);
    assert_eq!(f.state.highlighted(), None);
    key(&f.router, Key::Down);
    assert_eq!(f.state.highlighted(), Some(0));
    // Skips the disabled action, the heading and the separator.
    key(&f.router, Key::Down);
    assert_eq!(f.state.highlighted(), Some(4));
    // Wraps.
    key(&f.router, Key::Down);
    assert_eq!(f.state.highlighted(), Some(0));
    key(&f.router, Key::Up);
    assert_eq!(f.state.highlighted(), Some(4));
    key(&f.router, Key::Home);
    assert_eq!(f.state.highlighted(), Some(0));
    key(&f.router, Key::End);
    assert_eq!(f.state.highlighted(), Some(4));
    // Tab is the menu's too while it is up, and does nothing.
    key(&f.router, Key::Tab);
    assert_eq!(f.state.highlighted(), Some(4));
    assert!(key(&f.router, Key::Enter).is_some());
    assert_eq!(f.state.take_events(), vec![chosen("a/three")]);
    assert!(!f.state.is_open());
}

#[test]
fn up_from_no_highlight_goes_to_the_last_and_space_chooses() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    render(&f.router);
    key(&f.router, Key::Up);
    assert_eq!(f.state.highlighted(), Some(4));
    key(&f.router, Key::Up);
    assert_eq!(f.state.highlighted(), Some(0));
    key(&f.router, Key::Space);
    assert_eq!(f.state.take_events(), vec![chosen("a/one")]);
}

#[test]
fn enter_with_nothing_highlighted_does_nothing() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    render(&f.router);
    assert_eq!(key(&f.router, Key::Enter), None);
    assert!(f.state.is_open());
}

#[test]
fn hover_highlights_enabled_actions_only_and_draws_the_highlight() {
    let f = fixture();
    let highlight = PopupMenuTheme::default().highlight;
    f.state.open(ANCHOR, items());
    render(&f.router);
    f.router.deliver_mouse(MouseEvent::Move { pos: ONE });
    assert_eq!(f.state.highlighted(), Some(0));
    let list = render(&f.router);
    let h = list
        .instances
        .iter()
        .find(|i| i.color == highlight)
        .expect("a highlight");
    assert_eq!(h.position, [238.0, 60.0]);
    f.router.deliver_mouse(MouseEvent::Move { pos: TWO });
    assert_eq!(f.state.highlighted(), None);
    assert!(!has_bg(&render(&f.router), highlight));
    f.router.deliver_mouse(MouseEvent::Move { pos: THREE });
    assert_eq!(f.state.highlighted(), Some(4));
    // Leaving the menu clears it.
    f.router.deliver_mouse(MouseEvent::Move { pos: OUTSIDE });
    assert_eq!(f.state.highlighted(), None);
}

#[test]
fn tap_outside_dismisses_and_does_not_reach_the_screen_beneath() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    render(&f.router);
    let centre = BUTTON_RECT.center();
    tap(&f.router, centre);
    assert_eq!(f.state.take_events(), vec![PopupMenuEvent::Dismissed]);
    assert!(!f.state.is_open());
    assert!(!f.modal.contains(ID));
    assert!(!f.button.take_click());
    // Closed: the next tap reaches the button.
    tap(&f.router, centre);
    assert!(f.button.take_click());
}

#[test]
fn escape_and_back_dismiss() {
    for k in [Key::Escape, Key::Back] {
        let f = fixture();
        f.state.open(ANCHOR, items());
        render(&f.router);
        key(&f.router, k);
        assert_eq!(
            f.state.take_events(),
            vec![PopupMenuEvent::Dismissed],
            "{k:?}"
        );
        assert!(!f.modal.contains(ID));
    }
}

#[test]
fn the_apps_stack_calls_dismiss_it_too() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    f.modal.dismiss_all(DismissReason::App);
    assert!(!f.state.is_open());
    assert_eq!(f.state.take_events(), vec![PopupMenuEvent::Dismissed]);
}

#[test]
fn dismiss_on_can_keep_it_open_on_outside_taps() {
    let dismiss = libcontrols::DismissOn {
        outside_tap: false,
        ..Default::default()
    };
    let f = fixture_with(PopupMenuOptions::new().dismiss(dismiss));
    f.state.open(ANCHOR, items());
    render(&f.router);
    tap(&f.router, OUTSIDE);
    assert!(f.state.is_open());
    assert!(!f.button.take_click());
}

#[test]
fn it_opens_again_after_closing_and_reopening_replaces_items() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    render(&f.router);
    tap(&f.router, ONE);
    f.state.take_events();
    f.state
        .open(ANCHOR, vec![PopupItem::action("b/only", "Only")]);
    f.state
        .open(ANCHOR, vec![PopupItem::action("c/new", "New")]);
    assert_eq!(f.modal.depth(), 1);
    render(&f.router);
    tap(&f.router, ONE);
    assert_eq!(f.state.take_events(), vec![chosen("c/new")]);
}

#[test]
fn it_flips_above_an_anchor_with_no_room_below() {
    let f = fixture();
    let anchor = Rect::new(350.0, 550.0, 42.0, 42.0);
    f.state.open(anchor, items());
    render(&f.router);
    let layout = f.state.layout().unwrap();
    assert!(layout.above);
    assert_eq!(layout.frame.bottom(), 546.0);
    assert_eq!(layout.frame.x, 232.0);
}

#[test]
fn it_shifts_inside_the_viewport_for_an_anchor_at_the_left() {
    let f = fixture();
    let anchor = Rect::new(0.0, 8.0, 42.0, 42.0);
    f.state.open(anchor, items());
    render(&f.router);
    let layout = f.state.layout().unwrap();
    assert!(!layout.above);
    assert_eq!(layout.frame.x, 8.0);
}

#[test]
fn a_long_label_widens_it_up_to_max_width() {
    let f = fixture();
    let long = "x".repeat(100);
    f.state
        .open(ANCHOR, vec![PopupItem::action("a/long", long)]);
    render(&f.router);
    assert_eq!(f.state.layout().unwrap().frame.w, 320.0);
}

#[test]
fn theme_and_options_resolve_and_reach_the_layout() {
    let mut theme = PopupMenuTheme::default();
    theme.item_height = 50.0;
    let options = PopupMenuOptions::new().bg([1.0, 0.0, 0.0, 1.0]);
    let resolved = options.with_theme(&theme);
    assert_eq!(resolved.item_height, 50.0);
    assert_eq!(resolved.bg, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(resolved.padding, theme.padding);

    let f = fixture_with(options);
    let ctx = DrawContext::default().with_theme(ControlTheme {
        popup_menu: theme,
        ..ControlTheme::default()
    });
    f.state.open(ANCHOR, items());
    let list = render_with(&f.router, &ctx);
    assert!(has_bg(&list, [1.0, 0.0, 0.0, 1.0]));
    let layout = f.state.layout().unwrap();
    assert_eq!(layout.items[0], Rect::new(238.0, 60.0, 148.0, 50.0));
    // One at y 60..110 under the theme: a tap at 105 chooses it.
    tap(&f.router, [300.0, 105.0]);
    assert_eq!(f.state.take_events(), vec![chosen("a/one")]);
}

#[test]
fn attach_needs_a_modal_stack() {
    let mut router = Router::new();
    assert_eq!(
        router.attach_popup_menu(ID, PopupMenu::new(VIEWPORT)),
        Err(ModalError::NoStack)
    );
}

#[test]
fn unattached_state_records_open_but_shows_nothing() {
    let state = PopupMenuState::new();
    state.open(ANCHOR, items());
    assert!(state.is_open());
    assert_eq!(state.items().len(), 5);
    assert_eq!(state.anchor(), ANCHOR);
    state.close();
    assert!(!state.is_open());
    assert!(state.take_events().is_empty());
}

// ---- Toggles (plan 20261028_10, step 1) ----
//
// `toggles()` is 160 wide (min_width) at x 232, y 54, with the check column
// reserved: rows at x 238..386, y 60..100 (A), 100..140 (B), 140..180 (C,
// disabled), 180..220 (X).

const A: [f32; 2] = [300.0, 80.0];
const B: [f32; 2] = [300.0, 120.0];
const C: [f32; 2] = [300.0, 160.0];
const X: [f32; 2] = [300.0, 200.0];

fn toggles() -> Vec<PopupItem> {
    vec![
        PopupItem::toggle("t/a", "A", false),
        PopupItem::toggle("t/b", "B", true),
        PopupItem::toggle("t/c", "C", true).with_enabled(false),
        PopupItem::action("a/x", "X"),
    ]
}

fn toggled(id: &str, checked: bool) -> PopupMenuEvent {
    PopupMenuEvent::Toggled {
        id: Symbol::intern(id),
        checked,
    }
}

#[test]
fn a_toggle_constructor_is_an_enabled_choosable_item() {
    let t = PopupItem::toggle("t/a", "A", false);
    assert_eq!(t.kind, libcontrols::PopupItemKind::Toggle);
    assert_eq!(t.checked, Some(false));
    assert!(t.enabled);
    assert!(t.is_choosable());
    assert!(!t.clone().with_enabled(false).is_choosable());
    assert!(!PopupItem::heading("H").is_choosable());
}

#[test]
fn a_tap_on_a_toggle_flips_it_and_keeps_the_menu_open() {
    let f = fixture();
    f.state.open(ANCHOR, toggles());
    let before = render(&f.router);
    let layout = f.state.layout().unwrap();
    let action = tap(&f.router, A);
    assert_eq!(
        action,
        Some(ControlAction::Activated {
            part: ControlPart::Custom(0)
        })
    );
    assert!(f.state.is_open());
    assert!(f.modal.contains(ID));
    assert_eq!(f.state.take_events(), vec![toggled("t/a", true)]);
    assert_eq!(f.state.items()[0].checked, Some(true));
    let after = render(&f.router);
    // A's check: two more lines.
    assert_eq!(lines(&after), lines(&before) + 2);
    // The frame and rects are unchanged by the flip.
    assert_eq!(f.state.layout().unwrap(), layout);

    tap(&f.router, A);
    assert_eq!(f.state.take_events(), vec![toggled("t/a", false)]);
    assert_eq!(f.state.items()[0].checked, Some(false));
    assert!(f.state.is_open());
    assert!(!f.button.take_click());
}

#[test]
fn enter_and_space_flip_the_highlighted_toggle_and_keep_the_highlight() {
    let f = fixture();
    let highlight = PopupMenuTheme::default().highlight;
    f.state.open(ANCHOR, toggles());
    render(&f.router);
    key(&f.router, Key::Down);
    key(&f.router, Key::Down);
    assert_eq!(f.state.highlighted(), Some(1));
    assert!(key(&f.router, Key::Enter).is_some());
    assert_eq!(f.state.take_events(), vec![toggled("t/b", false)]);
    assert_eq!(f.state.highlighted(), Some(1));
    assert!(f.state.is_open());
    let list = render(&f.router);
    let h = list
        .instances
        .iter()
        .find(|i| i.color == highlight)
        .expect("the highlight is still drawn");
    assert_eq!(h.position, [238.0, 100.0]);
    key(&f.router, Key::Space);
    assert_eq!(f.state.take_events(), vec![toggled("t/b", true)]);
    assert_eq!(f.state.items()[1].checked, Some(true));
    assert_eq!(f.state.highlighted(), Some(1));
}

#[test]
fn a_disabled_toggle_takes_no_tap_and_is_skipped_by_the_keys() {
    let f = fixture();
    f.state.open(ANCHOR, toggles());
    render(&f.router);
    assert_eq!(tap(&f.router, C), None);
    assert!(f.state.take_events().is_empty());
    assert_eq!(f.state.items()[2].checked, Some(true));
    key(&f.router, Key::Down);
    key(&f.router, Key::Down);
    key(&f.router, Key::Down);
    assert_eq!(f.state.highlighted(), Some(3));
}

#[test]
fn an_action_after_toggles_is_chosen_and_closes() {
    let f = fixture();
    f.state.open(ANCHOR, toggles());
    render(&f.router);
    tap(&f.router, A);
    tap(&f.router, B);
    tap(&f.router, X);
    assert_eq!(
        f.state.take_events(),
        vec![toggled("t/a", true), toggled("t/b", false), chosen("a/x")]
    );
    assert!(!f.state.is_open());
    assert!(!f.modal.contains(ID));
}

#[test]
fn toggles_then_an_outside_tap_queue_in_order() {
    let f = fixture();
    f.state.open(ANCHOR, toggles());
    render(&f.router);
    tap(&f.router, A);
    tap(&f.router, BUTTON_RECT.center());
    assert_eq!(
        f.state.take_events(),
        vec![toggled("t/a", true), PopupMenuEvent::Dismissed]
    );
    assert!(!f.button.take_click());
}

#[test]
fn a_toggle_with_no_check_state_flips_on_and_keeps_the_width() {
    let f = fixture();
    f.state.open(
        ANCHOR,
        vec![
            PopupItem::toggle("t/n", "Nothing yet", true).with_checked(None),
            PopupItem::action("a/x", "X"),
        ],
    );
    let before = render(&f.router);
    let layout = f.state.layout().unwrap();
    assert_eq!(lines(&before), 0);
    tap(&f.router, A);
    assert_eq!(f.state.take_events(), vec![toggled("t/n", true)]);
    assert_eq!(f.state.items()[0].checked, Some(true));
    assert_eq!(lines(&render(&f.router)), 2);
    assert_eq!(f.state.layout().unwrap(), layout);
}

// ---- Rows of cells (plan 20261028_10, step 2) ----
//
// With `max_width` 320 the frame is at x 72, rows at x 78..386, cells from
// x0 92 with A = 280 px of room; no shaper: 6.16 px a char at 0.8, 7.7 at
// 1.0.

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn assert_columns(got: &[[f32; 2]], want: &[[f32; 2]]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            approx(g[0], w[0]) && approx(g[1], w[1]),
            "{got:?} != {want:?}"
        );
    }
}

fn plan_row(id: &str, nn: &str, date: &str, title: &str, status: &str) -> PopupRow {
    PopupItem::action(id, format!("{nn} {date} {title} {status}")).with_cells(vec![
        PopupCell::new(nn).align(TextAlign::Right).scale(0.8),
        PopupCell::new(date).align(TextAlign::Right).scale(0.8),
        PopupCell::new(title).flex(),
        PopupCell::new(status).dim(true).scale(0.8),
    ])
}

fn plan_rows(t1: &str, t2: &str) -> Vec<PopupRow> {
    vec![
        plan_row("plan/7", "7", "10/28", t1, "Draft"),
        plan_row("plan/10", "10", "10/27", t2, "InImpl"),
    ]
}

const LONG: &str = "A much longer plan title that will not fit";

fn laid_out(options: PopupMenuOptions, rows: Vec<PopupRow>) -> libcontrols::PopupMenuLayout {
    let f = fixture_with(options);
    f.state.open_rows(ANCHOR, rows);
    render(&f.router);
    f.state.layout().unwrap()
}

#[test]
fn cells_lay_out_in_shared_columns_and_the_flex_column_takes_the_excess() {
    let layout = laid_out(PopupMenuOptions::default(), plan_rows("Short", LONG));
    assert_eq!(layout.frame.x, 72.0);
    assert_eq!(layout.frame.w, 320.0);
    assert_columns(
        &layout.columns,
        &[
            [92.0, 12.32],
            [116.32, 30.8],
            [159.12, 163.92],
            [335.04, 36.96],
        ],
    );
    let last = layout.columns[3];
    assert!(approx(last[0] + last[1], 372.0));
}

#[test]
fn short_cells_size_the_frame_and_extra_room_goes_to_the_flex_column() {
    let layout = laid_out(PopupMenuOptions::default(), plan_rows("Short", "Other"));
    assert!(approx(layout.frame.w, 194.58), "{}", layout.frame.w);
    let x0 = layout.frame.x + 20.0;
    assert_columns(
        &layout.columns,
        &[
            [x0, 12.32],
            [x0 + 24.32, 30.8],
            [x0 + 67.12, 38.5],
            [x0 + 117.62, 36.96],
        ],
    );

    let wide = laid_out(
        PopupMenuOptions::new().min_width(260.0),
        plan_rows("Short", "Other"),
    );
    assert_eq!(wide.frame.w, 260.0);
    assert!(approx(wide.columns[2][1], 38.5 + 65.42));
    assert!(approx(wide.columns[0][1], 12.32));
    assert!(approx(wide.columns[3][1], 36.96));
}

#[test]
fn with_no_flex_cell_the_extra_goes_to_the_last_column() {
    let rows = vec![PopupItem::action("r/1", "a b").with_cells(vec![
        PopupCell::new("a").align(TextAlign::Right),
        PopupCell::new("b").align(TextAlign::Right),
    ])];
    let layout = laid_out(PopupMenuOptions::new().min_width(200.0), rows);
    // 200 wide: 160 of room; "a" keeps 7.7, the gap 12, "b" the rest.
    assert_columns(
        &layout.columns,
        &[[layout.frame.x + 20.0, 7.7], [layout.frame.x + 39.7, 140.3]],
    );
}

#[test]
fn a_fixed_cell_fixes_its_column_in_every_row() {
    let rows = vec![
        PopupItem::action("r/1", "1").with_cells(vec![
            PopupCell::new("a very long text indeed").fixed(40.0),
            PopupCell::new("x"),
        ]),
        PopupItem::action("r/2", "2").with_cells(vec![PopupCell::new("y"), PopupCell::new("z")]),
    ];
    let layout = laid_out(PopupMenuOptions::default(), rows);
    assert!(approx(layout.columns[0][1], 40.0));
    assert_eq!(
        PopupCell::new("q").fixed(40.0).width,
        PopupCellWidth::Fixed(40.0)
    );
}

#[test]
fn plain_items_have_no_columns_and_open_rows_matches_open() {
    let f = fixture();
    f.state.open(ANCHOR, items());
    render(&f.router);
    let plain = f.state.layout().unwrap();
    assert!(plain.columns.is_empty());
    assert_eq!(plain.frame, Rect::new(232.0, 54.0, 160.0, 181.0));

    let g = fixture();
    g.state
        .open_rows(ANCHOR, items().into_iter().map(PopupRow::from).collect());
    render(&g.router);
    assert_eq!(g.state.layout().unwrap(), plain);
    assert_eq!(g.state.items(), f.state.items());
    assert_eq!(
        g.state.rows(),
        items().into_iter().map(PopupRow::new).collect::<Vec<_>>()
    );
    assert_eq!(
        f.state
            .rows()
            .into_iter()
            .map(|r| r.item)
            .collect::<Vec<_>>(),
        f.state.items()
    );
}

#[test]
fn the_column_theme_values_resolve() {
    let resolved = PopupMenuOptions::new()
        .column_gap(4.0)
        .with_theme(&PopupMenuTheme::default());
    assert_eq!(resolved.column_gap, 4.0);
    assert_eq!(resolved.dim_text_color, None);
    assert_eq!(PopupMenuTheme::default().column_gap, 12.0);
    let mut theme = PopupMenuTheme::default();
    theme.dim_text_color = Some([0.5, 0.5, 0.5, 1.0]);
    assert_eq!(
        PopupMenuOptions::new().with_theme(&theme).dim_text_color,
        Some([0.5, 0.5, 0.5, 1.0])
    );
    let over = PopupMenuOptions::new().dim_text_color([1.0, 0.0, 0.0, 1.0]);
    assert_eq!(
        over.with_theme(&theme).dim_text_color,
        Some([1.0, 0.0, 0.0, 1.0])
    );
    // A smaller gap reaches the layout.
    let layout = laid_out(
        PopupMenuOptions::new().column_gap(4.0),
        plan_rows("Short", "Other"),
    );
    assert!(approx(
        layout.columns[1][0] - layout.columns[0][0],
        12.32 + 4.0
    ));
}

// ---- Drawing cells through the router (plan 20261028_10, step 3) ----

#[test]
fn a_toggle_row_with_cells_flips_and_draws_its_check() {
    let f = fixture();
    f.state.open_rows(
        ANCHOR,
        vec![
            PopupItem::toggle("t/a", "A on", false)
                .with_cells(vec![PopupCell::new("A"), PopupCell::new("on").dim(true)]),
        ],
    );
    assert_eq!(lines(&render(&f.router)), 0);
    tap(&f.router, A);
    assert_eq!(f.state.take_events(), vec![toggled("t/a", true)]);
    assert!(f.state.is_open());
    assert_eq!(lines(&render(&f.router)), 2);
    assert_eq!(f.state.rows()[0].item.checked, Some(true));
    assert_eq!(f.state.rows()[0].cells.len(), 2);
}

#[test]
fn a_heading_row_of_cells_takes_no_input_and_separator_cells_make_no_columns() {
    let f = fixture();
    f.state.open_rows(
        ANCHOR,
        vec![
            PopupItem::heading("NN Title")
                .with_cells(vec![PopupCell::new("NN"), PopupCell::new("Title")]),
            PopupItem::separator().with_cells(vec![
                PopupCell::new("a"),
                PopupCell::new("b"),
                PopupCell::new("c"),
            ]),
            PopupItem::action("a/x", "X").into(),
        ],
    );
    let list = render(&f.router);
    // The separator's line only.
    assert_eq!(lines(&list), 1);
    assert_eq!(f.state.layout().unwrap().columns.len(), 2);
    assert_eq!(tap(&f.router, A), None);
    assert!(f.state.is_open());
    assert!(f.state.take_events().is_empty());
}
