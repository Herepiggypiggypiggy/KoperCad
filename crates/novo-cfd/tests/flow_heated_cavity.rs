//! Differentially heated square cavity against de Vahl Davis (1983),
//! Int. J. Numer. Methods Fluids 3, Pr = 0.71.
//!
//! Prints the error numbers; run with `-- --nocapture` to see them.

use novo_cfd::flow::*;

/// Ra, mean Nusselt number, u_max on x = 0.5, v_max on y = 0.5 (both in
/// units of alpha / H).
const DE_VAHL_DAVIS: [(f64, f64, f64, f64); 4] = [
    (1e3, 1.118, 3.649, 3.697),
    (1e4, 2.243, 16.178, 19.617),
    (1e5, 4.519, 34.73, 68.59),
    (1e6, 8.800, 64.63, 219.36),
];

fn stretched(n: usize, s: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| 0.5 * (1.0 + (s * (2.0 * i as f64 / n as f64 - 1.0)).tanh() / s.tanh()))
        .collect()
}

/// Peak of samples refined by a parabola through the three highest points.
fn peak(values: &[f64]) -> f64 {
    let (i, &m) = values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();
    if i == 0 || i + 1 == values.len() {
        return m;
    }
    let (a, b, c) = (values[i - 1], values[i], values[i + 1]);
    let den = a - 2.0 * b + c;
    if den.abs() < 1e-30 {
        b
    } else {
        b - 0.125 * (c - a).powi(2) / den
    }
}

struct Result {
    nu: f64,
    u_max: f64,
    v_max: f64,
    steps: usize,
    energy_rel: f64,
}

fn run(ra: f64, n: usize, stretch: f64) -> Result {
    let pr = 0.71;
    let mesh = Mesh::from_edges_m([stretched(n, stretch), stretched(n, stretch), vec![0.0, 0.1]]);
    let mut cfg = FlowConfig::air();
    cfg.nu = (pr / ra).sqrt();
    cfg.alpha = 1.0 / (ra * pr).sqrt();
    cfg.beta = 1.0;
    cfg.gravity = [0.0, 1.0, 0.0];
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(0.5);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(-0.5);
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    let mut s = FlowSolver::new(mesh, cfg, 0.0);
    let scale = (ra * pr).sqrt(); // velocity in alpha / H
    let mut steps = 0;
    let mut settled = 0;
    while steps < 40_000 && settled < 20 {
        let dt = s.max_dt(0.4).min(2.0);
        let info = s.step(dt);
        steps += 1;
        // dimensionless drift: velocity change per thermal time
        if steps > 50 && info.max_accel * scale < 1e-6 && info.max_dtdt < 1e-8 {
            settled += 1;
        } else {
            settled = 0;
        }
    }
    let m = &s.mesh;
    let wall_flux = |x_side: usize| -> f64 {
        // Nusselt = mean over the wall of -dT/dx / dT, quadratic extrapolation
        let (i0, i1, sign, wall_x) = if x_side == 0 {
            (0, 1, 1.0, 0.0)
        } else {
            (n - 1, n - 2, -1.0, 1.0)
        };
        let tw = if x_side == 0 { 0.5 } else { -0.5 };
        let (h0, h1) = (
            (m.centre[0][i0] - wall_x).abs(),
            (m.centre[0][i1] - wall_x).abs(),
        );
        let mut q = 0.0;
        for j in 0..n {
            let (d0, d1) = (s.t[m.cell([i0, j, 0])] - tw, s.t[m.cell([i1, j, 0])] - tw);
            let a = (d0 * h1 * h1 - d1 * h0 * h0) / (h0 * h1 * (h1 - h0));
            q += -sign * a * m.dx[1][j]; // heat flux into the fluid for the hot wall
        }
        q.abs()
    };
    let (nu_hot, nu_cold) = (wall_flux(0), wall_flux(1));
    let ci = n / 2;
    let u_line: Vec<f64> = (0..n).map(|j| s.vel[0][m.idx(0, [ci, j, 0])]).collect();
    let v_line: Vec<f64> = (0..n).map(|i| -s.vel[1][m.idx(1, [i, ci, 0])]).collect();
    Result {
        nu: 0.5 * (nu_hot + nu_cold),
        u_max: peak(&u_line) * scale,
        v_max: peak(&v_line) * scale,
        steps,
        energy_rel: (nu_hot - nu_cold).abs() / nu_hot,
    }
}

fn check(ra: f64, n: usize, stretch: f64, tol_nu: f64, tol_vel: f64) {
    let (_, nu_ref, u_ref, v_ref) = *DE_VAHL_DAVIS.iter().find(|r| r.0 == ra).unwrap();
    let r = run(ra, n, stretch);
    let (e_nu, e_u, e_v) = (
        (r.nu - nu_ref) / nu_ref,
        (r.u_max - u_ref) / u_ref,
        (r.v_max - v_ref) / v_ref,
    );
    println!(
        "Ra {ra:.0e}, {n}x{n}: Nu {:.3} (ref {nu_ref}, {:+.2}%), u_max {:.2} ({:+.2}%), v_max {:.2} ({:+.2}%), hot/cold flux mismatch {:.1e}, {} steps",
        r.nu,
        100.0 * e_nu,
        r.u_max,
        100.0 * e_u,
        r.v_max,
        100.0 * e_v,
        r.energy_rel,
        r.steps
    );
    assert!(r.energy_rel < 0.01, "heat in and out differ");
    assert!(e_nu.abs() < tol_nu, "Nu error {e_nu}");
    assert!(
        e_u.abs() < tol_vel && e_v.abs() < tol_vel,
        "velocity errors {e_u} {e_v}"
    );
}

#[test]
fn ra_1e3() {
    // v_max converges slowly here: 4.5% at 32 stretched, 3.0% at 48, 1.7% at 96.
    check(1e3, 48, 1.0, 0.01, 0.04);
}

#[test]
fn ra_1e4() {
    check(1e4, 32, 1.3, 0.01, 0.03);
}

#[test]
fn ra_1e5() {
    check(1e5, 40, 1.5, 0.02, 0.05);
}

#[test]
fn ra_1e6() {
    check(1e6, 56, 1.8, 0.03, 0.06);
}
