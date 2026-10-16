//! Per level knobs of the cabinet solver.

use std::time::Duration;

use crate::result::Fidelity;

/// What one level does.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelTuning {
    /// Overrides the cell count of the level (tests use small grids).
    pub target_cells: Option<usize>,
    /// Steadiness: the rate of change of the stored heat over the last
    /// window, relative to the heating, must fall below this.
    pub steady_tol: f64,
    /// Limits of the false transient, simulated seconds and steps.
    pub max_time_s: f64,
    pub max_steps: usize,
    /// Divides the heat capacity of the solids on the way to the steady
    /// state (see `SolidsSettings::solid_speedup`).
    pub solid_speedup: f64,
    /// Courant number of the explicit convection.
    pub cfl: f64,
    /// A progress report (iteration, residuals) every this many steps.
    pub report_every: usize,
    /// The full fields are rebuilt for a published snapshot at most this
    /// often; reports in between carry the previous fields.
    pub full_snapshot_every: Duration,
    /// Relative residual of the pressure projection (see
    /// `FlowConfig::pressure_tol`).
    pub pressure_tol: f64,
    /// Start from the 2D estimate instead of the ambient temperature.
    pub warm_start: bool,
    /// Wall-clock budget of the level. A level that is not steady by then
    /// ends with `converged = false` and says so; `None` = no limit.
    pub max_wall: Option<Duration>,
}

/// Settings of [`super::CabinetSolver`].
#[derive(Debug, Clone, PartialEq)]
pub struct Tuning {
    pub coarse: LevelTuning,
    pub medium: LevelTuning,
    pub fine: LevelTuning,
}

impl Tuning {
    pub fn level(&self, f: Fidelity) -> Option<&LevelTuning> {
        match f {
            Fidelity::Preview2d => None,
            Fidelity::Coarse => Some(&self.coarse),
            Fidelity::Medium => Some(&self.medium),
            Fidelity::Fine => Some(&self.fine),
        }
    }

    /// Every level on a grid of about `cells` (tests).
    pub fn with_cells(mut self, cells: usize) -> Tuning {
        for l in [&mut self.coarse, &mut self.medium, &mut self.fine] {
            l.target_cells = Some(cells);
        }
        self
    }
}

fn level(steady_tol: f64, max_time_s: f64, max_steps: usize) -> LevelTuning {
    LevelTuning {
        target_cells: None,
        steady_tol,
        max_time_s,
        max_steps,
        solid_speedup: 100.0,
        cfl: 0.4,
        report_every: 5,
        full_snapshot_every: Duration::from_millis(1500),
        pressure_tol: 1e-5,
        warm_start: true,
        max_wall: None,
    }
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning {
            coarse: level(2.0e-3, 4000.0, 40_000),
            medium: level(1.0e-3, 4000.0, 60_000),
            fine: level(5.0e-4, 4000.0, 80_000),
        }
    }
}
