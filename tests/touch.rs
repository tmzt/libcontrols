//! The finger count and the touch tracker
//! (docs/design/20261119_01_RIBBON_TWO_FINGER_AND_THUMB_LABEL_PLAN.md,
//! step 3, with the tracker narrowed on the owner's answer).

mod common;

use std::sync::Arc;

use common::{Call, Recorder};
use libcontrols::{
    Button, ButtonState, InputEvent, MouseButton, MouseEvent, Rect, Router, TouchPhase,
    TouchTracker,
};

const LEFT: MouseButton = MouseButton::Left;

/// A host's own one-finger touch mapping, as flankr's drawer has it:
/// `Down` on the start, `Drag` on a move, `Up` on the end, and its tap
/// detection's `Click` after an `Up` that stayed within the slop.
#[derive(Default)]
struct OwnMapping {
    press: Option<(u64, [f32; 2], bool)>,
}

impl OwnMapping {
    const SLOP: f32 = 10.0;

    fn touch(&mut self, router: &Router, id: u64, phase: TouchPhase, pos: [f32; 2]) {
        match phase {
            TouchPhase::Started => {
                if self.press.is_some() {
                    return;
                }
                self.press = Some((id, pos, false));
                router.deliver_mouse(MouseEvent::Down { pos, button: LEFT });
            }
            TouchPhase::Moved => {
                let Some(p) = self.press.as_mut().filter(|p| p.0 == id) else {
                    return;
                };
                if (pos[0] - p.1[0]).hypot(pos[1] - p.1[1]) > Self::SLOP {
                    p.2 = true;
                }
                router.deliver_mouse(MouseEvent::Drag {
                    start_pos: p.1,
                    current_pos: pos,
                    delta: [0.0, 0.0],
                    button: LEFT,
                });
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let Some(p) = self.press.take().filter(|p| p.0 == id) else {
                    return;
                };
                router.deliver_mouse(MouseEvent::Up { pos, button: LEFT });
                if phase == TouchPhase::Ended && !p.2 {
                    router.deliver_mouse(MouseEvent::Click { pos, button: LEFT });
                }
            }
        }
    }
}

/// The same host with a tracker: its own mapping while the tracker passes
/// a touch through, the tracker's events (and no press of its own) after.
#[derive(Default)]
struct TrackedHost {
    own: OwnMapping,
    tracker: TouchTracker,
}

impl TrackedHost {
    fn touch(&mut self, router: &Router, id: u64, phase: TouchPhase, pos: [f32; 2]) {
        let out = self.tracker.touch(id, phase, pos);
        if let Some(n) = out.fingers {
            router.set_fingers(n);
        }
        if out.pass_through {
            self.own.touch(router, id, phase, pos);
        } else {
            self.own.press = None;
            for e in out.events {
                router.deliver_event(e);
            }
        }
    }
}

fn button_router() -> (Router, Arc<ButtonState>) {
    let mut router = Router::new();
    let b = Button::new("b", Rect::new(0.0, 0.0, 100.0, 100.0));
    let state = Arc::clone(&b.state);
    router.add(None, b);
    (router, state)
}

#[test]
fn a_one_finger_tap_through_the_tracker_clicks_as_without_it() {
    let (plain, plain_state) = button_router();
    let mut own = OwnMapping::default();
    own.touch(&plain, 1, TouchPhase::Started, [50.0, 50.0]);
    own.touch(&plain, 1, TouchPhase::Moved, [52.0, 51.0]);
    own.touch(&plain, 1, TouchPhase::Ended, [52.0, 51.0]);
    assert!(plain_state.take_click(), "the host's own tap clicks");

    let (tracked, tracked_state) = button_router();
    let mut host = TrackedHost::default();
    host.touch(&tracked, 1, TouchPhase::Started, [50.0, 50.0]);
    assert_eq!(tracked.fingers(), 1);
    host.touch(&tracked, 1, TouchPhase::Moved, [52.0, 51.0]);
    host.touch(&tracked, 1, TouchPhase::Ended, [52.0, 51.0]);
    assert!(
        tracked_state.take_click(),
        "the same click through the tracker"
    );
    assert_eq!(tracked.fingers(), 0);
}

#[test]
fn a_two_finger_tap_gives_no_click() {
    let (router, state) = button_router();
    // A host that sends its own Click anyway: the router drops it.
    router.set_fingers(1);
    router.deliver_mouse(MouseEvent::Down {
        pos: [50.0, 50.0],
        button: LEFT,
    });
    router.set_fingers(2);
    router.set_fingers(1);
    router.set_fingers(0);
    router.deliver_mouse(MouseEvent::Up {
        pos: [50.0, 50.0],
        button: LEFT,
    });
    let resp = router.deliver_mouse(MouseEvent::Click {
        pos: [50.0, 50.0],
        button: LEFT,
    });
    assert!(resp.handled && resp.target.is_none());
    assert!(!state.take_click());
    // The next one-finger press clicks again.
    router.set_fingers(1);
    router.deliver_mouse(MouseEvent::Down {
        pos: [50.0, 50.0],
        button: LEFT,
    });
    router.set_fingers(0);
    router.deliver_mouse(MouseEvent::Up {
        pos: [50.0, 50.0],
        button: LEFT,
    });
    router.deliver_mouse(MouseEvent::Click {
        pos: [50.0, 50.0],
        button: LEFT,
    });
    assert!(state.take_click());
    // Through the tracker, two fingers never click either.
    let mut host = TrackedHost::default();
    host.touch(&router, 1, TouchPhase::Started, [50.0, 50.0]);
    host.touch(&router, 2, TouchPhase::Started, [60.0, 50.0]);
    host.touch(&router, 2, TouchPhase::Ended, [60.0, 50.0]);
    host.touch(&router, 1, TouchPhase::Ended, [50.0, 50.0]);
    assert!(!state.take_click());
}

#[test]
fn the_finger_count_and_the_pair_lift_reach_a_control() {
    let mut router = Router::new();
    let id = router.add(None, Recorder::at(0.0));
    let rec = || router.get::<Recorder>(id).unwrap();
    router.advance(1.0);
    router.set_fingers(1);
    router.deliver_mouse(MouseEvent::Down {
        pos: [50.0, 50.0],
        button: LEFT,
    });
    router.set_fingers(2);
    router.deliver_event(InputEvent::Mouse(MouseEvent::Drag {
        start_pos: [50.0, 50.0],
        current_pos: [50.0, 52.0],
        delta: [0.0, 2.0],
        button: LEFT,
    }));
    router.advance(0.25);
    router.set_fingers(1);
    router.deliver_mouse(MouseEvent::Drag {
        start_pos: [50.0, 50.0],
        current_pos: [50.0, 54.0],
        delta: [0.0, 2.0],
        button: LEFT,
    });
    let seen: Vec<(u8, Option<f64>)> = rec()
        .take()
        .into_iter()
        .filter_map(|c| match c {
            Call::Mouse(_, ctx) => Some((ctx.fingers, ctx.pair_lifted_at)),
            _ => None,
        })
        .collect();
    assert_eq!(seen, [(1, None), (2, None), (1, Some(1.25))]);
}
