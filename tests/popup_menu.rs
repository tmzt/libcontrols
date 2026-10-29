//! PopupMenu through the router: opened by a call on its state, drawn as a
//! layer of the modal stack, chosen by pointer or keys, dismissed by an
//! outside tap, Escape or Back, placed to fit, themed
//! (docs/design/20261028_06_POPUP_MENU_PLAN.md).

use std::sync::Arc;

use libcontrols::{
    Button, ControlAction, ControlPart, ControlTheme, DismissReason, DrawContext, Key,
    KeyboardEvent, ModalError, ModalStack, ModalState, MouseButton, MouseEvent, PopupItem,
    PopupMenu, PopupMenuEvent, PopupMenuOptions, PopupMenuState, PopupMenuTheme, Rect, Router,
    Symbol,
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
