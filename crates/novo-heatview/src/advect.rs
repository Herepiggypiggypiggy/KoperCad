//! The heat cloud on the move: the solved air flow carries and mixes the air
//! temperature from a cold start, so the cloud can be watched growing off the
//! hot devices and drifting with the air.
//!
//! What it is. A transport model on the grid of a solved result. The air
//! velocity is frozen at the solver's, every solid cell (devices, plate, rails,
//! walls) is held at its solved temperature, and the air temperature obeys
//!
//! ```text
//! dT/dt = -u . grad T + div(alpha_eff grad T) + (h_film / (rho cp dx)) (T_solid - T)
//! ```
//!
//! with first-order upwind advection, `alpha_eff` the molecular plus the
//! turbulent diffusivity of the solved result (`nu_t / Pr_t`), and a film term
//! on every air face that touches a solid. Explicit time stepping, stable by
//! construction (the step is limited by the advective, diffusive and film
//! rates together).
//!
//! What it is not. It is not a second CFD solve: the flow does not react to the
//! warming (buoyancy is whatever the solved flow already contains), and
//! first-order upwind smears fronts. It shows how the heat moves and how long
//! it takes; the picture it settles on is close to, not identical with, the
//! solver's own temperature field. [`Advector::match_rmse_k`] says how close
//! (the settled air against the solved air, K) so the program can state it.
//! Frame as [`crate::cfd`]: `y` down, millimetres; time is in seconds.

use novo_cfd::CfdSnapshot;

/// Air density, kg/m3, and specific heat, J/kgK.
use crate::flux::{AIR_CP, AIR_DENSITY};

/// Molecular thermal diffusivity of air, m2/s, and kinematic viscosity, m2/s.
const ALPHA_AIR: f64 = 2.2e-5;
const NU_AIR: f64 = 1.5e-5;
/// Turbulent Prandtl number.
const PR_TURB: f64 = 0.85;
/// Heat transfer coefficient of the film between air and a solid face,
/// W/m2K: natural convection plus a little forced air, the order of what the
/// solved results show at the device faces.
const H_FILM: f64 = 8.0;
/// Normal speed above which a grid boundary face counts as an opening, m/s;
/// air entering through one arrives at ambient temperature.
const OPEN_SPEED: f64 = 0.03;
/// Safety factor on the step limit.
const SAFETY: f64 = 0.45;

/// The state of the transport model for one solved result.
#[derive(Debug, Clone)]
pub struct Advector {
    nx: usize,
    ny: usize,
    nz: usize,
    /// Cell sizes along each axis, m.
    h: [Vec<f64>; 3],
    /// Air cells (true) and solids (false).
    air: Vec<bool>,
    velocity: Vec<[f32; 3]>,
    /// Effective diffusivity of each cell, m2/s.
    alpha: Vec<f32>,
    /// Solved temperatures (the solids' values are held for ever), C.
    solved: Vec<f32>,
    /// Current temperatures, C.
    t: Vec<f32>,
    scratch: Vec<f32>,
    ambient: f32,
    time_s: f64,
    dt_max: f64,
}

impl Advector {
    /// A cold start: every air cell at `ambient_c`, every solid at its solved
    /// temperature. `None` when the snapshot is not consistent.
    pub fn new(snap: &CfdSnapshot, ambient_c: f64) -> Option<Advector> {
        if !snap.is_consistent() {
            return None;
        }
        let g = &snap.grid;
        let size = |e: &[f64]| -> Vec<f64> { e.windows(2).map(|w| (w[1] - w[0]) * 1e-3).collect() };
        let h = [
            size(&g.x_edges_mm),
            size(&g.y_edges_mm),
            size(&g.z_edges_mm),
        ];
        let air: Vec<bool> = snap.material.iter().map(|m| *m == 0).collect();
        let alpha: Vec<f32> = snap
            .turb_visc_ratio
            .iter()
            .map(|r| (ALPHA_AIR + NU_AIR * f64::from((*r).max(0.0)) / PR_TURB) as f32)
            .collect();
        let ambient = ambient_c as f32;
        let t: Vec<f32> = snap
            .temperature_c
            .iter()
            .zip(&air)
            .map(|(v, a)| if *a { ambient } else { *v })
            .collect();
        let mut me = Advector {
            nx: g.nx,
            ny: g.ny,
            nz: g.nz,
            h,
            air,
            velocity: snap.velocity.clone(),
            alpha,
            solved: snap.temperature_c.clone(),
            scratch: t.clone(),
            t,
            ambient,
            time_s: 0.0,
            dt_max: 1.0,
        };
        me.dt_max = me.stable_step();
        Some(me)
    }

    /// Back to the cold start.
    pub fn restart(&mut self) {
        for (n, v) in self.t.iter_mut().enumerate() {
            *v = if self.air[n] {
                self.ambient
            } else {
                self.solved[n]
            };
        }
        self.time_s = 0.0;
    }

    /// Time since the start, s.
    pub fn time_s(&self) -> f64 {
        self.time_s
    }

    /// The current temperature of every cell, C (solids at their solved value).
    pub fn temperature_c(&self) -> &[f32] {
        &self.t
    }

    /// The largest step the model takes, s.
    pub fn step_limit_s(&self) -> f64 {
        self.dt_max
    }

    /// Largest stable step, s: the sum of the advective, diffusive and film
    /// rates of the worst cell bounds `dt`.
    fn stable_step(&self) -> f64 {
        let mut worst = 1e-9_f64;
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let n = (k * self.ny + j) * self.nx + i;
                    if !self.air[n] {
                        continue;
                    }
                    let hh = [self.h[0][i], self.h[1][j], self.h[2][k]];
                    let v = self.velocity[n];
                    let a = f64::from(self.alpha[n]);
                    let mut rate = 0.0;
                    for ax in 0..3 {
                        rate += f64::from(v[ax]).abs() / hh[ax] + 4.0 * a / (hh[ax] * hh[ax]);
                    }
                    let smallest = hh[0].min(hh[1]).min(hh[2]);
                    rate += 6.0 * H_FILM / (AIR_DENSITY * AIR_CP * smallest);
                    worst = worst.max(rate);
                }
            }
        }
        (SAFETY / worst).min(1.0)
    }

    /// Advances by up to `seconds` of model time in at most `max_steps` steps;
    /// returns the time advanced, s. Fewer steps than the time needs leaves
    /// the model behind the clock, which the caller can tell from the return.
    pub fn advance(&mut self, seconds: f64, max_steps: usize) -> f64 {
        if seconds <= 0.0 || max_steps == 0 {
            return 0.0;
        }
        let steps = ((seconds / self.dt_max).ceil() as usize).clamp(1, max_steps);
        let dt = (seconds / steps as f64).min(self.dt_max);
        for _ in 0..steps {
            self.step(dt);
        }
        dt * steps as f64
    }

    fn step(&mut self, dt: f64) {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let dims = [nx, ny, nz];
        let stride = [1usize, nx, nx * ny];
        let film = H_FILM / (AIR_DENSITY * AIR_CP);
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let n = (k * ny + j) * nx + i;
                    if !self.air[n] {
                        self.scratch[n] = self.solved[n];
                        continue;
                    }
                    let here = f64::from(self.t[n]);
                    let at = [i, j, k];
                    let v = self.velocity[n];
                    let a = f64::from(self.alpha[n]);
                    let mut d_t = 0.0;
                    for ax in 0..3 {
                        let hc = self.h[ax][at[ax]];
                        // Neighbour values, with what lies beyond the grid.
                        let lo = (at[ax] > 0).then(|| n - stride[ax]);
                        let hi = (at[ax] + 1 < dims[ax]).then(|| n + stride[ax]);
                        let speed = f64::from(v[ax]);
                        // Advection, upwind.
                        let grad = if speed > 0.0 {
                            match lo {
                                Some(m) => (here - f64::from(self.t[m])) / hc,
                                None if speed > OPEN_SPEED => (here - f64::from(self.ambient)) / hc,
                                None => 0.0,
                            }
                        } else {
                            match hi {
                                Some(m) => (f64::from(self.t[m]) - here) / hc,
                                None if -speed > OPEN_SPEED => {
                                    (f64::from(self.ambient) - here) / hc
                                }
                                None => 0.0,
                            }
                        };
                        d_t -= speed * grad;
                        // Diffusion, across faces that exist; a solid neighbour
                        // is the film's business below.
                        let mut flux_sum = 0.0;
                        if let Some(m) = lo
                            && self.air[m]
                        {
                            let d = 0.5 * (hc + self.h[ax][at[ax] - 1]);
                            flux_sum += (f64::from(self.t[m]) - here) / d;
                        }
                        if let Some(m) = hi
                            && self.air[m]
                        {
                            let d = 0.5 * (hc + self.h[ax][at[ax] + 1]);
                            flux_sum += (f64::from(self.t[m]) - here) / d;
                        }
                        d_t += a * flux_sum / hc;
                        // Film to a solid on either face.
                        for m in [lo, hi].into_iter().flatten() {
                            if !self.air[m] {
                                d_t += film * (f64::from(self.t[m]) - here) / hc;
                            }
                        }
                    }
                    self.scratch[n] = (here + dt * d_t) as f32;
                }
            }
        }
        std::mem::swap(&mut self.t, &mut self.scratch);
        self.time_s += dt;
    }

    /// Root mean square difference between the model's air and the solved air,
    /// K, over the air cells: how close the settled picture is to the
    /// solver's.
    pub fn match_rmse_k(&self) -> f64 {
        let (mut sum, mut count) = (0.0, 0usize);
        for n in 0..self.t.len() {
            if self.air[n] {
                let d = f64::from(self.t[n] - self.solved[n]);
                sum += d * d;
                count += 1;
            }
        }
        if count == 0 {
            0.0
        } else {
            (sum / count as f64).sqrt()
        }
    }

    /// Mean air temperature, C.
    pub fn mean_air_c(&self) -> f64 {
        let (mut sum, mut count) = (0.0, 0usize);
        for n in 0..self.t.len() {
            if self.air[n] {
                sum += f64::from(self.t[n]);
                count += 1;
            }
        }
        if count == 0 {
            f64::from(self.ambient)
        } else {
            sum / count as f64
        }
    }

    /// How fast the picture still changes: the largest change of an air cell
    /// over the last `seconds` is not tracked, so ask by comparing two copies;
    /// this is the mean change per second of one step of `probe_s`, K/s, on a
    /// copy (the model itself is not moved).
    pub fn drift_k_per_s(&self, probe_s: f64) -> f64 {
        let mut copy = self.clone();
        let before: Vec<f32> = copy.t.clone();
        let done = copy.advance(probe_s, 8);
        if done <= 0.0 {
            return 0.0;
        }
        let (mut sum, mut count) = (0.0, 0usize);
        for ((now, was), air) in copy.t.iter().zip(&before).zip(&copy.air) {
            if *air {
                sum += f64::from((now - was).abs());
                count += 1;
            }
        }
        if count == 0 {
            0.0
        } else {
            sum / count as f64 / done
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_cfd::{
        AccuracyStatement, Fidelity, Grid3, MaterialInfo, MaterialKind, Progress, Residuals,
    };

    /// A row of `nx` cells of 10 mm (4 x 4 across, all air) with a uniform
    /// air velocity `u` along +x and temperature `t0`.
    fn row(nx: usize, u: f32, t0: f32) -> CfdSnapshot {
        let grid = Grid3::uniform([nx, 4, 4], [10.0 * nx as f64, 40.0, 40.0]);
        let n = grid.cells();
        CfdSnapshot {
            grid,
            temperature_c: vec![t0; n],
            velocity: vec![[u, 0.0, 0.0]; n],
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
            plate_rear_c: None,
            plate_field_c: None,
            wall_field_c: None,
            devices: Vec::new(),
            hot_spots: Vec::new(),
            opening_flows: Vec::new(),
            progress: Progress {
                fidelity: Fidelity::Coarse,
                iteration: 1,
                residuals: Residuals::default(),
                converged: true,
                elapsed_s: 0.0,
                energy_balance_rel: 0.0,
                monitor_drift_k: 0.0,
            },
            notes: Vec::new(),
            accuracy: AccuracyStatement::default(),
            heat_paths: Vec::new(),
            radiation_on: false,
        }
    }

    /// Cells along x of the row at (y, z) = (0, 0).
    fn line(a: &Advector) -> Vec<f32> {
        (0..a.nx).map(|i| a.t[i]).collect()
    }

    /// A hot plate across the whole inlet end held at 60 C (solid column
    /// x = 0), the air moving away from it at `u` m/s.
    fn heater(nx: usize, u: f32) -> CfdSnapshot {
        let mut s = row(nx, u, 20.0);
        s.materials.push(MaterialInfo {
            id: 1,
            kind: MaterialKind::Plate,
            name: "Plate".into(),
            conductivity_w_mk: 200.0,
        });
        for k in 0..4 {
            for j in 0..4 {
                let n = s.grid.index(0, j, k);
                s.material[n] = 1;
                s.temperature_c[n] = 60.0;
                s.velocity[n] = [0.0; 3];
            }
        }
        s
    }

    #[test]
    fn a_front_travels_at_the_air_speed() {
        // Hot air (40 C) enters at the left of a 1 m/s stream over a 20 C
        // room: after 0.4 s the front sits 0.4 m on, give or take the
        // smearing of the first-order scheme.
        let mut s = row(100, 1.0, 20.0);
        // Air entering is at the advector's ambient, so use a hot ambient and
        // a cold start by lowering the solved values: the model starts at
        // ambient everywhere, so check the transport of a blob instead.
        s.temperature_c.iter_mut().for_each(|t| *t = 20.0);
        let mut a = Advector::new(&s, 20.0).unwrap();
        for i in 10..20 {
            for j in 0..4 {
                for k in 0..4 {
                    let n = a.nx * (k * a.ny + j) + i;
                    a.t[n] = 40.0;
                }
            }
        }
        let centroid = |a: &Advector| {
            let l = line(a);
            let (mut w, mut s) = (0.0, 0.0);
            for (i, v) in l.iter().enumerate() {
                let rise = f64::from(*v - 20.0).max(0.0);
                w += rise;
                s += rise * (i as f64 + 0.5) * 0.010;
            }
            s / w
        };
        let c0 = centroid(&a);
        let done = a.advance(0.30, 100_000);
        let c1 = centroid(&a);
        assert!((done - 0.30).abs() < 1e-9);
        let travelled = c1 - c0;
        assert!(
            (travelled - 0.30).abs() < 0.02,
            "moved {travelled} m in {done} s at 1 m/s"
        );
    }

    #[test]
    fn heat_in_a_closed_box_with_no_flow_only_spreads() {
        // No flow, no diffusion to speak of, a blob stays where it is and the
        // total heat is conserved (closed ends).
        let s = row(40, 0.0, 20.0);
        let mut a = Advector::new(&s, 20.0).unwrap();
        for i in 15..25 {
            let n = i;
            a.t[n] = 50.0;
        }
        let total = |a: &Advector| a.t.iter().map(|v| f64::from(*v)).sum::<f64>();
        let before = total(&a);
        a.advance(60.0, 1_000_000);
        let after = total(&a);
        assert!(
            (after - before).abs() / before < 1e-4,
            "{before} -> {after}"
        );
    }

    #[test]
    fn air_warms_while_it_passes_a_hot_wall_and_settles() {
        // A hot wall at 60 C feeds a 0.2 m/s stream: the air downstream of it
        // is warmer than the room and the picture stops changing.
        let s = heater(30, 0.2);
        let mut a = Advector::new(&s, 20.0).unwrap();
        let start = a.mean_air_c();
        a.advance(20.0, 1_000_000);
        let mid = a.mean_air_c();
        assert!(mid > start + 0.1, "no warming: {start} -> {mid}");
        a.advance(200.0, 10_000_000);
        let late = a.drift_k_per_s(1.0);
        assert!(late < 1e-3, "still drifting at {late} K/s");
        // Warmest next to the wall, never above the wall, never below the room.
        let l = line(&a);
        assert!(l[1] > l[15], "{l:?}");
        assert!(l.iter().all(|v| *v <= 60.0 + 1e-3 && *v >= 20.0 - 1e-3));
    }

    #[test]
    fn restart_goes_back_to_the_cold_air() {
        let s = heater(20, 0.2);
        let mut a = Advector::new(&s, 20.0).unwrap();
        a.advance(10.0, 1_000_000);
        assert!(a.mean_air_c() > 20.0);
        a.restart();
        assert_eq!(a.time_s(), 0.0);
        assert!((a.mean_air_c() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn the_step_is_stable_for_a_fast_stream() {
        // 5 m/s through 10 mm cells would blow up a step of 0.1 s.
        let s = heater(30, 5.0);
        let mut a = Advector::new(&s, 20.0).unwrap();
        assert!(a.step_limit_s() < 0.005);
        a.advance(5.0, 10_000_000);
        assert!(
            a.t.iter()
                .all(|v| v.is_finite() && *v >= 19.9 && *v <= 60.1)
        );
    }

    #[test]
    fn a_settled_cloud_reports_how_far_it_is_from_the_solved_one() {
        // The solved field of the heater case is the cold ambient everywhere:
        // the model then settles warmer, and says by how much.
        let s = heater(30, 0.2);
        let mut a = Advector::new(&s, 20.0).unwrap();
        a.advance(200.0, 10_000_000);
        assert!(a.match_rmse_k() > 0.5);
        // Where the solved field is the settled field the match is perfect.
        let mut s2 = heater(30, 0.2);
        let mut b = Advector::new(&s2, 20.0).unwrap();
        b.advance(300.0, 10_000_000);
        s2.temperature_c = b.temperature_c().to_vec();
        let c = Advector::new(&s2, 20.0).unwrap();
        let mut c = c;
        c.t = s2.temperature_c.clone();
        assert!(c.match_rmse_k() < 1e-6);
    }

    #[test]
    fn an_inconsistent_snapshot_gives_no_model() {
        let mut s = row(10, 0.1, 20.0);
        s.temperature_c.pop();
        assert!(Advector::new(&s, 20.0).is_none());
    }
}
