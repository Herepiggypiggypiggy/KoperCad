//! Heat-up and load steps in 3D: [`TransientSolver`] for [`CabinetSolver`].
//!
//! The same grid, solids, ducts, openings and wall films as the steady
//! solve, marched with the real heat capacities of the solids
//! (`solid_speedup` 1) so the time is real time. The load schedule scales
//! every heat source through `FlowSolver::heat_scale`. A frame is published
//! every `frame_interval_s` of simulated time; only one is alive here, the
//! caller keeps the ones it wants.
//!
//! Starting from the steady state (a load step) runs the steady solve of
//! the same level first at the load of the first schedule entry and takes
//! its temperature field; the velocity field develops again within seconds.
//!
//! Limits: first-order time stepping with explicit convection and the CFL of
//! the level; laminar unless the solver carries a turbulence model (stepped
//! with the flow as in the false-transient march). The wall-clock budget of the level ends the run early
//! and says so in the notes of the last frame; the summary then stops at the
//! time reached.
//!
//! Where the user finds it: Cabinet view, Heat layer, "Heat-up over time"
//! and the time slider (app side: lane CFD-F).

use std::sync::Arc;
use std::time::Instant;

use novo_thermal::Scenario;

use super::CabinetSolver;
use super::snapshot::View;
use super::statement::{accuracy_with_turbulence, mark_radiation};
use crate::flow::FlowSolver;
use crate::mesh::MeshSettings;
use crate::result::{Fidelity, Progress, Residuals};
use crate::solids::{FlowSetup, SolidsSettings};
use crate::solver::{CfdError, Control, Solver};
use crate::transient::{
    DeviceSeries, TransientFrame, TransientSolver, TransientSpec, TransientSummary,
};
use crate::turb::{TurbModel, TurbState};

/// Shortest step worth taking when a frame or a load change is that close, s.
const MIN_STEP_S: f64 = 1e-6;
/// Gauss-Seidel sweeps of the turbulence equations per flow step (as in the march).
const TURB_SWEEPS: usize = 3;

impl TransientSolver for CabinetSolver {
    fn solve_transient(
        &self,
        scenario: &Scenario,
        fidelity: Fidelity,
        spec: &TransientSpec,
        control: &Control,
        publish: &mut dyn FnMut(&TransientFrame),
    ) -> Result<TransientSummary, CfdError> {
        let lvl = self
            .tuning
            .level(fidelity)
            .ok_or(CfdError::Unsupported(fidelity))?;
        if !spec.is_valid() {
            return Err(CfdError::Setup(
                "a transient run needs a duration, a frame interval and a sorted load schedule"
                    .into(),
            ));
        }
        let t0 = Instant::now();
        let mesh = MeshSettings::for_fidelity(fidelity)
            .map(|m| MeshSettings {
                target_cells: lvl.target_cells.unwrap_or(m.target_cells),
                ..lvl.mesh.unwrap_or(m)
            })
            .map_err(|e| CfdError::Setup(e.to_string()))?;
        let st = SolidsSettings {
            mesh,
            solid_speedup: 1.0,
            duct: self.duct,
            ..SolidsSettings::default()
        };
        let mut setup =
            FlowSetup::new(scenario, &st).map_err(|e| CfdError::Setup(e.to_string()))?;
        // surface radiation as in the steady levels (the solver ticks it in
        // `step`; the film radiation is off while it is on)
        if self.radiation.at(fidelity) {
            setup.enable_radiation(scenario, &crate::rad::RadSettings::default());
        }
        if control.is_cancelled() {
            return Err(CfdError::Cancelled);
        }
        let mut flow = setup.solver();
        flow.cfg.pressure_tol = lvl.pressure_tol;
        let mut notes = vec![
            "Heat-up over time with the real heat capacities of the solids (planning values; \
             devices are uniform blocks); first-order time stepping."
                .to_string(),
        ];
        if spec.start_from_steady {
            let mut at_start = scenario.clone();
            let load = spec.load_at(0.0);
            for d in &mut at_start.devices {
                d.loss_w *= load;
            }
            let steady = self.solve(&at_start, fidelity, control, &mut |_| {})?;
            if steady.temperature_c.len() != flow.t.len() {
                return Err(CfdError::Setup(
                    "the steady start used a different grid than the transient run".into(),
                ));
            }
            for (t, s) in flow.t.iter_mut().zip(&steady.temperature_c) {
                *t = f64::from(*s);
            }
            notes.push(format!(
                "Started from the steady state at {:.0} % load ({}).",
                100.0 * load,
                if steady.progress.converged {
                    "converged"
                } else {
                    "not fully converged"
                }
            ));
        } else {
            notes.push("Cold start: everything at ambient temperature.".into());
        }

        let view = View::new(&setup, scenario, fidelity);
        // The model of the solver, marched together with the flow (the same
        // coupling as the false-transient march).
        let mut turb = (self.turbulence != TurbModel::Laminar).then(|| {
            let h = flow.mesh.edges[1].last().copied().unwrap_or(1.0);
            TurbState::for_flow(&flow, self.turbulence, 0.1, 0.02 * h)
        });
        let cells = flow.mesh.cells();
        let air: Vec<(usize, f64)> = (0..cells)
            .filter(|&c| {
                setup
                    .cfg
                    .medium
                    .as_ref()
                    .is_none_or(|m| m.fluid_frac[c] >= 0.5)
            })
            .map(|c| (c, cell_volume(&flow, c)))
            .collect();
        let air_volume: f64 = air.iter().map(|a| a.1).sum();
        let mut sum = TransientSummary {
            fidelity,
            times_s: Vec::new(),
            load_factor: Vec::new(),
            mean_air_c: Vec::new(),
            max_air_c: Vec::new(),
            devices: scenario
                .devices
                .iter()
                .map(|d| DeviceSeries {
                    id: d.id,
                    surface_c: Vec::new(),
                    air_c: Vec::new(),
                })
                .collect(),
            time_constant_s: None,
        };
        let ambient = setup.ambient_c;
        let model = self.turbulence;
        let mut prev_stored = (flow.time, flow.stored_heat_j(ambient));
        let mut steps = 0usize;
        let mut last_info = (0.0, 0.0);

        let mut emit = |flow: &FlowSolver,
                        steps: usize,
                        last_info: (f64, f64),
                        done: bool,
                        extra: Option<String>,
                        turb: Option<&TurbState>,
                        sum: &mut TransientSummary| {
            let load = spec.load_at(flow.time);
            let heat = (setup.heat_w * load).max(1e-12);
            let stored = flow.stored_heat_j(ambient);
            let rate = if flow.time > prev_stored.0 {
                (stored - prev_stored.1) / (flow.time - prev_stored.0)
            } else {
                0.0
            };
            prev_stored = (flow.time, stored);
            let out: f64 =
                (0..6).map(|f| flow.wall_heat_w(f)).sum::<f64>() + flow.open_heat_out_w(ambient);
            let umax = (0..3)
                .flat_map(|c| flow.vel[c].iter())
                .fold(1e-3_f64, |a, v| a.max(v.abs()));
            let height = flow.mesh.edges[1].last().copied().unwrap_or(1.0);
            let progress = Progress {
                fidelity,
                iteration: steps,
                residuals: Residuals {
                    momentum: (last_info.0 * height / (umax * umax)).min(1.0),
                    continuity: flow.divergence_max().min(1.0),
                    energy: (rate.abs() / heat).min(1.0),
                },
                converged: done,
                elapsed_s: t0.elapsed().as_secs_f64(),
                // heat in, minus heat out, minus what the grid stores
                energy_balance_rel: ((heat - out - rate) / heat).abs(),
                monitor_drift_k: 0.0,
            };
            let mut snap = view.build(flow, &progress);
            if let Some(t) = turb {
                snap.turb_visc_ratio = t.viscosity_ratio();
                snap.accuracy = accuracy_with_turbulence(fidelity, model);
            }
            if snap.radiation_on {
                mark_radiation(&mut snap.accuracy);
            }
            snap.notes.extend(notes.iter().cloned());
            snap.notes.push(format!(
                "Time {:.0} s of {:.0} s, load {:.0} %.",
                flow.time,
                spec.duration_s,
                100.0 * load
            ));
            snap.notes.extend(extra);
            sum.times_s.push(flow.time);
            sum.load_factor.push(load);
            sum.mean_air_c.push(if air_volume > 0.0 {
                air.iter().map(|(c, v)| flow.t[*c] * v).sum::<f64>() / air_volume
            } else {
                ambient
            });
            sum.max_air_c.push(
                air.iter()
                    .map(|(c, _)| flow.t[*c])
                    .fold(f64::NEG_INFINITY, f64::max),
            );
            for (series, d) in sum.devices.iter_mut().zip(&snap.devices) {
                series.surface_c.push(d.surface_c);
                series.air_c.push(d.air_c);
            }
            publish(&TransientFrame {
                time_s: flow.time,
                load_factor: load,
                snapshot: Arc::new(snap),
            });
        };

        emit(&flow, 0, last_info, false, None, turb.as_ref(), &mut sum);
        let mut frame = 1usize;
        while flow.time < spec.duration_s - MIN_STEP_S {
            if control.is_cancelled() {
                return Err(CfdError::Cancelled);
            }
            if lvl.max_wall.is_some_and(|w| t0.elapsed() >= w) {
                // Out of time: the last frame says where the run stopped.
                emit(
                    &flow,
                    steps,
                    last_info,
                    false,
                    Some(format!(
                        "Stopped at {:.0} s of {:.0} s: the time budget of this level ran out.",
                        flow.time, spec.duration_s
                    )),
                    turb.as_ref(),
                    &mut sum,
                );
                break;
            }
            let t = flow.time;
            flow.heat_scale = spec.load_at(t);
            let next_frame = (frame as f64 * spec.frame_interval_s).min(spec.duration_s);
            let next_load = spec
                .load_schedule
                .iter()
                .map(|x| x.0)
                .filter(|x| *x > t + MIN_STEP_S)
                .fold(f64::INFINITY, f64::min);
            let target = next_frame.min(next_load);
            let dt = flow
                .max_dt(lvl.cfl)
                .clamp(1e-4, 5.0)
                .min(target - t)
                .max(MIN_STEP_S);
            if let Some(t) = &turb {
                t.push_to_flow(&mut flow);
            }
            let info = flow.step(dt);
            if let Some(t) = &mut turb {
                t.advance_with(&flow, dt, TURB_SWEEPS);
            }
            last_info = (info.max_accel, info.max_dtdt);
            steps += 1;
            if flow.divergence_max() > 1.0 || !flow.mean_temperature().is_finite() {
                return Err(CfdError::Diverged(steps));
            }
            if flow.time >= next_frame - MIN_STEP_S {
                frame += 1;
                let done = flow.time >= spec.duration_s - MIN_STEP_S;
                emit(&flow, steps, last_info, done, None, turb.as_ref(), &mut sum);
            }
        }
        sum.time_constant_s = time_constant(&sum.times_s, &sum.mean_air_c);
        Ok(sum)
    }
}

fn cell_volume(flow: &FlowSolver, c: usize) -> f64 {
    let n = flow.mesh.n;
    let (i, j, k) = (c % n[0], (c / n[0]) % n[1], c / (n[0] * n[1]));
    flow.mesh.volume([i, j, k])
}

/// Time for the mean air rise to reach 63.2 percent of its value at the end;
/// `None` when the series has not settled (the last tenth still moves by
/// more than 2 percent of the rise), has fewer than ten points or does not rise.
pub(crate) fn time_constant(times: &[f64], mean_air: &[f64]) -> Option<f64> {
    let (first, last) = (*mean_air.first()?, *mean_air.last()?);
    let rise = last - first;
    if rise < 0.05 || times.len() < 10 {
        return None;
    }
    let tail = mean_air[mean_air.len() - 1 - mean_air.len() / 10];
    if (last - tail).abs() > 0.02 * rise {
        return None;
    }
    let i = mean_air.iter().position(|v| v - first >= 0.632 * rise)?;
    if i == 0 {
        return Some(times[0]);
    }
    let (t0, t1) = (times[i - 1], times[i]);
    let (v0, v1) = (mean_air[i - 1] - first, mean_air[i] - first);
    let f = ((0.632 * rise - v0) / (v1 - v0).max(1e-12)).clamp(0.0, 1.0);
    Some(t0 + f * (t1 - t0))
}

#[cfg(test)]
mod tests {
    use super::time_constant;

    #[test]
    fn reads_the_time_constant_of_an_exponential_rise() {
        let tau = 500.0;
        let times: Vec<f64> = (0..=80).map(|i| 50.0 * i as f64).collect();
        let air: Vec<f64> = times
            .iter()
            .map(|t| 25.0 + 10.0 * (1.0 - (-t / tau).exp()))
            .collect();
        let found = time_constant(&times, &air).unwrap();
        assert!((found - tau).abs() < 0.02 * tau, "{found}");
    }

    #[test]
    fn an_unsettled_or_flat_series_has_none() {
        let times: Vec<f64> = (0..=40).map(|i| 50.0 * i as f64).collect();
        let rising: Vec<f64> = times.iter().map(|t| 25.0 + t / 100.0).collect();
        assert_eq!(time_constant(&times, &rising), None);
        let flat = vec![25.0; times.len()];
        assert_eq!(time_constant(&times, &flat), None);
        assert_eq!(time_constant(&times[..5], &rising[..5]), None);
    }
}
