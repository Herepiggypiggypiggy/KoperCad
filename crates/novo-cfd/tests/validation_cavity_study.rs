//! Mesh-convergence (Richardson) study and consistency checks on the
//! differentially heated cavity at Ra = 1e4, Pr = 0.71 (design section 5,
//! case 7, with the cavity standing in until the cabinet scenario solver
//! exists). Reference: de Vahl Davis (1983), Nu = 2.243.
//!
//! Run `-- --nocapture` to see the `VALIDATION` rows.

mod validation;
use validation::*;

const RA: f64 = 1e4;
const NU_REF: f64 = 2.243;

#[test]
fn richardson_study_of_nusselt() {
    let grids = [12usize, 24, 48];
    let nu: Vec<f64> = grids
        .iter()
        .map(|&n| {
            let c = heated_cavity(RA, n, 1.3, false, 1);
            let (hot, cold) = c.nusselt(false);
            assert!((hot - cold).abs() / hot < 0.01, "walls disagree at {n}");
            let nu = 0.5 * (hot + cold);
            report(&format!("Ra 1e4 cavity {n}x{n}"), "Nu", nu, NU_REF);
            nu
        })
        .collect();
    // r = 2 between nested grids
    let (d12, d23) = (nu[1] - nu[0], nu[2] - nu[1]);
    let p = (d12 / d23).abs().ln() / 2f64.ln();
    let extrapolated = nu[2] + d23 / (2f64.powf(p) - 1.0);
    let gci = 1.25 * (d23 / nu[2]).abs() / (2f64.powf(p) - 1.0);
    println!(
        "  observed order p = {p:.2}, extrapolated Nu = {extrapolated:.4}, GCI(fine) = {:.3}%",
        100.0 * gci
    );
    let e = report(
        "Ra 1e4 cavity Richardson",
        "Nu (extrapolated)",
        extrapolated,
        NU_REF,
    );
    // monotone convergence, order between first and third, benchmark within 1%
    assert!(d12 * d23 > 0.0, "not monotone: {nu:?}");
    assert!((1.0..=4.0).contains(&p), "observed order {p}");
    assert!(e.abs() < 0.01, "extrapolated Nu is {extrapolated}");
    assert!(gci < 0.01, "GCI {gci}");
}

/// The problem has a 180 degree rotation symmetry: T(x, y) = -T(1-x, 1-y).
#[test]
fn centro_symmetry_of_the_solution() {
    let c = heated_cavity(RA, 24, 1.3, false, 1);
    let (s, n) = (&c.solver, c.n);
    let mut worst: f64 = 0.0;
    for j in 0..n {
        for i in 0..n {
            let a = s.t[s.mesh.cell([i, j, 0])];
            let b = s.t[s.mesh.cell([n - 1 - i, n - 1 - j, 0])];
            worst = worst.max((a + b).abs());
        }
    }
    println!(
        "VALIDATION | Ra 1e4 cavity 24x24 | max |T(x,y)+T(1-x,1-y)| (dT 1) | {worst:.3e} | 0 | -"
    );
    assert!(worst < 5e-3, "asymmetry {worst}");
}

/// The same physical problem turned by 90 degrees must give the same Nusselt
/// number; a difference would be an axis-dependent bug in the solver.
#[test]
fn rotated_cavity_gives_the_same_answer() {
    let a = heated_cavity(RA, 24, 1.3, false, 1);
    let b = heated_cavity(RA, 24, 1.3, true, 1);
    let (na, nb) = (a.nusselt(false).0, b.nusselt(true).0);
    let e = report("Ra 1e4 cavity turned 90 deg", "Nu vs unturned", nb, na);
    assert!(e.abs() < 2e-3, "axis dependence {e}");
}

/// A 3D run with several cells across a symmetric depth is the 2D problem.
#[test]
fn depth_cells_do_not_change_the_answer() {
    let a = heated_cavity(RA, 24, 1.3, false, 1);
    let b = heated_cavity(RA, 24, 1.3, false, 4);
    let e = report(
        "Ra 1e4 cavity, 4 cells deep",
        "Nu vs 1 cell deep",
        b.nusselt(false).0,
        a.nusselt(false).0,
    );
    assert!(e.abs() < 1e-3, "depth dependence {e}");
}
