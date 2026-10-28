//! Spring-applied holding brake fed through a rectifier (lane L2a).
//!
//! The spring applies the brake; the coil, fed through a rectifier from the motor terminals
//! or a separate supply, releases it. Model: fixed delays entered by the user, from the data
//! sheet or a measurement. Switching the AC side of the rectifier lets the coil current decay
//! through the rectifier and the brake applies later than when the DC side is switched.

use crate::Fidelity;
use crate::error::{self, InputError};

/// Fidelity of all brake timing and torque numbers.
pub const BRAKE_FIDELITY: Fidelity = Fidelity::Model;

/// Brake state seen by the mechanics: [`Released`](BrakeState::Released) means no holding
/// torque; [`Applied`](BrakeState::Applied) means some holding torque is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrakeState {
    Applied,
    Released,
}

/// Where the rectifier is switched off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switching {
    AcSide,
    DcSide,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Applied,
    /// Coil fed, spring still holding until the release delay has passed.
    Releasing {
        elapsed_s: f64,
    },
    Released,
    /// Coil off, torque builds after the apply delay.
    Applying {
        elapsed_s: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringBrake {
    torque_nm: f64,
    release_delay_s: f64,
    apply_delay_ac_s: f64,
    apply_delay_dc_s: f64,
    torque_rise_s: f64,
    switching: Switching,
    phase: Phase,
}

impl SpringBrake {
    /// `torque_rise_s` is the time the torque takes to build from zero to full after the
    /// apply delay (0 = a step).
    pub fn new(
        torque_nm: f64,
        release_delay_s: f64,
        apply_delay_ac_s: f64,
        apply_delay_dc_s: f64,
        torque_rise_s: f64,
        switching: Switching,
    ) -> Result<Self, InputError> {
        error::positive("brake torque_nm", torque_nm)?;
        error::non_negative("release_delay_s", release_delay_s)?;
        error::non_negative("apply_delay_ac_s", apply_delay_ac_s)?;
        error::non_negative("apply_delay_dc_s", apply_delay_dc_s)?;
        error::non_negative("torque_rise_s", torque_rise_s)?;
        Ok(Self {
            torque_nm,
            release_delay_s,
            apply_delay_ac_s,
            apply_delay_dc_s,
            torque_rise_s,
            switching,
            phase: Phase::Applied,
        })
    }

    pub fn rated_torque_nm(&self) -> f64 {
        self.torque_nm
    }

    fn apply_delay_s(&self) -> f64 {
        match self.switching {
            Switching::AcSide => self.apply_delay_ac_s,
            Switching::DcSide => self.apply_delay_dc_s,
        }
    }

    /// Holding torque available now, in N m (always positive or zero).
    pub fn holding_torque_nm(&self) -> f64 {
        match self.phase {
            Phase::Applied | Phase::Releasing { .. } => self.torque_nm,
            Phase::Released => 0.0,
            Phase::Applying { elapsed_s } => {
                let building = elapsed_s - self.apply_delay_s();
                if building <= 0.0 {
                    0.0
                } else if self.torque_rise_s == 0.0 {
                    self.torque_nm
                } else {
                    self.torque_nm * (building / self.torque_rise_s).min(1.0)
                }
            }
        }
    }

    pub fn state(&self) -> BrakeState {
        if self.holding_torque_nm() > 0.0 {
            BrakeState::Applied
        } else {
            BrakeState::Released
        }
    }

    /// Advance by `dt_s`; `coil_fed` is whether the rectifier output is energised.
    pub fn step(&mut self, coil_fed: bool, dt_s: f64) {
        let apply_delay = self.apply_delay_s();
        let full_at = apply_delay + self.torque_rise_s;
        self.phase = match (self.phase, coil_fed) {
            (Phase::Applied, true) => self.advance_release(0.0, dt_s),
            (Phase::Releasing { elapsed_s }, true) => self.advance_release(elapsed_s, dt_s),
            (Phase::Released, true) => Phase::Released,
            // Fed again before any torque has built: the brake never closed.
            (Phase::Applying { elapsed_s }, true) if elapsed_s <= apply_delay => Phase::Released,
            // Fed again after torque had started to build: opens through the release delay.
            (Phase::Applying { .. }, true) => self.advance_release(0.0, dt_s),
            (Phase::Applied, false) => Phase::Applied,
            // Switched off before the brake opened: the spring never let go.
            (Phase::Releasing { .. }, false) => Phase::Applied,
            (Phase::Released, false) => self.advance_apply(0.0, dt_s, full_at),
            (Phase::Applying { elapsed_s }, false) => self.advance_apply(elapsed_s, dt_s, full_at),
        };
    }

    fn advance_release(&self, elapsed_s: f64, dt_s: f64) -> Phase {
        let elapsed_s = elapsed_s + dt_s;
        if elapsed_s >= self.release_delay_s {
            Phase::Released
        } else {
            Phase::Releasing { elapsed_s }
        }
    }

    fn advance_apply(&self, elapsed_s: f64, dt_s: f64, full_at: f64) -> Phase {
        let elapsed_s = elapsed_s + dt_s;
        if elapsed_s >= full_at {
            Phase::Applied
        } else {
            Phase::Applying { elapsed_s }
        }
    }
}
