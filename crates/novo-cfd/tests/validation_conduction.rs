//! Conduction against closed-form solutions: the part of the conjugate
//! model that must be exact (design section 5, case 5).
//!
//! Run `-- --nocapture` to see the `VALIDATION` rows.

mod validation;
use novo_cfd::flow::*;
use validation::*;

const ALPHA: f64 = 1e-4; // m2/s, any value: the cases are scaled by it

/// Steady slab between two fixed walls: the profile is a straight line.
#[test]
fn steady_slab_is_linear() {
    let (len, n) = (0.1, 20);
    let mut cfg = conduction_cfg(ALPHA);
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(80.0);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(20.0);
    let mut s = FlowSolver::new(slab_mesh(n, len), cfg, 50.0);
    // many diffusion times, big steps (implicit)
    for _ in 0..40 {
        s.step(len * len / ALPHA);
    }
    let mut worst: f64 = 0.0;
    for i in 0..n {
        let x = s.mesh.centre[0][i];
        let exact = 80.0 - 60.0 * x / len;
        worst = worst.max((s.t[s.mesh.cell([i, 0, 0])] - exact).abs());
    }
    let e = report("steady slab", "max |T - T_exact| K (dT 60 K)", worst, 0.0);
    assert!(e.abs() < 0.01, "worst error {worst} K");
}

/// Uniform heat generation q between two walls at the same temperature:
/// `T = Tw + q x (L - x) / (2 alpha)`, peak `q L^2 / (8 alpha)` in the middle.
#[test]
fn slab_with_heat_source_is_parabolic() {
    let len = 0.1;
    let q = 0.5; // K/s, heat / (rho cp)
    let exact_peak = q * len * len / (8.0 * ALPHA);
    for n in [10usize, 20, 40] {
        let mut cfg = conduction_cfg(ALPHA);
        cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(30.0);
        cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(30.0);
        cfg.heat_source = Some(vec![q; n]);
        let mut s = FlowSolver::new(slab_mesh(n, len), cfg, 30.0);
        for _ in 0..40 {
            s.step(len * len / ALPHA);
        }
        let mut worst: f64 = 0.0;
        for i in 0..n {
            let x = s.mesh.centre[0][i];
            let exact = 30.0 + q * x * (len - x) / (2.0 * ALPHA);
            worst = worst.max((s.t[s.mesh.cell([i, 0, 0])] - exact).abs());
        }
        let mean_rise = s.mean_temperature() - 30.0;
        // a quadratic is reproduced to rounding by a central scheme with the
        // half-cell wall treatment, up to the wall gradient order
        let e = report(
            &format!("slab with source, {n} cells"),
            "mean rise K",
            mean_rise,
            2.0 * exact_peak / 3.0,
        );
        println!("  max profile error {worst:.3e} K of peak rise {exact_peak:.2} K");
        assert!(
            e.abs() < 0.02 / (n as f64 / 10.0).powi(2) + 1e-6,
            "mean rise {e}"
        );
        assert!(worst < 0.03 * exact_peak, "profile error {worst}");
    }
}

/// Heat balance: with a source the walls carry away exactly what is made.
#[test]
fn heat_balance_of_a_source() {
    let (len, n) = (0.1, 30);
    let q = 2.0;
    let mut cfg = conduction_cfg(ALPHA);
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(10.0);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(10.0);
    cfg.heat_source = Some(vec![q; n]);
    let mut s = FlowSolver::new(slab_mesh(n, len), cfg, 10.0);
    for _ in 0..40 {
        s.step(len * len / ALPHA);
    }
    // flux out of both walls by the half-cell gradient, per unit area / (rho cp)
    let t0 = s.t[s.mesh.cell([0, 0, 0])];
    let t1 = s.t[s.mesh.cell([n - 1, 0, 0])];
    let h = 0.5 * s.mesh.dx[0][0];
    let out = ALPHA * ((t0 - 10.0) / h + (t1 - 10.0) / h);
    let made = q * len;
    let e = report("heat balance", "wall flux / generated", out, made);
    assert!(e.abs() < 1e-3, "balance {e}");
}

/// Transient slab: initial temperature 1, walls at 0, Fourier series.
#[test]
fn transient_slab_matches_fourier_series() {
    let (len, n) = (0.1, 50);
    let tau = len * len / ALPHA; // diffusion time
    let exact_mid = |t: f64| -> f64 {
        (0..200)
            .map(|k| {
                let m = (2 * k + 1) as f64;
                let w = m * std::f64::consts::PI;
                4.0 / w * (w * 0.5).sin() * (-(w * w) * ALPHA * t / (len * len)).exp()
            })
            .sum()
    };
    let mut errs = Vec::new();
    for dt_div in [200.0, 400.0] {
        let mut cfg = conduction_cfg(ALPHA);
        cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(0.0);
        cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(0.0);
        let mut s = FlowSolver::new(slab_mesh(n, len), cfg, 1.0);
        let dt = tau / dt_div;
        let target = 0.1 * tau;
        let steps = (target / dt).round() as usize;
        for _ in 0..steps {
            s.step(dt);
        }
        // centre value: average of the two middle cells
        let mid = 0.5 * (s.t[s.mesh.cell([n / 2 - 1, 0, 0])] + s.t[s.mesh.cell([n / 2, 0, 0])]);
        let e = report(
            &format!("transient slab, dt = tau/{dt_div}"),
            "T(centre) at t = 0.1 tau",
            mid,
            exact_mid(target),
        );
        errs.push(e.abs());
    }
    println!(
        "  time-step halving: error {:.3e} -> {:.3e} (backward Euler is first order)",
        errs[0], errs[1]
    );
    assert!(errs[1] < 0.02);
    assert!(errs[1] < 0.7 * errs[0], "error does not fall with dt");
}
