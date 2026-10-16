//! The steady path on the heated vertical plate in open surroundings (the
//! open-face law) against Churchill and Chu
//! (1975) for the laminar range:
//!
//! `Nu_H = 0.68 + 0.670 Ra^(1/4) / (1 + (0.492/Pr)^(9/16))^(4/9)`
//!
//! The plate is the left wall of a box open to still air on the other three
//! sides (free openings, ambient at the Boussinesq reference). Prints the
//! error numbers; run with `-- --nocapture`.

use novo_cfd::flow::*;
use novo_cfd::openings::{OpenKind, OpenPatch};

fn graded(n: usize, s: f64, both_sides: bool) -> Vec<f64> {
    (0..=n)
        .map(|i| {
            let f = i as f64 / n as f64;
            if both_sides {
                0.5 * (1.0 + (s * (2.0 * f - 1.0)).tanh() / s.tanh())
            } else {
                1.0 - (s * (1.0 - f)).tanh() / s.tanh()
            }
        })
        .collect()
}

fn churchill_chu(ra: f64, pr: f64) -> f64 {
    0.68 + 0.670 * ra.powf(0.25) / (1.0 + (0.492 / pr).powf(9.0 / 16.0)).powf(4.0 / 9.0)
}

fn run(ra: f64, nx: usize, ny: usize, width: f64) -> (f64, usize) {
    let pr = 0.71;
    let x: Vec<f64> = graded(nx, 2.0, false).iter().map(|v| v * width).collect();
    let mesh = Mesh::from_edges_m([x, graded(ny, 1.6, true), vec![0.0, 0.1]]);
    let mut cfg = FlowConfig::air();
    cfg.nu = (pr / ra).sqrt();
    cfg.alpha = 1.0 / (ra * pr).sqrt();
    cfg.conductivity = 1.0; // only the ratio to alpha matters here
    cfg.beta = 1.0;
    cfg.gravity = [0.0, 1.0, 0.0];
    cfg.t_ref = Some(0.0);
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(1.0);
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    let patch = |face, hi: [f64; 2]| OpenPatch {
        face,
        lo: [0.0, 0.0],
        hi,
        kind: OpenKind::Free,
        inflow_c: None,
    };
    // lo/hi are in the two other axes: x faces use (y, z), y faces use (x, z)
    cfg.openings = vec![
        patch(Face::XMax, [1.0, 0.1]),
        patch(Face::YMin, [width, 0.1]),
        patch(Face::YMax, [width, 0.1]),
    ];
    let mut s = FlowSolver::new(mesh, cfg, 0.0);
    let nusselt = |s: &FlowSolver| {
        let m = &s.mesh;
        let (h0, h1) = (m.centre[0][0], m.centre[0][1]);
        let mut q = 0.0;
        for j in 0..ny {
            let (d0, d1) = (s.t[m.cell([0, j, 0])] - 1.0, s.t[m.cell([1, j, 0])] - 1.0);
            let a = (d0 * h1 * h1 - d1 * h0 * h0) / (h0 * h1 * (h1 - h0));
            q += -a * m.dx[1][j];
        }
        q
    };
    let mut run = SteadyRun::staged(
        &s,
        vec![SteadyConfig::first_order(), SteadyConfig::second_order()],
        SteadyLimits::default(),
        0.0,
        Vec::new(),
    );
    let mut iterations = 0;
    while run.advance(&mut s, 50) == RunStatus::Running {
        iterations = run.report().iterations;
    }
    assert!(run.report().converged, "{:?}", run.report());
    (nusselt(&s), iterations)
}

fn check(ra: f64, nx: usize, ny: usize, width: f64, tol: f64) {
    let reference = churchill_chu(ra, 0.71);
    let (nu, steps) = run(ra, nx, ny, width);
    let err = (nu - reference) / reference;
    println!(
        "PLATE | Ra {ra:.0e}, {nx}x{ny}, width {width}: Nu {nu:.2}, Churchill-Chu {reference:.2}, error {:+.1}% ({steps} iterations)",
        100.0 * err
    );
    assert!(err.abs() < tol, "error {err}");
}

#[test]
fn steady_plate_ra_1e5() {
    check(1e5, 40, 40, 1.5, 0.04);
}

#[test]
fn steady_plate_ra_1e6() {
    check(1e6, 32, 32, 1.5, 0.04);
}
