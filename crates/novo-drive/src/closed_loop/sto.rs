//! Safe torque off: two channels, both must be energised for the drive to make torque.
//!
//! Exact by definition: one low channel removes torque at once. Model: the discrepancy
//! monitor (a fault when the channels disagree longer than the entered time, latched until
//! both channels have been low). This is a behavioural model, not a safety function
//! assessment: no SIL or PL claim.

/// What the monitor says after one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoStatus {
    /// Torque may be made (both channels energised, no latched discrepancy).
    pub torque_allowed: bool,
    /// Both channels low: the safety function is requested.
    pub requested: bool,
    /// The channels disagreed longer than the limit (latched).
    pub discrepancy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoMonitor {
    pub discrepancy_limit_s: f64,
    mismatch_s: f64,
    latched: bool,
    seen_both_low: bool,
}

impl StoMonitor {
    pub fn new(discrepancy_limit_s: f64) -> Self {
        Self {
            discrepancy_limit_s,
            mismatch_s: 0.0,
            latched: false,
            seen_both_low: false,
        }
    }

    /// `ch1` and `ch2` are the channel states, `true` = energised (torque allowed).
    pub fn step(&mut self, ch1: bool, ch2: bool, dt_s: f64) -> StoStatus {
        if ch1 == ch2 {
            self.mismatch_s = 0.0;
        } else {
            self.mismatch_s += dt_s;
            if self.mismatch_s > self.discrepancy_limit_s && !self.latched {
                self.latched = true;
                // An earlier ordinary request must not count as the cycle that clears this fault.
                self.seen_both_low = false;
            }
        }
        let requested = !ch1 && !ch2;
        if requested {
            self.seen_both_low = true;
        }
        // The latched fault is cleared by a full cycle: both channels low, then both high.
        if self.latched && ch1 && ch2 && self.seen_both_low {
            self.latched = false;
            self.seen_both_low = false;
        }
        StoStatus {
            torque_allowed: ch1 && ch2 && !self.latched,
            requested,
            discrepancy: self.latched,
        }
    }
}
