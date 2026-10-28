//! Drive, motor-at-frequency, mechanics and brake engine (skeleton, plan
//! `docs/design/drive-crane-simulation.md` lane L0).
//!
//! Averaged model: no PWM and no harmonics. Exact by definition: synchronous
//! speed and ramp arithmetic. Model: slip, current, thermal state, brake
//! behaviour. Not a replica of any manufacturer's drive and no compliance claim.
//! Lanes: L1 drive (`drive`), L2 motor and mechanics (`motor`, `mechanics`, `brake`).

pub mod brake;
pub mod drive;
pub mod mechanics;
pub mod motor;

/// Standard gravity in m/s2 (exact by definition).
pub const GRAVITY: f64 = 9.806_65;

/// Whether a quantity is exact by definition or comes from a model; shown next to numbers in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fidelity {
    Exact,
    Model,
}

#[cfg(test)]
mod tests {
    use super::{drive::Ramp, mechanics::hoist_holding_torque_nm, motor::sync_rpm};

    #[test]
    fn four_pole_at_25_hz_is_750_rpm() {
        assert!((sync_rpm(25.0, 4) - 750.0).abs() < 1e-9);
    }

    #[test]
    fn ramp_reaches_half_at_half_time() {
        let mut r = Ramp {
            frequency_hz: 0.0,
            accel_time_s: 5.0,
            decel_time_s: 5.0,
            reference_hz: 50.0,
        };
        for _ in 0..250 {
            r.step(50.0, 0.01);
        }
        assert!((r.frequency_hz - 25.0).abs() < 1e-9);
    }

    #[test]
    fn hoist_torque_by_hand() {
        // 5000 kg, r 0.2 m, i 31.5, eta 0.9 -> 5000*9.80665*0.2/28.35
        let t = hoist_holding_torque_nm(5000.0, 0.2, 31.5, 0.9);
        assert!((t - 345.9).abs() < 0.1, "{t}");
    }
}
