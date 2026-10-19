//! Heat flow in a solved cabinet: the direction and strength of heat transfer
//! at every cell, derived from the temperature and velocity fields.
//!
//! In the air the heat is carried by the moving air, `q = rho * cp * (T - T0)
//! * u` (W/m2, `T0` the ambient). In a solid it is conducted, `q = -k grad T`,
//! using the neighbours of the same material only, so a device face does not
//! read as a huge gradient against the air. Conduction through the air itself
//! (k about 0.026 W/mK) is far below the carried part and left out. Surface
//! radiation is not in the temperature and velocity fields and is not shown.
//! This is derived from the result, not a second solve: its accuracy is that of
//! the solved fields. Frame as [`crate::cfd`]: `y` down.

use novo_cfd::CfdSnapshot;

use crate::cfd::{Trace, taps, trace_field};

/// Air density, kg/m3, and specific heat, J/kgK, at room temperature.
pub const AIR_DENSITY: f64 = 1.16;
pub const AIR_CP: f64 = 1007.0;

/// Heat flux density per cell, W/m2, `y` down. Empty for an inconsistent
/// snapshot.
pub fn flux_field(snap: &CfdSnapshot, ambient_c: f64) -> Vec<[f32; 3]> {
    if !snap.is_consistent() {
        return Vec::new();
    }
    let g = &snap.grid;
    let k_of = |id: u16| {
        snap.materials
            .iter()
            .find(|m| m.id == id)
            .map_or(0.0, |m| m.conductivity_w_mk)
    };
    let mut out = vec![[0.0f32; 3]; g.cells()];
    for k in 0..g.nz {
        for j in 0..g.ny {
            for i in 0..g.nx {
                let n = g.index(i, j, k);
                let id = snap.material[n];
                if id == 0 {
                    let scale =
                        AIR_DENSITY * AIR_CP * (f64::from(snap.temperature_c[n]) - ambient_c);
                    out[n] = snap.velocity[n].map(|v| (scale * f64::from(v)) as f32);
                    continue;
                }
                let kc = k_of(id);
                let c = g.centre_mm(i, j, k);
                let at = |di: i64, dj: i64, dk: i64| -> Option<(usize, [f64; 3])> {
                    let (a, b, d) = (i as i64 + di, j as i64 + dj, k as i64 + dk);
                    if a < 0 || b < 0 || d < 0 {
                        return None;
                    }
                    let (a, b, d) = (a as usize, b as usize, d as usize);
                    if a >= g.nx || b >= g.ny || d >= g.nz {
                        return None;
                    }
                    let m = g.index(a, b, d);
                    (snap.material[m] == id).then(|| (m, g.centre_mm(a, b, d)))
                };
                let mut q = [0.0f64; 3];
                for (axis, dir) in [[1, 0, 0], [0, 1, 0], [0, 0, 1]].into_iter().enumerate() {
                    let lo = at(-dir[0], -dir[1], -dir[2]);
                    let hi = at(dir[0], dir[1], dir[2]);
                    // Central where both sides are the same solid, else one-sided.
                    let (t0, x0, t1, x1) = match (lo, hi) {
                        (Some((a, ca)), Some((b, cb))) => (
                            snap.temperature_c[a],
                            ca[axis],
                            snap.temperature_c[b],
                            cb[axis],
                        ),
                        (Some((a, ca)), None) => (
                            snap.temperature_c[a],
                            ca[axis],
                            snap.temperature_c[n],
                            c[axis],
                        ),
                        (None, Some((b, cb))) => (
                            snap.temperature_c[n],
                            c[axis],
                            snap.temperature_c[b],
                            cb[axis],
                        ),
                        (None, None) => continue,
                    };
                    // K per mm to K per m.
                    let grad = f64::from(t1 - t0) / ((x1 - x0) * 1e-3);
                    q[axis] = -kc * grad;
                }
                out[n] = q.map(|v| v as f32);
            }
        }
    }
    out
}

/// Length of a flux vector, W/m2.
pub fn magnitude(q: [f32; 3]) -> f32 {
    (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt()
}

/// Trilinear flux at a point, W/m2.
pub fn flux_at(snap: &CfdSnapshot, field: &[[f32; 3]], p: [f64; 3]) -> [f32; 3] {
    let t = taps(snap, p);
    let mut q = [0.0f32; 3];
    for n in 0..8 {
        for (c, out) in q.iter_mut().enumerate() {
            *out += field[t.idx[n]][c] * t.w[n];
        }
    }
    q
}

/// Strongest flux in the air, W/m2.
pub fn top_air_flux(snap: &CfdSnapshot, field: &[[f32; 3]]) -> f32 {
    field
        .iter()
        .zip(&snap.material)
        .filter(|(_, m)| **m == 0)
        .map(|(q, _)| magnitude(*q))
        .fold(0.0, f32::max)
}

/// Seeds for heat-flow lines: the strongest air cell in each of
/// `blocks[0] x blocks[1] x blocks[2]` blocks of the grid, so lines are dense
/// where much heat moves. Blocks whose best cell carries less than
/// `floor_share` of the strongest flux get none.
pub fn seeds(
    snap: &CfdSnapshot,
    field: &[[f32; 3]],
    blocks: [usize; 3],
    floor_share: f32,
) -> Vec<[f64; 3]> {
    let top = top_air_flux(snap, field);
    if top <= 0.0 || field.len() != snap.grid.cells() {
        return Vec::new();
    }
    let g = &snap.grid;
    let span = |n: usize, b: usize, i: usize| (n * i / b, (n * (i + 1) / b).max(n * i / b + 1));
    let mut out = Vec::new();
    for bz in 0..blocks[2] {
        for by in 0..blocks[1] {
            for bx in 0..blocks[0] {
                let (i0, i1) = span(g.nx, blocks[0], bx);
                let (j0, j1) = span(g.ny, blocks[1], by);
                let (k0, k1) = span(g.nz, blocks[2], bz);
                let mut best: Option<(f32, [f64; 3])> = None;
                for k in k0..k1.min(g.nz) {
                    for j in j0..j1.min(g.ny) {
                        for i in i0..i1.min(g.nx) {
                            let n = g.index(i, j, k);
                            if snap.material[n] != 0 {
                                continue;
                            }
                            let m = magnitude(field[n]);
                            if best.is_none_or(|(b, _)| m > b) {
                                best = Some((m, g.centre_mm(i, j, k)));
                            }
                        }
                    }
                }
                if let Some((m, p)) = best
                    && m >= floor_share * top
                {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// Heat-flow lines from `seeds`: traced along the flux through the air until
/// it falls under `min_flux` W/m2, a wall or a device is reached.
pub fn lines(
    snap: &CfdSnapshot,
    field: &[[f32; 3]],
    seeds: &[[f64; 3]],
    trace: Trace,
    min_flux: f32,
) -> Vec<Vec<[f64; 3]>> {
    if field.len() != snap.grid.cells() {
        return Vec::new();
    }
    trace_field(snap, seeds, trace, |p| {
        let q = flux_at(snap, field, p);
        let m = magnitude(q);
        (m >= min_flux && m > 0.0).then(|| {
            [
                f64::from(q[0] / m),
                f64::from(q[1] / m),
                f64::from(q[2] / m),
            ]
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_cfd::{
        AccuracyStatement, Fidelity, Grid3, MaterialInfo, MaterialKind, Progress, Residuals,
    };

    /// 10 x 4 x 4 cells of 10 mm. Air at 30 C moving +x at 0.5 m/s, except a
    /// plate (material 1, k = 200) on x < 50 whose temperature falls 1 K per
    /// cell along +x from 60 C.
    fn snap() -> CfdSnapshot {
        let grid = Grid3::uniform([10, 4, 4], [100.0, 40.0, 40.0]);
        let n = grid.cells();
        let (mut t, mut m, mut v) = (Vec::new(), Vec::new(), Vec::new());
        for k in 0..grid.nz {
            for j in 0..grid.ny {
                for i in 0..grid.nx {
                    let _ = (j, k);
                    if i < 5 {
                        t.push(60.0 - i as f32);
                        m.push(1);
                        v.push([0.0; 3]);
                    } else {
                        t.push(30.0);
                        m.push(0);
                        v.push([0.5, 0.0, 0.0]);
                    }
                }
            }
        }
        CfdSnapshot {
            grid,
            temperature_c: t,
            velocity: v,
            pressure_pa: vec![0.0; n],
            turb_visc_ratio: vec![0.0; n],
            material: m,
            materials: vec![
                MaterialInfo {
                    id: 0,
                    kind: MaterialKind::Air,
                    name: "Air".into(),
                    conductivity_w_mk: 0.026,
                },
                MaterialInfo {
                    id: 1,
                    kind: MaterialKind::Plate,
                    name: "Plate".into(),
                    conductivity_w_mk: 200.0,
                },
            ],
            wall_flux_w_m2: vec![0.0; n],
            devices: Vec::new(),
            hot_spots: Vec::new(),
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

    #[test]
    fn air_carries_rho_cp_dt_u_along_its_velocity() {
        let s = snap();
        let f = flux_field(&s, 20.0);
        let q = f[s.grid.index(7, 1, 1)];
        let want = (AIR_DENSITY * AIR_CP * 10.0 * 0.5) as f32;
        assert!((q[0] - want).abs() < 0.01 * want, "{q:?} {want}");
        assert!(q[1].abs() < 1e-6 && q[2].abs() < 1e-6);
    }

    #[test]
    fn air_at_ambient_carries_no_heat() {
        let s = snap();
        let f = flux_field(&s, 30.0);
        assert!(f[s.grid.index(7, 1, 1)].iter().all(|v| v.abs() < 1e-6));
    }

    #[test]
    fn a_solid_conducts_down_its_temperature_gradient() {
        let s = snap();
        let f = flux_field(&s, 20.0);
        // 1 K per 10 mm = 100 K/m; q = k * 100 = 20 kW/m2, towards +x.
        let q = f[s.grid.index(2, 1, 1)];
        assert!((q[0] - 20_000.0).abs() < 1.0, "{q:?}");
        // The face next to the air uses its solid neighbour only (one-sided).
        let edge = f[s.grid.index(4, 1, 1)];
        assert!((edge[0] - 20_000.0).abs() < 1.0, "{edge:?}");
    }

    #[test]
    fn the_strongest_air_flux_seeds_lines_and_weak_blocks_get_none() {
        let s = snap();
        let f = flux_field(&s, 20.0);
        let seeds = seeds(&s, &f, [2, 2, 2], 0.5);
        assert!(!seeds.is_empty());
        assert!(seeds.iter().all(|p| p[0] > 50.0), "seeds are in the air");
        // Nothing moves when the air is at ambient.
        let calm = flux_field(&s, 30.0);
        assert!(super::seeds(&s, &calm, [2, 2, 2], 0.1).is_empty());
    }

    #[test]
    fn lines_follow_the_flux_downwind_and_stop_at_the_wall() {
        let s = snap();
        let f = flux_field(&s, 20.0);
        let l = lines(&s, &f, &[[55.0, 20.0, 20.0]], Trace::default(), 1.0);
        assert_eq!(l.len(), 1);
        let line = &l[0];
        assert!(line.len() > 2);
        assert!(line.windows(2).all(|w| w[1][0] > w[0][0]));
        assert!(line.last().unwrap()[0] <= 100.0);
        // Below the floor nothing is followed.
        let none = lines(&s, &f, &[[55.0, 20.0, 20.0]], Trace::default(), 1e9);
        assert_eq!(none[0].len(), 1);
    }
}
