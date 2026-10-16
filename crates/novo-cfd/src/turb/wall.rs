//! Wall functions: law of the wall for momentum (Spalding) and heat (Kader).
//!
//! Spalding's single formula covers the viscous sublayer, the buffer layer
//! and the log layer, so one function serves resolved (`y+` near 1) and
//! coarse (`y+` 30 to 300) wall cells alike.

use super::KAPPA;

/// Spalding: `y+` as a function of `u+` (smooth from sublayer to log layer).
fn spalding_yplus(u_plus: f64) -> f64 {
    const B: f64 = 5.5;
    let ku = KAPPA * u_plus;
    u_plus + (-KAPPA * B).exp() * (ku.exp() - 1.0 - ku - ku * ku / 2.0 - ku * ku * ku / 6.0)
}

/// Friction velocity from the tangential speed `u` at distance `d` from the
/// wall, kinematic viscosity `nu`.
pub fn friction_velocity(u: f64, d: f64, nu: f64) -> f64 {
    let u = u.abs();
    if u < 1e-14 {
        return 0.0;
    }
    // Root of spalding(u+) = d u / (u+ nu): the left side grows with u+,
    // the right side falls, so the root is unique; bracketed Newton.
    let (mut lo, mut hi) = (1e-8, 80.0);
    let mut up = (u * d / nu).sqrt().clamp(lo, hi);
    for _ in 0..100 {
        let yp_target = d * u / (up * nu);
        let f = spalding_yplus(up) - yp_target;
        if f > 0.0 {
            hi = up;
        } else {
            lo = up;
        }
        let ku = KAPPA * up;
        let dy = 1.0
            + (-KAPPA * 5.5).exp() * KAPPA * (ku.exp() - 1.0 - ku - ku * ku / 2.0)
            + yp_target / up;
        let mut next = up - f / dy;
        if !(next > lo && next < hi) {
            next = 0.5 * (lo + hi);
        }
        if (next - up).abs() < 1e-13 * up {
            up = next;
            break;
        }
        up = next;
    }
    u / up
}

/// Kader's temperature law: `T+` for wall units `y+` and molecular Prandtl
/// number `pr`. `(T_wall - T) = T+ * q_w / (rho cp u_tau)`.
pub fn thermal_t_plus(y_plus: f64, pr: f64) -> f64 {
    let gamma = 0.01 * (pr * y_plus).powi(4) / (1.0 + 5.0 * pr.powi(3) * y_plus);
    let beta = (3.85 * pr.cbrt() - 1.3).powi(2) + 2.12 * pr.ln();
    pr * y_plus * (-gamma).exp()
        + (2.12 * (1.0 + y_plus).ln() + beta) * (-1.0 / gamma.max(1e-30)).exp()
}

/// What the flow and energy solvers need from one wall-adjacent cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallCell {
    pub cell: usize,
    pub axis: usize,
    pub high: bool,
    /// Cell centre to wall, m.
    pub dist: f64,
    pub area: f64,
    pub u_tau: f64,
    pub y_plus: f64,
    /// Wall shear stress over density, m2/s2.
    pub tau_w: f64,
    /// Viscosity to use for the wall-face shear: `shear = nu_wall * U / dist`.
    pub nu_wall: f64,
    /// Diffusivity for the wall-face heat flux: `q / (rho cp) = alpha_wall * dT / dist`.
    pub alpha_wall: f64,
}

/// Wall quantities for tangential speed `u` at distance `d`.
pub fn wall_quantities(u: f64, d: f64, nu: f64, pr: f64) -> (f64, f64, f64, f64, f64) {
    let u_tau = friction_velocity(u, d, nu);
    let y_plus = d * u_tau / nu;
    let tau_w = u_tau * u_tau;
    let nu_wall = if u.abs() > 1e-14 {
        tau_w * d / u.abs()
    } else {
        nu
    };
    // heat: alpha_wall = u_tau d / T+  (molecular limit alpha = nu / pr)
    let t_plus = thermal_t_plus(y_plus.max(1e-8), pr);
    let alpha_wall = (u_tau * d / t_plus).max(nu / pr);
    (u_tau, y_plus, tau_w, nu_wall.max(nu), alpha_wall)
}
