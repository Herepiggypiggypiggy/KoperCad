//! The supply a motor actually sees: a voltage level and a lost phase (lane L2d, hoist families).
//!
//! Exact by definition: a single-phase supply gives no torque at standstill (the two rotating
//! fields cancel), and torque scales with the square of the voltage for a given slip. Model:
//! the size of the single-phasing torque (each revolving field at half the three-phase
//! amplitude, [`SINGLE_PHASE_FIELD_FACTOR`]) and everything the Kloss fit leaves out.

use crate::Fidelity;
use crate::error::{self, InputError};
use crate::motor::{Direction, Winding};

/// Fidelity of the single-phasing torque.
pub const SINGLE_PHASE_FIDELITY: Fidelity = Fidelity::Model;

/// Amplitude of each of the two revolving fields of a motor on two phases, relative to the
/// three-phase field (double revolving field model). A model constant.
pub const SINGLE_PHASE_FIELD_FACTOR: f64 = 0.5;

/// One of the three supply phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    L1,
    L2,
    L3,
}

/// What the terminals carry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Supply {
    voltage_ratio: f64,
    lost: Option<Phase>,
}

impl Supply {
    /// Rated voltage on all three phases.
    pub const HEALTHY: Self = Self {
        voltage_ratio: 1.0,
        lost: None,
    };

    /// `voltage_ratio` is the line voltage over the rated voltage, 0 to 1.5; `lost` is the
    /// phase that is open, if any.
    pub fn new(voltage_ratio: f64, lost: Option<Phase>) -> Result<Self, InputError> {
        error::finite("voltage_ratio", voltage_ratio)?;
        if !(0.0..=1.5).contains(&voltage_ratio) {
            return Err(InputError::OutOfRange {
                field: "voltage_ratio",
                value: voltage_ratio,
                min: 0.0,
                max: 1.5,
            });
        }
        Ok(Self {
            voltage_ratio,
            lost,
        })
    }

    pub fn voltage_ratio(&self) -> f64 {
        self.voltage_ratio
    }

    pub fn phase_lost(&self) -> bool {
        self.lost.is_some()
    }

    /// Which phase is open, if one is.
    pub fn lost_phase(&self) -> Option<Phase> {
        self.lost
    }

    pub fn is_healthy(&self) -> bool {
        *self == Self::HEALTHY
    }

    /// The same supply with `phase` open.
    pub fn with_phase_lost(mut self, phase: Phase) -> Self {
        self.lost = Some(phase);
        self
    }
}

impl Default for Supply {
    fn default() -> Self {
        Self::HEALTHY
    }
}

/// Shaft torque of a winding fed with `supply`, in N m (signed like [`Winding::torque_nm`]).
///
/// Three phases: the Kloss torque times the voltage ratio squared. One phase lost: the forward
/// and the backward revolving field each give [`SINGLE_PHASE_FIELD_FACTOR`] of the Kloss torque
/// at their own slip, so the net is exactly zero at standstill and a third of the three-phase
/// torque near rated speed. The motor can keep turning on two phases once it runs, but it
/// cannot start from rest.
pub fn torque_nm(
    winding: &Winding,
    direction: Direction,
    omega_rad_s: f64,
    supply: &Supply,
) -> f64 {
    let v2 = supply.voltage_ratio * supply.voltage_ratio;
    let forward = winding.torque_nm(direction, omega_rad_s);
    if !supply.phase_lost() {
        return v2 * forward;
    }
    let against = match direction {
        Direction::Positive => Direction::Negative,
        Direction::Negative => Direction::Positive,
    };
    v2 * SINGLE_PHASE_FIELD_FACTOR * (forward + winding.torque_nm(against, omega_rad_s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn winding() -> Winding {
        Winding::new(4, 50.0, 4000.0, 1440.0, 2.2).unwrap()
    }

    #[test]
    fn torque_scales_with_voltage_squared() {
        let w = winding();
        let omega = 100.0;
        let full = torque_nm(&w, Direction::Positive, omega, &Supply::HEALTHY);
        let s = Supply::new(0.8, None).unwrap();
        let low = torque_nm(&w, Direction::Positive, omega, &s);
        assert!((low - 0.64 * full).abs() < 1e-9);
        assert!((full - w.torque_nm(Direction::Positive, omega)).abs() < 1e-12);
    }

    #[test]
    fn no_torque_at_standstill_on_two_phases() {
        let w = winding();
        let s = Supply::HEALTHY.with_phase_lost(Phase::L3);
        assert_eq!(torque_nm(&w, Direction::Positive, 0.0, &s), 0.0);
        assert_eq!(torque_nm(&w, Direction::Negative, 0.0, &s), 0.0);
    }

    #[test]
    fn two_phases_keep_about_half_the_torque_near_rated_speed() {
        let w = winding();
        let omega = 1440.0 * std::f64::consts::TAU / 60.0;
        let three = torque_nm(&w, Direction::Positive, omega, &Supply::HEALTHY);
        let two = torque_nm(
            &w,
            Direction::Positive,
            omega,
            &Supply::HEALTHY.with_phase_lost(Phase::L3),
        );
        // Half the forward torque minus half the backward torque (slip about 2, not small):
        // between a fifth and a half of the three-phase torque.
        assert!(two > 0.2 * three && two < 0.5 * three, "{two} vs {three}");
    }

    #[test]
    fn two_phases_are_far_weaker_than_three_near_standstill() {
        let w = winding();
        let s = Supply::HEALTHY.with_phase_lost(Phase::L1);
        let start = w.torque_nm(Direction::Positive, 0.0).abs();
        let creeping = torque_nm(&w, Direction::Positive, 5.0, &s).abs();
        assert!(creeping < 0.2 * start, "{creeping} vs {start}");
        // And it is odd in the field direction: the mirror image gives the mirror torque.
        let t = torque_nm(&w, Direction::Positive, 50.0, &s);
        let m = torque_nm(&w, Direction::Negative, -50.0, &s);
        assert!((t + m).abs() < 1e-9);
    }

    #[test]
    fn refuses_a_silly_voltage() {
        assert!(Supply::new(-0.1, None).is_err());
        assert!(Supply::new(f64::NAN, None).is_err());
        assert!(Supply::new(2.0, None).is_err());
    }
}
