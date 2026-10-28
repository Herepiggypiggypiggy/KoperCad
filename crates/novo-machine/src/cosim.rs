//! Fixed-step co-simulation loop (lane L5).

use novo_drive::InputError;

/// Longest step the loop accepts: beyond it the mechanics and the brake delays are no longer
/// resolved (a step this long skips a 20 ms brake release in one go).
pub const MAX_DT_S: f64 = 0.1;

/// Fixed time step, deterministic by design (no adaptive stepping by default).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepConfig {
    pub dt_s: f64,
}

impl StepConfig {
    /// A step in seconds: finite, greater than zero and at most [`MAX_DT_S`].
    pub fn new(dt_s: f64) -> Result<Self, InputError> {
        let config = Self { dt_s };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), InputError> {
        if !self.dt_s.is_finite() {
            return Err(InputError::NotFinite { field: "dt_s" });
        }
        if self.dt_s <= 0.0 || self.dt_s > MAX_DT_S {
            return Err(InputError::OutOfRange {
                field: "dt_s",
                value: self.dt_s,
                min: 0.0,
                max: MAX_DT_S,
            });
        }
        Ok(())
    }
}

impl Default for StepConfig {
    fn default() -> Self {
        Self { dt_s: 0.005 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_step_must_be_small_and_positive() {
        assert!(StepConfig::new(0.005).is_ok());
        assert!(StepConfig::new(MAX_DT_S).is_ok());
        for bad in [0.0, -0.005, 0.2, 1e9, f64::NAN, f64::INFINITY] {
            assert!(StepConfig::new(bad).is_err(), "{bad}");
        }
    }
}
