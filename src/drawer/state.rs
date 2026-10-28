//! `DrawerState`: a drawer's place and motion (plan 01, sections 3.2-3.4,
//! as amended at 11:22 EDT: driven by calls only).

use std::sync::{Arc, Mutex, MutexGuard};

use crate::ease::{CubicBezier, hermite_ease};

/// The shortest animation, in seconds, however little is left to move.
pub(crate) const MIN_DURATION: f32 = 0.120;

/// Where a drawer is in its motion.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DrawerPhase {
    #[default]
    Closed,
    /// Animating towards open.
    Opening,
    Open,
    /// Animating towards closed.
    Closing,
    /// The app holds the sheet at a progress it sets (`set_progress`).
    Dragging,
}

/// Why a drawer moved. Every call is the app's (plan 01, 11:22 EDT).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DrawerReason {
    App,
}

/// What a drawer reports, taken with [`DrawerState::take_events`].
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DrawerEvent {
    /// Left Closed (an animation began or the app set a progress).
    Shown { by: DrawerReason },
    /// Reached Open and settled; the content takes input.
    Opened { by: DrawerReason },
    /// Began to close.
    Closing { by: DrawerReason },
    /// Reached Closed. Every `Shown` is followed by exactly one `Hidden`.
    Hidden { by: DrawerReason },
}

/// The motion values the state animates with: the resolved theme's and
/// options' (set by the `Drawer` that owns the state).
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Motion {
    pub open_duration: f32,
    pub close_duration: f32,
    pub open_curve: CubicBezier,
    pub close_curve: CubicBezier,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            open_duration: 0.300,
            close_duration: 0.250,
            open_curve: CubicBezier::EMPHASIZED_DECELERATE,
            close_curve: CubicBezier::EMPHASIZED_ACCELERATE,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Curve {
    Bezier(CubicBezier),
    /// The velocity-matched release curve with its normalised start slope.
    Hermite(f32),
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct Anim {
    start: f64,
    t0: f32,
    target: f32,
    duration: f32,
    curve: Curve,
}

#[derive(Debug, Default)]
struct Inner {
    t: f32,
    phase: DrawerPhase,
    anim: Option<Anim>,
    time: f64,
    motion: Motion,
    events: Vec<DrawerEvent>,
}

/// One drawer's place and motion. Shared (`Arc`) between the drawer and the
/// app, like `ScrollState`, so it survives the drawer being rebuilt.
#[derive(Debug, Default)]
pub struct DrawerState {
    inner: Mutex<Inner>,
}

const APP: DrawerReason = DrawerReason::App;

impl DrawerState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Starts open, at t = 1, with no animation and no events.
    pub fn open_at_start() -> Arc<Self> {
        let s = Self::default();
        {
            let mut i = s.lock();
            i.t = 1.0;
            i.phase = DrawerPhase::Open;
        }
        Arc::new(s)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn set_motion(&self, motion: Motion) {
        self.lock().motion = motion;
    }

    #[cfg(test)]
    pub(crate) fn motion(&self) -> Motion {
        self.lock().motion
    }

    /// Animate to open with the open curve, over
    /// `max(0.120, open_duration * (1 - t))`. Nothing while Opening, Open or
    /// Dragging; turns round while Closing.
    pub fn open(&self) {
        let mut i = self.lock();
        match i.phase {
            DrawerPhase::Opening | DrawerPhase::Open | DrawerPhase::Dragging => {}
            DrawerPhase::Closed | DrawerPhase::Closing => i.animate_to(true, 0.0),
        }
    }

    /// Animate to closed with the close curve, over
    /// `max(0.120, close_duration * t)`. Nothing while Closing, Closed or
    /// Dragging; turns round while Opening.
    pub fn close(&self) {
        let mut i = self.lock();
        match i.phase {
            DrawerPhase::Closing | DrawerPhase::Closed | DrawerPhase::Dragging => {}
            DrawerPhase::Opening | DrawerPhase::Open => i.animate_to(false, 0.0),
        }
    }

    /// `close()` while Opening or Open, else `open()`.
    pub fn toggle(&self) {
        let phase = self.phase();
        if matches!(phase, DrawerPhase::Opening | DrawerPhase::Open) {
            self.close();
        } else {
            self.open();
        }
    }

    /// Jump without animating (restoring a saved layout), from any phase.
    pub fn set_open_now(&self, open: bool) {
        let mut i = self.lock();
        i.anim = None;
        if open {
            if i.phase == DrawerPhase::Open {
                return;
            }
            if i.phase == DrawerPhase::Closed {
                i.events.push(DrawerEvent::Shown { by: APP });
            }
            i.t = 1.0;
            i.phase = DrawerPhase::Open;
            i.events.push(DrawerEvent::Opened { by: APP });
        } else {
            if i.phase == DrawerPhase::Closed {
                return;
            }
            if i.phase != DrawerPhase::Closing {
                i.events.push(DrawerEvent::Closing { by: APP });
            }
            i.t = 0.0;
            i.phase = DrawerPhase::Closed;
            i.events.push(DrawerEvent::Hidden { by: APP });
        }
    }

    /// Hold the sheet at `t` (clamped to 0..=1), with no animation: for an
    /// app that drags the sheet with its own gesture. The phase becomes
    /// Dragging (a `Shown` when it was Closed) until `settle` or
    /// `set_open_now`.
    pub fn set_progress(&self, t: f32) {
        let mut i = self.lock();
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if i.phase == DrawerPhase::Closed {
            i.events.push(DrawerEvent::Shown { by: APP });
        }
        i.anim = None;
        i.phase = DrawerPhase::Dragging;
        i.t = t;
    }

    /// From the current `t`, animate to open or closed, leaving at
    /// `velocity` (t per second, towards open when positive): the
    /// velocity-matched curve of plan 01 section 3.4. A velocity of zero, or
    /// one pointing away from the target, is `open()`/`close()`'s curve.
    pub fn settle(&self, open: bool, velocity: f32) {
        let mut i = self.lock();
        if i.phase == DrawerPhase::Closed && !open {
            return;
        }
        let v = if velocity.is_finite() { velocity } else { 0.0 };
        let w = if open { v } else { -v }.max(0.0);
        i.animate_to(open, w);
    }

    /// Move time on by `dt_secs` (capped at 0.1 s, as the router caps it).
    /// Reaching the target sets Open or Closed exactly and queues `Opened`
    /// or `Hidden`. Returns whether it is still animating.
    pub fn advance(&self, dt_secs: f32) -> bool {
        let mut i = self.lock();
        let dt = if dt_secs.is_finite() {
            dt_secs.clamp(0.0, 0.1)
        } else {
            0.0
        };
        i.time += f64::from(dt);
        let Some(a) = i.anim else {
            return false;
        };
        let s = (i.time - a.start) as f32;
        let u = if a.duration > 0.0 {
            s / a.duration
        } else {
            1.0
        };
        if u >= 1.0 {
            i.anim = None;
            i.t = a.target;
            if a.target >= 1.0 {
                i.phase = DrawerPhase::Open;
                i.events.push(DrawerEvent::Opened { by: APP });
            } else {
                i.phase = DrawerPhase::Closed;
                i.events.push(DrawerEvent::Hidden { by: APP });
            }
            false
        } else {
            let e = match a.curve {
                Curve::Bezier(c) => c.ease(u),
                Curve::Hermite(m0) => hermite_ease(u, m0),
            };
            i.t = (a.t0 + (a.target - a.t0) * e).clamp(0.0, 1.0);
            true
        }
    }

    pub fn phase(&self) -> DrawerPhase {
        self.lock().phase
    }

    /// The eased progress, 0 closed .. 1 open: what is drawn, what the
    /// shader gets.
    pub fn progress(&self) -> f32 {
        self.lock().t
    }

    /// Not Closed.
    pub fn is_visible(&self) -> bool {
        self.phase() != DrawerPhase::Closed
    }

    /// Opening, Closing or Dragging.
    pub fn is_moving(&self) -> bool {
        matches!(
            self.phase(),
            DrawerPhase::Opening | DrawerPhase::Closing | DrawerPhase::Dragging
        )
    }

    /// The events since the last call, oldest first.
    pub fn take_events(&self) -> Vec<DrawerEvent> {
        std::mem::take(&mut self.lock().events)
    }

    /// The running animation's duration and normalised start slope (`None`
    /// for a Bézier curve), for tests.
    #[cfg(test)]
    pub(crate) fn anim_shape(&self) -> Option<(f32, Option<f32>)> {
        self.lock().anim.map(|a| {
            (
                a.duration,
                match a.curve {
                    Curve::Hermite(m) => Some(m),
                    Curve::Bezier(_) => None,
                },
            )
        })
    }
}

impl Inner {
    /// Start an animation from the current `t` to open or closed, leaving at
    /// `w` (t per second towards the target, 0 for the eased curve), and
    /// queue the events its start implies.
    fn animate_to(&mut self, open: bool, w: f32) {
        let target = if open { 1.0 } else { 0.0 };
        let was = self.phase;
        if open && was == DrawerPhase::Closed {
            self.events.push(DrawerEvent::Shown { by: APP });
        }
        if !open && !matches!(was, DrawerPhase::Closing | DrawerPhase::Closed) {
            self.events.push(DrawerEvent::Closing { by: APP });
        }
        let delta = (target - self.t).abs();
        let (full, curve) = if open {
            (self.motion.open_duration, self.motion.open_curve)
        } else {
            (self.motion.close_duration, self.motion.close_curve)
        };
        if delta <= 0.0 {
            self.anim = None;
            self.t = target;
            if open {
                self.phase = DrawerPhase::Open;
                if was != DrawerPhase::Open {
                    self.events.push(DrawerEvent::Opened { by: APP });
                }
            } else {
                self.phase = DrawerPhase::Closed;
                self.events.push(DrawerEvent::Hidden { by: APP });
            }
            return;
        }
        let longest = MIN_DURATION.max(full * delta);
        let (duration, curve) = if w > 0.0 {
            let duration = (3.0 * delta / w).clamp(MIN_DURATION, longest);
            let m0 = (w * duration / delta).min(3.0);
            (duration, Curve::Hermite(m0))
        } else {
            (longest, Curve::Bezier(curve))
        };
        self.phase = if open {
            DrawerPhase::Opening
        } else {
            DrawerPhase::Closing
        };
        self.anim = Some(Anim {
            start: self.time,
            t0: self.t,
            target,
            duration,
            curve,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &DrawerState, secs: f32) -> Vec<f32> {
        let mut ts = Vec::new();
        let steps = (secs / 0.004).round() as usize;
        for _ in 0..steps {
            s.advance(0.004);
            ts.push(s.progress());
        }
        ts
    }

    #[test]
    fn open_reaches_open_in_its_duration_exactly() {
        let s = DrawerState::new();
        s.open();
        assert_eq!(s.phase(), DrawerPhase::Opening);
        assert_eq!(s.take_events(), vec![DrawerEvent::Shown { by: APP }]);
        let ts = run(&s, 0.296);
        assert_eq!(s.phase(), DrawerPhase::Opening);
        assert!(ts.windows(2).all(|w| w[1] >= w[0]));
        assert!(*ts.last().unwrap() < 1.0);
        s.advance(0.004);
        s.advance(0.0);
        assert_eq!(s.phase(), DrawerPhase::Open);
        assert_eq!(s.progress(), 1.0);
        assert_eq!(s.take_events(), vec![DrawerEvent::Opened { by: APP }]);
        assert!(!s.advance(0.016));
    }

    #[test]
    fn the_open_curve_is_emphasized_decelerate() {
        let s = DrawerState::new();
        s.open();
        s.advance(0.075);
        let want = CubicBezier::EMPHASIZED_DECELERATE.ease(0.25);
        assert!((s.progress() - want).abs() < 1e-3, "{}", s.progress());
    }

    #[test]
    fn close_halfway_through_opening_turns_round_shortened() {
        let s = DrawerState::new();
        s.open();
        run(&s, 0.020);
        let t0 = s.progress();
        assert!(t0 > 0.0 && t0 < 1.0);
        s.take_events();
        s.close();
        assert_eq!(s.phase(), DrawerPhase::Closing);
        assert_eq!(s.take_events(), vec![DrawerEvent::Closing { by: APP }]);
        let (d, slope) = s.anim_shape().unwrap();
        assert_eq!(slope, None);
        assert!((d - MIN_DURATION.max(0.25 * t0)).abs() < 1e-6);
        // Starts from t0 with no jump.
        s.advance(0.0);
        assert!((s.progress() - t0).abs() < 1e-6);
        run(&s, d + 0.01);
        assert_eq!(s.phase(), DrawerPhase::Closed);
        assert_eq!(s.progress(), 0.0);
        assert_eq!(s.take_events(), vec![DrawerEvent::Hidden { by: APP }]);
    }

    #[test]
    fn a_long_close_is_its_full_duration_and_accelerates() {
        let s = DrawerState::open_at_start();
        s.close();
        let (d, _) = s.anim_shape().unwrap();
        assert!((d - 0.25).abs() < 1e-6);
        s.advance(0.0625);
        let want = 1.0 - CubicBezier::EMPHASIZED_ACCELERATE.ease(0.25);
        assert!((s.progress() - want).abs() < 1e-3);
    }

    #[test]
    fn repeated_calls_do_nothing() {
        let s = DrawerState::open_at_start();
        s.open();
        assert_eq!(s.phase(), DrawerPhase::Open);
        assert!(s.take_events().is_empty());
        let c = DrawerState::new();
        c.close();
        assert_eq!(c.phase(), DrawerPhase::Closed);
        assert!(c.take_events().is_empty());
        c.open();
        c.take_events();
        c.open();
        assert!(c.take_events().is_empty());
    }

    #[test]
    fn toggle_alternates() {
        let s = DrawerState::new();
        s.toggle();
        assert_eq!(s.phase(), DrawerPhase::Opening);
        s.advance(0.05);
        s.toggle();
        assert_eq!(s.phase(), DrawerPhase::Closing);
        s.toggle();
        assert_eq!(s.phase(), DrawerPhase::Opening);
        // Closing from t = 0 has nothing to move: it is Closed at once.
        let c = DrawerState::new();
        c.open();
        c.take_events();
        c.close();
        assert_eq!(c.phase(), DrawerPhase::Closed);
        assert_eq!(
            c.take_events(),
            vec![
                DrawerEvent::Closing { by: APP },
                DrawerEvent::Hidden { by: APP }
            ]
        );
    }

    #[test]
    fn set_open_now_jumps_with_only_its_events() {
        let s = DrawerState::new();
        s.set_open_now(true);
        assert_eq!((s.phase(), s.progress()), (DrawerPhase::Open, 1.0));
        assert_eq!(
            s.take_events(),
            vec![
                DrawerEvent::Shown { by: APP },
                DrawerEvent::Opened { by: APP }
            ]
        );
        s.set_open_now(true);
        assert!(s.take_events().is_empty());
        s.set_open_now(false);
        assert_eq!((s.phase(), s.progress()), (DrawerPhase::Closed, 0.0));
        assert_eq!(
            s.take_events(),
            vec![
                DrawerEvent::Closing { by: APP },
                DrawerEvent::Hidden { by: APP }
            ]
        );
        assert!(!s.advance(0.1));
    }

    #[test]
    fn set_progress_holds_the_sheet_and_blocks_open_close() {
        let s = DrawerState::new();
        s.set_progress(0.4);
        assert_eq!((s.phase(), s.progress()), (DrawerPhase::Dragging, 0.4));
        assert_eq!(s.take_events(), vec![DrawerEvent::Shown { by: APP }]);
        s.set_progress(1.7);
        assert_eq!(s.progress(), 1.0);
        s.set_progress(-1.0);
        assert_eq!(s.progress(), 0.0);
        s.set_progress(f32::NAN);
        assert_eq!(s.progress(), 0.0);
        s.open();
        s.close();
        assert_eq!(s.phase(), DrawerPhase::Dragging);
        assert!(!s.advance(0.1));
        assert!(s.is_moving() && s.is_visible());
        assert!(s.take_events().is_empty());
    }

    #[test]
    fn settle_with_no_velocity_is_open_and_close() {
        let s = DrawerState::new();
        s.set_progress(0.5);
        s.settle(true, 0.0);
        let (d, slope) = s.anim_shape().unwrap();
        assert_eq!(slope, None);
        assert!((d - 0.15).abs() < 1e-6);
        s.set_progress(0.5);
        s.settle(true, -3.0); // pointing away: no velocity to match
        assert_eq!(s.anim_shape().unwrap().1, None);
    }

    #[test]
    fn settle_matches_velocity_and_never_overshoots() {
        // t0 = 0.4, opening at 2 t/s: delta 0.6, w 2.
        let s = DrawerState::new();
        s.set_progress(0.4);
        s.take_events();
        s.settle(true, 2.0);
        assert_eq!(s.phase(), DrawerPhase::Opening);
        let (d, m0) = s.anim_shape().unwrap();
        let longest = MIN_DURATION.max(0.3 * 0.6);
        let want_d = (3.0f32 * 0.6 / 2.0).clamp(MIN_DURATION, longest);
        assert!((d - want_d).abs() < 1e-6);
        let want_m0 = (2.0 * want_d / 0.6f32).min(3.0);
        assert!((m0.unwrap() - want_m0).abs() < 1e-5);
        // It leaves at the finger's speed: the first step moves ~ w * dt.
        s.advance(0.001);
        let first = (s.progress() - 0.4) / 0.001;
        assert!((first - 0.6 * want_m0 / want_d).abs() < 0.05 * first.abs());
        let ts = run(&s, 0.5);
        assert!(ts.iter().all(|&t| (0.0..=1.0).contains(&t)));
        assert!(ts.windows(2).all(|w| w[1] >= w[0] - 1e-6));
        assert_eq!(s.phase(), DrawerPhase::Open);
        assert_eq!(s.take_events(), vec![DrawerEvent::Opened { by: APP }]);
    }

    #[test]
    fn a_fast_settle_is_short_and_its_slope_capped() {
        let s = DrawerState::open_at_start();
        s.set_progress(0.8);
        s.settle(false, -100.0);
        assert_eq!(s.take_events(), vec![DrawerEvent::Closing { by: APP }]);
        let (d, m0) = s.anim_shape().unwrap();
        assert!((d - MIN_DURATION).abs() < 1e-6);
        assert_eq!(m0, Some(3.0));
        let ts = run(&s, 0.2);
        assert!(ts.windows(2).all(|w| w[1] <= w[0] + 1e-6));
        assert_eq!(s.phase(), DrawerPhase::Closed);
    }

    #[test]
    fn settle_at_the_target_finishes_at_once() {
        let s = DrawerState::new();
        s.set_progress(1.0);
        s.take_events();
        s.settle(true, 0.0);
        assert_eq!(s.phase(), DrawerPhase::Open);
        assert_eq!(s.take_events(), vec![DrawerEvent::Opened { by: APP }]);
        s.settle(false, 0.0);
        s.set_progress(0.0);
        s.take_events();
        s.settle(false, 0.0);
        assert_eq!(s.phase(), DrawerPhase::Closed);
        assert_eq!(
            s.take_events(),
            vec![
                DrawerEvent::Closing { by: APP },
                DrawerEvent::Hidden { by: APP }
            ]
        );
        // Closed and asked to settle closed: nothing.
        s.settle(false, 1.0);
        assert!(s.take_events().is_empty());
    }

    #[test]
    fn advance_caps_dt() {
        let s = DrawerState::new();
        s.open();
        s.advance(10.0);
        assert!(s.progress() < 1.0, "one step moves at most 0.1 s");
    }

    #[test]
    fn every_shown_has_one_hidden() {
        let s = DrawerState::new();
        s.open();
        run(&s, 0.05);
        s.close();
        run(&s, 0.05);
        s.open();
        run(&s, 0.4);
        s.close();
        run(&s, 0.4);
        let ev = s.take_events();
        let shown = ev
            .iter()
            .filter(|e| matches!(e, DrawerEvent::Shown { .. }))
            .count();
        let hidden = ev
            .iter()
            .filter(|e| matches!(e, DrawerEvent::Hidden { .. }))
            .count();
        assert_eq!((shown, hidden), (1, 1), "{ev:?}");
    }
}
