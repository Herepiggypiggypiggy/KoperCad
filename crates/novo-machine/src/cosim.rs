//! Fixed-step co-simulation loop (lane L5).

/// Fixed time step, deterministic by design (no adaptive stepping by default).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepConfig {
    pub dt_s: f64,
}

impl Default for StepConfig {
    fn default() -> Self {
        Self { dt_s: 0.005 }
    }
}
