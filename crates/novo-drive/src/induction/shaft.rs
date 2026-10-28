//! Shaft dynamics: the motor's quasi-static torque against inertia and a load.

use std::f64::consts::TAU;

use super::circuit::{InductionMotor, OperatingPoint, Supply};
use crate::error::{self, InputError};

/// Most substeps one call may take.
const MAX_SUBSTEPS: usize = 20_000;

/// Longest substep in seconds.
const MAX_SUBSTEP_S: f64 = 0.02;

/// Result of one [`InductionMotor::step_shaft`] call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShaftStep {
    /// Operating point at the end of the step.
    pub point: OperatingPoint,
    /// Electrical energy taken from the supply over the step, J (negative when fed back).
    pub electrical_energy_j: f64,
    /// Copper, core, friction and windage losses over the step, J.
    pub loss_energy_j: f64,
    /// Work the load did on the shaft over the step, J (negative when the motor drives it).
    pub load_work_j: f64,
    pub substeps: u32,
}

impl InductionMotor {
    /// Advances the shaft speed `omega_rad_s` by `dt_s` under a fixed supply:
    /// `J dw/dt = T_shaft(w) + T_load(w)`. `load_nm(w)` is the torque the load applies to the
    /// shaft, positive in the positive shaft sense (a hoist hanging load is negative when
    /// raising). The torque follows the steady-state curve at once (no flux transient);
    /// substeps are chosen from the electromechanical time constant `J / |dT/dw|` and
    /// integrated with Heun's method. Model.
    pub fn step_shaft(
        &self,
        omega_rad_s: &mut f64,
        supply: &Supply,
        inertia_kg_m2: f64,
        dt_s: f64,
        mut load_nm: impl FnMut(f64) -> f64,
    ) -> Result<ShaftStep, InputError> {
        error::finite("omega_rad_s", *omega_rad_s)?;
        error::positive("inertia_kg_m2", inertia_kg_m2)?;
        error::non_negative("dt_s", dt_s)?;
        let mut point = self.evaluate(supply, *omega_rad_s)?;
        let mut out = ShaftStep {
            point,
            electrical_energy_j: 0.0,
            loss_energy_j: 0.0,
            load_work_j: 0.0,
            substeps: 0,
        };
        if dt_s == 0.0 {
            return Ok(out);
        }
        let dw = 0.01 * TAU * self.rated_rpm() / 60.0;
        let mut w = *omega_rad_s;
        let mut remaining = dt_s;
        while remaining > 0.0 && (out.substeps as usize) < MAX_SUBSTEPS {
            // Stiffness of the motor torque (N m per rad/s) from a small speed difference, so the
            // substep follows the electromechanical time constant `J / |dT/dw|` wherever the
            // speed is now.
            let t_hi = self.evaluate(supply, w + dw)?.torque_nm;
            let t_lo = self.evaluate(supply, w - dw)?.torque_nm;
            let stiffness = ((t_hi - t_lo) / (2.0 * dw)).abs().max(1e-9);
            let h = if out.substeps as usize + 1 >= MAX_SUBSTEPS {
                remaining
            } else {
                remaining
                    .min(0.1 * inertia_kg_m2 / stiffness)
                    .min(MAX_SUBSTEP_S)
            };
            let a0 = (point.torque_nm + load_nm(w)) / inertia_kg_m2;
            let w_pred = w + h * a0;
            let p1 = self.evaluate(supply, w_pred)?;
            let a1 = (p1.torque_nm + load_nm(w_pred)) / inertia_kg_m2;
            let w_next = w + 0.5 * h * (a0 + a1);
            let p2 = self.evaluate(supply, w_next)?;
            out.electrical_energy_j += 0.5 * (point.input_power_w + p2.input_power_w) * h;
            out.loss_energy_j += 0.5 * (point.losses_w() + p2.losses_w()) * h;
            out.load_work_j += 0.5 * (load_nm(w) * w + load_nm(w_next) * w_next) * h;
            w = w_next;
            point = p2;
            remaining -= h;
            out.substeps += 1;
        }
        *omega_rad_s = w;
        out.point = point;
        Ok(out)
    }
}
