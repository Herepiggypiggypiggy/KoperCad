//! Helpers shared by the validation tests (lane CFD-G).
//!
//! Each validation test prints one `VALIDATION | case | quantity | value |
//! reference | error` line so `docs/thermal-validation.md` can be refreshed
//! from `cargo test -p novo-cfd --test 'validation_*' --release -- --nocapture`.
#![allow(dead_code)]

use novo_cfd::flow::*;

/// Prints one row of the error table.
pub fn report(case: &str, quantity: &str, value: f64, reference: f64) -> f64 {
    let err = if reference.abs() > 1e-300 {
        (value - reference) / reference
    } else {
        value - reference
    };
    if reference.abs() > 1e-300 {
        println!(
            "VALIDATION | {case} | {quantity} | {value:.6} | {reference:.6} | {:+.4}%",
            100.0 * err
        );
    } else {
        println!("VALIDATION | {case} | {quantity} | {value:.3e} | 0 | abs {err:.3e}");
    }
    err
}

/// Nested tanh stretching of `[0, 1]`: edges of `n` cells are every second
/// edge of the `2n` version, so refinement studies stay geometrically similar.
pub fn stretched(n: usize, s: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| 0.5 * (1.0 + (s * (2.0 * i as f64 / n as f64 - 1.0)).tanh() / s.tanh()))
        .collect()
}

pub fn uniform(n: usize, len: f64) -> Vec<f64> {
    (0..=n).map(|i| len * i as f64 / n as f64).collect()
}

/// Config with no flow: gravity off, so only conduction acts.
pub fn conduction_cfg(alpha: f64) -> FlowConfig {
    let mut cfg = FlowConfig::air();
    cfg.alpha = alpha;
    cfg.beta = 0.0;
    cfg.gravity = [0.0; 3];
    cfg
}

/// Thin slab: `n` cells along x, one in y and z, adiabatic sides.
pub fn slab_mesh(n: usize, len_m: f64) -> Mesh {
    Mesh::from_edges_m([uniform(n, len_m), vec![0.0, 0.01], vec![0.0, 0.01]])
}

/// Differentially heated square cavity of the de Vahl Davis set-up in
/// dimensionless units (side 1, dT 1, Pr 0.71). `hot_axis` is the axis along
/// which the temperature difference lies (gravity points along the other one
/// of x/y), `depth` the number of cells across the symmetric third axis.
pub struct Cavity {
    pub solver: FlowSolver,
    pub n: usize,
    pub steps: usize,
}

pub fn heated_cavity(ra: f64, n: usize, stretch: f64, swap_xy: bool, depth: usize) -> Cavity {
    let pr = 0.71;
    let e = stretched(n, stretch);
    let z = uniform(depth, 0.1);
    let mesh = Mesh::from_edges_m([e.clone(), e, z]);
    let mut cfg = FlowConfig::air();
    cfg.nu = (pr / ra).sqrt();
    cfg.alpha = 1.0 / (ra * pr).sqrt();
    cfg.beta = 1.0;
    // x-hot/cold with gravity along y, or the 90 degree rotated problem:
    // y-hot/cold with gravity along x.
    let (hot, cold, g) = if swap_xy {
        (Face::YMin, Face::YMax, [1.0, 0.0, 0.0])
    } else {
        (Face::XMin, Face::XMax, [0.0, 1.0, 0.0])
    };
    cfg.gravity = g;
    cfg.faces[hot as usize].thermal = ThermalBc::Fixed(0.5);
    cfg.faces[cold as usize].thermal = ThermalBc::Fixed(-0.5);
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    let mut s = FlowSolver::new(mesh, cfg, 0.0);
    let scale = (ra * pr).sqrt();
    let mut steps = 0;
    let mut settled = 0;
    while steps < 40_000 && settled < 20 {
        let dt = s.max_dt(0.4).min(2.0);
        let info = s.step(dt);
        steps += 1;
        if steps > 50 && info.max_accel * scale < 1e-6 && info.max_dtdt < 1e-8 {
            settled += 1;
        } else {
            settled = 0;
        }
    }
    Cavity {
        solver: s,
        n,
        steps,
    }
}

impl Cavity {
    /// Mean Nusselt number on the hot wall by a quadratic wall-gradient
    /// extrapolation from the first two cells (the same estimator as the
    /// de Vahl Davis test, so the numbers are comparable). Returns
    /// `(hot, cold)`.
    pub fn nusselt(&self, swap_xy: bool) -> (f64, f64) {
        let s = &self.solver;
        let m = &s.mesh;
        let n = self.n;
        // axis of the temperature difference
        let a = usize::from(swap_xy);
        let flux = |side: usize| -> f64 {
            let (c0, c1, wall) = if side == 0 {
                (0, 1, 0.0)
            } else {
                (n - 1, n - 2, 1.0)
            };
            let tw = if side == 0 { 0.5 } else { -0.5 };
            let (h0, h1) = (
                (m.centre[a][c0] - wall).abs(),
                (m.centre[a][c1] - wall).abs(),
            );
            let mut q = 0.0;
            for t in 0..n {
                let (ix0, ix1) = if swap_xy {
                    ([t, c0, 0], [t, c1, 0])
                } else {
                    ([c0, t, 0], [c1, t, 0])
                };
                let (d0, d1) = (s.t[m.cell(ix0)] - tw, s.t[m.cell(ix1)] - tw);
                let g = (d0 * h1 * h1 - d1 * h0 * h0) / (h0 * h1 * (h1 - h0));
                q += g.abs() * m.dx[1 - a][t];
            }
            q
        };
        (flux(0), flux(1))
    }
}
