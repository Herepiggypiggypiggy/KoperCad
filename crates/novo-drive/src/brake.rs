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

/// Default pull-in excitation (share of the rated voltage). A model value, set per brake.
pub const DEFAULT_PULL_IN: f64 = 0.8;

/// Default drop-out excitation (share of the rated voltage). A model value, set per brake.
pub const DEFAULT_DROP_OUT: f64 = 0.4;

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
    /// Wear and ageing (see [`SpringBrake::set_wear`]).
    delay_scale: f64,
    torque_scale: f64,
    /// Excitation thresholds for [`SpringBrake::step_excitation`].
    pull_in: f64,
    drop_out: f64,
    /// The coil counts as fed (hysteresis state of `step_excitation`).
    coil_on: bool,
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
            delay_scale: 1.0,
            torque_scale: 1.0,
            pull_in: DEFAULT_PULL_IN,
            drop_out: DEFAULT_DROP_OUT,
            coil_on: false,
        })
    }

    /// Ageing and wear: every delay and the torque build-up take `delay_scale` times as long
    /// (a sticking armature, a wide air gap), and the holding torque is `torque_scale` times
    /// the rated one (worn lining, oil). Both greater than zero; `torque_scale` at most 1.
    pub fn set_wear(&mut self, delay_scale: f64, torque_scale: f64) -> Result<(), InputError> {
        error::positive("brake delay_scale", delay_scale)?;
        error::fraction("brake torque_scale", torque_scale)?;
        self.delay_scale = delay_scale;
        self.torque_scale = torque_scale;
        Ok(())
    }

    pub fn delay_scale(&self) -> f64 {
        self.delay_scale
    }

    pub fn torque_scale(&self) -> f64 {
        self.torque_scale
    }

    /// Pull-in and drop-out excitation, as a fraction of the rated coil voltage (or, for a
    /// field-released brake, of the rated motor voltage). The brake starts to release at
    /// `pull_in` and keeps open down to `drop_out` (hysteresis: the air gap is smaller once the
    /// armature has pulled in). `0 < drop_out <= pull_in <= 1`.
    pub fn set_thresholds(&mut self, pull_in: f64, drop_out: f64) -> Result<(), InputError> {
        error::fraction("brake pull_in", pull_in)?;
        error::fraction("brake drop_out", drop_out)?;
        if drop_out > pull_in {
            return Err(InputError::Inconsistent {
                what: "brake drop-out above pull-in",
            });
        }
        self.pull_in = pull_in;
        self.drop_out = drop_out;
        Ok(())
    }

    pub fn pull_in(&self) -> f64 {
        self.pull_in
    }

    pub fn drop_out(&self) -> f64 {
        self.drop_out
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
            Phase::Applied | Phase::Releasing { .. } => self.torque_nm * self.torque_scale,
            Phase::Released => 0.0,
            Phase::Applying { elapsed_s } => {
                let building = elapsed_s - self.apply_delay_s();
                let full = self.torque_nm * self.torque_scale;
                if building <= 0.0 {
                    0.0
                } else if self.torque_rise_s == 0.0 {
                    full
                } else {
                    full * (building / self.torque_rise_s).min(1.0)
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
        self.coil_on = coil_fed;
        self.advance(coil_fed, dt_s, 1.0);
    }

    /// Advance by `dt_s` with an analogue excitation (fraction of the rated voltage, 0 = off)
    /// instead of an on/off coil. The coil counts as fed from `pull_in` upwards and stays fed
    /// down to `drop_out`. `release_speedup` (1 = normal) shortens the release delay, for an
    /// over-excited coil.
    pub fn step_excitation(&mut self, excitation: f64, release_speedup: f64, dt_s: f64) {
        let on = if self.coil_on {
            excitation >= self.drop_out
        } else {
            excitation >= self.pull_in
        };
        self.coil_on = on;
        self.advance(on, dt_s, release_speedup.max(1.0));
    }

    /// Whether the coil counts as fed after the last step (for the reading in the UI).
    pub fn coil_on(&self) -> bool {
        self.coil_on
    }

    fn advance(&mut self, coil_fed: bool, dt_s: f64, release_speedup: f64) {
        let apply_delay = self.apply_delay_s();
        let full_at = apply_delay + self.torque_rise_s;
        // Delays are stretched by wear, so time passes slower on the brake's own clock.
        let release_dt = dt_s * release_speedup / self.delay_scale;
        let dt_s = dt_s / self.delay_scale;
        self.phase = match (self.phase, coil_fed) {
            (Phase::Applied, true) => self.advance_release(0.0, release_dt),
            (Phase::Releasing { elapsed_s }, true) => self.advance_release(elapsed_s, release_dt),
            (Phase::Released, true) => Phase::Released,
            // Fed again before any torque has built: the brake never closed.
            (Phase::Applying { elapsed_s }, true) if elapsed_s <= apply_delay => Phase::Released,
            // Fed again after torque had started to build: opens through the release delay.
            (Phase::Applying { .. }, true) => self.advance_release(0.0, release_dt),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn brake() -> SpringBrake {
        SpringBrake::new(60.0, 0.10, 0.20, 0.02, 0.0, Switching::AcSide).unwrap()
    }

    fn steps_until(b: &mut SpringBrake, fed: bool, want: BrakeState) -> usize {
        for n in 1..10_000 {
            b.step(fed, 0.001);
            if b.holding_torque_nm() == 0.0 && want == BrakeState::Released
                || b.holding_torque_nm() > 0.0 && want == BrakeState::Applied
            {
                return n;
            }
        }
        panic!("never reached {want:?}");
    }

    #[test]
    fn wear_stretches_release_and_apply_by_the_same_factor() {
        let mut a = brake();
        let mut b = brake();
        b.set_wear(3.0, 1.0).unwrap();
        let ra = steps_until(&mut a, true, BrakeState::Released);
        let rb = steps_until(&mut b, true, BrakeState::Released);
        assert_eq!(ra, 100);
        assert_eq!(rb, 300);
        let pa = steps_until(&mut a, false, BrakeState::Applied);
        let pb = steps_until(&mut b, false, BrakeState::Applied);
        // Apply delay 0.2 s; torque rises as a step after it.
        assert!((pa as i64 - 201).abs() <= 1, "{pa}");
        assert!((pb as i64 - 601).abs() <= 3, "{pb}");
    }

    #[test]
    fn wear_scales_the_torque_and_refuses_nonsense() {
        let mut b = brake();
        b.set_wear(1.0, 0.5).unwrap();
        assert_eq!(b.holding_torque_nm(), 30.0);
        assert!(b.set_wear(0.0, 1.0).is_err());
        assert!(b.set_wear(1.0, 0.0).is_err());
        assert!(b.set_wear(1.0, 1.2).is_err());
        assert!(b.set_wear(f64::NAN, 1.0).is_err());
        assert_eq!(b.holding_torque_nm(), 30.0);
    }

    #[test]
    fn excitation_opens_at_pull_in_and_stays_open_down_to_drop_out() {
        let mut b = brake();
        b.set_thresholds(0.8, 0.4).unwrap();
        for _ in 0..300 {
            b.step_excitation(0.7, 1.0, 0.001);
        }
        assert!(b.holding_torque_nm() > 0.0, "0.7 is below pull-in");
        for _ in 0..300 {
            b.step_excitation(0.85, 1.0, 0.001);
        }
        assert_eq!(b.holding_torque_nm(), 0.0);
        for _ in 0..500 {
            b.step_excitation(0.5, 1.0, 0.001);
        }
        assert_eq!(b.holding_torque_nm(), 0.0, "0.5 keeps it open");
        for _ in 0..500 {
            b.step_excitation(0.3, 1.0, 0.001);
        }
        assert!(b.holding_torque_nm() > 0.0, "0.3 is below drop-out");
        assert!(b.set_thresholds(0.3, 0.5).is_err());
    }
}
