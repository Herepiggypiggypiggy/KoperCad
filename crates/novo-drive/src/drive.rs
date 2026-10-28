//! Frequency drive (lane L1).

/// Linear ramp state: moves `frequency_hz` towards a target at a fixed rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    pub frequency_hz: f64,
    /// Time from 0 Hz to `reference_hz` in seconds.
    pub accel_time_s: f64,
    /// Time from `reference_hz` to 0 Hz in seconds.
    pub decel_time_s: f64,
    /// Frequency the two times refer to (usually the rated frequency).
    pub reference_hz: f64,
}

impl Ramp {
    /// Advance by `dt_s` seconds towards `target_hz` (exact ramp arithmetic).
    pub fn step(&mut self, target_hz: f64, dt_s: f64) {
        let rate = if target_hz.abs() >= self.frequency_hz.abs() {
            self.reference_hz / self.accel_time_s
        } else {
            self.reference_hz / self.decel_time_s
        };
        let max = rate * dt_s;
        let d = target_hz - self.frequency_hz;
        self.frequency_hz += d.clamp(-max, max);
    }
}
