//! Second-order convection of the turbulence scalars (lane CFD-B): a step
//! carried by a uniform stream stays between its bounds and stays sharper
//! than with first-order upwind.

use novo_cfd::result::Grid3;
use novo_cfd::turb::{Equation, FaceKind, TurbMesh};

/// Carry a step 30 cells downstream; returns (min, max, 10-90 % width in cells).
fn run(tvd: bool) -> (f64, f64, f64) {
    let n = 120;
    let grid = Grid3::uniform([n, 1, 1], [1200.0, 100.0, 100.0]);
    let m = TurbMesh::new(&grid, None, [FaceKind::Symmetry; 6]);
    let u = 1.0;
    let vel = [
        vec![u; m.vel_len(0)],
        vec![0.0; m.vel_len(1)],
        vec![0.0; m.vel_len(2)],
    ];
    let zero = vec![0.0; n];
    let none = vec![false; n];
    let eq = Equation {
        mesh: &m,
        vel: &vel,
        gamma: &zero,
        su: &zero,
        sp: &zero,
        fixed: &none,
        tvd,
    };
    let mut phi: Vec<f64> = (0..n).map(|i| if i < 50 { 1.0 } else { 0.0 }).collect();
    // the plateau is long enough that the open upstream end never reaches the step
    let dx = 1.0 / 100.0;
    let dt = 0.4 * dx / u;
    let steps = (30.0 * dx / (u * dt)).round() as usize;
    for _ in 0..steps {
        let old = phi.clone();
        eq.solve(&mut phi, &old, dt, 30);
    }
    let (lo, hi) = phi
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    // transition width: cells between phi = 0.9 and phi = 0.1 downstream of the step
    let first = (40..n).find(|i| phi[*i] < 0.9).unwrap();
    let last = (40..n).find(|i| phi[*i] < 0.1).unwrap();
    (lo, hi, (last - first) as f64)
}

#[test]
fn limited_second_order_convection_is_bounded_and_sharper_than_upwind() {
    let (lo1, hi1, w1) = run(false);
    let (lo2, hi2, w2) = run(true);
    println!(
        "upwind: [{lo1:.4}, {hi1:.4}] width {w1} cells; limited 2nd order: [{lo2:.4}, {hi2:.4}] width {w2} cells"
    );
    assert!(lo2 >= -1e-9 && hi2 <= 1.0 + 1e-9, "overshoot {lo2} {hi2}");
    assert!(lo1 >= -1e-9 && hi1 <= 1.0 + 1e-9);
    assert!(w2 < 0.8 * w1, "{w2} vs {w1}");
}
