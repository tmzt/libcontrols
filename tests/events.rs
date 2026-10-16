//! Gesture, modifier and pixel-scroll events, and controls written before them, in a router.

use std::cell::RefCell;

use libcontrols::{
    ControlAction, ControlPart, DrawContext, DrawControl, EventContext, FocusEvent, GestureEvent,
    HostedControl, InputEvent, Key, KeyboardEvent, Modifiers, MouseButton, MouseEvent, Rect,
    Router, Slider,
};
use libmsdf::drawlist::DrawList;

/// A control implementing only the methods 0.1 had.
struct Legacy {
    rect: Rect,
    seen: RefCell<Vec<MouseEvent>>,
}

impl DrawControl for Legacy {
    fn draw(&self, _list: &mut DrawList, _ctx: &DrawContext) {}
}

impl HostedControl for Legacy {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.rect
            .contains(pos)
            .then(|| (ControlPart::Body, self.rect.normalized_x(pos[0])))
    }

    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.seen.borrow_mut().push(*event);
        matches!(event, MouseEvent::Down { .. })
            .then_some(ControlAction::Activated { part: ctx.part })
    }

    fn handle_keyboard(
        &self,
        _event: &KeyboardEvent,
        _ctx: &EventContext,
    ) -> Option<ControlAction> {
        None
    }
}

/// A control using the new hooks.
struct Recorder {
    rect: Rect,
    gestures: RefCell<Vec<GestureEvent>>,
    focus: RefCell<Vec<FocusEvent>>,
}

impl DrawControl for Recorder {
    fn draw(&self, _list: &mut DrawList, _ctx: &DrawContext) {}
}

impl HostedControl for Recorder {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.rect.contains(pos).then_some((ControlPart::Body, 0.0))
    }

    fn handle_mouse(&self, _event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        None
    }

    fn handle_keyboard(
        &self,
        _event: &KeyboardEvent,
        _ctx: &EventContext,
    ) -> Option<ControlAction> {
        None
    }

    fn handle_gesture(&self, event: &GestureEvent, _ctx: &EventContext) -> Option<ControlAction> {
        self.gestures.borrow_mut().push(*event);
        Some(ControlAction::Custom("gesture".into()))
    }

    fn handle_focus(&self, event: FocusEvent) {
        self.focus.borrow_mut().push(event);
    }
}

fn recorder(x: f32) -> Recorder {
    Recorder {
        rect: Rect::new(x, 0.0, 100.0, 100.0),
        gestures: RefCell::new(Vec::new()),
        focus: RefCell::new(Vec::new()),
    }
}

#[test]
fn a_control_written_against_the_old_trait_behaves_the_same() {
    let legacy = Legacy {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        seen: RefCell::new(Vec::new()),
    };
    let mut router = Router::new();
    let id = router.add_named(None, "legacy", legacy).unwrap();
    let down = router.deliver_mouse(MouseEvent::Down {
        pos: [10.0, 10.0],
        button: MouseButton::Left,
    });
    assert!(down.handled);
    assert_eq!(
        down.action,
        Some(ControlAction::Activated {
            part: ControlPart::Body
        })
    );
    assert_eq!(router.focused(), Some(id));
    let up = router.deliver_mouse(MouseEvent::Up {
        pos: [500.0, 10.0],
        button: MouseButton::Left,
    });
    assert!(up.handled, "the grab still lets go off the control");

    // The new events reach it through the defaults and change nothing.
    let pinch = router.deliver_event(InputEvent::Gesture(GestureEvent::Pinch { delta: 0.2 }));
    assert_eq!(pinch.action, None);
    let mods = router.deliver_event(InputEvent::Modifiers(Modifiers {
        shift: true,
        ..Modifiers::default()
    }));
    assert!(!mods.handled);
    let legacy = router.get::<Legacy>(id).unwrap();
    assert_eq!(legacy.seen.borrow().len(), 2);
}

#[test]
fn an_existing_slider_reads_line_scrolls_and_ignores_pixel_ones() {
    let slider = Slider::new(50.0, 0.0, 100.0, Rect::new(0.0, 0.0, 200.0, 20.0));
    let mut router = Router::new();
    router.add(None, slider);
    let pixels = router.deliver_mouse(MouseEvent::ScrollPixels {
        pos: [100.0, 10.0],
        delta: [0.0, 30.0],
    });
    assert!(pixels.handled);
    assert_eq!(pixels.action, None);
    let lines = router.deliver_mouse(MouseEvent::Scroll {
        pos: [100.0, 10.0],
        delta: [0.0, 1.0],
    });
    assert!(matches!(
        lines.action,
        Some(ControlAction::ValueChange { .. })
    ));
}

#[test]
fn a_gesture_goes_to_the_control_under_the_last_pointer() {
    let mut router = Router::new();
    let left = router.add(None, recorder(0.0));
    let right = router.add(None, recorder(200.0));

    let before = router.deliver_event(InputEvent::Gesture(GestureEvent::DoubleTap));
    assert!(!before.handled, "no pointer yet, so no target");

    router.deliver_mouse(MouseEvent::Move { pos: [250.0, 50.0] });
    assert_eq!(router.pointer(), Some([250.0, 50.0]));
    let turn = router.deliver_event(InputEvent::Gesture(GestureEvent::Rotate { delta: 0.5 }));
    assert_eq!(turn.target, Some(right));
    let (left, right) = (
        router.get::<Recorder>(left).unwrap(),
        router.get::<Recorder>(right).unwrap(),
    );
    assert_eq!(turn.action, Some(ControlAction::Custom("gesture".into())));
    assert!(left.gestures.borrow().is_empty());
    assert_eq!(
        right.gestures.borrow().as_slice(),
        &[GestureEvent::Rotate { delta: 0.5 }]
    );
}

#[test]
fn modifiers_are_recorded_and_told_to_no_control() {
    let mut router = Router::new();
    let a = router.add(None, recorder(0.0));
    let b = router.add(None, recorder(200.0));
    let held = Modifiers {
        alt: true,
        meta: true,
        ..Modifiers::default()
    };
    let resp = router.deliver_event(InputEvent::Modifiers(held));
    assert_eq!(router.modifiers(), held);
    assert!(!resp.handled, "no control is told of a modifier change");
    for id in [a, b] {
        let r = router.get::<Recorder>(id).unwrap();
        assert!(r.gestures.borrow().is_empty() && r.focus.borrow().is_empty());
    }
}

#[test]
fn pixel_scroll_has_a_position_and_no_button() {
    let event = MouseEvent::ScrollPixels {
        pos: [3.0, 4.0],
        delta: [1.0, 2.0],
    };
    assert_eq!(event.pos(), [3.0, 4.0]);
    assert_eq!(event.button(), None);
}

/// A control that logs the keys it is given.
struct KeyLog {
    rect: Rect,
    keys: RefCell<Vec<KeyboardEvent>>,
}

impl DrawControl for KeyLog {
    fn draw(&self, _list: &mut DrawList, _ctx: &DrawContext) {}
}

impl HostedControl for KeyLog {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.rect.contains(pos).then_some((ControlPart::Body, 0.0))
    }

    fn handle_mouse(&self, _event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        None
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, _ctx: &EventContext) -> Option<ControlAction> {
        self.keys.borrow_mut().push(*event);
        None
    }
}

fn key_log() -> KeyLog {
    KeyLog {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        keys: RefCell::new(Vec::new()),
    }
}

#[test]
fn back_goes_to_the_focused_control_and_keeps_focus() {
    let mut router = Router::new();
    let id = router.add(None, key_log());
    router.set_focus(Some(id));
    let back = KeyboardEvent::KeyDown { key: Key::Back };
    let resp = router.deliver_keyboard(back);
    assert_eq!(resp.target, Some(id), "Back is an ordinary key");
    let log = router.get::<KeyLog>(id).unwrap();
    assert_eq!(log.keys.borrow().as_slice(), &[back]);
    assert_eq!(router.focused(), Some(id), "Back must not clear the focus");
}

#[test]
fn escape_still_clears_focus() {
    let mut router = Router::new();
    let id = router.add(None, key_log());
    router.set_focus(Some(id));
    router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Escape });
    assert_eq!(router.focused(), None, "Escape clears the focus");
    let log = router.get::<KeyLog>(id).unwrap();
    assert!(log.keys.borrow().is_empty(), "and reaches no control");
}
