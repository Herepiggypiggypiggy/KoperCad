//! Convection schemes: face values from the cells or faces around a face.

/// How a transported quantity is interpolated to a face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// First order, always bounded, smears gradients.
    Upwind,
    /// Second order, can oscillate at high cell Peclet numbers.
    Central,
    /// Second order TVD with the van Leer limiter (default).
    VanLeer,
    /// Bounded QUICK (third order on smooth fields).
    Quick,
}

/// A sample: position (m) and value.
pub type Sample = (f64, f64);

/// Value at `xf` between `l` and `u`. `ll` and `uu` are the next samples
/// beyond them (absent at a boundary, which falls back to upwind). `forward`
/// is true when the flow goes from `l` to `u`.
pub fn face_value(
    scheme: Scheme,
    xf: f64,
    ll: Option<Sample>,
    l: Sample,
    u: Sample,
    uu: Option<Sample>,
    forward: bool,
) -> f64 {
    let (c, d, up) = if forward { (l, u, ll) } else { (u, l, uu) };
    match scheme {
        Scheme::Upwind => c.1,
        Scheme::Central => c.1 + (xf - c.0) / (d.0 - c.0) * (d.1 - c.1),
        Scheme::VanLeer | Scheme::Quick => {
            let Some(up) = up else {
                return c.1;
            };
            let gd = (d.1 - c.1) / (d.0 - c.0);
            let gu = (c.1 - up.1) / (c.0 - up.0);
            if gd.abs() < 1e-30 || gu * gd <= 0.0 {
                return c.1;
            }
            let r = gu / gd;
            let psi = if scheme == Scheme::VanLeer {
                (r + r.abs()) / (1.0 + r.abs())
            } else {
                (2.0 * r).min((3.0 + r) / 4.0).clamp(0.0, 2.0)
            };
            c.1 + psi * (xf - c.0) / (d.0 - c.0) * (d.1 - c.1)
        }
    }
}
