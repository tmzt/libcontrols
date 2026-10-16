//! The modal stack: state, filter, modals and the hosted stack.

mod common;

use std::cell::RefCell;
use std::sync::Arc;

use common::{Call, Recorder};
use libcontrols::{
    Backdrop, Button, ContentReveal, ControlHandle, ControlHost, ControlKey, ControlPart,
    DismissOn, DismissReason, DrawContext, DrawControl, Edit, FocusEvent, GestureEvent,
    HostedControl, InputEvent, Key, KeyboardEvent, MAX_MODAL_DEPTH, Modal, ModalEvent, ModalFilter,
    ModalFrame, ModalPhase, ModalStack, ModalState, ModalTransition, ModalView, Modifiers,
    MouseButton, MouseEvent, Options, PanelFrame, Rect, Slider, Symbol, TooltipStyle, sym,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

const TIMED: ModalTransition = ModalTransition::Timed {
    open_secs: 0.2,
    close_secs: 0.1,
};

fn timed() -> ModalState {
    ModalState::new().with_transition(TIMED)
}

fn instant() -> ModalState {
    ModalState::new().with_transition(ModalTransition::Instant)
}

fn ids(state: &ModalState) -> Vec<(Symbol, ModalPhase, bool)> {
    state
        .stack()
        .iter()
        .map(|e| (e.id, e.phase, e.is_top))
        .collect()
}

#[test]
fn a_push_opens_over_the_top_and_leaves_the_lower_one_alone() {
    let state = timed();
    assert!(state.push("settings"));
    state.advance(1.0);
    state.take_events();
    assert!(state.push("confirm"), "a second modal goes over the first");
    assert_eq!(
        ids(&state),
        [
            (sym!("settings"), ModalPhase::Open, false),
            (sym!("confirm"), ModalPhase::Opening, true),
        ]
    );
    assert_eq!(state.progress("confirm"), 0.0);
    assert!(
        state.take_events().is_empty(),
        "the lower one emits nothing"
    );
}

#[test]
fn a_duplicate_push_is_refused() {
    let state = timed();
    assert!(state.push("settings"));
    assert!(
        !state.push(sym!("settings")),
        "an id already open is refused"
    );
    assert_eq!(state.depth(), 1);
}

#[test]
fn a_fifth_modal_is_refused() {
    let state = instant();
    for name in ["a", "b", "c", "d"] {
        assert!(state.push(name));
    }
    assert_eq!(state.depth(), MAX_MODAL_DEPTH);
    assert!(!state.push("e"), "the stack holds four");
    state.take_events();
    assert!(state.take_events().is_empty());
}

#[test]
fn dismiss_closes_only_the_top_and_the_one_below_becomes_top() {
    let state = timed();
    state.push("settings");
    state.push("confirm");
    state.advance(1.0);
    state.take_events();
    state.dismiss(DismissReason::Escape);
    assert_eq!(
        state.take_events(),
        [ModalEvent::Dismissed {
            id: sym!("confirm"),
            by: DismissReason::Escape
        }]
    );
    assert_eq!(state.top(), Some(sym!("settings")));
    assert_eq!(state.phase("confirm"), ModalPhase::Closing);
    assert_eq!(state.phase("settings"), ModalPhase::Open, "never closed");
    state.advance(0.05);
    assert!(
        state.progress("confirm") > 0.0,
        "progress runs down from where it was"
    );
    state.advance(1.0);
    assert_eq!(
        state.take_events(),
        [ModalEvent::Closed {
            id: sym!("confirm"),
            by: DismissReason::Escape
        }]
    );
    assert_eq!(state.depth(), 1);
}

#[test]
fn replace_top_reports_replaced_and_opens_at_once() {
    let state = timed();
    state.push("pause");
    state.advance(1.0);
    state.take_events();
    assert!(state.replace_top("quit"));
    assert_eq!(
        state.take_events(),
        [ModalEvent::Dismissed {
            id: sym!("pause"),
            by: DismissReason::Replaced
        }]
    );
    assert_eq!(
        ids(&state),
        [
            (sym!("pause"), ModalPhase::Closing, false),
            (sym!("quit"), ModalPhase::Opening, true),
        ],
        "both animate together"
    );
    let empty = timed();
    assert!(empty.replace_top("only"), "with no top it is a push");
    assert_eq!(empty.top(), Some(sym!("only")));
}

#[test]
fn replaced_is_reported_only_by_replace_top() {
    let state = instant();
    state.push("a");
    state.pop();
    state.push("b");
    state.close("b");
    state.push("c");
    state.dismiss_all(DismissReason::App);
    let replaced = state.take_events().into_iter().any(|e| {
        matches!(
            e,
            ModalEvent::Dismissed {
                by: DismissReason::Replaced,
                ..
            } | ModalEvent::Closed {
                by: DismissReason::Replaced,
                ..
            }
        )
    });
    assert!(!replaced, "pop, close and dismiss_all never say Replaced");
}

#[test]
fn dismiss_all_closes_top_first() {
    let state = timed();
    state.push("a");
    state.push("b");
    state.push("c");
    state.take_events();
    state.dismiss_all(DismissReason::Back);
    let order: Vec<Symbol> = state
        .take_events()
        .into_iter()
        .map(|e| match e {
            ModalEvent::Dismissed { id, .. } => id,
            other => panic!("only Dismissed expected, got {other:?}"),
        })
        .collect();
    assert_eq!(order, [sym!("c"), sym!("b"), sym!("a")]);
    assert_eq!(state.top(), None);
}

#[test]
fn pushing_an_id_that_is_closing_waits_for_its_close() {
    let state = timed();
    state.push("settings");
    state.advance(1.0);
    state.pop();
    state.take_events();
    assert!(state.push("settings"), "queued, so accepted");
    assert_eq!(state.depth(), 1, "not pushed yet");
    state.advance(1.0);
    assert_eq!(
        state.take_events(),
        [ModalEvent::Closed {
            id: sym!("settings"),
            by: DismissReason::App
        }]
    );
    assert_eq!(
        state.phase("settings"),
        ModalPhase::Opening,
        "pushed after the close"
    );
}

#[test]
fn timed_and_driven_entries_move_independently() {
    let state = ModalState::new().with_transition(ModalTransition::Driven);
    state.push("book");
    state.push_with("confirm", TIMED);
    state.advance(0.1);
    assert_eq!(
        state.progress("book"),
        0.0,
        "advance does not move a Driven entry"
    );
    assert!((state.progress("confirm") - 0.5).abs() < 1e-5);
    state.set_progress("book", 0.4);
    assert!((state.progress("book") - 0.4).abs() < 1e-5);
    assert!((state.progress("confirm") - 0.5).abs() < 1e-5);
    state.set_progress("book", 1.5);
    assert_eq!(state.progress("book"), 1.0, "progress is clamped");
    assert_eq!(state.phase("book"), ModalPhase::Open);
    state.close("book");
    state.advance(10.0);
    assert_eq!(
        state.phase("book"),
        ModalPhase::Closing,
        "a Driven entry stays until driven to 0"
    );
    state.set_progress("book", 0.0);
    assert_eq!(state.phase("book"), ModalPhase::Closed);
}

#[test]
fn an_instant_modal_is_open_at_once() {
    let state = instant();
    state.push("a");
    assert_eq!(state.phase("a"), ModalPhase::Open);
    assert_eq!(state.progress("a"), 1.0);
    assert_eq!(state.take_events(), [ModalEvent::Opened { id: sym!("a") }]);
    state.pop();
    assert_eq!(
        state.take_events(),
        [
            ModalEvent::Dismissed {
                id: sym!("a"),
                by: DismissReason::App
            },
            ModalEvent::Closed {
                id: sym!("a"),
                by: DismissReason::App
            }
        ],
        "an instant close finishes in the same call"
    );
    let zero = ModalState::new().with_transition(ModalTransition::Timed {
        open_secs: 0.0,
        close_secs: 0.0,
    });
    zero.push("z");
    assert_eq!(
        zero.phase("z"),
        ModalPhase::Open,
        "a zero duration is instant"
    );
}

#[test]
fn every_dismissed_is_followed_by_one_closed_with_the_same_id() {
    let state = timed();
    for name in ["a", "b", "c"] {
        state.push(name);
    }
    state.advance(1.0);
    state.dismiss(DismissReason::Escape);
    state.replace_top("d");
    state.close("a");
    state.dismiss_all(DismissReason::OutsideTap);
    for _ in 0..20 {
        state.advance(0.05);
    }
    let events = state.take_events();
    let dismissed: Vec<(Symbol, DismissReason)> = events
        .iter()
        .filter_map(|e| match *e {
            ModalEvent::Dismissed { id, by } => Some((id, by)),
            _ => None,
        })
        .collect();
    let closed: Vec<(Symbol, DismissReason)> = events
        .iter()
        .filter_map(|e| match *e {
            ModalEvent::Closed { id, by } => Some((id, by)),
            _ => None,
        })
        .collect();
    assert_eq!(dismissed.len(), closed.len());
    for d in &dismissed {
        assert_eq!(closed.iter().filter(|c| *c == d).count(), 1, "{d:?}");
    }
    assert_eq!(state.depth(), 0);
}

#[test]
fn dismissing_with_no_top_does_nothing() {
    let state = timed();
    state.dismiss(DismissReason::Escape);
    state.pop();
    assert!(state.take_events().is_empty());
    state.push("a");
    state.advance(1.0);
    state.pop();
    state.take_events();
    state.dismiss(DismissReason::Escape);
    assert!(
        state.take_events().is_empty(),
        "a second Escape while closing does nothing"
    );
}

#[test]
fn events_are_taken_once() {
    let state = instant();
    state.push("a");
    assert_eq!(state.take_events().len(), 1);
    assert!(state.take_events().is_empty());
}

#[test]
fn advance_ignores_negative_and_nan_dt() {
    let state = timed();
    state.push("a");
    state.advance(-1.0);
    state.advance(f32::NAN);
    state.advance(f32::INFINITY);
    assert_eq!(state.progress("a"), 0.0);
    assert!(state.advance(0.1), "still animating");
}

#[test]
fn captures_input_follows_the_entries_without_a_lock() {
    let state = timed();
    assert!(!state.captures_input());
    state.push("a");
    assert!(state.captures_input(), "from the push");
    state.advance(1.0);
    state.pop();
    assert!(state.captures_input(), "through Closing");
    state.advance(0.05);
    assert!(state.captures_input());
    state.advance(1.0);
    assert!(!state.captures_input(), "false once the last entry is gone");
}

#[test]
fn ids_are_symbols_and_events_are_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<ModalEvent>();
    assert_copy::<DismissReason>();
    let state = instant();
    state.push(String::from("built"));
    assert!(state.contains(sym!("built")));
    assert!(state.contains("built"));
    let event = state.take_events()[0];
    let again = event;
    assert_eq!(event, again);
    let control = DismissReason::Control(sym!("close"));
    assert_eq!(control, DismissReason::Control(Symbol::intern("close")));
}

fn every_kind() -> [InputEvent; 4] {
    [
        InputEvent::Mouse(MouseEvent::Down {
            pos: [10.0, 10.0],
            button: MouseButton::Left,
        }),
        InputEvent::Mouse(MouseEvent::Up {
            pos: [10.0, 10.0],
            button: MouseButton::Left,
        }),
        InputEvent::Keyboard(KeyboardEvent::KeyDown { key: Key::Tab }),
        InputEvent::Gesture(GestureEvent::DoubleTap),
    ]
}

#[test]
fn the_filter_passes_while_the_state_is_empty() {
    let modals = Arc::new(timed());
    let filter = modals.filter(sym!("modals"));
    let base = Recorder::at(0.0);
    let sink = Recorder::at(500.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&base))
        .with_named_control(sym!("modals"), ControlHandle::new(&sink))
        .with_filter(&filter)
        .build();
    let resp = host.deliver_mouse(MouseEvent::Down {
        pos: [10.0, 10.0],
        button: MouseButton::Left,
    });
    assert_eq!(resp.target_index, Some(0), "routing as usual");
    assert!(sink.take().is_empty());
}

#[test]
fn the_filter_redirects_to_its_target_while_any_modal_exists() {
    let modals = Arc::new(ModalState::new().with_transition(ModalTransition::Driven));
    let filter = ModalFilter::new(Arc::clone(&modals), "modals");
    assert_eq!(filter.target(), sym!("modals"));
    modals.push("settings");
    for phase in [ModalPhase::Opening, ModalPhase::Open, ModalPhase::Closing] {
        match phase {
            ModalPhase::Open => modals.set_progress("settings", 1.0),
            ModalPhase::Closing => modals.pop(),
            _ => {}
        }
        assert_eq!(modals.phase("settings"), phase);
        let base = Recorder::at(0.0);
        let sink = Recorder::at(500.0);
        let mut host = ControlHost::builder()
            .with_control(ControlHandle::new(&base))
            .with_named_control("modals", ControlHandle::new(&sink))
            .with_filter(&filter)
            .build();
        for event in every_kind() {
            let resp = host.deliver_event(event);
            assert_eq!(resp.target_id, Some(sym!("modals")), "{phase:?}: {event:?}");
        }
        let handled = sink
            .take()
            .iter()
            .filter(|c| matches!(c, Call::Mouse(..) | Call::Key(..) | Call::Gesture(..)))
            .count();
        assert_eq!(handled, 4, "{phase:?}: the stack gets every event");
        assert!(base.take().is_empty(), "{phase:?}: the base gets nothing");
    }
}

#[test]
fn a_modifier_change_is_consumed_while_a_modal_exists() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    modals.push("a");
    let sink = Recorder::at(0.0);
    let mut host = ControlHost::builder()
        .with_named_control("modals", ControlHandle::new(&sink))
        .with_filter(&filter)
        .build();
    let resp = host.deliver_event(InputEvent::Modifiers(Modifiers {
        shift: true,
        ..Modifiers::default()
    }));
    assert!(resp.handled && resp.target_index.is_none());
    assert!(sink.take().is_empty());
}

#[test]
fn a_filter_whose_target_is_not_hosted_still_traps_input() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    modals.push("a");
    let base = Recorder::at(0.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&base))
        .with_filter(&filter)
        .build();
    for event in every_kind() {
        let resp = host.deliver_event(event);
        assert!(resp.handled && resp.target_index.is_none(), "{event:?}");
    }
    assert!(base.take().is_empty(), "nothing reaches the base");
}

#[test]
fn one_filter_serves_many_builds() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    let sink = Recorder::at(0.0);
    let mut seen = Vec::new();
    for round in 0..3 {
        if round == 1 {
            modals.push("a");
        }
        let mut host = ControlHost::builder()
            .with_named_control("modals", ControlHandle::new(&sink))
            .with_filter(&filter)
            .build();
        let resp = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Enter });
        seen.push(resp.target_id);
    }
    assert_eq!(seen, [None, Some(sym!("modals")), Some(sym!("modals"))]);
}

const VIEWPORT: Rect = Rect::new(0.0, 0.0, 800.0, 600.0);
const FRAME: Rect = Rect::new(200.0, 150.0, 400.0, 300.0);
const CONTENT: Rect = Rect::new(220.0, 170.0, 360.0, 260.0);
const FRAME_COLOR: [f32; 4] = [0.2, 0.3, 0.4, 1.0];
const BUTTON_BG: [f32; 4] = [0.0, 0.6, 0.0, 1.0];
const TIP_BG: [f32; 4] = [0.6, 0.0, 0.6, 1.0];

/// A frame painter that draws one box and remembers every view it is given.
#[derive(Default)]
struct RecordingFrame {
    views: RefCell<Vec<ModalView>>,
}

impl ModalFrame for RecordingFrame {
    fn draw_frame(&self, list: &mut DrawList, _ctx: &DrawContext, view: &ModalView) {
        self.views.borrow_mut().push(*view);
        list.push(SdfInstance {
            kind: SdfKind::Box,
            position: view.frame.pos(),
            size: view.frame.size(),
            color: FRAME_COLOR,
            anim: 0,
        });
    }
}

fn driven() -> Arc<ModalState> {
    Arc::new(ModalState::new().with_transition(ModalTransition::Driven))
}

fn content_button() -> Button {
    Button::new("OK", Rect::new(240.0, 200.0, 80.0, 30.0)).with_bg(BUTTON_BG)
}

fn draw(control: &dyn DrawControl) -> DrawList {
    let mut list = DrawList::new();
    control.draw(&mut list, &DrawContext::default());
    list
}

fn position_of(list: &DrawList, color: [f32; 4]) -> Option<usize> {
    list.instances.iter().position(|i| i.color == color)
}

#[test]
fn a_modal_with_no_entry_draws_nothing() {
    let modals = driven();
    let button = content_button();
    let modal = Modal::new(Arc::clone(&modals), "settings", VIEWPORT, FRAME, CONTENT)
        .with_control(button.handle());
    assert!(modal.view().is_none());
    assert!(draw(&modal).instances.is_empty());
}

#[test]
fn the_backdrop_covers_the_viewport_and_fades_with_progress() {
    let modals = driven();
    modals.push("settings");
    modals.set_progress("settings", 0.5);
    let modal = Modal::new(Arc::clone(&modals), "settings", VIEWPORT, FRAME, CONTENT)
        .with_backdrop(Some(Backdrop {
            color: [0.0, 0.0, 0.0, 0.8],
            fade_with_progress: true,
        }));
    let list = draw(&modal);
    let backdrop = list.instances[0];
    assert_eq!(backdrop.position, VIEWPORT.pos());
    assert_eq!(backdrop.size, VIEWPORT.size());
    assert!(
        (backdrop.color[3] - 0.4).abs() < 1e-5,
        "0.8 at half progress"
    );
    let steady = Modal::new(Arc::clone(&modals), "settings", VIEWPORT, FRAME, CONTENT)
        .with_backdrop(Some(Backdrop {
            color: [0.0, 0.0, 0.0, 0.8],
            fade_with_progress: false,
        }));
    assert!((draw(&steady).instances[0].color[3] - 0.8).abs() < 1e-5);
}

#[test]
fn no_backdrop_means_no_backdrop_instance() {
    let modals = driven();
    modals.push("settings");
    let modal = Modal::new(Arc::clone(&modals), "settings", VIEWPORT, FRAME, CONTENT)
        .with_backdrop(None)
        .with_frame(RecordingFrame::default());
    let list = draw(&modal);
    assert_eq!(list.instances.len(), 1, "only the frame");
    assert_eq!(list.instances[0].color, FRAME_COLOR);
}

#[test]
fn the_frame_painter_is_called_with_the_view() {
    let modals = driven();
    modals.push("settings");
    modals.push("confirm");
    modals.set_progress("settings", 1.0);
    modals.set_progress("confirm", 0.25);
    let frame = RecordingFrame::default();
    let lower =
        Modal::new(Arc::clone(&modals), "settings", VIEWPORT, FRAME, CONTENT).with_frame(&frame);
    draw(&lower);
    let upper =
        Modal::new(Arc::clone(&modals), "confirm", VIEWPORT, FRAME, CONTENT).with_frame(&frame);
    draw(&upper);
    let views = frame.views.borrow();
    assert_eq!(views.len(), 2);
    let (l, u) = (views[0], views[1]);
    assert_eq!(
        (l.id, l.phase, l.depth, l.is_top),
        (sym!("settings"), ModalPhase::Open, 0, false)
    );
    assert_eq!(
        (u.id, u.phase, u.depth, u.is_top),
        (sym!("confirm"), ModalPhase::Opening, 1, true)
    );
    assert!((u.progress - 0.25).abs() < 1e-5);
    assert_eq!((u.viewport, u.frame, u.content), (VIEWPORT, FRAME, CONTENT));
}

#[test]
fn when_open_content_is_hidden_while_opening() {
    let modals = driven();
    modals.push("book");
    modals.set_progress("book", 0.9);
    let button = content_button();
    let modal = Modal::new(Arc::clone(&modals), "book", VIEWPORT, FRAME, CONTENT)
        .with_reveal(ContentReveal::WhenOpen)
        .with_control(button.handle());
    assert!(
        position_of(&draw(&modal), BUTTON_BG).is_none(),
        "hidden while opening"
    );
    modals.set_progress("book", 1.0);
    assert!(
        position_of(&draw(&modal), BUTTON_BG).is_some(),
        "shown once open"
    );
}

#[test]
fn fade_scales_content_alpha_by_progress() {
    let modals = driven();
    modals.push("settings");
    modals.set_progress("settings", 0.5);
    let button = content_button();
    let modal = Modal::new(Arc::clone(&modals), "settings", VIEWPORT, FRAME, CONTENT)
        .with_frame(RecordingFrame::default())
        .with_control(button.handle());
    let list = draw(&modal);
    let faded_bg = [BUTTON_BG[0], BUTTON_BG[1], BUTTON_BG[2], 0.5];
    let content = position_of(&list, faded_bg).expect("content alpha is halved");
    let frame = position_of(&list, FRAME_COLOR).expect("the frame is drawn");
    assert_eq!(list.instances[0].size, VIEWPORT.size(), "backdrop first");
    assert!(
        frame > 0 && content > frame,
        "backdrop, frame, then content"
    );
}

#[test]
fn content_is_clipped_and_its_tooltips_are_not() {
    let modals = driven();
    modals.push("settings");
    modals.set_progress("settings", 1.0);
    let grid = Options::new(&["SV - Save"], Rect::new(240.0, 200.0, 100.0, 20.0))
        .with_tooltip_style(TooltipStyle::default().with_bg(TIP_BG));
    grid.set_hover(Some(ControlPart::Custom(0)));
    let button = content_button();
    let modal = Modal::new(Arc::clone(&modals), "settings", VIEWPORT, FRAME, CONTENT)
        .with_control(button.handle())
        .with_control(grid.handle());
    let list = draw(&modal);
    let content = position_of(&list, BUTTON_BG).expect("content drawn");
    let tip = position_of(&list, TIP_BG).expect("the hovered cell's tip is drawn");
    assert!(list.instance_clip(content).is_some(), "content is clipped");
    assert!(list.instance_clip(tip).is_none(), "its tooltip is not");
}

#[test]
fn only_the_top_draws_content_tooltips() {
    let modals = driven();
    modals.push("settings");
    modals.push("confirm");
    let grid = Options::new(&["SV - Save"], Rect::new(240.0, 200.0, 100.0, 20.0))
        .with_tooltip_style(TooltipStyle::default().with_bg(TIP_BG));
    grid.set_hover(Some(ControlPart::Custom(0)));
    let lower = Modal::new(Arc::clone(&modals), "settings", VIEWPORT, FRAME, CONTENT)
        .with_reveal(ContentReveal::Always)
        .with_control(grid.handle());
    assert!(
        position_of(&draw(&lower), TIP_BG).is_none(),
        "not the top: no tip"
    );
    modals.close("confirm");
    modals.set_progress("confirm", 0.0);
    assert!(
        position_of(&draw(&lower), TIP_BG).is_some(),
        "the top again: tip"
    );
}

#[test]
fn split_draw_passes_match_draw() {
    let grid = Options::new(&["SV - Save"], Rect::new(0.0, 0.0, 100.0, 20.0))
        .with_tooltip_style(TooltipStyle::default().with_bg(TIP_BG));
    grid.set_hover(Some(ControlPart::Custom(0)));
    let button = content_button();
    let host = ControlHost::builder()
        .with_control(grid.handle())
        .with_control(button.handle())
        .build();
    let ctx = DrawContext::default();
    let whole = draw(&host);
    let mut split = DrawList::new();
    host.draw_controls(&mut split, &ctx);
    host.draw_control_overlays(&mut split, &ctx);
    assert_eq!(whole.instances, split.instances, "instance for instance");
}

/// A recorder at `(x, y)`, 60 pixels square, inside the content rect.
fn rec(x: f32, y: f32) -> Recorder {
    Recorder {
        rect: Rect::new(x, y, 60.0, 60.0),
        ..Recorder::at(0.0)
    }
}

fn m_down(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Down {
        pos: [x, y],
        button: MouseButton::Left,
    })
}

fn m_up(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Up {
        pos: [x, y],
        button: MouseButton::Left,
    })
}

fn m_click(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Click {
        pos: [x, y],
        button: MouseButton::Left,
    })
}

fn m_move(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Move { pos: [x, y] })
}

fn m_drag(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Drag {
        start_pos: [250.0, 210.0],
        current_pos: [x, y],
        delta: [x - 250.0, y - 210.0],
        button: MouseButton::Left,
    })
}

fn k(key: Key) -> InputEvent {
    InputEvent::Keyboard(KeyboardEvent::KeyDown { key })
}

const NO_MODS: Modifiers = Modifiers {
    shift: false,
    alt: false,
    ctrl: false,
    meta: false,
};

/// An open modal "m" on an instant stack.
fn open_state() -> Arc<ModalState> {
    let modals = Arc::new(instant());
    modals.push("m");
    modals.take_events();
    modals
}

fn modal<'a>(state: &Arc<ModalState>) -> Modal<'a> {
    Modal::new(Arc::clone(state), "m", VIEWPORT, FRAME, CONTENT)
}

fn dismissed(state: &ModalState) -> Vec<DismissReason> {
    state
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            ModalEvent::Dismissed { by, .. } => Some(by),
            _ => None,
        })
        .collect()
}

fn refused(state: &ModalState) -> Vec<DismissReason> {
    state
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            ModalEvent::DismissRefused { by, .. } => Some(by),
            _ => None,
        })
        .collect()
}

#[test]
fn escape_dismisses_and_a_second_escape_does_nothing() {
    let modals = Arc::new(timed());
    modals.push("m");
    modals.advance(1.0);
    modals.take_events();
    let m = modal(&modals);
    m.deliver_event(k(Key::Escape), NO_MODS);
    assert_eq!(dismissed(&modals), [DismissReason::Escape]);
    m.deliver_event(k(Key::Escape), NO_MODS);
    assert!(modals.take_events().is_empty(), "already closing");
}

#[test]
fn back_dismisses_with_back() {
    let modals = open_state();
    modal(&modals).deliver_event(k(Key::Back), NO_MODS);
    assert_eq!(dismissed(&modals), [DismissReason::Back]);
}

#[test]
fn escape_and_back_dismiss_with_an_edit_focused() {
    for key in [Key::Escape, Key::Back] {
        let modals = open_state();
        let edit = Edit::new("name", Rect::new(240.0, 200.0, 200.0, 30.0));
        let m = modal(&modals)
            .with_named_control("name", edit.handle())
            .with_initial_focus(ControlKey::named("name"));
        m.deliver_event(k(Key::Char('a')), NO_MODS);
        m.deliver_event(k(key), NO_MODS);
        assert_eq!(dismissed(&modals).len(), 1, "{key:?} dismisses even then");
    }
}

#[test]
fn escape_reaches_the_content_when_disabled() {
    let modals = open_state();
    let r = rec(240.0, 200.0);
    let m = modal(&modals)
        .with_dismiss(DismissOn {
            escape: false,
            ..DismissOn::default()
        })
        .with_control(ControlHandle::new(&r))
        .with_initial_focus(ControlKey::at(0));
    m.deliver_event(k(Key::Escape), NO_MODS);
    assert!(modals.take_events().is_empty());
    assert_eq!(
        r.focus(),
        [FocusEvent::Gained, FocusEvent::Lost],
        "the content host clears its focus"
    );
}

#[test]
fn back_can_be_turned_off() {
    let modals = open_state();
    let r = rec(240.0, 200.0);
    let m = modal(&modals)
        .with_dismiss(DismissOn {
            back: false,
            ..DismissOn::default()
        })
        .with_control(ControlHandle::new(&r))
        .with_initial_focus(ControlKey::at(0));
    m.deliver_event(k(Key::Back), NO_MODS);
    assert!(modals.take_events().is_empty());
    assert!(
        r.take().iter().any(|c| matches!(c, Call::Key(..))),
        "Back goes to the focused content control"
    );
}

#[test]
fn a_press_and_release_outside_dismisses() {
    let modals = open_state();
    let m = modal(&modals);
    m.deliver_event(m_down(50.0, 50.0), NO_MODS);
    assert!(modals.take_events().is_empty(), "not on the press");
    m.deliver_event(m_up(60.0, 50.0), NO_MODS);
    assert_eq!(dismissed(&modals), [DismissReason::OutsideTap]);
}

#[test]
fn a_release_outside_without_a_press_outside_does_not_dismiss() {
    let modals = open_state();
    let m = modal(&modals);
    let resp = m.deliver_event(m_up(50.0, 50.0), NO_MODS);
    assert!(resp.handled, "consumed");
    assert!(
        modals.take_events().is_empty(),
        "the tail of the opening tap"
    );
}

#[test]
fn a_drag_from_a_slider_to_outside_does_not_dismiss() {
    let modals = open_state();
    let slider = Slider::new(0.5, 0.0, 1.0, Rect::new(240.0, 200.0, 200.0, 20.0));
    let m = modal(&modals).with_control(slider.handle());
    m.deliver_event(m_down(250.0, 210.0), NO_MODS);
    m.deliver_event(m_drag(50.0, 50.0), NO_MODS);
    m.deliver_event(m_up(50.0, 50.0), NO_MODS);
    assert!(modals.take_events().is_empty());
}

#[test]
fn a_click_outside_dismisses_for_click_only_shells() {
    let modals = open_state();
    modal(&modals).deliver_event(m_click(50.0, 50.0), NO_MODS);
    assert_eq!(dismissed(&modals), [DismissReason::OutsideTap]);
}

#[test]
fn outside_tap_can_be_turned_off() {
    let modals = open_state();
    let m = modal(&modals).with_dismiss(DismissOn {
        outside_tap: false,
        ..DismissOn::default()
    });
    m.deliver_event(m_down(50.0, 50.0), NO_MODS);
    m.deliver_event(m_up(50.0, 50.0), NO_MODS);
    m.deliver_event(m_click(50.0, 50.0), NO_MODS);
    assert!(modals.take_events().is_empty());
}

#[test]
fn a_close_control_dismisses_with_its_symbol() {
    let modals = open_state();
    let close = Button::new("Close", Rect::new(240.0, 200.0, 80.0, 30.0));
    let m = modal(&modals)
        .with_named_control(sym!("close"), close.handle())
        .with_close_control(sym!("close"));
    let resp = m.deliver_event(m_down(250.0, 210.0), NO_MODS);
    assert_eq!(
        resp.target_id,
        Some(sym!("close")),
        "the response is the content's"
    );
    assert_eq!(dismissed(&modals), [DismissReason::Control(sym!("close"))]);
}

#[test]
fn hold_open_refuses_user_dismissals_and_reports_them() {
    let modals = open_state();
    let close = Button::new("Close", Rect::new(240.0, 200.0, 80.0, 30.0));
    let m = modal(&modals)
        .with_hold_open(true)
        .with_named_control("close", close.handle())
        .with_close_control("close");
    m.deliver_event(k(Key::Escape), NO_MODS);
    m.deliver_event(k(Key::Back), NO_MODS);
    m.deliver_event(m_down(50.0, 50.0), NO_MODS);
    m.deliver_event(m_up(50.0, 50.0), NO_MODS);
    m.deliver_event(m_down(250.0, 210.0), NO_MODS);
    assert_eq!(
        refused(&modals),
        [
            DismissReason::Escape,
            DismissReason::Back,
            DismissReason::OutsideTap,
            DismissReason::Control(sym!("close")),
        ]
    );
    assert_eq!(modals.phase("m"), ModalPhase::Open, "it stays open");
}

#[test]
fn hold_open_does_not_stop_app_dismissals() {
    let modals = open_state();
    let _m = modal(&modals).with_hold_open(true);
    modals.pop();
    assert_eq!(dismissed(&modals), [DismissReason::App]);
}

#[test]
fn a_disabled_action_is_not_reported_as_refused() {
    let modals = open_state();
    let m = modal(&modals).with_hold_open(true).with_dismiss(DismissOn {
        escape: false,
        back: true,
        outside_tap: false,
    });
    m.deliver_event(k(Key::Escape), NO_MODS);
    m.deliver_event(m_click(50.0, 50.0), NO_MODS);
    assert!(modals.take_events().is_empty());
}

#[test]
fn content_receives_nothing_while_opening_or_closing() {
    let modals = Arc::new(timed());
    modals.push("m");
    let r = rec(240.0, 200.0);
    let m = modal(&modals).with_control(ControlHandle::new(&r));
    m.deliver_event(m_down(250.0, 210.0), NO_MODS);
    m.deliver_event(k(Key::Enter), NO_MODS);
    assert!(r.take().is_empty(), "nothing while opening");
    modals.advance(1.0);
    modals.pop();
    m.deliver_event(m_down(250.0, 210.0), NO_MODS);
    assert!(r.take().is_empty(), "nothing while closing");
}

#[test]
fn focus_drag_and_hover_inside_the_modal_survive_a_rebuild() {
    let modals = open_state();
    let a = rec(240.0, 200.0);
    let b = rec(340.0, 200.0);
    let build = || {
        modal(&modals)
            .with_control(ControlHandle::new(&a))
            .with_control(ControlHandle::new(&b))
    };
    build().deliver_event(m_move(250.0, 210.0), NO_MODS);
    build().deliver_event(m_down(250.0, 210.0), NO_MODS);
    a.take();
    build().deliver_event(m_drag(500.0, 400.0), NO_MODS);
    let mouse: Vec<Call> = a
        .take()
        .into_iter()
        .filter(|c| matches!(c, Call::Mouse(..)))
        .collect();
    assert!(
        matches!(mouse.as_slice(), [Call::Mouse(MouseEvent::Drag { .. }, _)]),
        "the grab is carried (a rebuilt modal also re-sends Gained to its focus)"
    );
    build().deliver_event(m_up(500.0, 400.0), NO_MODS);
    a.take();
    build().deliver_event(m_move(500.0, 400.0), NO_MODS);
    assert!(
        a.hovers().contains(&None),
        "the hover is carried, so leaving clears it"
    );
    a.take();
    b.take();
    build().deliver_event(k(Key::Tab), NO_MODS);
    assert!(
        a.focus().contains(&FocusEvent::Lost),
        "the focus is carried"
    );
    assert!(b.focus().contains(&FocusEvent::Gained));
}

#[test]
fn initial_focus_is_applied_once_per_push() {
    let modals = open_state();
    let a = rec(240.0, 200.0);
    let b = rec(340.0, 200.0);
    let build = || {
        modal(&modals)
            .with_control(ControlHandle::new(&a))
            .with_control(ControlHandle::new(&b))
            .with_initial_focus(ControlKey::at(1))
    };
    build().deliver_event(k(Key::Enter), NO_MODS);
    assert!(
        b.take().iter().any(|c| matches!(c, Call::Key(..))),
        "b starts focused"
    );
    build().deliver_event(k(Key::Tab), NO_MODS);
    build().deliver_event(k(Key::Enter), NO_MODS);
    assert!(
        a.take().iter().any(|c| matches!(c, Call::Key(..))),
        "a rebuild keeps the moved focus instead of the initial one"
    );
    modals.pop();
    modals.push("m");
    b.take();
    build().deliver_event(k(Key::Enter), NO_MODS);
    assert!(
        b.take().iter().any(|c| matches!(c, Call::Key(..))),
        "a new push seeds it again"
    );
}

#[test]
fn the_content_sees_the_modifiers_the_outer_host_saw() {
    let modals = open_state();
    let r = rec(240.0, 200.0);
    let m = modal(&modals).with_control(ControlHandle::new(&r));
    let shift = Modifiers {
        shift: true,
        ..NO_MODS
    };
    m.deliver_event(m_down(250.0, 210.0), shift);
    assert_eq!(r.mouse_contexts()[0].modifiers, shift);
}

#[test]
fn tab_cycles_the_content_only() {
    let modals = open_state();
    let a = rec(240.0, 200.0);
    let b = rec(340.0, 200.0);
    let m = modal(&modals)
        .with_control(ControlHandle::new(&a))
        .with_control(ControlHandle::new(&b));
    common::take_focus_journal();
    for _ in 0..3 {
        m.deliver_event(k(Key::Tab), NO_MODS);
    }
    let order: Vec<(f32, FocusEvent)> = common::take_focus_journal();
    assert_eq!(
        order,
        [
            (240.0, FocusEvent::Gained),
            (240.0, FocusEvent::Lost),
            (340.0, FocusEvent::Gained),
            (340.0, FocusEvent::Lost),
            (240.0, FocusEvent::Gained),
        ],
        "Tab wraps within the content"
    );
}

/// A frame whose tappable shape is only the left half of its rect.
struct LeftHalf;

impl ModalFrame for LeftHalf {
    fn draw_frame(&self, _list: &mut DrawList, _ctx: &DrawContext, _view: &ModalView) {}

    fn contains(&self, pos: [f32; 2], view: &ModalView) -> bool {
        view.frame.contains(pos) && pos[0] < view.frame.x + view.frame.w / 2.0
    }
}

#[test]
fn a_frame_shape_decides_what_is_outside() {
    let modals = open_state();
    let m = modal(&modals).with_frame(LeftHalf);
    m.deliver_event(m_down(300.0, 300.0), NO_MODS);
    m.deliver_event(m_up(300.0, 300.0), NO_MODS);
    assert!(
        modals.take_events().is_empty(),
        "the left half is on the modal"
    );
    m.deliver_event(m_down(500.0, 300.0), NO_MODS);
    m.deliver_event(m_up(500.0, 300.0), NO_MODS);
    assert_eq!(
        dismissed(&modals),
        [DismissReason::OutsideTap],
        "the right half is not"
    );
}

const LOWER_BG: [f32; 4] = [0.9, 0.1, 0.1, 1.0];
const UPPER_BG: [f32; 4] = [0.1, 0.1, 0.9, 1.0];
const UPPER_FRAME: Rect = Rect::new(300.0, 250.0, 200.0, 100.0);

fn lower<'a>(state: &Arc<ModalState>) -> Modal<'a> {
    Modal::new(Arc::clone(state), "lower", VIEWPORT, FRAME, CONTENT).with_frame(PanelFrame {
        bg_color: Some(LOWER_BG),
        ..PanelFrame::default()
    })
}

fn upper<'a>(state: &Arc<ModalState>) -> Modal<'a> {
    Modal::new(
        Arc::clone(state),
        "upper",
        VIEWPORT,
        UPPER_FRAME,
        UPPER_FRAME.pad(-10.0),
    )
    .with_frame(PanelFrame {
        bg_color: Some(UPPER_BG),
        ..PanelFrame::default()
    })
}

fn handler_calls(r: &Recorder) -> usize {
    r.take()
        .iter()
        .filter(|c| matches!(c, Call::Mouse(..) | Call::Key(..) | Call::Gesture(..)))
        .count()
}

#[test]
fn an_empty_stack_is_transparent_and_draws_nothing() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    let base = Recorder::at(0.0);
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT);
    assert_eq!(stack.hit_test([10.0, 10.0]), None);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&base))
        .with_named_control("modals", stack.handle())
        .with_filter(&filter)
        .build();
    let resp = host.deliver_event(m_down(10.0, 10.0));
    assert_eq!(resp.target_index, Some(0), "the base gets it");
    let mut list = DrawList::new();
    stack.draw_overlay(&mut list, &DrawContext::default());
    stack.draw(&mut list, &DrawContext::default());
    assert!(list.instances.is_empty());
}

#[test]
fn tab_in_the_base_skips_the_stack() {
    let modals = Arc::new(instant());
    let a = Recorder::at(0.0);
    let b = Recorder::at(200.0);
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&a))
        .with_control(ControlHandle::new(&b))
        .with_named_control("modals", stack.handle())
        .build();
    let mut seen = Vec::new();
    for _ in 0..3 {
        host.deliver_event(k(Key::Tab));
        seen.push(host.focused_index());
    }
    assert_eq!(
        seen,
        [Some(0), Some(1), Some(0)],
        "never index 2, the stack"
    );
}

#[test]
fn base_controls_receive_nothing_while_any_modal_exists() {
    let modals = driven();
    let filter = modals.filter("modals");
    let base = rec(240.0, 200.0);
    modals.push("lower");
    for phase in [ModalPhase::Opening, ModalPhase::Open, ModalPhase::Closing] {
        match phase {
            ModalPhase::Open => modals.set_progress("lower", 1.0),
            ModalPhase::Closing => modals.pop(),
            _ => {}
        }
        let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT).with_modal(lower(&modals));
        let mut host = ControlHost::builder()
            .with_control(ControlHandle::new(&base))
            .with_named_control("modals", stack.handle())
            .with_filter(&filter)
            .build();
        for event in [
            m_down(250.0, 210.0),
            m_up(250.0, 210.0),
            k(Key::Enter),
            InputEvent::Gesture(GestureEvent::DoubleTap),
        ] {
            host.deliver_event(event);
        }
        assert_eq!(handler_calls(&base), 0, "{phase:?}");
    }
}

#[test]
fn base_hover_is_left_and_a_base_grab_dropped_on_the_first_redirected_event() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    let base = rec(240.0, 200.0);
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&base))
        .with_named_control("modals", stack.handle())
        .with_filter(&filter)
        .build();
    host.deliver_event(m_move(250.0, 210.0));
    host.deliver_event(m_down(250.0, 210.0));
    assert!(host.active_drag().is_some());
    base.take();
    modals.push("lower");
    host.deliver_event(m_drag(260.0, 210.0));
    assert_eq!(base.take(), [Call::Hover(None)], "left, and nothing else");
    assert_eq!(host.active_drag(), None, "the base grab is dropped");
}

#[test]
fn base_focus_is_unchanged_after_a_modal_closes() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    let a = Recorder::at(0.0);
    let b = Recorder::at(200.0);
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&a))
        .with_control(ControlHandle::new(&b))
        .with_named_control("modals", stack.handle())
        .with_filter(&filter)
        .build();
    host.set_focused_index(Some(1));
    b.take();
    modals.push("lower");
    host.deliver_event(k(Key::Tab));
    host.deliver_event(k(Key::Escape));
    modals.dismiss_all(DismissReason::App);
    assert_eq!(host.focused_index(), Some(1));
    assert!(
        a.focus().is_empty() && b.focus().is_empty(),
        "no focus event at all"
    );
}

#[test]
fn lower_modals_receive_nothing_and_lose_their_hover() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    let content = rec(220.0, 180.0);
    modals.push("lower");
    {
        let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT)
            .with_modal(lower(&modals).with_control(ControlHandle::new(&content)));
        let mut host = ControlHost::builder()
            .with_named_control("modals", stack.handle())
            .with_filter(&filter)
            .build();
        host.deliver_event(m_move(230.0, 190.0));
    }
    assert_eq!(content.hovers(), [Some(ControlPart::Body)]);
    modals.push("upper");
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT)
        .with_modal(lower(&modals).with_control(ControlHandle::new(&content)))
        .with_modal(upper(&modals));
    let mut host = ControlHost::builder()
        .with_named_control("modals", stack.handle())
        .with_filter(&filter)
        .build();
    host.deliver_event(m_move(235.0, 190.0));
    host.deliver_event(k(Key::Enter));
    let calls = content.take();
    assert_eq!(
        calls,
        [Call::Hover(None)],
        "loses its hover, receives nothing"
    );
}

#[test]
fn escape_and_back_close_only_the_top() {
    for key in [Key::Escape, Key::Back] {
        let modals = Arc::new(instant());
        let filter = modals.filter("modals");
        modals.push("lower");
        modals.push("upper");
        modals.take_events();
        let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT)
            .with_modal(lower(&modals))
            .with_modal(upper(&modals));
        let mut host = ControlHost::builder()
            .with_named_control("modals", stack.handle())
            .with_filter(&filter)
            .build();
        host.deliver_event(k(key));
        assert!(!modals.contains("upper"), "{key:?} closes the top");
        assert!(modals.contains("lower"), "{key:?} leaves the one below");
    }
}

#[test]
fn a_tap_on_a_lower_modal_is_outside_the_top() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    modals.push("lower");
    modals.push("upper");
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT)
        .with_modal(lower(&modals))
        .with_modal(upper(&modals));
    let mut host = ControlHost::builder()
        .with_named_control("modals", stack.handle())
        .with_filter(&filter)
        .build();
    host.deliver_event(m_down(220.0, 170.0));
    host.deliver_event(m_up(220.0, 170.0));
    assert!(
        !modals.contains("upper"),
        "a tap on the lower frame closes the top"
    );
    assert!(modals.contains("lower"));
}

#[test]
fn focus_is_kept_per_entry() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    let a = rec(220.0, 180.0);
    let b = rec(300.0, 180.0);
    let build_and_send = |event: InputEvent| {
        let mut stack = ModalStack::new(Arc::clone(&modals), VIEWPORT).with_modal(
            lower(&modals)
                .with_control(ControlHandle::new(&a))
                .with_control(ControlHandle::new(&b)),
        );
        if modals.contains("upper") {
            stack = stack.with_modal(upper(&modals));
        }
        let mut host = ControlHost::builder()
            .with_named_control("modals", stack.handle())
            .with_filter(&filter)
            .build();
        host.deliver_event(event);
    };
    modals.push("lower");
    build_and_send(k(Key::Tab));
    build_and_send(k(Key::Tab));
    modals.push("upper");
    build_and_send(k(Key::Tab));
    modals.pop();
    a.take();
    b.take();
    build_and_send(k(Key::Enter));
    assert!(
        b.take().iter().any(|c| matches!(c, Call::Key(..))),
        "the lower modal's focus is where it was: on b"
    );
}

#[test]
fn modals_are_ordered_by_the_stack_not_by_build_order() {
    let modals = Arc::new(instant());
    modals.push("lower");
    modals.push("upper");
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT)
        .with_modal(upper(&modals))
        .with_modal(lower(&modals));
    let mut list = DrawList::new();
    stack.draw_overlay(&mut list, &DrawContext::default());
    let l = position_of(&list, LOWER_BG).expect("lower drawn");
    let u = position_of(&list, UPPER_BG).expect("upper drawn");
    assert!(l < u, "the lower entry draws first");
    assert_eq!(stack.top_modal().map(|m| m.id()), Some(sym!("upper")));
}

#[test]
fn input_is_consumed_when_the_top_has_no_modal_built() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    let base = rec(240.0, 200.0);
    modals.push("unbuilt");
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&base))
        .with_named_control("modals", stack.handle())
        .with_filter(&filter)
        .build();
    let resp = host.deliver_event(m_down(250.0, 210.0));
    assert_eq!(resp.target_id, Some(sym!("modals")));
    assert_eq!(resp.action, None);
    assert_eq!(handler_calls(&base), 0);
}

#[test]
fn the_response_names_the_stack_and_carries_the_inner_action() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    let ok = Button::new("OK", Rect::new(240.0, 200.0, 80.0, 30.0));
    modals.push("lower");
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT)
        .with_modal(lower(&modals).with_named_control("ok", ok.handle()));
    let mut host = ControlHost::builder()
        .with_named_control(sym!("modals"), stack.handle())
        .with_filter(&filter)
        .build();
    let resp = host.deliver_event(m_down(250.0, 210.0));
    assert_eq!(resp.target_id, Some(sym!("modals")));
    assert!(matches!(
        resp.action,
        Some(libcontrols::ControlAction::Activated { .. })
    ));
    assert!(ok.state.take_click(), "the button inside was pressed");
}

#[test]
fn each_level_draws_its_own_backdrop_bottom_to_top() {
    let modals = Arc::new(instant());
    modals.push("lower");
    modals.push("upper");
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT)
        .with_modal(lower(&modals))
        .with_modal(upper(&modals));
    let mut list = DrawList::new();
    stack.draw_overlay(&mut list, &DrawContext::default());
    let backdrops: Vec<usize> = list
        .instances
        .iter()
        .enumerate()
        .filter(|(_, i)| i.size == VIEWPORT.size())
        .map(|(n, _)| n)
        .collect();
    assert_eq!(backdrops.len(), 2, "one backdrop per level");
    let l = position_of(&list, LOWER_BG).unwrap();
    let u = position_of(&list, UPPER_BG).unwrap();
    assert!(backdrops[0] < l && l < backdrops[1] && backdrops[1] < u);
}

#[test]
fn the_stack_draws_after_every_base_overlay() {
    let modals = Arc::new(instant());
    let filter = modals.filter("modals");
    let base = Options::new(&["SV - Save"], Rect::new(10.0, 10.0, 100.0, 20.0))
        .with_tooltip_style(TooltipStyle::default().with_bg(TIP_BG));
    let button = content_button();
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT)
        .with_modal(lower(&modals).with_control(button.handle()))
        .with_modal(upper(&modals));
    let mut host = ControlHost::builder()
        .with_control(base.handle())
        .with_named_control("modals", stack.handle())
        .with_filter(&filter)
        .build();
    host.deliver_event(m_move(15.0, 15.0));
    modals.push("lower");
    modals.push("upper");
    let mut list = DrawList::new();
    host.draw(&mut list, &DrawContext::default());
    let tip = position_of(&list, TIP_BG).expect("the base overlay is drawn");
    let backdrops: Vec<usize> = list
        .instances
        .iter()
        .enumerate()
        .filter(|(_, i)| i.size == VIEWPORT.size())
        .map(|(n, _)| n)
        .collect();
    let l = position_of(&list, LOWER_BG).unwrap();
    let content = position_of(&list, BUTTON_BG).unwrap();
    let u = position_of(&list, UPPER_BG).unwrap();
    assert!(tip > 0, "the base control comes before its overlay");
    assert!(
        tip < backdrops[0]
            && backdrops[0] < l
            && l < content
            && content < backdrops[1]
            && backdrops[1] < u,
        "base, its overlay, lower backdrop, frame, content, upper backdrop, frame"
    );

    // A hovered base recorder under the same two modals.
    let watcher = rec(500.0, 500.0);
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT)
        .with_modal(lower(&modals))
        .with_modal(upper(&modals));
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&watcher))
        .with_named_control("modals", stack.handle())
        .with_filter(&filter)
        .build();
    host.set_hovered_key(Some(ControlKey::at(0)));
    for event in [m_move(505.0, 505.0), m_down(505.0, 505.0), k(Key::Enter)] {
        host.deliver_event(event);
    }
    assert_eq!(
        watcher.take(),
        [Call::Hover(None)],
        "one clear and no handler call"
    );
}

#[test]
fn a_host_with_only_a_stack_and_its_filter_works() {
    let modals = Arc::new(instant());
    let filter = modals.filter(sym!("modals"));
    let close = Button::new("Close", Rect::new(240.0, 200.0, 80.0, 30.0));
    modals.push("lower");
    modals.take_events();
    let stack = ModalStack::new(Arc::clone(&modals), VIEWPORT).with_modal(
        lower(&modals)
            .with_named_control("close", close.handle())
            .with_close_control("close"),
    );
    let mut host = ControlHost::builder()
        .with_named_control(sym!("modals"), stack.handle())
        .with_filter(&filter)
        .build();
    host.deliver_event(m_down(250.0, 210.0));
    assert_eq!(dismissed(&modals), [DismissReason::Control(sym!("close"))]);
    assert!(!modals.captures_input());
}

/// The example in README.md's "Modals" section, with buttons for its controls.
#[test]
fn the_readme_two_level_example_runs() {
    let modals = Arc::new(ModalState::new());
    let modal_filter = modals.filter(sym!("modals"));
    let quit = Button::new("Quit", Rect::new(220.0, 170.0, 80.0, 30.0));
    let cancel = Button::new("Cancel", Rect::new(290.0, 260.0, 80.0, 30.0));
    let unsaved_changes = true;
    let send = |event: InputEvent| {
        let mut stack = ModalStack::new(modals.clone(), VIEWPORT);
        for entry in modals.stack() {
            stack = stack.with_modal(if entry.id == sym!("settings") {
                Modal::centered(modals.clone(), entry.id, VIEWPORT, [400.0, 300.0], 16.0)
                    .with_hold_open(unsaved_changes)
                    .with_named_control(sym!("quit"), quit.handle())
            } else {
                Modal::centered(modals.clone(), entry.id, VIEWPORT, [240.0, 120.0], 12.0)
                    .with_named_control(sym!("cancel"), cancel.handle())
                    .with_close_control(sym!("cancel"))
            });
        }
        let mut host = ControlHost::builder()
            .with_named_control(sym!("modals"), stack.handle())
            .with_filter(&modal_filter)
            .build();
        host.deliver_event(event)
    };

    modals.push(sym!("settings"));
    modals.advance(1.0);
    send(m_down(230.0, 180.0));
    assert!(quit.state.take_click(), "Quit inside settings is pressed");
    modals.push(sym!("confirm"));
    modals.advance(1.0);
    modals.take_events();

    send(m_down(300.0, 270.0));
    assert_eq!(
        modals.take_events(),
        [ModalEvent::Dismissed {
            id: sym!("confirm"),
            by: DismissReason::Control(sym!("cancel"))
        }],
        "Cancel closes the confirm"
    );
    modals.advance(1.0);
    modals.take_events();
    assert_eq!(modals.top(), Some(sym!("settings")));

    send(k(Key::Escape));
    assert_eq!(
        modals.take_events(),
        [ModalEvent::DismissRefused {
            id: sym!("settings"),
            by: DismissReason::Escape
        }],
        "unsaved settings hold open"
    );
}
