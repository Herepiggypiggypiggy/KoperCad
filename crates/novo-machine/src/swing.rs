//! Hook swing along one travel direction: a pendulum hung from an axis that accelerates (lane L4).
//!
//! The hook hangs on a rope of length `L` from a carriage (the trolley for the cross direction,
//! the bridge for the runway direction). With the carriage accelerating at `a` along the axis,
//! the angle `theta` of the rope from the vertical (positive towards the forward direction)
//! follows
//!
//! ```text
//! theta'' = -(g / L) sin(theta) - (a / L) cos(theta) - 2 zeta w0 theta'     w0 = sqrt(g / L)
//! ```
//!
//! Exact: the equation of a point mass on a massless rope hung from an accelerating point, the
//! small-angle period `2 pi sqrt(L / g)` and the lean `atan(a / g)` of a hook pulled at constant
//! acceleration. Model: the damping ratio `zeta` (a number to enter), a rope that never changes
//! length or stretches, no load that turns, no swing in the other direction (two pendulums, one
//! per axis, do not couple), no coupling back to the drive.

use novo_drive::{GRAVITY, InputError};

/// What the swing model leaves out, shown next to the picture.
pub const SWING_MODEL_LEAVES_OUT: &[&str] = &[
    "One pendulum per travel direction, a point mass on a rope that keeps its length: no load that turns, no coupling between the two directions",
    "Damping is the number you enter; a real rope, block and load damp differently",
    "The swing does not push back on the drive or the motor",
];

/// Default damping ratio of the swing (a few percent of critical).
pub const DEFAULT_DAMPING_RATIO: f64 = 0.02;

/// Shortest rope length the swing runs with, in metres (a hook right under the drum swings very
/// fast; the picture stops caring below this).
pub const MIN_LENGTH_M: f64 = 0.3;

/// Largest step the integrator takes, as a fraction of the swing period.
const STEP_PER_PERIOD: f64 = 200.0;

/// A swinging hook in one direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swing {
    angle_rad: f64,
    rate_rad_s: f64,
    length_m: f64,
    damping_ratio: f64,
}

impl Swing {
    /// A hook at rest under its carriage. `length_m` is the rope length (clamped up to
    /// [`MIN_LENGTH_M`]); `damping_ratio` is 0 (no loss) to 1 (critical).
    pub fn new(length_m: f64, damping_ratio: f64) -> Result<Self, InputError> {
        if !length_m.is_finite() {
            return Err(InputError::NotFinite { field: "length_m" });
        }
        if !damping_ratio.is_finite() {
            return Err(InputError::NotFinite {
                field: "damping_ratio",
            });
        }
        if !(0.0..=1.0).contains(&damping_ratio) {
            return Err(InputError::OutOfRange {
                field: "damping_ratio",
                value: damping_ratio,
                min: 0.0,
                max: 1.0,
            });
        }
        Ok(Self {
            angle_rad: 0.0,
            rate_rad_s: 0.0,
            length_m: length_m.max(MIN_LENGTH_M),
            damping_ratio,
        })
    }

    /// Angle of the rope from the vertical, radians, positive towards the forward direction.
    pub fn angle_rad(&self) -> f64 {
        self.angle_rad
    }

    pub fn rate_rad_s(&self) -> f64 {
        self.rate_rad_s
    }

    pub fn length_m(&self) -> f64 {
        self.length_m
    }

    /// The rope was paid out or hauled in: the length changes, the angle stays. (The angular rate
    /// is kept too; a real rope changing length also changes the rate, which this leaves out.)
    pub fn set_length_m(&mut self, length_m: f64) {
        if length_m.is_finite() {
            self.length_m = length_m.max(MIN_LENGTH_M);
        }
    }

    /// Back to hanging still.
    pub fn reset(&mut self) {
        self.angle_rad = 0.0;
        self.rate_rad_s = 0.0;
    }

    /// Small-angle period in seconds.
    pub fn period_s(&self) -> f64 {
        std::f64::consts::TAU * (self.length_m / GRAVITY).sqrt()
    }

    /// Advances `dt_s` seconds with the carriage accelerating at `accel_m_s2` (forward positive).
    /// The step is split so the integrator stays far finer than the swing period.
    pub fn step(&mut self, dt_s: f64, accel_m_s2: f64) {
        if !(dt_s.is_finite() && dt_s > 0.0 && accel_m_s2.is_finite()) {
            return;
        }
        let max_h = self.period_s() / STEP_PER_PERIOD;
        let n = (dt_s / max_h).ceil().max(1.0);
        let h = dt_s / n;
        let w0 = (GRAVITY / self.length_m).sqrt();
        for _ in 0..n as usize {
            let alpha = -(GRAVITY / self.length_m) * self.angle_rad.sin()
                - (accel_m_s2 / self.length_m) * self.angle_rad.cos()
                - 2.0 * self.damping_ratio * w0 * self.rate_rad_s;
            // Semi-implicit Euler: stable for an oscillator at this step size.
            self.rate_rad_s += alpha * h;
            self.angle_rad += self.rate_rad_s * h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kick_swings_at_the_small_angle_period() {
        let mut s = Swing::new(4.0, 0.0).unwrap();
        // A short push of 0.5 m/s^2 for 0.1 s, then hang free: measure the period between
        // upward zero crossings.
        s.step(0.1, 0.5);
        let mut t = 0.0;
        let h = 0.002;
        let mut crossings = Vec::new();
        let mut last = s.angle_rad();
        while t < 30.0 {
            s.step(h, 0.0);
            t += h;
            if last < 0.0 && s.angle_rad() >= 0.0 {
                crossings.push(t);
            }
            last = s.angle_rad();
        }
        assert!(crossings.len() >= 4, "{crossings:?}");
        let period = (crossings[crossings.len() - 1] - crossings[0]) / (crossings.len() - 1) as f64;
        let exact = 2.0 * std::f64::consts::PI * (4.0 / GRAVITY).sqrt();
        assert!((period - exact).abs() / exact < 0.01, "{period} vs {exact}");
    }

    #[test]
    fn without_acceleration_a_hook_at_rest_stays_at_rest() {
        let mut s = Swing::new(3.0, 0.02).unwrap();
        for _ in 0..1000 {
            s.step(0.01, 0.0);
        }
        assert_eq!(s.angle_rad(), 0.0);
    }

    #[test]
    fn a_hook_pulled_at_constant_acceleration_leans_back_by_atan_a_over_g() {
        let mut s = Swing::new(5.0, 0.3).unwrap();
        for _ in 0..6000 {
            s.step(0.005, 1.2);
        }
        let want = -(1.2_f64 / GRAVITY).atan();
        assert!(
            (s.angle_rad() - want).abs() < 1e-3,
            "{} vs {want}",
            s.angle_rad()
        );
    }

    #[test]
    fn braking_swings_the_hook_forward_of_the_carriage() {
        let mut s = Swing::new(4.0, 0.02).unwrap();
        // Accelerate, then brake to a stop: the hook leans back, then swings forward past upright.
        for _ in 0..400 {
            s.step(0.005, 0.5);
        }
        let back = s.angle_rad();
        assert!(back < 0.0);
        let mut max_fwd: f64 = 0.0;
        for _ in 0..400 {
            s.step(0.005, -0.5);
            max_fwd = max_fwd.max(s.angle_rad());
        }
        for _ in 0..600 {
            s.step(0.005, 0.0);
            max_fwd = max_fwd.max(s.angle_rad());
        }
        assert!(max_fwd > 0.0, "{max_fwd}");
    }

    #[test]
    fn damping_shrinks_the_swing_and_none_keeps_it() {
        let swing_after = |zeta: f64| {
            let mut s = Swing::new(4.0, zeta).unwrap();
            s.step(0.2, 1.0);
            let mut peak = 0.0_f64;
            for _ in 0..20_000 {
                s.step(0.005, 0.0);
                peak = peak.max(s.angle_rad().abs());
            }
            // The last 10 s only.
            let mut late = 0.0_f64;
            for _ in 0..2000 {
                s.step(0.005, 0.0);
                late = late.max(s.angle_rad().abs());
            }
            (peak, late)
        };
        let (peak, late) = swing_after(0.0);
        assert!(late > 0.95 * peak, "{late} vs {peak}");
        let (peak, late) = swing_after(0.05);
        assert!(late < 0.2 * peak, "{late} vs {peak}");
    }

    #[test]
    fn a_short_rope_is_clamped_and_silly_numbers_are_refused() {
        assert_eq!(Swing::new(0.01, 0.0).unwrap().length_m(), MIN_LENGTH_M);
        assert_eq!(Swing::new(-5.0, 0.0).unwrap().length_m(), MIN_LENGTH_M);
        assert!(Swing::new(f64::NAN, 0.0).is_err());
        assert!(Swing::new(4.0, -0.1).is_err());
        assert!(Swing::new(4.0, 2.0).is_err());
        let mut s = Swing::new(4.0, 0.0).unwrap();
        s.step(f64::NAN, 1.0);
        s.step(0.01, f64::NAN);
        s.step(-1.0, 1.0);
        assert_eq!(s.angle_rad(), 0.0);
    }

    #[test]
    fn the_period_follows_the_rope_length_and_reset_hangs_it_still() {
        let mut s = Swing::new(4.0, 0.0).unwrap();
        let p4 = s.period_s();
        s.set_length_m(9.0);
        assert!((s.period_s() / p4 - 1.5).abs() < 1e-9);
        s.step(0.5, 2.0);
        assert!(s.angle_rad() != 0.0);
        s.reset();
        assert_eq!((s.angle_rad(), s.rate_rad_s()), (0.0, 0.0));
    }
}
