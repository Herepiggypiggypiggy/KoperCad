//! What a motor on a V/f drive can do at each output frequency: the capability table that the
//! app shows and the drive's torque limit can read.

use super::circuit::{InductionMotor, SelfVentilation, Supply, TorqueLimitedBy};
use crate::drive::VfCurve;
use crate::error::{self, InputError};
use crate::motor::Direction;

/// Region of the V/f characteristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// Below the base frequency: voltage follows the V/f law, flux is about constant.
    ConstantFlux,
    /// Above the base frequency: voltage is at its limit and the flux falls as `1/f`.
    FieldWeakening,
}

/// One row of the capability table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrequencyRow {
    pub frequency_hz: f64,
    /// Line voltage from the V/f law, V.
    pub volts_ll: f64,
    pub region: Region,
    pub sync_rpm: f64,
    /// Air-gap breakdown torque (motoring), N m.
    pub breakdown_nm: f64,
    pub breakdown_slip: f64,
    /// Whether the breakdown lies at a slip of at most 1 (otherwise the standstill torque is
    /// the most the motor can give).
    pub breakdown_reachable: bool,
    /// Largest torque within the drive's current limit, N m (air gap).
    pub current_limited_nm: f64,
    pub limited_by: TorqueLimitedBy,
    /// Speed at that torque, rpm.
    pub speed_at_limit_rpm: f64,
    /// Shaft power at that torque, W.
    pub power_at_limit_w: f64,
    /// Air-gap flux per unit of rated at that torque.
    pub flux_pu: f64,
    /// Speed at rated shaft torque on this supply (steady state), rpm; `None` when rated torque
    /// is not reachable at this frequency.
    pub rated_torque_rpm: Option<f64>,
    /// Stator current at rated shaft torque, A.
    pub rated_torque_current_a: Option<f64>,
    /// Continuous torque of a self-ventilated motor at the speed of rated torque, N m.
    pub continuous_nm: f64,
}

impl InductionMotor {
    /// The capability at one output frequency under a V/f law and a current limit (model).
    pub fn frequency_row(
        &self,
        vf: &VfCurve,
        frequency_hz: f64,
        current_limit_a: f64,
        cooling: &SelfVentilation,
    ) -> Result<FrequencyRow, InputError> {
        error::non_negative("frequency_hz", frequency_hz)?;
        error::positive("current_limit_a", current_limit_a)?;
        let volts = vf.volts_for_frequency(frequency_hz);
        let supply = Supply::new(frequency_hz, volts, Direction::Positive)?;
        let b = self.breakdown(&supply);
        let lim = self.torque_at_current_limit(&supply, current_limit_a, false)?;
        let rated = self.steady_state(&supply, self.rated_torque_nm())?;
        let speed_for_cooling = rated.map_or(lim.point.speed_rpm, |p| p.speed_rpm);
        Ok(FrequencyRow {
            frequency_hz,
            volts_ll: volts,
            region: if frequency_hz > vf.base_hz {
                Region::FieldWeakening
            } else {
                Region::ConstantFlux
            },
            sync_rpm: self.sync_rpm_at(frequency_hz),
            breakdown_nm: b.motoring_nm,
            breakdown_slip: b.motoring_slip,
            breakdown_reachable: b.motoring_reachable,
            current_limited_nm: lim.torque_nm,
            limited_by: lim.limited_by,
            speed_at_limit_rpm: lim.point.speed_rpm,
            power_at_limit_w: lim.point.shaft_power_w,
            flux_pu: lim.point.flux_pu,
            rated_torque_rpm: rated.map(|p| p.speed_rpm),
            rated_torque_current_a: rated.map(|p| p.stator_current_a),
            continuous_nm: self.continuous_torque_nm(speed_for_cooling, cooling),
        })
    }
}
