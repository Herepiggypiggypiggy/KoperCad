//! Free convection from a heated vertical plate in open air against
//! Churchill and Chu (1975), laminar branch (Ra_H < 1e9):
//! `Nu = 0.68 + 0.670 Ra^(1/4) / [1 + (0.492/Pr)^(9/16)]^(4/9)`.
//! The correlation itself is good to about 5% against experiments, so a
//! result inside 6% on the finer mesh is a pass and the signed error is reported.
//!
//! Domain: a 2D slice (symmetry in depth), plate on the x min wall at
//! 20 K above ambient, open at the bottom, top and far side.
//! Run `-- --nocapture` to see the `VALIDATION` rows.

mod validation;
use novo_cfd::flow::*;
use novo_cfd::openings::*;
use validation::report;

const T_AMB: f64 = 25.0;

fn churchill_chu(ra: f64, pr: f64) -> f64 {
    0.68 + 0.670 * ra.powf(0.25) / (1.0 + (0.492 / pr).powf(9.0 / 16.0)).powf(4.0 / 9.0)
}

/// Mean Nusselt number of a plate of height `h` (m) and temperature excess
/// `dt` (K), on `nx` x `ny` cells. Returns (Nu, Ra, Pr, steps).
fn plate(h: f64, dt: f64, nx: usize, ny: usize) -> (f64, f64, f64, usize) {
    let width = 0.5 * h * 2.0; // far side one plate height away
    let stretch = |n: usize, s: f64, len: f64| -> Vec<f64> {
        (0..=n)
            .map(|i| len * (1.0 - (s * (1.0 - i as f64 / n as f64)).tanh() / s.tanh()))
            .collect()
    };
    // y runs downwards: the plate occupies the whole x min wall
    let ex = stretch(nx, 2.2, width);
    let ey = (0..=ny).map(|i| h * i as f64 / ny as f64).collect();
    let mesh = Mesh::from_edges_m([ex, ey, vec![0.0, 0.02]]);
    let mut cfg = FlowConfig::air();
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(T_AMB + dt);
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    let free = |face, lo, hi| OpenPatch {
        face,
        lo,
        hi,
        kind: OpenKind::Free,
        inflow_c: None,
    };
    cfg.openings = vec![
        free(Face::YMin, [0.0, 0.0], [width, 0.02]),
        free(Face::YMax, [0.0, 0.0], [width, 0.02]),
        free(Face::XMax, [0.0, 0.0], [h, 0.02]),
    ];
    cfg.t_ref = Some(T_AMB);
    let (nu, alpha, beta, g) = (cfg.nu, cfg.alpha, cfg.beta, 9.81);
    let pr = nu / alpha;
    let ra = g * beta * dt * h.powi(3) / (nu * alpha);
    let mut s = FlowSolver::new(mesh, cfg, T_AMB);
    let mut steps = 0;
    let mut settled = 0;
    while steps < 20_000 && settled < 20 {
        let dt_s = s.max_dt(0.4).min(0.05);
        let info = s.step(dt_s);
        steps += 1;
        if steps > 100 && info.max_accel < 1e-5 && info.max_dtdt < 1e-5 {
            settled += 1;
        } else {
            settled = 0;
        }
    }
    // mean wall flux: quadratic extrapolation of T from the first two cells
    let m = &s.mesh;
    let (h0, h1) = (m.centre[0][0], m.centre[0][1]);
    let tw = T_AMB + dt;
    let mut q = 0.0;
    for j in 0..ny {
        let (d0, d1) = (s.t[m.cell([0, j, 0])] - tw, s.t[m.cell([1, j, 0])] - tw);
        let grad = (d0 * h1 * h1 - d1 * h0 * h0) / (h0 * h1 * (h1 - h0));
        q += -grad * m.dx[1][j];
    }
    let nu_mean = (q / h) * h / dt; // (mean flux) * H / (alpha dt), alpha cancels in K m
    (nu_mean, ra, pr, steps)
}

#[test]
fn heated_plate_matches_churchill_chu() {
    let mut errs = Vec::new();
    for (nx, ny) in [(24, 40), (36, 60)] {
        let (nu, ra, pr, steps) = plate(0.3, 20.0, nx, ny);
        let reference = churchill_chu(ra, pr);
        println!("  Ra_H {ra:.3e}, Pr {pr:.3}, {nx}x{ny}, {steps} steps");
        let e = report(
            &format!("vertical plate {nx}x{ny}"),
            "mean Nu vs Churchill-Chu",
            nu,
            reference,
        );
        errs.push(e.abs());
    }
    // the error falls with the mesh and ends inside the correlation's own scatter
    assert!(errs[1] < errs[0], "error does not fall: {errs:?}");
    assert!(errs[1] < 0.06, "error {}", errs[1]);
}
