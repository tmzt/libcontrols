//! A scroll pane's fling, as a customisable profile
//! (docs/design/20261119_01_RIBBON_TWO_FINGER_AND_THUMB_LABEL_PLAN.md, the
//! 18:31 EST amendment, items 1 to 5): how the release speed is measured,
//! curved, multiplied for two fingers, boosted and capped; how it decays;
//! what counts as a flick; and where a fling ends.
//!
//! Units are the router's pixels and seconds. [`FlingProfile::flankr`]
//! (flankr 349ed92's agent view) is the default; [`FlingProfile::classic`]
//! is libcontrols 0.11.0's pane fling.

use crate::sections::SectionId;

/// How the release speed is fitted to the finger's recent samples.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum SpeedFit {
    /// The first and last sample in the window (libcontrols 0.11.0).
    Endpoints,
    /// A least-squares line weighted 1 at the newest sample down to
    /// `oldest_weight` (0 to 1) at the window's age (flankr).
    WeightedLine { oldest_weight: f32 },
}

/// Which section tops a fling may end on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Boundaries {
    /// Every section's top.
    #[default]
    Sections,
    /// Tops whose [`level`](crate::Section::level) is at most n.
    AtMost(u8),
    /// Tops of sections that have a bookmark.
    Bookmarked,
}

/// How a counted flick's speed maps to a number of boundaries (the 18:31
/// EST amendment, item 4).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct SkipMapping {
    /// px/s: slower flicks pass one boundary (default 1500).
    pub skip_threshold: f32,
    /// Boundaries more per doubling of the speed above it (default 2.0,
    /// 0.5-8).
    pub per_doubling: f32,
    /// The one-finger cap (default 12, at least 1).
    pub max: u32,
    /// A two-finger flick passes this many times as many (default 4, 1-16).
    pub two_finger_count_factor: u32,
    /// The two-finger cap, and the cap of flicks added up (default 48, at
    /// least `max`).
    pub two_finger_max: u32,
    /// The flight's length for one boundary (s, default 0.30, 0.05-2).
    pub duration_base: f32,
    /// Longer by this per boundary after the first (s, default 0.05, 0-0.5).
    pub duration_per: f32,
    /// The longest flight (s, default 0.80, at least `duration_base`).
    pub duration_max: f32,
}

impl Default for SkipMapping {
    fn default() -> Self {
        Self {
            skip_threshold: 1500.0,
            per_doubling: 2.0,
            max: 12,
            two_finger_count_factor: 4,
            two_finger_max: 48,
            duration_base: 0.30,
            duration_per: 0.05,
            duration_max: 0.80,
        }
    }
}

/// Clamp a finite `v` to `lo..=hi`; a non-finite one keeps `old`.
fn bound(v: f32, lo: f32, hi: f32, old: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { old }
}

impl SkipMapping {
    /// Set `skip_threshold` (more than 0).
    #[must_use]
    pub fn skip_threshold(mut self, v: f32) -> Self {
        self.skip_threshold = bound(v, 1.0, f32::MAX, self.skip_threshold);
        self
    }

    /// Set `per_doubling` (0.5-8).
    #[must_use]
    pub fn per_doubling(mut self, v: f32) -> Self {
        self.per_doubling = bound(v, 0.5, 8.0, self.per_doubling);
        self
    }

    /// Set `max` (at least 1; `two_finger_max` rises to it).
    #[must_use]
    pub fn max(mut self, v: u32) -> Self {
        self.max = v.max(1);
        self.two_finger_max = self.two_finger_max.max(self.max);
        self
    }

    /// Set `two_finger_count_factor` (1-16).
    #[must_use]
    pub fn two_finger_count_factor(mut self, v: u32) -> Self {
        self.two_finger_count_factor = v.clamp(1, 16);
        self
    }

    /// Set `two_finger_max` (at least `max`).
    #[must_use]
    pub fn two_finger_max(mut self, v: u32) -> Self {
        self.two_finger_max = v.max(self.max);
        self
    }

    /// Set `duration_base` (0.05-2 s; `duration_max` rises to it).
    #[must_use]
    pub fn duration_base(mut self, v: f32) -> Self {
        self.duration_base = bound(v, 0.05, 2.0, self.duration_base);
        self.duration_max = self.duration_max.max(self.duration_base);
        self
    }

    /// Set `duration_per` (0-0.5 s).
    #[must_use]
    pub fn duration_per(mut self, v: f32) -> Self {
        self.duration_per = bound(v, 0.0, 0.5, self.duration_per);
        self
    }

    /// Set `duration_max` (at least `duration_base`).
    #[must_use]
    pub fn duration_max(mut self, v: f32) -> Self {
        self.duration_max = bound(v, self.duration_base, f32::MAX, self.duration_max);
        self
    }

    /// The boundaries a flick of measured release speed `v_r` (px/s, its
    /// magnitude) passes: one below the threshold, then stepped by the
    /// speed's doublings; four times as many with two fingers.
    pub fn count(&self, v_r: f32, two_fingers: bool) -> u32 {
        let v = v_r.abs();
        let n1 = if v < self.skip_threshold {
            1
        } else {
            let steps = (self.per_doubling * (v / self.skip_threshold).log2()).floor();
            (2 + steps.max(0.0) as u32).min(self.max)
        };
        if two_fingers {
            (n1 * self.two_finger_count_factor).min(self.two_finger_max)
        } else {
            n1
        }
    }

    /// How long a flight past `n` boundaries takes (s).
    pub fn duration(&self, n: u32) -> f32 {
        let extra = n.saturating_sub(1) as f32;
        (self.duration_base + self.duration_per * extra).min(self.duration_max)
    }
}

/// Where a fling ends.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum FlingStop {
    /// A distance fling, ending where its decay takes it.
    #[default]
    Free,
    /// A distance fling whose end moves to the first boundary at or past
    /// its natural landing point. Built with [`FlingStop::boundary`].
    #[non_exhaustive]
    Boundary { boundaries: Boundaries },
    /// A flick counts boundaries; any other fling is `Free`. Built with
    /// [`FlingStop::count`].
    #[non_exhaustive]
    Count {
        boundaries: Boundaries,
        skip: SkipMapping,
    },
}

impl FlingStop {
    /// Distance flings land on the first of `boundaries` at or past their
    /// natural landing point.
    pub fn boundary(boundaries: Boundaries) -> Self {
        Self::Boundary { boundaries }
    }

    /// Flicks pass a counted number of `boundaries`, by `skip`.
    pub fn count(boundaries: Boundaries, skip: SkipMapping) -> Self {
        Self::Count { boundaries, skip }
    }
}

/// A pane's fling: every value the release, the decay and the stop read.
/// Each setter clamps to the value's bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct FlingProfile {
    /// s: the release speed is measured over this span back from the last
    /// move (0.016-0.5).
    pub speed_window: f32,
    /// s: a shorter span gives no speed (0 to `speed_window`).
    pub speed_span_min: f32,
    /// How the speed is fitted to the samples.
    pub speed_fit: SpeedFit,
    /// px/s: the measured finger speed's cap (more than 0).
    pub max_release_speed: f32,
    /// px/s: below it a fling follows the finger exactly (more than 0).
    pub curve_knee: f32,
    /// Above the knee, `knee · (v / knee)^power` (1-3).
    pub curve_power: f32,
    /// px/s: the cap after the curve, the boost and two fingers (at least
    /// `min_speed`).
    pub max_speed: f32,
    /// 1/s: the speed falls to `e^-decay` of itself each second (0.1-20).
    pub decay: f32,
    /// px/s: slower, a fling stops or does not start; also the overscroll
    /// spring's snap (at least 0).
    pub min_speed: f32,
    /// s: a finger still this long before lifting does not fling (0-1).
    pub rest: f32,
    /// s: the longest frame a fling advances by (0.001-0.1).
    pub dt_max: f32,
    /// s: a gesture lifted within this long of its touch-down, the same
    /// way as the fling running at touch-down, adds to it; 0 is off (0-2).
    pub boost_window: f32,
    /// The share of the running fling's speed added (0-2).
    pub boost_factor: f32,
    /// Two-finger drag movement and distance-fling speed, times this
    /// (1-20).
    pub two_finger_factor: f32,
    /// s: the fingers count as lifted together within this (0-0.5).
    pub two_finger_lift: f32,
    /// s: touch-down to the last lift, at most, for a flick (0.05-0.5).
    pub flick_max_duration: f32,
    /// px: the pointer's travel, at most, for a flick (16-2000).
    pub flick_max_travel: f32,
    /// px/s: the measured release speed, at least, for a flick (at least
    /// `min_speed`).
    pub flick_min_speed: f32,
    /// Where a fling ends.
    pub stop: FlingStop,
}

impl Default for FlingProfile {
    fn default() -> Self {
        Self::flankr()
    }
}

macro_rules! profile_setter {
    ($name:ident, $lo:expr, $hi:expr) => {
        #[doc = concat!("Set `", stringify!($name), "`, clamped to its bounds.")]
        #[must_use]
        pub fn $name(mut self, v: f32) -> Self {
            let (lo, hi): (f32, f32) = ($lo(&self), $hi(&self));
            self.$name = bound(v, lo, hi, self.$name);
            self
        }
    };
}

impl FlingProfile {
    /// flankr 349ed92's agent-view fling: the default.
    pub fn flankr() -> Self {
        Self {
            speed_window: 0.100,
            speed_span_min: 0.016,
            speed_fit: SpeedFit::WeightedLine { oldest_weight: 0.5 },
            max_release_speed: 8000.0,
            curve_knee: 1000.0,
            curve_power: 1.5,
            max_speed: 16000.0,
            decay: 2.0,
            min_speed: 30.0,
            rest: 0.100,
            dt_max: 1.0 / 30.0,
            boost_window: 0.400,
            boost_factor: 1.0,
            two_finger_factor: 5.0,
            two_finger_lift: 0.100,
            flick_max_duration: 0.200,
            flick_max_travel: 320.0,
            flick_min_speed: 600.0,
            stop: FlingStop::Free,
        }
    }

    /// libcontrols 0.11.0's pane fling.
    pub fn classic() -> Self {
        Self {
            speed_span_min: 0.0,
            speed_fit: SpeedFit::Endpoints,
            curve_power: 1.0,
            max_speed: 8000.0,
            decay: 3.2,
            dt_max: 0.1,
            boost_window: 0.0,
            boost_factor: 0.0,
            ..Self::flankr()
        }
    }

    profile_setter!(speed_window, |_: &Self| 0.016, |_: &Self| 0.5);
    profile_setter!(speed_span_min, |_: &Self| 0.0, |s: &Self| s.speed_window);
    profile_setter!(max_release_speed, |_: &Self| 1.0, |_: &Self| f32::MAX);
    profile_setter!(curve_knee, |_: &Self| 1.0, |_: &Self| f32::MAX);
    profile_setter!(curve_power, |_: &Self| 1.0, |_: &Self| 3.0);
    profile_setter!(max_speed, |s: &Self| s.min_speed, |_: &Self| f32::MAX);
    profile_setter!(decay, |_: &Self| 0.1, |_: &Self| 20.0);
    profile_setter!(min_speed, |_: &Self| 0.0, |s: &Self| s.max_speed);
    profile_setter!(rest, |_: &Self| 0.0, |_: &Self| 1.0);
    profile_setter!(dt_max, |_: &Self| 0.001, |_: &Self| 0.1);
    profile_setter!(boost_window, |_: &Self| 0.0, |_: &Self| 2.0);
    profile_setter!(boost_factor, |_: &Self| 0.0, |_: &Self| 2.0);
    profile_setter!(two_finger_factor, |_: &Self| 1.0, |_: &Self| 20.0);
    profile_setter!(two_finger_lift, |_: &Self| 0.0, |_: &Self| 0.5);
    profile_setter!(flick_max_duration, |_: &Self| 0.05, |_: &Self| 0.5);
    profile_setter!(flick_max_travel, |_: &Self| 16.0, |_: &Self| 2000.0);
    profile_setter!(flick_min_speed, |s: &Self| s.min_speed, |_: &Self| f32::MAX);

    /// Set the speed fit (a weighted line's weight clamped to 0-1).
    #[must_use]
    pub fn speed_fit(mut self, fit: SpeedFit) -> Self {
        self.speed_fit = match fit {
            SpeedFit::WeightedLine { oldest_weight } => SpeedFit::WeightedLine {
                oldest_weight: bound(oldest_weight, 0.0, 1.0, 0.5),
            },
            other => other,
        };
        self
    }

    /// Set where a fling ends.
    #[must_use]
    pub fn stop(mut self, stop: FlingStop) -> Self {
        self.stop = stop;
        self
    }

    /// The finger's speed at release (px/s, signed along the axis) from its
    /// samples `(time, position)`, oldest first: over the span from the
    /// window's start to the last move (with one sample before it when the
    /// window holds too short a span), none for a span under
    /// `speed_span_min`, capped at `max_release_speed`.
    pub(crate) fn release_speed(&self, samples: &[(f64, f32)]) -> f32 {
        let Some(&(last_t, _)) = samples.last() else {
            return 0.0;
        };
        let window = f64::from(self.speed_window);
        let span_min = f64::from(self.speed_span_min);
        let mut i = samples
            .iter()
            .position(|&(t, _)| last_t - t <= window + 1e-9)
            .unwrap_or(0);
        if last_t - samples[i].0 < span_min {
            i = i.saturating_sub(1);
        }
        let win = &samples[i..];
        let span = last_t - win[0].0;
        if span < span_min || span <= 0.0 {
            return 0.0;
        }
        let v = match self.speed_fit {
            SpeedFit::Endpoints => {
                let (t0, p0) = win[0];
                let (t1, p1) = win[win.len() - 1];
                (p1 - p0) / (t1 - t0) as f32
            }
            SpeedFit::WeightedLine { oldest_weight } => {
                let (mut sw, mut st, mut sy, mut stt, mut sty) = (0.0f32, 0.0, 0.0, 0.0, 0.0);
                for &(t, y) in win {
                    let age = (last_t - t) as f32;
                    let x = -age;
                    let w = 1.0 - (1.0 - oldest_weight) * (age / self.speed_window).min(1.0);
                    sw += w;
                    st += w * x;
                    sy += w * y;
                    stt += w * x * x;
                    sty += w * x * y;
                }
                let den = sw * stt - st * st;
                if den.abs() < f32::EPSILON {
                    return 0.0;
                }
                (sw * sty - st * sy) / den
            }
        };
        v.clamp(-self.max_release_speed, self.max_release_speed)
    }

    /// A finger speed to a fling's: the same up to the knee, growing as
    /// its power above it.
    pub(crate) fn curve(&self, v: f32) -> f32 {
        let knee = self.curve_knee;
        if v.abs() <= knee {
            v
        } else {
            v.signum() * knee * (v.abs() / knee).powf(self.curve_power)
        }
    }

    /// The release of a gesture: its measured speed, its distance fling's
    /// speed (0 for none), and whether it is a flick.
    pub(crate) fn release(&self, g: &Gesture<'_>, now: f64) -> Release {
        let v_r = self.release_speed(g.samples);
        let rested = now - g.last_move > f64::from(self.rest);
        let mut v = self.curve(v_r);
        if g.pair {
            v *= self.two_finger_factor;
        }
        let quick = self.boost_window > 0.0 && now - g.down <= f64::from(self.boost_window);
        if quick && v * g.carried > 0.0 {
            v += self.boost_factor * g.carried;
        }
        let v = v.clamp(-self.max_speed, self.max_speed);
        let v = if !rested && v.abs() > self.min_speed {
            v
        } else {
            0.0
        };
        let flick = !rested
            && now - g.down <= f64::from(self.flick_max_duration)
            && g.travel <= self.flick_max_travel
            && v_r.abs() >= self.flick_min_speed;
        Release {
            v_r,
            v,
            flick,
            quick: now - g.down <= f64::from(self.boost_window),
        }
    }
}

/// A gesture at its last lift, as a pane saw it.
pub(crate) struct Gesture<'a> {
    /// `(router time, pointer position along the axis)`, oldest first.
    pub(crate) samples: &'a [(f64, f32)],
    /// Touch-down, and the last move, on router time.
    pub(crate) down: f64,
    pub(crate) last_move: f64,
    /// The pointer's travel along the axis, before any factor.
    pub(crate) travel: f32,
    /// The fling running at touch-down (px/s, signed as the release).
    pub(crate) carried: f32,
    /// The fingers lifted as a pair within `two_finger_lift`.
    pub(crate) pair: bool,
}

/// What a release gives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Release {
    /// The measured release speed, before the curve and any factor.
    pub(crate) v_r: f32,
    /// The distance fling's starting speed; 0 for none.
    pub(crate) v: f32,
    /// The gesture is a flick.
    pub(crate) flick: bool,
    /// Lifted within `boost_window` of its touch-down.
    pub(crate) quick: bool,
}

/// One pane's overrides of its theme's fling profile (`None` takes the
/// theme's value).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct FlingOptions {
    pub speed_window: Option<f32>,
    pub speed_span_min: Option<f32>,
    pub speed_fit: Option<SpeedFit>,
    pub max_release_speed: Option<f32>,
    pub curve_knee: Option<f32>,
    pub curve_power: Option<f32>,
    pub max_speed: Option<f32>,
    pub decay: Option<f32>,
    pub min_speed: Option<f32>,
    pub rest: Option<f32>,
    pub dt_max: Option<f32>,
    pub boost_window: Option<f32>,
    pub boost_factor: Option<f32>,
    pub two_finger_factor: Option<f32>,
    pub two_finger_lift: Option<f32>,
    pub flick_max_duration: Option<f32>,
    pub flick_max_travel: Option<f32>,
    pub flick_min_speed: Option<f32>,
    pub stop: Option<FlingStop>,
}

macro_rules! options_setter {
    ($name:ident: $t:ty) => {
        #[doc = concat!("Override the theme's `", stringify!($name), "`.")]
        #[must_use]
        pub fn $name(mut self, value: $t) -> Self {
            self.$name = Some(value);
            self
        }
    };
}

impl From<FlingProfile> for FlingOptions {
    /// Every field overridden: a whole preset.
    fn from(p: FlingProfile) -> Self {
        Self {
            speed_window: Some(p.speed_window),
            speed_span_min: Some(p.speed_span_min),
            speed_fit: Some(p.speed_fit),
            max_release_speed: Some(p.max_release_speed),
            curve_knee: Some(p.curve_knee),
            curve_power: Some(p.curve_power),
            max_speed: Some(p.max_speed),
            decay: Some(p.decay),
            min_speed: Some(p.min_speed),
            rest: Some(p.rest),
            dt_max: Some(p.dt_max),
            boost_window: Some(p.boost_window),
            boost_factor: Some(p.boost_factor),
            two_finger_factor: Some(p.two_finger_factor),
            two_finger_lift: Some(p.two_finger_lift),
            flick_max_duration: Some(p.flick_max_duration),
            flick_max_travel: Some(p.flick_max_travel),
            flick_min_speed: Some(p.flick_min_speed),
            stop: Some(p.stop),
        }
    }
}

impl FlingOptions {
    /// Overrides nothing.
    pub fn new() -> Self {
        Self::default()
    }

    options_setter!(speed_window: f32);
    options_setter!(speed_span_min: f32);
    options_setter!(speed_fit: SpeedFit);
    options_setter!(max_release_speed: f32);
    options_setter!(curve_knee: f32);
    options_setter!(curve_power: f32);
    options_setter!(max_speed: f32);
    options_setter!(decay: f32);
    options_setter!(min_speed: f32);
    options_setter!(rest: f32);
    options_setter!(dt_max: f32);
    options_setter!(boost_window: f32);
    options_setter!(boost_factor: f32);
    options_setter!(two_finger_factor: f32);
    options_setter!(two_finger_lift: f32);
    options_setter!(flick_max_duration: f32);
    options_setter!(flick_max_travel: f32);
    options_setter!(flick_min_speed: f32);
    options_setter!(stop: FlingStop);

    /// Each `Some` wins, through the profile's clamping setter; each `None`
    /// takes the theme's value. The resolved type is the profile itself.
    pub fn with_theme(&self, theme: &FlingProfile) -> FlingProfile {
        let mut p = *theme;
        macro_rules! take {
            ($($f:ident),*) => { $( if let Some(v) = self.$f { p = p.$f(v); } )* };
        }
        // Widening values first, so a value's bound reads the override.
        take!(
            speed_window,
            speed_span_min,
            speed_fit,
            max_release_speed,
            curve_knee,
            curve_power,
            max_speed,
            min_speed,
            decay,
            rest,
            dt_max,
            boost_window,
            boost_factor,
            two_finger_factor,
            two_finger_lift,
            flick_max_duration,
            flick_max_travel,
            flick_min_speed,
            stop
        );
        p
    }
}

/// What a pane's flings report, read with
/// [`ScrollState::take_fling_events`](crate::ScrollState::take_fling_events).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FlingEvent {
    /// A counted flick was released: it will pass `n` boundaries, landing
    /// on `target`.
    #[non_exhaustive]
    Counted { target: SectionId, n: u32 },
    /// A boundary was crossed; `remaining` are still to pass.
    #[non_exhaustive]
    Passed { id: SectionId, remaining: u32 },
    /// The flight's target changed: removed, split or merged, or a flick
    /// was added to it.
    #[non_exhaustive]
    Retargeted { target: SectionId },
    /// The fling landed on section `id`'s top.
    #[non_exhaustive]
    Landed { id: SectionId },
}

/// Where a fling or flight ends: a section's top, held by id with its
/// last known place in the list, or an end of the range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Target {
    Section { id: SectionId, index: usize },
    Start,
    End,
}

/// A target resolved this frame: what it is now, its top, and the section
/// it names (an end names its end's section).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stop {
    pub(crate) target: Target,
    pub(crate) top: f32,
    pub(crate) id: SectionId,
}

/// The cubic ease-out of a counted flight.
pub(crate) fn ease_out(x: f32) -> f32 {
    let u = 1.0 - x.clamp(0.0, 1.0);
    1.0 - u * u * u
}

#[cfg(test)]
mod tests {
    use super::*;

    /// flankr 349ed92's `release_speed`, in seconds, at scale 1.
    fn flankr_release_speed(samples: &[(f64, f32)]) -> f32 {
        const SPEED_WINDOW: f64 = 0.100;
        const SPEED_SPAN_MIN: f64 = 0.016;
        let Some(&(last_t, _)) = samples.last() else {
            return 0.0;
        };
        let mut i = samples
            .iter()
            .position(|&(t, _)| last_t - t <= SPEED_WINDOW + 1e-9)
            .unwrap_or(0);
        if last_t - samples[i].0 < SPEED_SPAN_MIN {
            i = i.saturating_sub(1);
        }
        let window = &samples[i..];
        let span = last_t - window[0].0;
        if span < SPEED_SPAN_MIN {
            return 0.0;
        }
        let (mut sw, mut st, mut sy, mut stt, mut sty) = (0.0f32, 0.0, 0.0, 0.0, 0.0);
        for &(t, y) in window {
            let age = (last_t - t) as f32;
            let x = -age;
            let w = 1.0 - 0.5 * (age / SPEED_WINDOW as f32).min(1.0);
            sw += w;
            st += w * x;
            sy += w * y;
            stt += w * x * x;
            sty += w * x * y;
        }
        let den = sw * stt - st * st;
        if den.abs() < f32::EPSILON {
            return 0.0;
        }
        ((sw * sty - st * sy) / den).clamp(-8000.0, 8000.0)
    }

    fn sample_sets() -> Vec<Vec<(f64, f32)>> {
        let mut sets = Vec::new();
        // flankr's accelerating flick: 16 ms frames, 10 px more each.
        let mut y = 0.0;
        let mut s = vec![(0.0, 0.0)];
        for i in 1..=6 {
            y += 10.0 * i as f32;
            s.push((0.016 * f64::from(i), y));
        }
        sets.push(s);
        // A steady drag over 300 ms.
        sets.push(
            (0..=30)
                .map(|i| (0.010 * f64::from(i), -4.0 * i as f32))
                .collect(),
        );
        // A burst: two moves microseconds apart after a pause.
        sets.push(vec![
            (0.0, 0.0),
            (0.050, 20.0),
            (0.120, 90.0),
            (0.120_01, 91.0),
        ]);
        // Too short to measure.
        sets.push(vec![(0.0, 0.0), (0.005, 10.0)]);
        // Faster than the cap.
        sets.push(vec![(0.0, 0.0), (0.016, 400.0), (0.032, 800.0)]);
        sets
    }

    #[test]
    fn the_weighted_line_is_flankrs_release_speed() {
        let p = FlingProfile::flankr();
        for s in sample_sets() {
            let ours = p.release_speed(&s);
            let theirs = flankr_release_speed(&s);
            assert!((ours - theirs).abs() < 1e-3, "{ours} vs {theirs} on {s:?}");
        }
        let accel = &sample_sets()[0];
        let average = 210.0 / 0.096;
        assert!(p.release_speed(accel) > average);
    }

    #[test]
    fn the_curve_is_flankrs_knee_and_power() {
        let p = FlingProfile::flankr();
        assert_eq!(p.curve(800.0), 800.0);
        assert_eq!(p.curve(-1000.0), -1000.0);
        assert!((p.curve(4000.0) - 8000.0).abs() < 1e-2, "1000 · 4^1.5");
        assert!((p.curve(-2250.0) + 1000.0 * 2.25f32.powf(1.5)).abs() < 1e-2);
        assert_eq!(FlingProfile::classic().curve(4000.0), 4000.0);
    }

    #[test]
    fn classic_is_the_old_endpoints_fit() {
        let p = FlingProfile::classic();
        let s: Vec<(f64, f32)> = (0..=6)
            .map(|i| (0.016 * f64::from(i), -10.0 * i as f32))
            .collect();
        let v = p.release_speed(&s);
        assert!((v + 60.0 / 0.096).abs() < 0.5, "{v}");
    }

    fn gesture(samples: &[(f64, f32)], down: f64, carried: f32, pair: bool) -> Gesture<'_> {
        Gesture {
            samples,
            down,
            last_move: samples.last().unwrap().0,
            travel: (samples.last().unwrap().1 - samples[0].1).abs(),
            carried,
            pair,
        }
    }

    #[test]
    fn the_boost_adds_within_the_window_and_not_after() {
        let p = FlingProfile::flankr();
        // 100 px over 100 ms: 1000 px/s, at the knee.
        let s: Vec<(f64, f32)> = (0..=10)
            .map(|i| (1.0 + 0.010 * f64::from(i), 10.0 * i as f32))
            .collect();
        let alone = p.release(&gesture(&s, 1.0, 0.0, false), 1.1).v;
        let boosted = p.release(&gesture(&s, 1.0, 3000.0, false), 1.1).v;
        assert!((boosted - alone - 3000.0).abs() < 1.0, "{boosted} {alone}");
        let late = p.release(&gesture(&s, 0.6, 3000.0, false), 1.1).v;
        assert!((late - alone).abs() < 1e-3, "held 500 ms: no boost");
        let other_way = p.release(&gesture(&s, 1.0, -3000.0, false), 1.1).v;
        assert!((other_way - alone).abs() < 1e-3);
        let pair = p.release(&gesture(&s, 1.0, 0.0, true), 1.1).v;
        assert!((pair - 5.0 * alone).abs() < 1.0);
        let capped = p.release(&gesture(&s, 1.0, 20_000.0, true), 1.1).v;
        assert_eq!(capped, 16_000.0);
        let rested = p.release(&gesture(&s, 1.0, 0.0, false), 1.3);
        assert_eq!((rested.v, rested.flick), (0.0, false));
    }

    #[test]
    fn the_classifier_tells_a_flick_from_a_scroll() {
        let p = FlingProfile::flankr();
        // (duration s, travel px, speed px/s): steady motion, lifted at once.
        let make = |dur: f64, travel: f32| -> Vec<(f64, f32)> {
            (0..=10)
                .map(|i| (dur * f64::from(i) / 10.0, travel * i as f32 / 10.0))
                .collect()
        };
        let flick = |dur: f64, travel: f32| {
            let s = make(dur, travel);
            p.release(&gesture(&s, 0.0, 0.0, false), dur).flick
        };
        // 150 ms, 200 px is 1333 px/s; and 2000 px/s at 300 px over 150 ms.
        assert!(flick(0.150, 200.0));
        assert!(flick(0.150, 300.0));
        assert!(!flick(0.300, 200.0), "300 ms");
        assert!(!flick(0.150, 400.0), "400 px");
        assert!(!flick(0.150, 60.0), "400 px/s");
        let s = make(0.150, 300.0);
        assert!(
            !p.release(&gesture(&s, 0.0, 0.0, false), 0.150 + 0.2).flick,
            "rested"
        );
    }

    #[test]
    fn the_count_and_the_duration_follow_the_table() {
        let m = SkipMapping::default();
        let table = [
            (600.0, 1),
            (1499.0, 1),
            (1500.0, 2),
            (2121.4, 3),
            (3000.0, 4),
            (4242.7, 5),
            (6000.0, 6),
            (8000.0, 6),
        ];
        for (v, n) in table {
            assert_eq!(m.count(v, false), n, "{v}");
            assert_eq!(m.count(v, true), n * 4, "{v} with two fingers");
        }
        assert_eq!(m.count(1_000_000.0, false), 12);
        assert_eq!(m.count(1_000_000.0, true), 48);
        assert_eq!(m.duration(1), 0.30);
        assert!((m.duration(3) - 0.40).abs() < 1e-6);
        assert_eq!(m.duration(48), 0.80);
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert!((ease_out(0.5) - 0.875).abs() < 1e-6);
    }

    #[test]
    fn setters_clamp_and_options_resolve_against_the_theme() {
        let p = FlingProfile::flankr()
            .decay(100.0)
            .curve_power(0.5)
            .dt_max(1.0);
        assert_eq!((p.decay, p.curve_power, p.dt_max), (20.0, 1.0, 0.1));
        assert_eq!(FlingProfile::default(), FlingProfile::flankr());
        let theme = FlingProfile::flankr().decay(4.0);
        let r = FlingOptions::new()
            .two_finger_factor(4.0)
            .with_theme(&theme);
        assert_eq!((r.two_finger_factor, r.decay), (4.0, 4.0));
        assert_eq!(FlingOptions::default().with_theme(&theme), theme);
        let whole = FlingOptions::from(FlingProfile::classic()).with_theme(&theme);
        assert_eq!(whole, FlingProfile::classic());
        let c = FlingProfile::classic();
        assert_eq!(
            (c.decay, c.min_speed, c.max_release_speed, c.rest, c.dt_max),
            (3.2, 30.0, 8000.0, 0.100, 0.1)
        );
        let m = SkipMapping::default().max(60).per_doubling(100.0);
        assert_eq!((m.max, m.two_finger_max, m.per_doubling), (60, 60, 8.0));
    }
}
