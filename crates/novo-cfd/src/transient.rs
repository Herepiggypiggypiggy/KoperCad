//! Transient runs in the result contract: frames over time.
//!
//! [`Solver`](crate::Solver) answers "what is the steady state"; a
//! [`TransientSolver`] answers "how does the cabinet heat up, and what does a
//! load step do". Frames stream through a callback, so the solver never
//! holds more than one; the caller keeps as many as its memory allows (a
//! Coarse frame is about 1.5 MB, a Medium frame about 15 MB). [`CfdSnapshot`]
//! itself is unchanged, so existing consumers keep compiling.
//!
//! Where the user finds it: Cabinet view, Heat layer, time slider (lane
//! CFD-F), started from the Simulate dialog with "Heat-up over time".

use std::sync::Arc;

use novo_thermal::Scenario;

use crate::result::{CfdSnapshot, Fidelity};
use crate::solver::{CfdError, Control};

/// What to simulate over time.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientSpec {
    /// Simulated time, s.
    pub duration_s: f64,
    /// A frame is published this often (simulated seconds); the first frame
    /// is at 0 and the last at `duration_s`.
    pub frame_interval_s: f64,
    /// `(from time s, share of the rated loss 0..1)`; the share before the
    /// first entry is 1.
    pub load_schedule: Vec<(f64, f64)>,
    /// Begin from the steady state at the first load (a load step) instead
    /// of from uniform ambient temperature (a cold start).
    pub start_from_steady: bool,
}

impl Default for TransientSpec {
    fn default() -> Self {
        TransientSpec {
            duration_s: 3600.0,
            frame_interval_s: 60.0,
            load_schedule: Vec::new(),
            start_from_steady: false,
        }
    }
}

impl TransientSpec {
    /// Share of the rated loss at time `t`.
    pub fn load_at(&self, t: f64) -> f64 {
        self.load_schedule
            .iter()
            .rfind(|(from, _)| *from <= t)
            .map_or(1.0, |(_, f)| f.clamp(0.0, 1.0))
    }

    /// Frames a full run publishes (including the first and the last).
    pub fn frame_count(&self) -> usize {
        if self.frame_interval_s > 0.0 && self.duration_s > 0.0 {
            (self.duration_s / self.frame_interval_s).ceil() as usize + 1
        } else {
            0
        }
    }

    pub fn is_valid(&self) -> bool {
        self.duration_s > 0.0
            && self.duration_s.is_finite()
            && self.frame_interval_s > 0.0
            && self.frame_interval_s.is_finite()
    }
}

/// The solution at one moment of a transient run.
#[derive(Debug, Clone)]
pub struct TransientFrame {
    pub time_s: f64,
    /// Share of the rated loss that was on at this moment.
    pub load_factor: f64,
    pub snapshot: Arc<CfdSnapshot>,
}

/// Surface temperature of one device over time.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceSeries {
    pub id: u64,
    /// Mean surface temperature at each frame time, degrees C.
    pub surface_c: Vec<f64>,
    /// Mean air temperature around the device at each frame time.
    pub air_c: Vec<f64>,
}

/// What a transient run returns; the frames themselves went to the callback.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientSummary {
    pub fidelity: Fidelity,
    /// Frame times, s.
    pub times_s: Vec<f64>,
    pub load_factor: Vec<f64>,
    pub mean_air_c: Vec<f64>,
    pub max_air_c: Vec<f64>,
    /// In `Scenario::devices` order.
    pub devices: Vec<DeviceSeries>,
    /// Time for the mean air rise to reach 63 percent of its value at the
    /// end, s; `None` when the run ends before it settles.
    pub time_constant_s: Option<f64>,
}

/// Turns a scenario into frames over time. Same rules as
/// [`Solver`](crate::Solver): runs on the caller's thread, checks `control`
/// every step, never touches a UI.
pub trait TransientSolver: Send + Sync {
    fn solve_transient(
        &self,
        scenario: &Scenario,
        fidelity: Fidelity,
        spec: &TransientSpec,
        control: &Control,
        publish: &mut dyn FnMut(&TransientFrame),
    ) -> Result<TransientSummary, CfdError>;
}
