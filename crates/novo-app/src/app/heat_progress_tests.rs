//! Progress bar fraction and sparkline geometry.

use novo_cfd::{Fidelity, Progress, Residuals};

use super::heat_progress::{fraction, push, spark_points};

fn progress(fidelity: Fidelity, residual: f64, converged: bool) -> Progress {
    Progress {
        fidelity,
        iteration: 3,
        residuals: Residuals {
            momentum: residual,
            continuity: residual / 2.0,
            energy: residual / 10.0,
        },
        converged,
        elapsed_s: 1.0,
        energy_balance_rel: 0.0,
        monitor_drift_k: 0.0,
    }
}

const LEVELS: [Fidelity; 3] = [Fidelity::Coarse, Fidelity::Medium, Fidelity::Fine];

#[test]
fn the_bar_only_moves_forward_through_levels() {
    let start = fraction(&LEVELS, &progress(Fidelity::Coarse, 1.0, false));
    let mid = fraction(&LEVELS, &progress(Fidelity::Coarse, 1e-2, false));
    let done = fraction(&LEVELS, &progress(Fidelity::Coarse, 1e-9, true));
    let next = fraction(&LEVELS, &progress(Fidelity::Medium, 1.0, false));
    assert_eq!(start, 0.0);
    assert!(start < mid && mid < done);
    assert!((done - next).abs() < 1e-6, "{done} vs {next}");
    assert_eq!(
        fraction(&LEVELS, &progress(Fidelity::Fine, 1e-9, true)),
        1.0
    );
}

#[test]
fn a_rising_residual_does_not_leave_the_bar_range() {
    let f = fraction(&LEVELS, &progress(Fidelity::Fine, 50.0, false));
    assert!((0.0..=1.0).contains(&f));
}

#[test]
fn the_sparkline_falls_with_the_residual() {
    let mut h = Vec::new();
    for r in [1.0, 0.1, 0.01] {
        push(&mut h, &progress(Fidelity::Coarse, r, false));
    }
    let pts = spark_points(&h, 90.0, 16.0);
    assert_eq!(pts.len(), 3);
    assert_eq!(pts[0], (0.0, 0.0));
    assert!(pts[2].1 > pts[1].1 && pts[1].1 > pts[0].1);
    assert!((pts[2].0 - 90.0).abs() < 1e-4);
}

#[test]
fn history_is_capped_and_ignores_bad_values() {
    let mut h = Vec::new();
    push(&mut h, &progress(Fidelity::Coarse, f64::NAN, false));
    push(&mut h, &progress(Fidelity::Coarse, 0.0, false));
    assert!(h.is_empty());
    for _ in 0..1000 {
        push(&mut h, &progress(Fidelity::Coarse, 1.0, false));
    }
    assert_eq!(h.len(), 400);
}
