//! Stand-in solver: extrudes the 2D estimate of `novo-thermal` through the
//! depth. Lets the scheduler, meshing, validation and app lanes work against
//! the contract before the flow core exists. Not physics in `z`.

use std::time::Instant;

use novo_thermal::{Options, Scenario, simulate_steady_with};

use crate::result::*;
use crate::solver::{CfdError, Control, Solver};

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
                    temp.push(r.air_temp.sample(x, y) as f32);
                    let (gx, gy) = (r.velocity.grid, y);
                    let fi = ((x / gx.cell_w_mm) as usize).min(gx.cols - 1);
                    let fj = ((gy / gx.cell_h_mm) as usize).min(gx.rows - 1);
                    let (vx, vy) = r.velocity.at(fi, fj);
                    vel.push([vx as f32, vy as f32, 0.0]);
                }
            }
        }
        let snap = CfdSnapshot {
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
            devices: r.devices,
            hot_spots: r.hot_spots,
            progress: Progress {
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
            notes: vec![
                "Stub solver: the 2D estimate extruded through the depth, not a 3D solution."
                    .into(),
            ],
            accuracy: AccuracyStatement {
                validated: false,
                headline: "Not validated: placeholder result.".into(),
                benchmarks: Vec::new(),
                limits: vec!["No flow or heat transfer is solved in the depth direction.".into()],
            },
        };
        publish(&snap);
        Ok(snap)
    }
}
