//! Crane kinematics (lane L4).

/// Axes of an overhead crane. Positions in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CraneAxes {
    pub bridge_mm: f64,
    pub trolley_mm: f64,
    pub hoist_mm: f64,
}

/// Pendulum period in seconds for a hook on a rope of `length_m` (small angles).
pub fn pendulum_period_s(length_m: f64) -> f64 {
    2.0 * std::f64::consts::PI * (length_m / novo_drive::GRAVITY).sqrt()
}
