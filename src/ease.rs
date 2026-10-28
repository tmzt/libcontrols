//! Easing curves (docs/design/20261028_01_DRAWER_PLAN.md, section 3.4).

/// A CSS-style cubic Bézier timing curve from (0, 0) to (1, 1) with control
/// points (x1, y1) and (x2, y2).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    /// Material 3 standard: cubic-bezier(0.2, 0.0, 0.0, 1.0).
    pub const STANDARD: Self = Self::new(0.2, 0.0, 0.0, 1.0);
    /// Material 3 emphasized decelerate (entering): cubic-bezier(0.05, 0.7, 0.1, 1.0).
    pub const EMPHASIZED_DECELERATE: Self = Self::new(0.05, 0.7, 0.1, 1.0);
    /// Material 3 emphasized accelerate (exiting): cubic-bezier(0.3, 0.0, 0.8, 0.15).
    pub const EMPHASIZED_ACCELERATE: Self = Self::new(0.3, 0.0, 0.8, 0.15);

    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// One coordinate of the curve at parameter `s`, for control values `a`
    /// and `b` (the end points are 0 and 1).
    fn coord(a: f64, b: f64, s: f64) -> f64 {
        let r = 1.0 - s;
        3.0 * r * r * s * a + 3.0 * r * s * s * b + s * s * s
    }

    fn slope(a: f64, b: f64, s: f64) -> f64 {
        let r = 1.0 - s;
        3.0 * r * r * a + 6.0 * r * s * (b - a) + 3.0 * s * s * (1.0 - b)
    }

    /// y at x, for x in [0, 1] (clamped): Newton's method on x(s), 8
    /// iterations, then bisection if it has not converged to 1e-6.
    pub fn ease(&self, x: f32) -> f32 {
        let x = f64::from(x.clamp(0.0, 1.0));
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        let (x1, x2) = (f64::from(self.x1), f64::from(self.x2));
        let mut s = x;
        let mut converged = false;
        for _ in 0..8 {
            let err = Self::coord(x1, x2, s) - x;
            if err.abs() < 1e-6 {
                converged = true;
                break;
            }
            let d = Self::slope(x1, x2, s);
            if d.abs() < 1e-9 {
                break;
            }
            s = (s - err / d).clamp(0.0, 1.0);
        }
        if !converged && (Self::coord(x1, x2, s) - x).abs() >= 1e-6 {
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..60 {
                s = 0.5 * (lo + hi);
                let v = Self::coord(x1, x2, s);
                if (v - x).abs() < 1e-7 {
                    break;
                }
                if v < x {
                    lo = s;
                } else {
                    hi = s;
                }
            }
        }
        Self::coord(f64::from(self.y1), f64::from(self.y2), s) as f32
    }
}

/// The release curve of section 3.4: the cubic Hermite
/// `h(u) = m0 (u³ - 2u² + u) + 3u² - 2u³`, from 0 at slope `m0` to 1 at
/// slope 0. Monotonic on [0, 1] for `0 <= m0 <= 3` (`m0` is clamped to
/// that); with `m0 = 0` it is smoothstep.
pub fn hermite_ease(u: f32, m0: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    let m0 = m0.clamp(0.0, 3.0);
    let u2 = u * u;
    let u3 = u2 * u;
    (m0 * (u3 - 2.0 * u2 + u) + 3.0 * u2 - 2.0 * u3).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURVES: [CubicBezier; 3] = [
        CubicBezier::STANDARD,
        CubicBezier::EMPHASIZED_DECELERATE,
        CubicBezier::EMPHASIZED_ACCELERATE,
    ];

    #[test]
    fn curves_hit_their_ends_and_are_monotonic() {
        for c in CURVES {
            assert_eq!(c.ease(0.0), 0.0);
            assert_eq!(c.ease(1.0), 1.0);
            let mut last = 0.0;
            for i in 0..=1000 {
                let y = c.ease(i as f32 / 1000.0);
                assert!(y >= last - 1e-6, "{c:?} not monotonic at {i}");
                last = y;
            }
        }
    }

    /// Reference values solved independently (bisection in f64 on x(s)).
    #[test]
    fn curves_match_reference_values() {
        fn reference(c: CubicBezier, x: f64) -> f64 {
            let f = |a: f64, b: f64, s: f64| {
                3.0 * (1.0 - s) * (1.0 - s) * s * a + 3.0 * (1.0 - s) * s * s * b + s * s * s
            };
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..200 {
                let m = 0.5 * (lo + hi);
                if f(c.x1 as f64, c.x2 as f64, m) < x {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            f(c.y1 as f64, c.y2 as f64, 0.5 * (lo + hi))
        }
        for c in CURVES {
            for x in [0.25, 0.5, 0.75] {
                let want = reference(c, x) as f32;
                let got = c.ease(x as f32);
                assert!((got - want).abs() < 1e-4, "{c:?} at {x}: {got} vs {want}");
            }
        }
        // Spot values: decelerate is far ahead early, accelerate far behind.
        assert!(CubicBezier::EMPHASIZED_DECELERATE.ease(0.25) > 0.75);
        assert!(CubicBezier::EMPHASIZED_ACCELERATE.ease(0.25) < 0.1);
    }

    #[test]
    fn hermite_starts_at_its_slope_ends_flat_and_stays_in_range() {
        for m0 in [0.0, 1.5, 3.0] {
            let h = 1e-4;
            let start = hermite_ease(h, m0) / h;
            assert!((start - m0).abs() < 1e-2, "start slope {start} for {m0}");
            let end = (hermite_ease(1.0, m0) - hermite_ease(1.0 - h, m0)) / h;
            assert!(end.abs() < 1e-2, "end slope {end} for {m0}");
            let mut last = 0.0;
            for i in 0..=1000 {
                let y = hermite_ease(i as f32 / 1000.0, m0);
                assert!((0.0..=1.0).contains(&y));
                assert!(y >= last - 1e-6);
                last = y;
            }
            assert_eq!(hermite_ease(0.0, m0), 0.0);
            assert_eq!(hermite_ease(1.0, m0), 1.0);
        }
    }
}
