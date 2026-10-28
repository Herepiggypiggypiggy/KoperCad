//! Shaft mechanics: inertia, gearbox, hoist gravity (lane L2).

use crate::GRAVITY;

/// Motor-shaft torque in N m needed to hold a hoisted mass (positive = motoring).
pub fn hoist_holding_torque_nm(
    mass_kg: f64,
    drum_radius_m: f64,
    ratio: f64,
    efficiency: f64,
) -> f64 {
    mass_kg * GRAVITY * drum_radius_m / (ratio * efficiency)
}
