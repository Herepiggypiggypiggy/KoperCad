//! Lid-driven cavity against Ghia, Ghia and Shin (1982), J. Comput. Phys. 48.
//!
//! Prints the error numbers; run with `-- --nocapture` to see them.

use novo_cfd::flow::*;

/// (y from the bottom, u) on the vertical centreline x = 0.5.
const GHIA_U_100: [(f64, f64); 15] = [
    (0.0547, -0.03717),
    (0.0625, -0.04192),
    (0.0703, -0.04775),
    (0.1016, -0.06434),
    (0.1719, -0.10150),
    (0.2813, -0.15662),
    (0.4531, -0.21090),
    (0.5000, -0.20581),
    (0.6172, -0.13641),
    (0.7344, 0.00332),
    (0.8516, 0.23151),
    (0.9531, 0.68717),
    (0.9609, 0.73722),
    (0.9688, 0.78871),
    (0.9766, 0.84123),
];
/// (x, v) on the horizontal centreline y = 0.5 (v positive up).
const GHIA_V_100: [(f64, f64); 15] = [
    (0.0625, 0.09233),
    (0.0703, 0.10091),
    (0.0781, 0.10890),
    (0.0938, 0.12317),
    (0.1563, 0.16077),
    (0.2266, 0.17507),
    (0.2344, 0.17527),
    (0.5000, 0.05454),
    (0.8047, -0.24533),
    (0.8594, -0.22445),
    (0.9063, -0.16914),
    (0.9453, -0.10313),
    (0.9531, -0.08864),
    (0.9609, -0.07391),
    (0.9688, -0.05906),
];
const GHIA_U_1000: [(f64, f64); 15] = [
    (0.0547, -0.18109),
    (0.0625, -0.20196),
    (0.0703, -0.22220),
    (0.1016, -0.29730),
    (0.1719, -0.38289),
    (0.2813, -0.27805),
    (0.4531, -0.10648),
    (0.5000, -0.06080),
    (0.6172, 0.05702),
    (0.7344, 0.18719),
    (0.8516, 0.33304),
    (0.9531, 0.46604),
    (0.9609, 0.51117),
    (0.9688, 0.57492),
    (0.9766, 0.65928),
];
const GHIA_V_1000: [(f64, f64); 15] = [
    (0.0625, 0.27485),
    (0.0703, 0.29012),
    (0.0781, 0.30353),
    (0.0938, 0.32627),
    (0.1563, 0.37095),
    (0.2266, 0.33075),
    (0.2344, 0.32235),
    (0.5000, 0.02526),
    (0.8047, -0.31966),
    (0.8594, -0.42665),
    (0.9063, -0.51550),
    (0.9453, -0.39188),
    (0.9531, -0.33714),
    (0.9609, -0.27669),
    (0.9688, -0.21388),
];

fn stretched(n: usize, s: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| 0.5 * (1.0 + (s * (2.0 * i as f64 / n as f64 - 1.0)).tanh() / s.tanh()))
        .collect()
}

/// Linear interpolation of samples (positions ascending) with wall values.
fn interp(pos: &[f64], val: &[f64], x: f64) -> f64 {
    let i = pos.partition_point(|p| *p < x).clamp(1, pos.len() - 1);
    let t = (x - pos[i - 1]) / (pos[i] - pos[i - 1]);
    val[i - 1] * (1.0 - t) + val[i] * t
}

struct Errors {
    rms: f64,
    max: f64,
}

fn run(re: f64, n: usize, stretch: f64) -> (Errors, Errors, usize, f64) {
    // 2D case: one cell deep, symmetry on both z faces.
    let mesh = Mesh::from_edges_m([stretched(n, stretch), stretched(n, stretch), vec![0.0, 0.1]]);
    let mut cfg = FlowConfig::air();
    cfg.nu = 1.0 / re;
    cfg.gravity = [0.0; 3];
    cfg.faces[Face::YMin as usize].wall_velocity = [1.0, 0.0, 0.0]; // lid, top in y-down
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    let mut s = FlowSolver::new(mesh, cfg, 0.0);
    let mut steps = 0;
    let mut accel = f64::MAX;
    while steps < 20_000 && (accel > 2e-5 || steps < 200) {
        let dt = s.max_dt(0.4).min(0.02);
        accel = s.step(dt).max_accel;
        steps += 1;
    }
    let m = &s.mesh;
    // u along x = 0.5: x-edge n/2, sampled at the y centres (y down: flip).
    let ci = n / 2;
    let mut ys = vec![0.0];
    let mut us = vec![0.0];
    for j in (0..n).rev() {
        ys.push(1.0 - m.centre[1][j]);
        us.push(s.vel[0][m.idx(0, [ci, j, 0])]);
    }
    ys.push(1.0);
    us.push(1.0);
    let mut xs = vec![0.0];
    let mut vs = vec![0.0];
    for i in 0..n {
        xs.push(m.centre[0][i]);
        vs.push(-s.vel[1][m.idx(1, [i, ci, 0])]); // up positive
    }
    xs.push(1.0);
    vs.push(0.0);
    let err = |table: &[(f64, f64)], pos: &[f64], val: &[f64]| {
        let d: Vec<f64> = table
            .iter()
            .map(|(x, r)| interp(pos, val, *x) - r)
            .collect();
        Errors {
            rms: (d.iter().map(|e| e * e).sum::<f64>() / d.len() as f64).sqrt(),
            max: d.iter().fold(0.0, |a: f64, e| a.max(e.abs())),
        }
    };
    let (tu, tv) = if re < 500.0 {
        (&GHIA_U_100, &GHIA_V_100)
    } else {
        (&GHIA_U_1000, &GHIA_V_1000)
    };
    (
        err(tu, &ys, &us),
        err(tv, &xs, &vs),
        steps,
        s.divergence_max(),
    )
}

#[test]
fn lid_cavity_re100_matches_ghia() {
    let (eu, ev, steps, div) = run(100.0, 48, 1.0);
    println!(
        "Re 100, 48x48: u rms {:.4} max {:.4}; v rms {:.4} max {:.4} (lid speed 1); {steps} steps, div {div:.1e}",
        eu.rms, eu.max, ev.rms, ev.max
    );
    assert!(div < 1e-8);
    assert!(eu.max < 0.02 && ev.max < 0.02, "u {} v {}", eu.max, ev.max);
}

#[test]
fn lid_cavity_re1000_matches_ghia() {
    let (eu, ev, steps, div) = run(1000.0, 48, 1.3);
    println!(
        "Re 1000, 48x48 stretched: u rms {:.4} max {:.4}; v rms {:.4} max {:.4}; {steps} steps, div {div:.1e}",
        eu.rms, eu.max, ev.rms, ev.max
    );
    assert!(div < 1e-8);
    assert!(eu.max < 0.04 && ev.max < 0.04, "u {} v {}", eu.max, ev.max);
}
