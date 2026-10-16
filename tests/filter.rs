//! The filter stack: app-owned filters asked before routing.

mod common;

use std::cell::{Cell, RefCell};

use common::{Call, Recorder};
use libcontrols::{
    ControlAction, ControlHandle, ControlHost, ControlKey, ControlPart, FilterContext, Filtered,
    GestureEvent, InputEvent, InputFilter, Key, KeyboardEvent, Modifiers, MouseButton, MouseEvent,
    sym,
};

/// A filter that logs what it is asked and answers with a fixed rule.
struct Scripted<F: Fn(&InputEvent) -> Filtered> {
    answer: F,
    seen: RefCell<Vec<(InputEvent, FilterContext)>>,
}

impl<F: Fn(&InputEvent) -> Filtered> Scripted<F> {
    fn new(answer: F) -> Self {
        Self {
            answer,
            seen: RefCell::new(Vec::new()),
        }
    }
}

impl<F: Fn(&InputEvent) -> Filtered> InputFilter for Scripted<F> {
    fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered {
        self.seen.borrow_mut().push((*event, *ctx));
        (self.answer)(event)
    }
}

fn pass() -> Scripted<impl Fn(&InputEvent) -> Filtered> {
    Scripted::new(|_| Filtered::Pass)
}

fn down(x: f32) -> MouseEvent {
    MouseEvent::Down {
        pos: [x, 10.0],
        button: MouseButton::Left,
    }
}

fn up(x: f32) -> MouseEvent {
    MouseEvent::Up {
        pos: [x, 10.0],
        button: MouseButton::Left,
    }
}

fn key(key: Key) -> KeyboardEvent {
    KeyboardEvent::KeyDown { key }
}

const SHIFT: Modifiers = Modifiers {
    shift: true,
    alt: false,
    ctrl: false,
    meta: false,
};

/// The same script of events, for comparing routing with and without filters.
fn script(host: &mut ControlHost<'_>) {
    host.deliver_mouse(MouseEvent::Move { pos: [10.0, 10.0] });
    host.deliver_mouse(down(10.0));
    host.deliver_mouse(up(210.0));
    host.deliver_keyboard(key(Key::Tab));
    host.deliver_keyboard(key(Key::Enter));
    host.deliver_gesture(GestureEvent::DoubleTap);
    host.deliver_event(InputEvent::Modifiers(SHIFT));
}

#[test]
fn with_no_filter_registered_routing_is_unchanged() {
    let plain = [Recorder::at(0.0), Recorder::at(200.0)];
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&plain[0]))
        .with_control(ControlHandle::new(&plain[1]))
        .build();
    script(&mut host);

    let passing = pass();
    let filtered = [Recorder::at(0.0), Recorder::at(200.0)];
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&filtered[0]))
        .with_control(ControlHandle::new(&filtered[1]))
        .with_filter(&passing)
        .build();
    script(&mut host);

    for (a, b) in plain.iter().zip(&filtered) {
        assert_eq!(a.take(), b.take(), "a passing filter must change nothing");
    }
    assert_eq!(passing.seen.borrow().len(), 7, "but it sees every event");
}

#[test]
fn the_last_filter_added_is_asked_first() {
    let order = RefCell::new(Vec::new());
    struct Named<'o>(&'static str, &'o RefCell<Vec<&'static str>>);
    impl InputFilter for Named<'_> {
        fn filter(&self, _: &InputEvent, _: &FilterContext) -> Filtered {
            self.1.borrow_mut().push(self.0);
            Filtered::Pass
        }
    }
    let shortcuts = Named("shortcuts", &order);
    let modal = Named("modal", &order);
    let mut host = ControlHost::builder()
        .with_filter(&shortcuts)
        .with_filter(&modal)
        .build();
    host.deliver_keyboard(key(Key::Enter));
    assert_eq!(*order.borrow(), ["modal", "shortcuts"]);
}

#[test]
fn a_passing_filter_leaves_routing_alone() {
    let r = Recorder::at(0.0);
    let passing = pass();
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .with_filter(&passing)
        .build();
    let resp = host.deliver_mouse(down(10.0));
    assert_eq!(resp.target_index, Some(0));
    assert_eq!(host.focused_index(), Some(0));
    assert!(r.take().iter().any(|c| matches!(c, Call::Mouse(..))));
}

#[test]
fn a_consumed_event_reaches_no_control_and_reports_handled() {
    let r = Recorder::at(0.0);
    let eat = Scripted::new(|_| Filtered::Consumed(None));
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .with_filter(&eat)
        .build();
    let resp = host.deliver_mouse(down(10.0));
    assert!(resp.handled, "a consumed event is handled");
    assert_eq!(resp.target_index, None, "but names no control");
    assert!(r.take().is_empty(), "no control sees it, not even hover");
    assert_eq!(host.focused_index(), None);
}

#[test]
fn a_consumed_action_is_returned_to_the_app() {
    let save = Scripted::new(|_| Filtered::Consumed(Some(ControlAction::Custom("save".into()))));
    let mut host = ControlHost::builder().with_filter(&save).build();
    let resp = host.deliver_keyboard(key(Key::Char('s')));
    assert_eq!(resp.action, Some(ControlAction::Custom("save".into())));
}

#[test]
fn a_filter_sees_tab_escape_back_and_modifier_changes() {
    let passing = pass();
    let mut host = ControlHost::builder().with_filter(&passing).build();
    for k in [Key::Tab, Key::BackTab, Key::Escape, Key::Back] {
        host.deliver_keyboard(key(k));
    }
    host.deliver_event(InputEvent::Modifiers(SHIFT));
    let seen: Vec<InputEvent> = passing.seen.borrow().iter().map(|(e, _)| *e).collect();
    assert_eq!(
        seen,
        [
            InputEvent::Keyboard(key(Key::Tab)),
            InputEvent::Keyboard(key(Key::BackTab)),
            InputEvent::Keyboard(key(Key::Escape)),
            InputEvent::Keyboard(key(Key::Back)),
            InputEvent::Modifiers(SHIFT),
        ]
    );
}

#[test]
fn the_context_reports_pointer_modifiers_focus_and_grab() {
    let r = Recorder::at(0.0);
    let passing = pass();
    let mut host = ControlHost::builder()
        .with_named_control("rec", ControlHandle::new(&r))
        .with_filter(&passing)
        .build();
    host.deliver_event(InputEvent::Modifiers(SHIFT));
    host.deliver_mouse(down(10.0));
    host.deliver_mouse(MouseEvent::Move { pos: [40.0, 20.0] });
    let seen = passing.seen.borrow();
    let (_, on_modifiers) = seen[0];
    assert_eq!(
        on_modifiers.modifiers, SHIFT,
        "recorded before the filter is asked"
    );
    let (_, on_down) = seen[1];
    assert_eq!(on_down.pointer, Some([10.0, 10.0]));
    assert_eq!(on_down.focused, None, "the press has not focused yet");
    let (_, on_move) = seen[2];
    assert_eq!(on_move.pointer, Some([40.0, 20.0]));
    assert_eq!(on_move.focused, Some(ControlKey::named("rec")));
    assert_eq!(on_move.grab, Some(ControlKey::named("rec")));
    assert_eq!(on_move.modifiers, SHIFT);
}

fn to_sink(_: &InputEvent) -> Filtered {
    Filtered::Redirect(ControlKey::named("sink"))
}

#[test]
fn a_redirected_mouse_event_reaches_only_the_target_even_off_its_area() {
    let below = Recorder::at(0.0);
    let sink = Recorder::at(500.0);
    let redirect = Scripted::new(to_sink);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&below))
        .with_named_control(sym!("sink"), ControlHandle::new(&sink))
        .with_filter(&redirect)
        .build();
    let resp = host.deliver_mouse(down(10.0));
    assert_eq!(resp.target_index, Some(1));
    assert_eq!(resp.target_id, Some(sym!("sink")));
    assert_eq!(
        resp.target_part,
        Some(ControlPart::Body),
        "off its area: Body"
    );
    assert!(
        below.take().is_empty(),
        "the control under the pointer hears nothing"
    );
    let calls = sink.take();
    assert!(matches!(
        calls.as_slice(),
        [Call::Mouse(MouseEvent::Down { .. }, _)]
    ));
}

#[test]
fn a_redirected_key_skips_the_hosts_tab_and_escape() {
    let sink = Recorder::at(0.0);
    let redirect = Scripted::new(to_sink);
    let mut host = ControlHost::builder()
        .with_named_control("sink", ControlHandle::new(&sink))
        .with_filter(&redirect)
        .build();
    host.deliver_keyboard(key(Key::Tab));
    host.deliver_keyboard(key(Key::Escape));
    assert_eq!(host.focused_index(), None, "Tab moved no focus");
    let keys: Vec<Key> = sink
        .take()
        .into_iter()
        .filter_map(|c| match c {
            Call::Key(k, _) => k.key(),
            _ => None,
        })
        .collect();
    assert_eq!(
        keys,
        [Key::Tab, Key::Escape],
        "the target gets them instead"
    );
}

#[test]
fn a_redirected_modifier_change_is_consumed() {
    let sink = Recorder::at(0.0);
    let redirect = Scripted::new(to_sink);
    let mut host = ControlHost::builder()
        .with_named_control("sink", ControlHandle::new(&sink))
        .with_filter(&redirect)
        .build();
    let resp = host.deliver_event(InputEvent::Modifiers(SHIFT));
    assert!(resp.handled && resp.target_index.is_none());
    assert!(sink.take().is_empty());
    assert_eq!(host.modifiers(), SHIFT, "still recorded");
}

#[test]
fn a_redirect_to_a_missing_control_is_consumed() {
    let below = Recorder::at(0.0);
    let redirect = Scripted::new(to_sink);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&below))
        .with_filter(&redirect)
        .build();
    let resp = host.deliver_mouse(down(10.0));
    assert!(resp.handled, "input stays trapped");
    assert_eq!(resp.target_index, None);
    assert!(below.take().is_empty(), "nothing falls through to the area");
}

#[test]
fn a_redirect_leaves_focus_alone_and_starts_no_grab() {
    let other = Recorder::at(0.0);
    let sink = Recorder::at(200.0);
    let redirect = Scripted::new(to_sink);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&other))
        .with_named_control("sink", ControlHandle::new(&sink))
        .with_filter(&redirect)
        .build();
    host.set_focused_index(Some(0));
    host.deliver_mouse(down(210.0));
    assert_eq!(
        host.focused_index(),
        Some(0),
        "the press did not move the focus"
    );
    assert_eq!(host.active_drag(), None, "and started no grab");
}

#[test]
fn a_redirect_moves_hover_off_the_hovered_control() {
    let below = Recorder::at(0.0);
    let sink = Recorder::at(200.0);
    let redirecting = Cell::new(false);
    let redirect = Scripted::new(|_| {
        if redirecting.get() {
            to_sink(&InputEvent::Gesture(GestureEvent::DoubleTap))
        } else {
            Filtered::Pass
        }
    });
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&below))
        .with_named_control("sink", ControlHandle::new(&sink))
        .with_filter(&redirect)
        .build();
    host.deliver_mouse(MouseEvent::Move { pos: [10.0, 10.0] });
    below.take();
    redirecting.set(true);
    host.deliver_mouse(MouseEvent::Move { pos: [20.0, 10.0] });
    assert_eq!(
        below.hovers(),
        [None],
        "the hovered control is told it left"
    );
}

#[test]
fn a_redirect_drops_another_controls_grab() {
    let other = Recorder::at(0.0);
    let sink = Recorder::at(200.0);
    let redirecting = Cell::new(false);
    let redirect = Scripted::new(|e| {
        if redirecting.get() {
            to_sink(e)
        } else {
            Filtered::Pass
        }
    });
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&other))
        .with_named_control("sink", ControlHandle::new(&sink))
        .with_filter(&redirect)
        .build();
    host.deliver_mouse(down(10.0));
    assert!(host.active_drag().is_some());
    redirecting.set(true);
    host.deliver_keyboard(key(Key::Enter));
    assert_eq!(host.active_drag(), None, "a redirect drops the grab");
}

#[test]
fn a_target_whose_hit_test_misses_is_never_hovered() {
    let sink = Recorder::at(500.0);
    let redirect = Scripted::new(to_sink);
    let mut host = ControlHost::builder()
        .with_named_control("sink", ControlHandle::new(&sink))
        .with_filter(&redirect)
        .build();
    host.deliver_mouse(MouseEvent::Move { pos: [10.0, 10.0] });
    assert!(sink.hovers().is_empty());
    assert_eq!(host.hovered_key(), None);
}

#[test]
fn one_filter_object_serves_two_builds() {
    struct Counter(Cell<u32>);
    impl InputFilter for Counter {
        fn filter(&self, _: &InputEvent, _: &FilterContext) -> Filtered {
            self.0.set(self.0.get() + 1);
            Filtered::Pass
        }
    }
    let counter = Counter(Cell::new(0));
    for _ in 0..2 {
        let mut host = ControlHost::builder().with_filter(&counter).build();
        host.deliver_keyboard(key(Key::Enter));
    }
    assert_eq!(counter.0.get(), 2);
}

#[test]
fn a_sink_filter_takes_everything_and_leaves_one_hover_clear_beneath() {
    let beneath = Recorder::at(0.0);
    let sink = Recorder::at(500.0);
    let redirecting = Cell::new(false);
    let redirect = Scripted::new(|e| {
        if redirecting.get() {
            to_sink(e)
        } else {
            Filtered::Pass
        }
    });
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&beneath))
        .with_named_control("sink", ControlHandle::new(&sink))
        .with_filter(&redirect)
        .build();
    host.deliver_mouse(MouseEvent::Move { pos: [10.0, 10.0] });
    beneath.take();
    redirecting.set(true);
    host.deliver_mouse(down(10.0));
    host.deliver_mouse(up(10.0));
    host.deliver_keyboard(key(Key::Enter));
    host.deliver_gesture(GestureEvent::Pinch { delta: 1.0 });
    host.deliver_event(InputEvent::Modifiers(SHIFT));
    assert_eq!(
        beneath.take(),
        [Call::Hover(None)],
        "one clear and nothing else"
    );
    let kinds: Vec<&str> = sink
        .take()
        .iter()
        .map(|c| match c {
            Call::Mouse(..) => "mouse",
            Call::Key(..) => "key",
            Call::Gesture(..) => "gesture",
            Call::Hover(_) => "hover",
            Call::Focus(_) => "focus",
        })
        .collect();
    assert_eq!(kinds, ["mouse", "mouse", "key", "gesture"]);
}

/// The example in README.md's "Input routing" section.
struct SaveShortcut;
impl InputFilter for SaveShortcut {
    fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered {
        match event {
            InputEvent::Keyboard(KeyboardEvent::KeyDown {
                key: Key::Char('s'),
            }) if ctx.modifiers.ctrl => {
                Filtered::Consumed(Some(ControlAction::Custom("save".into())))
            }
            _ => Filtered::Pass,
        }
    }
}

#[test]
fn the_readme_shortcut_filter_consumes_ctrl_s() {
    let r = Recorder::at(0.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&r))
        .with_initial_focus(0)
        .with_filter(&SaveShortcut)
        .build();
    r.take();
    let plain = host.deliver_keyboard(key(Key::Char('s')));
    assert_eq!(plain.target_index, Some(0), "a bare s goes to the control");
    host.deliver_event(InputEvent::Modifiers(Modifiers {
        ctrl: true,
        ..Modifiers::default()
    }));
    r.take();
    let save = host.deliver_keyboard(key(Key::Char('s')));
    assert_eq!(save.action, Some(ControlAction::Custom("save".into())));
    assert!(r.take().is_empty(), "Ctrl+S never reaches the control");
}
