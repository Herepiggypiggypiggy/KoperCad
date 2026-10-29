//! Drive, motor-at-frequency, mechanics and brake engine (skeleton, plan
//! `docs/design/drive-crane-simulation.md` lane L0).
//!
//! Averaged model: no PWM and no harmonics. Exact by definition: synchronous
//! speed and ramp arithmetic. Model: slip, current, thermal state, brake
//! behaviour. Not a replica of any manufacturer's drive and no compliance claim.
//! Lanes: L1 drive (`drive`), L2 motor and mechanics (`motor`, `induction`, `sequencer`, `mechanics`, `brake`, `hoist_unit`),
//! L2c open-loop drive hoist (`hoist_logic`),
//! L2d hoist families (`supply`, `brake_system`: conical-rotor and rectifier brakes, lost phase, faults).

pub mod brake;
pub mod brake_system;
pub mod drive;
pub mod error;
pub mod hoist_logic;
pub mod hoist_unit;
pub mod induction;
pub mod mechanics;
pub mod motor;
pub mod sequencer;
pub mod supply;

pub use error::InputError;

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
    use super::drive::Ramp;

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
}
