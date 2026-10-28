//! Frequency ramps: linear (exact arithmetic) and S-ramp (jerk limited).
//!
//! Both are safe against NaN, infinity, zero or negative times and a zero or negative step:
//! a time of 0 (or less) is an instant step, a bad reference slope is an instant step, a
//! step of 0 s (or less, or NaN) changes nothing and a non-finite target holds the output.
//! A reversal through zero decelerates to 0 first (decel time) and then accelerates (accel
//! time); the time left over in the step carries across zero.

/// True for a usable positive finite number.
fn ok(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

/// Linear ramp state: moves `frequency_hz` towards a target at a fixed rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    pub frequency_hz: f64,
    /// Time from 0 Hz to `reference_hz` in seconds. 0 or less = instant.
    pub accel_time_s: f64,
    /// Time from `reference_hz` to 0 Hz in seconds. 0 or less = instant.
    pub decel_time_s: f64,
    /// Frequency the two times refer to (usually the rated frequency).
    pub reference_hz: f64,
}

impl Ramp {
    /// Slope in Hz/s for a full-scale time; infinite (instant) when the time or reference is unusable.
    fn rate(&self, time_s: f64) -> f64 {
        if ok(time_s) && ok(self.reference_hz) {
            self.reference_hz / time_s
        } else {
            f64::INFINITY
        }
    }

    /// Move towards `target` along one straight segment; returns the step time left over.
    fn segment(&mut self, target: f64, dt_s: f64, rate: f64) -> f64 {
        let dist = (target - self.frequency_hz).abs();
        if dist == 0.0 {
            return dt_s;
        }
        let need = if rate.is_infinite() { 0.0 } else { dist / rate };
        if dt_s >= need {
            self.frequency_hz = target;
            dt_s - need
        } else {
            self.frequency_hz += (target - self.frequency_hz).signum() * rate * dt_s;
            0.0
        }
    }

    /// Advance by `dt_s` seconds towards `target_hz` (exact ramp arithmetic).
    pub fn step(&mut self, target_hz: f64, dt_s: f64) {
        if !self.frequency_hz.is_finite() {
            self.frequency_hz = 0.0;
        }
        if !target_hz.is_finite() || dt_s.is_nan() || dt_s <= 0.0 {
            return;
        }
        let mut left = dt_s;
        if self.frequency_hz * target_hz < 0.0 {
            let decel = self.rate(self.decel_time_s);
            left = self.segment(0.0, left, decel);
            if left <= 0.0 {
                return;
            }
        }
        let outward = target_hz.abs() > self.frequency_hz.abs();
        let rate = self.rate(if outward {
            self.accel_time_s
        } else {
            self.decel_time_s
        });
        self.segment(target_hz, left, rate);
    }
}

/// Jerk-limited (S-shaped) ramp.
///
/// `jerk_fraction` is the share of the full-scale time spent on each of the two acceleration
/// edges (0 = linear ramp, 0.2 = 20 % rising and 20 % falling). With full-scale time `T` and
/// edge time `t_r = fraction * T` the plateau acceleration is `reference / (T - t_r)` and the
/// jerk is plateau acceleration over `t_r`. End points are exact; the shape between them is
/// the usual trapezoid of acceleration (a model of what a drive does, not a replica).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SRamp {
    pub frequency_hz: f64,
    /// Current acceleration in Hz/s (state).
    pub accel_hz_s: f64,
    pub accel_time_s: f64,
    pub decel_time_s: f64,
    pub reference_hz: f64,
    pub jerk_fraction: f64,
}

impl SRamp {
    pub fn new(
        accel_time_s: f64,
        decel_time_s: f64,
        reference_hz: f64,
        jerk_fraction: f64,
    ) -> Self {
        Self {
            frequency_hz: 0.0,
            accel_hz_s: 0.0,
            accel_time_s,
            decel_time_s,
            reference_hz,
            jerk_fraction,
        }
    }

    /// Plateau acceleration (Hz/s) and jerk (Hz/s2) for a full-scale time; `None` = instant.
    fn limits(&self, time_s: f64) -> Option<(f64, f64)> {
        if !(ok(time_s) && ok(self.reference_hz)) {
            return None;
        }
        let fr = if self.jerk_fraction.is_finite() {
            self.jerk_fraction.clamp(0.0, 0.5)
        } else {
            0.0
        };
        let a_max = self.reference_hz / (time_s * (1.0 - fr));
        let jerk = if fr > 0.0 {
            a_max / (fr * time_s)
        } else {
            f64::INFINITY
        };
        Some((a_max, jerk))
    }

    fn snap(&mut self, target: f64) {
        self.frequency_hz = target;
        self.accel_hz_s = 0.0;
    }

    /// Advance by `dt_s` seconds towards `target_hz`.
    pub fn step(&mut self, target_hz: f64, dt_s: f64) {
        if !self.frequency_hz.is_finite() || !self.accel_hz_s.is_finite() {
            self.snap(0.0);
        }
        if !target_hz.is_finite() || dt_s.is_nan() || dt_s <= 0.0 {
            return;
        }
        // Through zero: go to 0 first on the decel limits, then on to the target.
        let aim = if self.frequency_hz * target_hz < 0.0 {
            0.0
        } else {
            target_hz
        };
        let outward = aim.abs() > self.frequency_hz.abs();
        let Some((a_max, jerk)) = self.limits(if outward {
            self.accel_time_s
        } else {
            self.decel_time_s
        }) else {
            self.snap(aim);
            return;
        };
        let err = aim - self.frequency_hz;
        if err == 0.0 {
            self.accel_hz_s = 0.0;
            return;
        }
        let s = err.signum();
        let a = self.accel_hz_s;
        let a_after = if jerk.is_infinite() {
            // Zero edge time: plain trapezoid-free ramp, acceleration jumps.
            let stopping = 0.0;
            if s * (self.frequency_hz + stopping) >= s * aim {
                0.0
            } else {
                s * a_max
            }
        } else {
            // Frequency gained while the acceleration is brought back to zero at full jerk.
            let coast = a * a.abs() / (2.0 * jerk);
            let j = if s * a > 0.0 && s * (self.frequency_hz + coast) >= s * aim {
                -s * jerk
            } else {
                s * jerk
            };
            let next = (a + j * dt_s).clamp(-a_max, a_max);
            // Easing off must not flip the sign of the acceleration.
            if j * a < 0.0 && next * a < 0.0 {
                0.0
            } else {
                next
            }
        };
        // Zero edge time: acceleration jumps, so the step is a plain linear move.
        let a_mean = if jerk.is_infinite() {
            a_after
        } else {
            0.5 * (a + a_after)
        };
        let f_new = self.frequency_hz + a_mean * dt_s;
        self.accel_hz_s = a_after;
        if s * (aim - f_new) <= 0.0 {
            self.snap(aim);
        } else {
            self.frequency_hz = f_new;
        }
    }
}
