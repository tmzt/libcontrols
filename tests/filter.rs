//! The global filter chain: app-wide rules a root router asks before routing.
//!
//! Ported from `tests/filter.rs` (the `ControlHost` era). Filters added with
//! `FilterStack::add` are owned by the chain, so the logs a test reads are
//! shared through `Rc`s; redirect targets are `NodeId`s captured after `add`.

mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::{Call, Recorder};
use libcontrols::filter::{FilterContext, FilterStack, Filtered, InputFilter};
use libcontrols::{
    ControlAction, ControlPart, GestureEvent, InputEvent, Key, KeyboardEvent, Modifiers,
    MouseButton, MouseEvent, NodeId, Router,
};

type Seen = Rc<RefCell<Vec<(InputEvent, FilterContext)>>>;

/// A filter that logs what it is asked and answers with a fixed rule.
struct Scripted<F: Fn(&InputEvent) -> Filtered> {
    answer: F,
    seen: Seen,
}

impl<F: Fn(&InputEvent) -> Filtered> InputFilter for Scripted<F> {
    fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered {
        self.seen.borrow_mut().push((*event, *ctx));
        (self.answer)(event)
    }
}

/// Add a scripted filter to `stack`; returns its log.
fn install(stack: &FilterStack, answer: impl Fn(&InputEvent) -> Filtered + 'static) -> Seen {
    let seen = Seen::default();
    stack.add(Scripted {
        answer,
        seen: seen.clone(),
    });
    seen
}

/// Add a filter that passes everything; returns its log.
fn pass(stack: &FilterStack) -> Seen {
    install(stack, |_| Filtered::Pass)
}

/// A router given a fresh filter chain, and a handle to that chain.
fn filtered_router() -> (Router, FilterStack) {
    let stack = FilterStack::new();
    (Router::new().with_filter_stack(stack.clone()), stack)
}

/// The recorder a node owns.
fn rec(router: &Router, id: NodeId) -> &Recorder {
    router.get::<Recorder>(id).expect("a recorder node")
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
fn script(router: &Router) {
    router.deliver_mouse(MouseEvent::Move { pos: [10.0, 10.0] });
    router.deliver_mouse(down(10.0));
    router.deliver_mouse(up(210.0));
    router.deliver_keyboard(key(Key::Tab));
    router.deliver_keyboard(key(Key::Enter));
    router.deliver_gesture(GestureEvent::DoubleTap);
    router.deliver_event(InputEvent::Modifiers(SHIFT));
}

#[test]
fn with_no_filter_registered_routing_is_unchanged() {
    let mut plain = Router::new();
    let plain_ids = [
        plain.add(None, Recorder::at(0.0)),
        plain.add(None, Recorder::at(200.0)),
    ];
    script(&plain);

    let (mut filtered, stack) = filtered_router();
    let passing = pass(&stack);
    let filtered_ids = [
        filtered.add(None, Recorder::at(0.0)),
        filtered.add(None, Recorder::at(200.0)),
    ];
    script(&filtered);

    for (&a, &b) in plain_ids.iter().zip(&filtered_ids) {
        assert_eq!(
            rec(&plain, a).take(),
            rec(&filtered, b).take(),
            "a passing filter must change nothing"
        );
    }
    assert_eq!(passing.borrow().len(), 7, "but it sees every event");
}

#[test]
fn the_last_filter_added_is_asked_first() {
    let order = Rc::new(RefCell::new(Vec::new()));
    struct Named(&'static str, Rc<RefCell<Vec<&'static str>>>);
    impl InputFilter for Named {
        fn filter(&self, _: &InputEvent, _: &FilterContext) -> Filtered {
            self.1.borrow_mut().push(self.0);
            Filtered::Pass
        }
    }
    let (router, stack) = filtered_router();
    stack.add(Named("shortcuts", order.clone()));
    stack.add(Named("modal", order.clone()));
    router.deliver_keyboard(key(Key::Enter));
    assert_eq!(*order.borrow(), ["modal", "shortcuts"]);
}

#[test]
fn a_passing_filter_leaves_routing_alone() {
    let (mut router, stack) = filtered_router();
    let id = router.add(None, Recorder::at(0.0));
    pass(&stack);
    let resp = router.deliver_mouse(down(10.0));
    assert_eq!(resp.target, Some(id));
    assert_eq!(router.focused(), Some(id));
    assert!(
        rec(&router, id)
            .take()
            .iter()
            .any(|c| matches!(c, Call::Mouse(..)))
    );
}

#[test]
fn a_consumed_event_reaches_no_control_and_reports_handled() {
    let (mut router, stack) = filtered_router();
    let id = router.add(None, Recorder::at(0.0));
    install(&stack, |_| Filtered::Consumed(None));
    let resp = router.deliver_mouse(down(10.0));
    assert!(resp.handled, "a consumed event is handled");
    assert_eq!(resp.target, None, "but names no node");
    assert!(
        rec(&router, id).take().is_empty(),
        "no control sees it, not even hover"
    );
    assert_eq!(router.focused(), None);
}

#[test]
fn a_consumed_action_is_returned_to_the_app() {
    let (router, stack) = filtered_router();
    install(&stack, |_| {
        Filtered::Consumed(Some(ControlAction::Custom("save".into())))
    });
    let resp = router.deliver_keyboard(key(Key::Char('s')));
    assert_eq!(resp.action, Some(ControlAction::Custom("save".into())));
}

#[test]
fn a_filter_sees_tab_escape_back_and_modifier_changes() {
    let (router, stack) = filtered_router();
    let passing = pass(&stack);
    for k in [Key::Tab, Key::BackTab, Key::Escape, Key::Back] {
        router.deliver_keyboard(key(k));
    }
    router.deliver_event(InputEvent::Modifiers(SHIFT));
    let seen: Vec<InputEvent> = passing.borrow().iter().map(|(e, _)| *e).collect();
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
    let (mut router, stack) = filtered_router();
    let id = router
        .add_named(None, "rec", Recorder::at(0.0))
        .expect("a free alias");
    let passing = pass(&stack);
    router.deliver_event(InputEvent::Modifiers(SHIFT));
    router.deliver_mouse(down(10.0));
    router.deliver_mouse(MouseEvent::Move { pos: [40.0, 20.0] });
    let seen = passing.borrow();
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
    assert_eq!(on_move.focused, Some(id));
    assert_eq!(on_move.grab, Some(id));
    assert_eq!(on_move.modifiers, SHIFT);
}

/// A rule that sends everything to `sink`.
fn to_sink(sink: NodeId) -> impl Fn(&InputEvent) -> Filtered + 'static {
    move |_| Filtered::Redirect(sink)
}

#[test]
fn a_redirected_mouse_event_reaches_only_the_target_even_off_its_area() {
    let (mut router, stack) = filtered_router();
    let below = router.add(None, Recorder::at(0.0));
    let sink = router
        .add_named(None, "sink", Recorder::at(500.0))
        .expect("a free alias");
    install(&stack, to_sink(sink));
    let resp = router.deliver_mouse(down(10.0));
    assert_eq!(resp.target, Some(sink));
    assert_eq!(router.find("sink"), resp.target, "the node named sink");
    assert_eq!(
        resp.target_part,
        Some(ControlPart::Body),
        "off its area: Body"
    );
    assert!(
        rec(&router, below).take().is_empty(),
        "the control under the pointer hears nothing"
    );
    let calls = rec(&router, sink).take();
    assert!(matches!(
        calls.as_slice(),
        [Call::Mouse(MouseEvent::Down { .. }, _)]
    ));
}

#[test]
fn a_redirected_key_skips_the_hosts_tab_and_escape() {
    let (mut router, stack) = filtered_router();
    let sink = router.add(None, Recorder::at(0.0));
    install(&stack, to_sink(sink));
    router.deliver_keyboard(key(Key::Tab));
    router.deliver_keyboard(key(Key::Escape));
    assert_eq!(router.focused(), None, "Tab moved no focus");
    let keys: Vec<Key> = rec(&router, sink)
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
    let (mut router, stack) = filtered_router();
    let sink = router.add(None, Recorder::at(0.0));
    install(&stack, to_sink(sink));
    let resp = router.deliver_event(InputEvent::Modifiers(SHIFT));
    assert!(resp.handled && resp.target.is_none());
    assert!(rec(&router, sink).take().is_empty());
    assert_eq!(router.modifiers(), SHIFT, "still recorded");
}

/// The missing target is a removed node's id, which never resolves again.
#[test]
fn a_redirect_to_a_missing_control_is_consumed() {
    let (mut router, stack) = filtered_router();
    let below = router.add(None, Recorder::at(0.0));
    let gone = router.add(None, Recorder::at(500.0));
    router.remove(gone);
    install(&stack, to_sink(gone));
    let resp = router.deliver_mouse(down(10.0));
    assert!(resp.handled, "input stays trapped");
    assert_eq!(resp.target, None);
    assert!(
        rec(&router, below).take().is_empty(),
        "nothing falls through to the area"
    );
}

#[test]
fn a_redirect_leaves_focus_alone_and_starts_no_grab() {
    let (mut router, stack) = filtered_router();
    let other = router.add(None, Recorder::at(0.0));
    let sink = router.add(None, Recorder::at(200.0));
    install(&stack, to_sink(sink));
    router.set_focus(Some(other));
    router.deliver_mouse(down(210.0));
    assert_eq!(
        router.focused(),
        Some(other),
        "the press did not move the focus"
    );
    assert_eq!(router.active_drag(), None, "and started no grab");
}

/// A rule that passes until `on` is set, then sends everything to `sink`.
fn switched_to_sink(on: &Rc<Cell<bool>>, sink: NodeId) -> impl Fn(&InputEvent) -> Filtered + use<> {
    let on = on.clone();
    move |_| {
        if on.get() {
            Filtered::Redirect(sink)
        } else {
            Filtered::Pass
        }
    }
}

#[test]
fn a_redirect_moves_hover_off_the_hovered_control() {
    let (mut router, stack) = filtered_router();
    let below = router.add(None, Recorder::at(0.0));
    let sink = router.add(None, Recorder::at(200.0));
    let redirecting = Rc::new(Cell::new(false));
    install(&stack, switched_to_sink(&redirecting, sink));
    router.deliver_mouse(MouseEvent::Move { pos: [10.0, 10.0] });
    rec(&router, below).take();
    redirecting.set(true);
    router.deliver_mouse(MouseEvent::Move { pos: [20.0, 10.0] });
    assert_eq!(
        rec(&router, below).hovers(),
        [None],
        "the hovered control is told it left"
    );
}

#[test]
fn a_redirect_drops_another_controls_grab() {
    let (mut router, stack) = filtered_router();
    router.add(None, Recorder::at(0.0));
    let sink = router.add(None, Recorder::at(200.0));
    let redirecting = Rc::new(Cell::new(false));
    install(&stack, switched_to_sink(&redirecting, sink));
    router.deliver_mouse(down(10.0));
    assert!(router.active_drag().is_some());
    redirecting.set(true);
    router.deliver_keyboard(key(Key::Enter));
    assert_eq!(router.active_drag(), None, "a redirect drops the grab");
}

#[test]
fn a_target_whose_hit_test_misses_is_never_hovered() {
    let (mut router, stack) = filtered_router();
    let sink = router.add(None, Recorder::at(500.0));
    install(&stack, to_sink(sink));
    router.deliver_mouse(MouseEvent::Move { pos: [10.0, 10.0] });
    assert!(rec(&router, sink).hovers().is_empty());
    assert_eq!(router.hovered(), None);
}

/// Was `one_filter_object_serves_two_builds`: there are no builds; one
/// filter object, pushed by `Rc`, serves two chains, and leaves both when
/// its last `Rc` is dropped.
#[test]
fn one_filter_object_serves_two_chains() {
    struct Counter(Rc<Cell<u32>>);
    impl InputFilter for Counter {
        fn filter(&self, _: &InputEvent, _: &FilterContext) -> Filtered {
            self.0.set(self.0.get() + 1);
            Filtered::Pass
        }
    }
    let count = Rc::new(Cell::new(0));
    let rule: Rc<dyn InputFilter> = Rc::new(Counter(count.clone()));
    let stacks = [FilterStack::new(), FilterStack::new()];
    let routers: Vec<Router> = stacks
        .iter()
        .map(|s| {
            s.push(&rule);
            Router::new().with_filter_stack(s.clone())
        })
        .collect();
    for router in &routers {
        router.deliver_keyboard(key(Key::Enter));
    }
    assert_eq!(count.get(), 2);
    drop(rule);
    for router in &routers {
        router.deliver_keyboard(key(Key::Enter));
    }
    assert_eq!(count.get(), 2, "a dropped rule is asked no more");
    assert!(stacks.iter().all(FilterStack::is_empty), "and leaves both");
}

#[test]
fn a_sink_filter_takes_everything_and_leaves_one_hover_clear_beneath() {
    let (mut router, stack) = filtered_router();
    let beneath = router.add(None, Recorder::at(0.0));
    let sink = router.add(None, Recorder::at(500.0));
    let redirecting = Rc::new(Cell::new(false));
    install(&stack, switched_to_sink(&redirecting, sink));
    router.deliver_mouse(MouseEvent::Move { pos: [10.0, 10.0] });
    rec(&router, beneath).take();
    redirecting.set(true);
    router.deliver_mouse(down(10.0));
    router.deliver_mouse(up(10.0));
    router.deliver_keyboard(key(Key::Enter));
    router.deliver_gesture(GestureEvent::Pinch { delta: 1.0 });
    router.deliver_event(InputEvent::Modifiers(SHIFT));
    assert_eq!(
        rec(&router, beneath).take(),
        [Call::Hover(None)],
        "one clear and nothing else"
    );
    let kinds: Vec<&str> = rec(&router, sink)
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
    let (mut router, stack) = filtered_router();
    let id = router.add(None, Recorder::at(0.0));
    router.set_focus(Some(id));
    stack.add(SaveShortcut);
    rec(&router, id).take();
    let plain = router.deliver_keyboard(key(Key::Char('s')));
    assert_eq!(plain.target, Some(id), "a bare s goes to the control");
    router.deliver_event(InputEvent::Modifiers(Modifiers {
        ctrl: true,
        ..Modifiers::default()
    }));
    rec(&router, id).take();
    let save = router.deliver_keyboard(key(Key::Char('s')));
    assert_eq!(save.action, Some(ControlAction::Custom("save".into())));
    assert!(
        rec(&router, id).take().is_empty(),
        "Ctrl+S never reaches the control"
    );
}
