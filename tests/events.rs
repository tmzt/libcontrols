//! Gesture, modifier and pixel-scroll events, and controls written before them.

use std::cell::{Cell, RefCell};

use libcontrols::{
    ControlAction, ControlHandle, ControlHost, ControlPart, DrawContext, DrawControl, GestureEvent,
    HostedControl, InputEvent, KeyboardEvent, Modifiers, MouseButton, MouseEvent, Rect, Slider,
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

    fn handle_mouse(&self, event: &MouseEvent, part: ControlPart) -> Option<ControlAction> {
        self.seen.borrow_mut().push(*event);
        matches!(event, MouseEvent::Down { .. }).then_some(ControlAction::Activated { part })
    }

    fn handle_keyboard(&self, _event: &KeyboardEvent) -> Option<ControlAction> {
        None
    }
}

/// A control using the new hooks.
struct Recorder {
    rect: Rect,
    gestures: RefCell<Vec<GestureEvent>>,
    modifiers: Cell<Modifiers>,
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

    fn handle_mouse(&self, _event: &MouseEvent, _part: ControlPart) -> Option<ControlAction> {
        None
    }

    fn handle_keyboard(&self, _event: &KeyboardEvent) -> Option<ControlAction> {
        None
    }

    fn handle_gesture(&self, event: &GestureEvent, _part: ControlPart) -> Option<ControlAction> {
        self.gestures.borrow_mut().push(*event);
        Some(ControlAction::Custom("gesture".into()))
    }

    fn set_modifiers(&self, modifiers: Modifiers) {
        self.modifiers.set(modifiers);
    }
}

fn recorder(x: f32) -> Recorder {
    Recorder {
        rect: Rect::new(x, 0.0, 100.0, 100.0),
        gestures: RefCell::new(Vec::new()),
        modifiers: Cell::new(Modifiers::default()),
    }
}

#[test]
fn a_control_written_against_the_old_trait_behaves_the_same() {
    let legacy = Legacy {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        seen: RefCell::new(Vec::new()),
    };
    let mut host = ControlHost::builder()
        .with_named_control("legacy", ControlHandle::new(&legacy))
        .build();
    let down = host.deliver_mouse(MouseEvent::Down {
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
    assert_eq!(host.focused_index(), Some(0));
    let up = host.deliver_mouse(MouseEvent::Up {
        pos: [500.0, 10.0],
        button: MouseButton::Left,
    });
    assert!(up.handled, "the grab still lets go off the control");

    // The new events reach it through the defaults and change nothing.
    let pinch = host.deliver_event(InputEvent::Gesture(GestureEvent::Pinch { delta: 0.2 }));
    assert_eq!(pinch.action, None);
    let mods = host.deliver_event(InputEvent::Modifiers(Modifiers {
        shift: true,
        ..Modifiers::default()
    }));
    assert!(!mods.handled);
    assert_eq!(legacy.seen.borrow().len(), 2);
}

#[test]
fn an_existing_slider_reads_line_scrolls_and_ignores_pixel_ones() {
    let slider = Slider::new(50.0, 0.0, 100.0, Rect::new(0.0, 0.0, 200.0, 20.0));
    let mut host = ControlHost::builder().with_control(slider.handle()).build();
    let pixels = host.deliver_mouse(MouseEvent::ScrollPixels {
        pos: [100.0, 10.0],
        delta: [0.0, 30.0],
    });
    assert!(pixels.handled);
    assert_eq!(pixels.action, None);
    let lines = host.deliver_mouse(MouseEvent::Scroll {
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
    let left = recorder(0.0);
    let right = recorder(200.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&left))
        .with_control(ControlHandle::new(&right))
        .build();

    let before = host.deliver_event(InputEvent::Gesture(GestureEvent::DoubleTap));
    assert!(!before.handled, "no pointer yet, so no target");

    host.deliver_mouse(MouseEvent::Move { pos: [250.0, 50.0] });
    assert_eq!(host.pointer(), Some([250.0, 50.0]));
    let turn = host.deliver_event(InputEvent::Gesture(GestureEvent::Rotate { delta: 0.5 }));
    assert_eq!(turn.target_index, Some(1));
    assert_eq!(turn.action, Some(ControlAction::Custom("gesture".into())));
    assert!(left.gestures.borrow().is_empty());
    assert_eq!(
        right.gestures.borrow().as_slice(),
        &[GestureEvent::Rotate { delta: 0.5 }]
    );
}

#[test]
fn modifiers_are_recorded_and_told_to_every_control() {
    let a = recorder(0.0);
    let b = recorder(200.0);
    let mut host = ControlHost::builder()
        .with_control(ControlHandle::new(&a))
        .with_control(ControlHandle::new(&b))
        .build();
    let held = Modifiers {
        alt: true,
        meta: true,
        ..Modifiers::default()
    };
    host.deliver_event(InputEvent::Modifiers(held));
    assert_eq!(host.modifiers(), held);
    assert_eq!(a.modifiers.get(), held);
    assert_eq!(b.modifiers.get(), held);
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
