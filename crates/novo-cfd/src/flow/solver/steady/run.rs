//! A steady run: iterations, window-mean convergence test and averaged fields.
//!
//! Laminar plumes above hot devices do not settle to a fixed point: the
//! iterates wobble (a few percent of the velocity, a fraction of a kelvin).
//! Like the time march, the run therefore judges convergence on window
//! means, not on the change of one iteration, and hands back the mean of
//! the last window as the answer.
//!
//! Two windows of `window` iterations are compared. The run has converged
//! when, over the newer one,
//! - the heat leaving through walls and openings matches the heating within
//!   `tol_balance` (relative), and
//! - the mean temperature of the probe cells (or of all cells) moved by less
//!   than `tol_drift` of its rise above the ambient since the older window, and
//! - the rms velocity moved by less than `tol_flow` (relative).

use super::{SteadyConfig, SteadyInfo, SteadyState};
use crate::flow::solver::FlowSolver;

#[derive(Debug, Clone, PartialEq)]
pub struct SteadyLimits {
    /// Iterations per window.
    pub window: usize,
    /// Heat balance of the newer window, `|in - out| / in`.
    pub tol_balance: f64,
    /// Drift of the probe temperature between the two windows, relative to
    /// its rise above the ambient (`ambient_c` of the run).
    pub tol_drift: f64,
    /// Drift of the rms velocity between the two windows, relative.
    pub tol_flow: f64,
    pub max_iterations: usize,
    /// A window that does not converge while the iterates wobble by more than
    /// this (std of the heat out over the heating) doubles, up to
    /// `max_window_factor` times `window`: a longer mean has less noise.
    pub grow_wobble: f64,
    /// Iterations over which the wobble of an iterate is correlated: the
    /// standard error of a window mean is `std * sqrt(corr_len / window)`.
    /// The tolerances are widened by two standard errors (three for a
    /// difference of two windows), so a noisy flow is judged on whether its
    /// mean still moves, not on the noise.
    pub corr_len: f64,
    pub max_window_factor: usize,
}

impl Default for SteadyLimits {
    fn default() -> Self {
        SteadyLimits {
            window: 100,
            tol_balance: 0.02,
            tol_drift: 0.004,
            tol_flow: 0.005,
            max_iterations: 5000,
            grow_wobble: 0.005,
            corr_len: 50.0,
            max_window_factor: 8,
        }
    }
}

/// Why `advance` returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Converged,
    OutOfIterations,
}

/// How far the run got.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteadyReport {
    pub iterations: usize,
    /// Index of the running stage.
    pub stage: usize,
    pub converged: bool,
    /// `(heating - heat out) / heating` over the last full window (1 before
    /// the first), 0 = balanced.
    pub imbalance: f64,
    /// Drift of the probe temperature between the last two windows, K
    /// (infinite before the second).
    pub drift_k: f64,
    /// Relative drift of the rms velocity between the last two windows.
    pub flow_drift: f64,
    /// Standard deviation of the heat out over the last window relative to
    /// the heating: how much the iterates wobble.
    pub wobble: f64,
}

pub struct SteadyRun {
    pub state: SteadyState,
    pub limits: SteadyLimits,
    stages: Vec<SteadyConfig>,
    stage: usize,
    pub last: Option<SteadyInfo>,
    probes: Vec<usize>,
    heat_in_w: f64,
    ambient_c: f64,
    out_w: Vec<f64>,
    probe_t: Vec<f64>,
    rms: Vec<f64>,
    prev_window_t: Option<(f64, f64)>,
    window_now: usize,
    since_eval: usize,
    sums: Option<Sums>,
    report: SteadyReport,
    done: Option<RunStatus>,
}

struct Sums {
    n: usize,
    t: Vec<f64>,
    p: Vec<f64>,
    vel: [Vec<f64>; 3],
}

impl Sums {
    fn new(flow: &FlowSolver) -> Sums {
        Sums {
            n: 0,
            t: vec![0.0; flow.t.len()],
            p: vec![0.0; flow.p.len()],
            vel: std::array::from_fn(|c| vec![0.0; flow.vel[c].len()]),
        }
    }

    fn add(&mut self, flow: &FlowSolver) {
        self.n += 1;
        for (s, v) in self.t.iter_mut().zip(&flow.t) {
            *s += v;
        }
        for (s, v) in self.p.iter_mut().zip(&flow.p) {
            *s += v;
        }
        for c in 0..3 {
            for (s, v) in self.vel[c].iter_mut().zip(&flow.vel[c]) {
                *s += v;
            }
        }
    }

    fn apply(&self, flow: &mut FlowSolver) {
        let k = 1.0 / self.n.max(1) as f64;
        for (v, s) in flow.t.iter_mut().zip(&self.t) {
            *v = s * k;
        }
        for (v, s) in flow.p.iter_mut().zip(&self.p) {
            *v = s * k;
        }
        for c in 0..3 {
            for (v, s) in flow.vel[c].iter_mut().zip(&self.vel[c]) {
                *v = s * k;
            }
        }
    }
}

impl SteadyRun {
    /// `probes`: cells whose mean temperature is the drift monitor (the
    /// device cells); empty = all cells.
    pub fn new(
        flow: &FlowSolver,
        cfg: SteadyConfig,
        limits: SteadyLimits,
        ambient_c: f64,
        probes: Vec<usize>,
    ) -> SteadyRun {
        SteadyRun::staged(flow, vec![cfg], limits, ambient_c, probes)
    }

    /// Runs the stages one after the other, each to window convergence, the
    /// next one starting from the fields the last left (the usual order:
    /// [`SteadyConfig::first_order`] then [`SteadyConfig::second_order`]).
    pub fn staged(
        flow: &FlowSolver,
        stages: Vec<SteadyConfig>,
        limits: SteadyLimits,
        ambient_c: f64,
        probes: Vec<usize>,
    ) -> SteadyRun {
        assert!(!stages.is_empty(), "a steady run needs a stage");
        let window0 = limits.window.max(2);
        SteadyRun {
            state: SteadyState::new(flow, stages[0].clone()),
            stages,
            stage: 0,
            limits,
            last: None,
            probes,
            heat_in_w: flow.heating_w(),
            ambient_c,
            out_w: Vec::new(),
            probe_t: Vec::new(),
            rms: Vec::new(),
            prev_window_t: None,
            window_now: window0,
            since_eval: 0,
            sums: None,
            report: SteadyReport {
                iterations: 0,
                stage: 0,
                converged: false,
                imbalance: 1.0,
                drift_k: f64::INFINITY,
                flow_drift: f64::INFINITY,
                wobble: 0.0,
            },
            done: None,
        }
    }

    pub fn report(&self) -> SteadyReport {
        self.report
    }

    pub fn heating_w(&self) -> f64 {
        self.heat_in_w
    }

    /// Runs up to `iterations` more iterations. On convergence (or at the
    /// iteration limit) the fields of `flow` are replaced by the mean of the
    /// last full window.
    pub fn advance(&mut self, flow: &mut FlowSolver, iterations: usize) -> RunStatus {
        self.advance_with(flow, iterations, &mut |_| {}, &mut |_, _| {})
    }

    /// [`SteadyRun::advance`] with hooks around every iteration: `before`
    /// sees the flow just before the iteration (put eddy viscosity in, say),
    /// `after` sees it afterwards with the pseudo step used, s (advance a
    /// turbulence model).
    pub fn advance_with(
        &mut self,
        flow: &mut FlowSolver,
        iterations: usize,
        before: &mut dyn FnMut(&mut FlowSolver),
        after: &mut dyn FnMut(&FlowSolver, f64),
    ) -> RunStatus {
        if let Some(s) = self.done {
            return s;
        }
        for _ in 0..iterations.max(1) {
            before(flow);
            let info = flow.steady_iterate(&mut self.state);
            after(flow, info.dt);
            self.last = Some(info);
            self.report.iterations += 1;
            let out: f64 = (0..6).map(|f| flow.wall_heat_w(f)).sum::<f64>()
                + flow.open_heat_out_w(self.ambient_c);
            self.out_w.push(out);
            self.probe_t.push(if self.probes.is_empty() {
                flow.mean_temperature()
            } else {
                self.probes.iter().map(|&c| flow.t[c]).sum::<f64>() / self.probes.len() as f64
            });
            self.rms.push(flow.rms_velocity());
            self.sums.get_or_insert_with(|| Sums::new(flow)).add(flow);
            self.since_eval += 1;
            let w = self.window_now;
            if self.since_eval >= w {
                self.since_eval = 0;
                let n = self.out_w.len();
                let (out_w, probe_t) = (&self.out_w[n - w..], &self.probe_t[n - w..]);
                let rms_w = &self.rms[n - w..];
                let mean_rms = rms_w.iter().sum::<f64>() / w as f64;
                let std_of = |x: &[f64], mean: f64| {
                    (x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / w as f64).sqrt()
                };
                let se = (self.limits.corr_len / w as f64).min(1.0).sqrt();
                let mean_out = out_w.iter().sum::<f64>() / w as f64;
                let mean_t = probe_t.iter().sum::<f64>() / w as f64;
                let var = out_w.iter().map(|x| (x - mean_out).powi(2)).sum::<f64>() / w as f64;
                let scale = self.heat_in_w.max(1e-12);
                self.report.imbalance = if self.heat_in_w > 0.0 {
                    (self.heat_in_w - mean_out) / self.heat_in_w
                } else {
                    0.0
                };
                self.report.wobble = var.sqrt() / scale;
                (self.report.drift_k, self.report.flow_drift) =
                    self.prev_window_t
                        .map_or((f64::INFINITY, f64::INFINITY), |p| {
                            (
                                (mean_t - p.0).abs(),
                                (mean_rms - p.1).abs() / mean_rms.max(1e-12),
                            )
                        });
                self.prev_window_t = Some((mean_t, mean_rms));
                let se_out = std_of(out_w, mean_out) * se / scale;
                let se_t = std_of(probe_t, mean_t) * se;
                let se_rms = std_of(rms_w, mean_rms) * se / mean_rms.max(1e-12);
                let ok = self.report.imbalance.abs() < self.limits.tol_balance + 2.0 * se_out
                    && self.report.drift_k
                        < self.limits.tol_drift * (mean_t - self.ambient_c).abs().max(1.0)
                            + 3.0 * se_t
                    && self.report.flow_drift < self.limits.tol_flow + 3.0 * se_rms;
                let out_of = self.report.iterations >= self.limits.max_iterations;
                if ok && self.stage + 1 < self.stages.len() {
                    self.stage += 1;
                    self.report.stage = self.stage;
                    self.state.cfg = self.stages[self.stage].clone();
                    self.out_w.clear();
                    self.probe_t.clear();
                    self.rms.clear();
                    self.prev_window_t = None;
                    self.window_now = self.limits.window.max(2);
                    self.sums = None;
                    continue;
                }
                if ok || out_of {
                    if let Some(s) = &self.sums {
                        s.apply(flow);
                    }
                    self.report.converged = ok;
                    let status = if ok {
                        RunStatus::Converged
                    } else {
                        RunStatus::OutOfIterations
                    };
                    self.done = Some(status);
                    return status;
                }
                self.sums = None;
                if self.report.wobble > self.limits.grow_wobble
                    && self.window_now < self.limits.window * self.limits.max_window_factor
                {
                    self.window_now *= 2;
                }
            }
        }
        RunStatus::Running
    }
}

impl FlowSolver {
    /// Total heating of the cells, W.
    pub fn heating_w(&self) -> f64 {
        let m = &self.mesh;
        let mut sum = 0.0;
        for k in 0..m.n[2] {
            for j in 0..m.n[1] {
                for i in 0..m.n[0] {
                    let id = m.cell([i, j, k]);
                    sum +=
                        self.heat_scale * self.q_cell[id] * self.c_cell[id] * m.volume([i, j, k]);
                }
            }
        }
        sum
    }
}

impl FlowSolver {
    /// Root mean square of the velocity samples (all faces), m/s.
    pub fn rms_velocity(&self) -> f64 {
        let (mut sum, mut n) = (0.0, 0usize);
        for c in 0..3 {
            sum += self.vel[c].iter().map(|v| v * v).sum::<f64>();
            n += self.vel[c].len();
        }
        (sum / n.max(1) as f64).sqrt()
    }
}
