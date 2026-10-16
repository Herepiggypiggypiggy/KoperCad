//! Stand-in solver: extrudes the 2D estimate of `novo-thermal` through the
//! depth. Lets the scheduler, meshing, validation and app lanes work against
//! the contract before the flow core exists. Not physics in `z`.

use std::time::Instant;

use std::sync::Arc;

use novo_thermal::{
    DeviceResult, Error as ThermalError, Field, HotSpot, Options, Scenario, TransientSettings,
    VectorField, simulate_steady_with, simulate_transient,
};

use crate::result::*;
use crate::solver::{CfdError, Control, Solver};
use crate::transient::{
    DeviceSeries, TransientFrame, TransientSolver, TransientSpec, TransientSummary,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct StubSolver;

impl Solver for StubSolver {
    fn solve(
        &self,
        scn: &Scenario,
        fidelity: Fidelity,
        control: &Control,
        publish: &mut dyn FnMut(&CfdSnapshot),
    ) -> Result<CfdSnapshot, CfdError> {
        if fidelity != Fidelity::Coarse {
            return Err(CfdError::Unsupported(fidelity));
        }
        let t0 = Instant::now();
        let r = simulate_steady_with(
            scn,
            Options {
                bands: false,
                advice: false,
            },
        )?;
        if control.is_cancelled() {
            return Err(CfdError::Cancelled);
        }
        let snap = extrude(
            scn,
            &r.air_temp,
            &r.velocity,
            r.devices,
            r.hot_spots,
            Progress {
                fidelity,
                iteration: r.iterations,
                residuals: Residuals::default(),
                converged: r.converged,
                elapsed_s: t0.elapsed().as_secs_f64(),
                energy_balance_rel: if r.balance.loss_w > 0.0 {
                    (r.balance.residual_w / r.balance.loss_w).abs()
                } else {
                    0.0
                },
                monitor_drift_k: 0.0,
            },
            vec![
                "Stub solver: the 2D estimate extruded through the depth, not a 3D solution."
                    .into(),
            ],
        );
        publish(&snap);
        Ok(snap)
    }
}

impl TransientSolver for StubSolver {
    /// The transient run of the 2D engine, each snapshot extruded. Frames are
    /// where the 2D engine keeps snapshots (every `frame_interval_s`).
    fn solve_transient(
        &self,
        scn: &Scenario,
        fidelity: Fidelity,
        spec: &TransientSpec,
        control: &Control,
        publish: &mut dyn FnMut(&TransientFrame),
    ) -> Result<TransientSummary, CfdError> {
        if fidelity != Fidelity::Coarse {
            return Err(CfdError::Unsupported(fidelity));
        }
        if !spec.is_valid() {
            return Err(CfdError::Scenario(ThermalError::Invalid(
                "a transient run needs a duration and a frame interval".into(),
            )));
        }
        let t0 = Instant::now();
        let steps = (spec.frame_count() * 4).clamp(8, 4000);
        let r = simulate_transient(
            scn,
            &TransientSettings {
                duration_s: spec.duration_s,
                steps,
                start_from_steady: spec.start_from_steady,
                load_schedule: spec.load_schedule.clone(),
                snapshot_every_s: Some(spec.frame_interval_s),
            },
        )?;
        let sample_at = |t: f64| {
            r.samples
                .iter()
                .min_by(|a, b| (a.t_s - t).abs().total_cmp(&(b.t_s - t).abs()))
        };
        for snap in &r.snapshots {
            if control.is_cancelled() {
                return Err(CfdError::Cancelled);
            }
            let mut devices = r.last.devices.clone();
            if let Some(sample) = sample_at(snap.t_s) {
                for (d, c) in devices.iter_mut().zip(&sample.device_c) {
                    d.surface_c = *c;
                }
            }
            let frame = extrude(
                scn,
                &snap.air_temp,
                &snap.velocity,
                devices,
                Vec::new(),
                Progress {
                    fidelity,
                    iteration: 0,
                    residuals: Residuals::default(),
                    converged: (snap.t_s - spec.duration_s).abs() < 1e-6,
                    elapsed_s: t0.elapsed().as_secs_f64(),
                    energy_balance_rel: 0.0,
                    monitor_drift_k: 0.0,
                },
                vec![format!(
                    "Stub solver: 2D transient at {:.0} s extruded through the depth.",
                    snap.t_s
                )],
            );
            publish(&TransientFrame {
                time_s: snap.t_s,
                load_factor: spec.load_at(snap.t_s),
                snapshot: Arc::new(frame),
            });
        }
        Ok(TransientSummary {
            fidelity,
            times_s: r.samples.iter().map(|s| s.t_s).collect(),
            load_factor: r.samples.iter().map(|s| spec.load_at(s.t_s)).collect(),
            mean_air_c: r.samples.iter().map(|s| s.mean_air_c).collect(),
            max_air_c: r.samples.iter().map(|s| s.max_air_c).collect(),
            devices: r
                .last
                .devices
                .iter()
                .enumerate()
                .map(|(i, d)| DeviceSeries {
                    id: d.id,
                    surface_c: r
                        .samples
                        .iter()
                        .map(|s| s.device_c.get(i).copied().unwrap_or(d.surface_c))
                        .collect(),
                    air_c: vec![d.air_c; r.samples.len()],
                })
                .collect(),
            time_constant_s: r.time_constant_s,
        })
    }
}

/// The 2D fields repeated through the depth.
fn extrude(
    scn: &Scenario,
    air_temp: &Field,
    velocity: &VectorField,
    devices: Vec<DeviceResult>,
    hot_spots: Vec<HotSpot>,
    progress: Progress,
    notes: Vec<String>,
) -> CfdSnapshot {
    let e = &scn.enclosure;
    let (w, h, d) = (
        e.interior_width_mm(),
        e.interior_height_mm(),
        e.interior_depth_mm(),
    );
    let grid = Grid3::uniform([40, 50, 25], [w, h, d]);
    let n = grid.cells();
    let (mut temp, mut vel) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for k in 0..grid.nz {
        for j in 0..grid.ny {
            for i in 0..grid.nx {
                let [x, y, _] = grid.centre_mm(i, j, k);
                temp.push(air_temp.sample(x, y) as f32);
                let gx = velocity.grid;
                let fi = ((x / gx.cell_w_mm) as usize).min(gx.cols - 1);
                let fj = ((y / gx.cell_h_mm) as usize).min(gx.rows - 1);
                let (vx, vy) = velocity.at(fi, fj);
                vel.push([vx as f32, vy as f32, 0.0]);
            }
        }
    }
    CfdSnapshot {
        grid,
        temperature_c: temp,
        velocity: vel,
        pressure_pa: vec![0.0; n],
        turb_visc_ratio: vec![0.0; n],
        material: vec![0; n],
        materials: vec![MaterialInfo {
            id: 0,
            kind: MaterialKind::Air,
            name: "Air".into(),
            conductivity_w_mk: 0.026,
        }],
        wall_flux_w_m2: vec![0.0; n],
        devices,
        hot_spots,
        progress,
        notes,
        accuracy: AccuracyStatement {
            validated: false,
            headline: "Not validated: placeholder result.".into(),
            benchmarks: Vec::new(),
            limits: vec!["No flow or heat transfer is solved in the depth direction.".into()],
        },
    }
}
