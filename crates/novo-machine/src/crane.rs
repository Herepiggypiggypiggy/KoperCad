//! Crane kinematics (lane L4).

/// Axes of an overhead crane. Positions in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CraneAxes {
    pub bridge_mm: f64,
    pub trolley_mm: f64,
    pub hoist_mm: f64,
}

/// Pendulum period in seconds for a hook on a rope of `length_m` (small angles).
/// The length must be finite and greater than zero.
pub fn pendulum_period_s(length_m: f64) -> Result<f64, novo_drive::InputError> {
    if !length_m.is_finite() {
        return Err(novo_drive::InputError::NotFinite { field: "length_m" });
    }
    if length_m <= 0.0 {
        return Err(novo_drive::InputError::NotPositive {
            field: "length_m",
            value: length_m,
        });
    }
    Ok(2.0 * std::f64::consts::PI * (length_m / novo_drive::GRAVITY).sqrt())
}
