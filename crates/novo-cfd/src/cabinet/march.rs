//! The false-transient march to a steady state, with progress.
//!
//! The same steadiness test as `FlowSetup::run_steady` (the rate of change of
//! the stored heat over a window, relative to the heating), but resumable: the
//! caller gets control every few steps to publish progress and to check the
//! cancel flag.

use novo_thermal::Scenario;

use super::tuning::LevelTuning;
use crate::flow::FlowSolver;
use crate::result::{Fidelity, Progress, Residuals};
use crate::solids::{FlowSetup, STEADY_WINDOW_S};
use crate::solver::Control;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarchLimits {
    pub tol: f64,
    pub max_time_s: f64,
    pub max_steps: usize,
    pub cfl: f64,
}

impl From<&LevelTuning> for MarchLimits {
    fn from(l: &LevelTuning) -> MarchLimits {
        MarchLimits {
            tol: l.steady_tol,
            max_time_s: l.max_time_s,
            max_steps: l.max_steps,
            cfl: l.cfl,
        }
    }
}

/// Why `advance` returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarchStatus {
    /// Ran `report_every` steps, not done.
    Running,
    Converged,
    OutOfTime,
    Cancelled,
}

impl MarchStatus {
    pub fn finished(self) -> bool {
        self != MarchStatus::Running
    }
}

/// Sample every this many steps for the stored-heat window.
const SAMPLE_EVERY: usize = 10;

/// One sample of the heat bookkeeping.
#[derive(Debug, Clone, Copy)]
struct Sample {
    time: f64,
    stored_j: f64,
    walls_w: f64,
    openings_w: f64,
    /// Mean device temperature, degrees C.
    devices_c: f64,
}

pub struct MarchState {
    pub steps: usize,
    pub converged: bool,
    pub storage_rate: f64,
    samples: Vec<Sample>,
    last_info: (f64, f64),
    device_cells: Vec<usize>,
    heat_w: f64,
    ambient_c: f64,
}

impl MarchState {
    pub fn new(setup: &FlowSetup, flow: &FlowSolver) -> MarchState {
        let device_cells = (0..setup.classified.grid.cells())
            .filter(|&c| {
                setup.classified.material[c] != 0
                    && setup
                        .cfg
                        .medium
                        .as_ref()
                        .is_some_and(|m| m.heat_w_m3[c] > 0.0)
            })
            .collect();
        let mut s = MarchState {
            steps: 0,
            converged: false,
            storage_rate: f64::INFINITY,
            samples: Vec::new(),
            last_info: (0.0, 0.0),
            device_cells,
            heat_w: setup.heat_w,
            ambient_c: setup.ambient_c,
        };
        s.sample(flow);
        s
    }

    fn sample(&mut self, flow: &FlowSolver) {
        let w: f64 = (0..6).map(|f| flow.wall_heat_w(f)).sum();
        let dev = if self.device_cells.is_empty() {
            0.0
        } else {
            self.device_cells.iter().map(|&c| flow.t[c]).sum::<f64>()
                / self.device_cells.len() as f64
        };
        self.samples.push(Sample {
            time: flow.time,
            stored_j: flow.stored_heat_j(self.ambient_c),
            walls_w: w,
            openings_w: flow.open_heat_out_w(self.ambient_c),
            devices_c: dev,
        });
    }

    /// The samples inside the last window; `None` before the window is half
    /// filled.
    fn window(&self) -> Option<(Sample, Sample)> {
        let last = *self.samples.last()?;
        let first = self
            .samples
            .partition_point(|x| x.time < last.time - STEADY_WINDOW_S);
        let a = self.samples[first];
        (first > 0 && last.time - a.time > 0.5 * STEADY_WINDOW_S).then_some((a, last))
    }

    /// Marches `report_every` steps or until the run is over.
    pub fn advance(
        &mut self,
        _setup: &FlowSetup,
        flow: &mut FlowSolver,
        lim: &MarchLimits,
        control: &Control,
        report_every: usize,
    ) -> MarchStatus {
        for _ in 0..report_every.max(1) {
            if control.is_cancelled() {
                return MarchStatus::Cancelled;
            }
            if self.steps >= lim.max_steps || flow.time >= lim.max_time_s {
                return MarchStatus::OutOfTime;
            }
            let dt = flow.max_dt(lim.cfl).clamp(1e-4, 5.0);
            let info = flow.step(dt);
            self.last_info = (info.max_accel, info.max_dtdt);
            self.steps += 1;
            if self.steps.is_multiple_of(SAMPLE_EVERY) {
                self.sample(flow);
                if let Some((a, b)) = self.window() {
                    self.storage_rate = if self.heat_w > 0.0 {
                        (b.stored_j - a.stored_j) / (b.time - a.time) / self.heat_w
                    } else {
                        0.0
                    };
                    if self.storage_rate.abs() < lim.tol {
                        self.converged = true;
                        return MarchStatus::Converged;
                    }
                }
            }
        }
        MarchStatus::Running
    }

    /// Progress of the run so far.
    ///
    /// Residuals of a false transient, all dimensionless and falling towards
    /// zero at a steady state:
    /// - `energy`: rate of change of the stored heat over the heating (the
    ///   quantity the stop test uses; 1 until the first window is full),
    /// - `momentum`: largest acceleration over the convective scale
    ///   `max(u)^2 / H`,
    /// - `continuity`: the largest cell divergence over the largest face flux.
    pub fn progress(
        &self,
        fidelity: Fidelity,
        elapsed_s: f64,
        flow: &FlowSolver,
        setup: &FlowSetup,
    ) -> Progress {
        let umax = (0..3)
            .flat_map(|c| flow.vel[c].iter())
            .fold(1e-3_f64, |a, v| a.max(v.abs()));
        let height = flow.mesh.edges[1].last().copied().unwrap_or(1.0);
        let momentum = (self.last_info.0 * height / (umax * umax)).min(1.0);
        let energy = if self.storage_rate.is_finite() {
            self.storage_rate.abs().min(1.0)
        } else {
            1.0
        };
        let (balance, drift) = self.window().map_or((1.0, f64::NAN), |(a, b)| {
            let n = self
                .samples
                .iter()
                .filter(|x| x.time >= a.time)
                .count()
                .max(1) as f64;
            let walls = self
                .samples
                .iter()
                .filter(|x| x.time >= a.time)
                .map(|x| x.walls_w)
                .sum::<f64>()
                / n;
            let open = self
                .samples
                .iter()
                .filter(|x| x.time >= a.time)
                .map(|x| x.openings_w)
                .sum::<f64>()
                / n;
            let imb = if setup.heat_w > 0.0 {
                ((setup.heat_w - walls - open) / setup.heat_w).abs()
            } else {
                0.0
            };
            (imb, b.devices_c - a.devices_c)
        });
        Progress {
            fidelity,
            iteration: self.steps,
            residuals: Residuals {
                momentum,
                continuity: flow.divergence_max().min(1.0),
                energy,
            },
            converged: self.converged,
            elapsed_s,
            energy_balance_rel: balance,
            monitor_drift_k: if drift.is_finite() { drift.abs() } else { 0.0 },
        }
    }
}

/// Scenario heat the grid did not receive, for the notes.
pub fn lost_heat_note(setup: &FlowSetup, scn: &Scenario) -> Option<String> {
    (setup.lost_heat_w > 1e-9).then(|| {
        format!(
            "{:.0} W of {:.0} W device loss lies outside the enclosure and is not simulated.",
            setup.lost_heat_w,
            scn.devices.iter().map(|d| d.loss_w).sum::<f64>()
        )
    })
}
