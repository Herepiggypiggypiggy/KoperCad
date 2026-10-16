//! Law of the wall in a turbulent channel (lane CFD-B).
//!
//! Reference: Reichardt's law for the mean profile, and the centreline
//! velocity of the DNS at Re_tau = 590 (Moser, Kim, Mansour 1999,
//! `U+_cl = 21.3`) for the full profile. Errors are printed (`--nocapture`).

mod common_channel;
use common_channel::{channel, reichardt};
use novo_cfd::turb::TurbModel;

fn profile_error(model: TurbModel, n: usize, first_plus: f64) -> (f64, f64, f64) {
    let mut ch = channel(model, 590.0, n, first_plus);
    ch.converge(6000);
    let mut max_err: f64 = 0.0;
    for j in 0..n {
        let yp = ch.y[j] * ch.re_tau;
        if (30.0..=300.0).contains(&yp) {
            let target = reichardt(yp);
            max_err = max_err.max((ch.u[j] - target).abs() / target);
        }
    }
    let u_tau_wall = ch.state.walls()[0].u_tau;
    (max_err, ch.u[n - 1], u_tau_wall)
}

#[test]
fn sst_resolved_wall_matches_the_law_of_the_wall() {
    let (err, u_cl, u_tau) = profile_error(TurbModel::KOmegaSst, 80, 0.5);
    println!(
        "SST y+~0.5: log-region error {err:.4}, centreline U+ {u_cl:.2} (DNS 21.3), u_tau {u_tau:.4}"
    );
    assert!(err < 0.05, "log layer error {err}");
    assert!((u_cl - 21.3).abs() / 21.3 < 0.06, "centreline {u_cl}");
    assert!((u_tau - 1.0).abs() < 0.03, "wall shear {u_tau}");
}

#[test]
fn sst_wall_function_on_a_coarse_grid() {
    let (err, u_cl, u_tau) = profile_error(TurbModel::KOmegaSst, 14, 80.0);
    println!(
        "SST y+~80 (wall function): error {err:.4}, centreline U+ {u_cl:.2}, u_tau {u_tau:.4}"
    );
    assert!(err < 0.08, "log layer error {err}");
    assert!((u_cl - 21.3).abs() / 21.3 < 0.10, "centreline {u_cl}");
    assert!((u_tau - 1.0).abs() < 0.05, "wall shear {u_tau}");
}

#[test]
fn k_epsilon_wall_function_agrees_with_sst() {
    let (e_ke, ucl_ke, _) = profile_error(TurbModel::KEpsilon, 14, 80.0);
    let (e_sst, ucl_sst, _) = profile_error(TurbModel::KOmegaSst, 14, 80.0);
    let diff = (ucl_ke - ucl_sst).abs() / ucl_sst;
    println!(
        "k-eps: error {e_ke:.4}, U+cl {ucl_ke:.2}; SST: error {e_sst:.4}, U+cl {ucl_sst:.2}; difference {diff:.4}"
    );
    assert!(e_ke < 0.10, "k-epsilon log layer error {e_ke}");
    assert!(diff < 0.08, "models disagree by {diff}");
}
