//! Heat and wear of a holding brake that slips (lane L2c-open, "heat over a duty cycle").
//!
//! A spring brake dissipates `torque x |shaft speed|` while the shaft turns against it: a late
//! engagement after a trip or a lost mains, a brake closed before the hook has stopped, a brake
//! that is only partly open. The energy goes into one lumped mass (the friction disc and the
//! housing) that cools toward ambient with one time constant, and it wears the lining.
//!
//! Exact by definition: the dissipated power (torque times speed) and its integral. Model: one
//! heat capacity and one cooling time constant for the whole brake, a constant torque while it
//! slips, wear proportional to the dissipated energy up to a lining life that is a parameter
//! (generic default, to be replaced by the maker's number). No fade of the friction with
//! temperature.

use crate::error::{self, InputError};

/// Brake heat parameters. The defaults are generic and labelled so in the UI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrakeHeatParams {
    /// Heat capacity of the brake, J/K.
    pub heat_capacity_j_per_k: f64,
    /// Cooling time constant toward ambient, s.
    pub cooling_tau_s: f64,
    /// Rise over ambient at which the model calls the brake too hot, K.
    pub limit_rise_k: f64,
    /// Energy the lining can dissipate in total, J.
    pub lining_life_j: f64,
}

impl BrakeHeatParams {
    /// A small spring brake on a 5.5 kW hoist motor: 25 kJ/K, 15 min, 250 K, 20 MJ.
    pub const GENERIC: Self = Self {
        heat_capacity_j_per_k: 25_000.0,
        cooling_tau_s: 900.0,
        limit_rise_k: 250.0,
        lining_life_j: 20.0e6,
    };

    pub fn validate(&self) -> Result<(), InputError> {
        error::positive("brake_heat_capacity_j_per_k", self.heat_capacity_j_per_k)?;
        error::positive("brake_cooling_tau_s", self.cooling_tau_s)?;
        error::positive("brake_limit_rise_k", self.limit_rise_k)?;
        error::positive("brake_lining_life_j", self.lining_life_j)?;
        Ok(())
    }
}

/// The brake's heat and wear state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrakeHeat {
    params: BrakeHeatParams,
    rise_k: f64,
    energy_j: f64,
}

impl BrakeHeat {
    pub fn new(params: BrakeHeatParams) -> Result<Self, InputError> {
        params.validate()?;
        Ok(Self {
            params,
            rise_k: 0.0,
            energy_j: 0.0,
        })
    }

    pub fn params(&self) -> &BrakeHeatParams {
        &self.params
    }

    /// Rise over ambient, K.
    pub fn rise_k(&self) -> f64 {
        self.rise_k
    }

    /// Energy dissipated so far, J.
    pub fn energy_j(&self) -> f64 {
        self.energy_j
    }

    /// Share of the lining life used (1 = worn out).
    pub fn wear_share(&self) -> f64 {
        self.energy_j / self.params.lining_life_j
    }

    pub fn too_hot(&self) -> bool {
        self.rise_k >= self.params.limit_rise_k
    }

    /// Advances by `dt_s` with the brake pressing with `torque_nm` on a shaft at `omega_rad_s`.
    /// The energy is added at once to the mass; the cooling is the exact solution of the first
    /// order law over the step.
    pub fn step(&mut self, torque_nm: f64, omega_rad_s: f64, dt_s: f64) {
        let e = (torque_nm * omega_rad_s).abs() * dt_s;
        self.energy_j += e;
        let decay = (-dt_s / self.params.cooling_tau_s).exp();
        self.rise_k = self.rise_k * decay + e / self.params.heat_capacity_j_per_k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heat() -> BrakeHeat {
        BrakeHeat::new(BrakeHeatParams::GENERIC).unwrap()
    }

    #[test]
    fn a_stop_from_speed_dissipates_the_torque_times_the_travel_angle() {
        // 80 N m against a shaft that slows from 100 rad/s to rest at constant deceleration
        // turns 100 x t / 2 rad: the energy is 80 x that angle.
        let mut b = heat();
        let (dt, t_stop) = (0.001, 0.5);
        let n = (t_stop / dt) as usize;
        for i in 0..n {
            let w = 100.0 * (1.0 - (i as f64 + 0.5) / n as f64);
            b.step(80.0, w, dt);
        }
        let expected = 80.0 * 100.0 * t_stop / 2.0;
        assert!((b.energy_j() - expected).abs() < 1.0, "{}", b.energy_j());
        assert!(b.rise_k() > 0.0);
    }

    #[test]
    fn it_cools_with_the_time_constant_when_nothing_slips() {
        let mut b = heat();
        b.step(80.0, 100.0, 1.0);
        let r0 = b.rise_k();
        for _ in 0..900 {
            b.step(80.0, 0.0, 1.0);
        }
        assert!((b.rise_k() / r0 - (-1.0f64).exp()).abs() < 1e-6);
        assert_eq!(b.energy_j(), 8000.0);
    }

    #[test]
    fn wear_is_the_share_of_the_lining_life() {
        let mut b = BrakeHeat::new(BrakeHeatParams {
            lining_life_j: 1000.0,
            ..BrakeHeatParams::GENERIC
        })
        .unwrap();
        b.step(10.0, 25.0, 1.0);
        assert!((b.wear_share() - 0.25).abs() < 1e-12);
    }

    #[test]
    fn bad_parameters_are_refused() {
        for p in [
            BrakeHeatParams {
                heat_capacity_j_per_k: 0.0,
                ..BrakeHeatParams::GENERIC
            },
            BrakeHeatParams {
                lining_life_j: -1.0,
                ..BrakeHeatParams::GENERIC
            },
        ] {
            assert!(BrakeHeat::new(p).is_err());
        }
    }
}
