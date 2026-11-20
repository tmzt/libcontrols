//! Touches to the router's pointer events for a host with touch ids
//! (docs/design/20261119_01_RIBBON_TWO_FINGER_AND_THUMB_LABEL_PLAN.md,
//! section 4.2, as narrowed in `CHANGES.md` on the owner's answer).
//!
//! The router's input is the mouse model, one pointer. A host keeps its
//! own touch-to-mouse mapping (its `Down`, `Drag`, `Up` and the `Click` of
//! its tap detection) for one finger, exactly as before, and gives every
//! touch to a [`TouchTracker`] too. The tracker adds only what is new:
//!
//! - the finger count, for [`Router::set_fingers`](crate::Router::set_fingers),
//!   whenever it changes;
//! - once a second finger comes down, the rest of that gesture: a `Drag`
//!   at the fingers' summed mean position on every move, so a finger
//!   joining or leaving does not jump the pointer (flankr's `Drag::pos`),
//!   and the `Up` at that position when the last finger lifts. For those
//!   touches [`TouchOutput::pass_through`] is `false`: the host delivers
//!   the tracker's events instead of its own mapping's, and drops its own
//!   press (no `Click`: two fingers are never a tap).

use crate::event::{InputEvent, MouseButton, MouseEvent};

/// A touch's phase, as the host's platform reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// What one touch gives.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct TouchOutput {
    /// The router events to deliver, in order, in place of the host's own
    /// mapping (empty while `pass_through`).
    pub events: Vec<InputEvent>,
    /// The finger count, when it changed: the host calls
    /// `router.set_fingers(n)` before delivering this output's events, or
    /// its own mapping's.
    pub fingers: Option<u8>,
    /// The host's own one-finger mapping handles this touch, as it did
    /// before the tracker: true until a second finger joins the gesture,
    /// false from then until its last finger lifts.
    pub pass_through: bool,
}

/// One finger down.
#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    pos: [f32; 2],
}

/// The fingers down by id and, once a gesture has had two, its summed
/// mean position.
#[derive(Clone, Debug, Default)]
pub struct TouchTracker {
    fingers: Vec<Finger>,
    /// Where the gesture's first finger came down.
    start: [f32; 2],
    /// The summed mean position, once the gesture is the tracker's.
    multi: Option<[f32; 2]>,
}

impl TouchTracker {
    /// No fingers down.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many fingers are down.
    pub fn fingers(&self) -> u8 {
        self.fingers.len().min(u8::MAX as usize) as u8
    }

    /// One touch: the events to deliver, the finger count when it changed,
    /// and whether the host's own mapping handles it.
    pub fn touch(&mut self, id: u64, phase: TouchPhase, pos: [f32; 2]) -> TouchOutput {
        let before = self.fingers();
        let mut events = Vec::new();
        let known = self.fingers.iter().position(|f| f.id == id);
        // Whether the touch belongs to a gesture the tracker took over,
        // judged before the touch (the last lift of one is the tracker's).
        let mut pass_through = self.multi.is_none();
        match phase {
            TouchPhase::Started => {
                if known.is_none() {
                    if self.fingers.is_empty() {
                        self.start = pos;
                    }
                    self.fingers.push(Finger { id, pos });
                    if self.fingers.len() >= 2 && self.multi.is_none() {
                        // The pointer is where the first finger is now.
                        self.multi = Some(self.fingers[0].pos);
                    }
                    pass_through = self.multi.is_none();
                }
            }
            TouchPhase::Moved => {
                if let Some(i) = known {
                    let n = self.fingers.len() as f32;
                    let last = self.fingers[i].pos;
                    self.fingers[i].pos = pos;
                    if let Some(sum) = &mut self.multi {
                        let d = [(pos[0] - last[0]) / n, (pos[1] - last[1]) / n];
                        *sum = [sum[0] + d[0], sum[1] + d[1]];
                        if d != [0.0, 0.0] {
                            events.push(InputEvent::Mouse(MouseEvent::Drag {
                                start_pos: self.start,
                                current_pos: *sum,
                                delta: d,
                                button: MouseButton::Left,
                            }));
                        }
                    }
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if let Some(i) = known {
                    self.fingers.remove(i);
                    if self.fingers.is_empty()
                        && let Some(sum) = self.multi.take()
                    {
                        // `Cancelled` on the last finger ends it the same
                        // way; the router's cancel rules do the rest.
                        events.push(InputEvent::Mouse(MouseEvent::Up {
                            pos: sum,
                            button: MouseButton::Left,
                        }));
                    }
                }
            }
        }
        let after = self.fingers();
        TouchOutput {
            events,
            fingers: (after != before).then_some(after),
            pass_through,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drag_at(out: &TouchOutput) -> Option<[f32; 2]> {
        match out.events.as_slice() {
            [InputEvent::Mouse(MouseEvent::Drag { current_pos, .. })] => Some(*current_pos),
            _ => None,
        }
    }

    #[test]
    fn one_finger_passes_through_and_reports_its_count() {
        let mut t = TouchTracker::new();
        let o = t.touch(1, TouchPhase::Started, [10.0, 10.0]);
        assert_eq!(
            (o.events.len(), o.fingers, o.pass_through),
            (0, Some(1), true)
        );
        let o = t.touch(1, TouchPhase::Moved, [10.0, 40.0]);
        assert_eq!((o.events.len(), o.fingers, o.pass_through), (0, None, true));
        let o = t.touch(1, TouchPhase::Ended, [10.0, 40.0]);
        assert_eq!(
            (o.events.len(), o.fingers, o.pass_through),
            (0, Some(0), true)
        );
    }

    #[test]
    fn a_second_finger_joining_and_leaving_does_not_move_the_pointer() {
        let mut t = TouchTracker::new();
        t.touch(1, TouchPhase::Started, [10.0, 100.0]);
        t.touch(1, TouchPhase::Moved, [10.0, 90.0]);
        let o = t.touch(2, TouchPhase::Started, [200.0, 300.0]);
        assert_eq!(
            (o.events.len(), o.fingers, o.pass_through),
            (0, Some(2), false)
        );
        // Both move up by 20: the mean moves 20, from finger 1's place.
        assert_eq!(
            drag_at(&t.touch(1, TouchPhase::Moved, [10.0, 70.0])),
            Some([10.0, 80.0])
        );
        assert_eq!(
            drag_at(&t.touch(2, TouchPhase::Moved, [200.0, 280.0])),
            Some([10.0, 70.0])
        );
        // Finger 1 lifts: no jump, and the rest is still the tracker's.
        let o = t.touch(1, TouchPhase::Ended, [10.0, 70.0]);
        assert_eq!(
            (o.events.len(), o.fingers, o.pass_through),
            (0, Some(1), false)
        );
        let o = t.touch(2, TouchPhase::Moved, [200.0, 270.0]);
        assert_eq!(drag_at(&o), Some([10.0, 60.0]), "1x with one finger");
        assert!(!o.pass_through);
        let o = t.touch(2, TouchPhase::Ended, [200.0, 270.0]);
        assert_eq!(
            o.events,
            [InputEvent::Mouse(MouseEvent::Up {
                pos: [10.0, 60.0],
                button: MouseButton::Left
            })]
        );
        assert_eq!((o.fingers, o.pass_through), (Some(0), false));
        // The next gesture is the host's again.
        assert!(t.touch(3, TouchPhase::Started, [0.0, 0.0]).pass_through);
    }

    #[test]
    fn the_pointer_moves_by_the_fingers_mean_movement() {
        let mut t = TouchTracker::new();
        t.touch(1, TouchPhase::Started, [0.0, 0.0]);
        t.touch(2, TouchPhase::Started, [100.0, 0.0]);
        // One finger alone moving 40 moves the mean 20.
        assert_eq!(
            drag_at(&t.touch(2, TouchPhase::Moved, [100.0, 40.0])),
            Some([0.0, 20.0])
        );
        t.touch(3, TouchPhase::Started, [50.0, 0.0]);
        assert_eq!(
            drag_at(&t.touch(3, TouchPhase::Moved, [50.0, 30.0])),
            Some([0.0, 30.0]),
            "a third of 30"
        );
        assert_eq!(t.fingers(), 3);
        assert!(
            t.touch(3, TouchPhase::Moved, [50.0, 30.0])
                .events
                .is_empty()
        );
    }

    #[test]
    fn cancelled_ends_a_gesture_like_a_lift() {
        let mut t = TouchTracker::new();
        t.touch(1, TouchPhase::Started, [0.0, 0.0]);
        t.touch(2, TouchPhase::Started, [10.0, 0.0]);
        t.touch(1, TouchPhase::Cancelled, [0.0, 0.0]);
        let o = t.touch(2, TouchPhase::Cancelled, [10.0, 0.0]);
        assert!(matches!(
            o.events.as_slice(),
            [InputEvent::Mouse(MouseEvent::Up { pos, .. })] if *pos == [0.0, 0.0]
        ));
        assert_eq!(o.fingers, Some(0));
        // One finger cancelled passes through to the host's mapping.
        t.touch(5, TouchPhase::Started, [0.0, 0.0]);
        let o = t.touch(5, TouchPhase::Cancelled, [0.0, 0.0]);
        assert_eq!(
            (o.events.len(), o.fingers, o.pass_through),
            (0, Some(0), true)
        );
    }

    #[test]
    fn the_count_is_reported_only_when_it_changes() {
        let mut t = TouchTracker::new();
        assert_eq!(t.touch(1, TouchPhase::Started, [0.0, 0.0]).fingers, Some(1));
        assert_eq!(t.touch(1, TouchPhase::Started, [0.0, 0.0]).fingers, None);
        assert_eq!(t.touch(1, TouchPhase::Moved, [0.0, 5.0]).fingers, None);
        assert_eq!(t.touch(9, TouchPhase::Ended, [0.0, 0.0]).fingers, None);
        assert_eq!(t.touch(2, TouchPhase::Started, [0.0, 0.0]).fingers, Some(2));
        assert_eq!(t.touch(9, TouchPhase::Moved, [0.0, 0.0]).fingers, None);
        assert_eq!(t.touch(2, TouchPhase::Ended, [0.0, 0.0]).fingers, Some(1));
        assert_eq!(t.touch(1, TouchPhase::Ended, [0.0, 0.0]).fingers, Some(0));
    }
}
