//! Shaft mechanics: inertia, hoist gravity and losses, stiction (lane L2a).
//!
//! Sign convention: positive shaft speed raises the hook. Gravity always pulls the hook down,
//! so the gravity torque on the shaft is negative.
//!
//! Efficiency: `eta` is the drivetrain efficiency (gearbox, drum, ropes) and acts as a
//! direction-dependent loss torque. At constant speed the motor delivers `m g r / (i eta)`
//! when raising and `m g r eta / i` when lowering (it still holds back the load). At
//! standstill no efficiency applies to the weight itself: the static holding torque is
//! `m g r / i`; the losses only decide whether the load starts to move.

use crate::error::{self, InputError};
use crate::{Fidelity, GRAVITY};

/// Fidelity of the gravity and efficiency arithmetic: exact for the stated efficiency, which
/// itself is a model (one constant per direction, no speed or load dependence).
pub const HOIST_FIDELITY: Fidelity = Fidelity::Model;

fn check_hoist(
    mass_kg: f64,
    drum_radius_m: f64,
    ratio: f64,
    efficiency: f64,
) -> Result<(), InputError> {
    error::positive("mass_kg", mass_kg)?;
    error::positive("drum_radius_m", drum_radius_m)?;
    error::positive("ratio", ratio)?;
    error::fraction("efficiency", efficiency)?;
    Ok(())
}

/// Motor-shaft torque in N m to raise a mass at constant speed: `m g r / (i eta)`.
/// (Not a static holding torque: see [`hoist_static_torque_nm`].)
pub fn hoist_raising_torque_nm(
    mass_kg: f64,
    drum_radius_m: f64,
    ratio: f64,
    efficiency: f64,
) -> Result<f64, InputError> {
    check_hoist(mass_kg, drum_radius_m, ratio, efficiency)?;
    Ok(mass_kg * GRAVITY * drum_radius_m / (ratio * efficiency))
}

/// Motor-shaft torque in N m that holds back the load when lowering at constant speed:
/// `m g r eta / i`.
pub fn hoist_lowering_torque_nm(
    mass_kg: f64,
    drum_radius_m: f64,
    ratio: f64,
    efficiency: f64,
) -> Result<f64, InputError> {
    check_hoist(mass_kg, drum_radius_m, ratio, efficiency)?;
    Ok(mass_kg * GRAVITY * drum_radius_m * efficiency / ratio)
}

/// Shaft torque in N m the weight exerts at standstill: `m g r / i`. This is the figure a
/// holding brake is checked against (no help from friction).
pub fn hoist_static_torque_nm(
    mass_kg: f64,
    drum_radius_m: f64,
    ratio: f64,
) -> Result<f64, InputError> {
    check_hoist(mass_kg, drum_radius_m, ratio, 1.0)?;
    Ok(mass_kg * GRAVITY * drum_radius_m / ratio)
}

/// Rope drum, gearbox and hook load as seen from the motor shaft.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hoist {
    mass_kg: f64,
    drum_radius_m: f64,
    ratio: f64,
    falls: u32,
    efficiency: f64,
}

impl Hoist {
    /// `mass_kg` is payload plus hook block, `ratio` the gearbox ratio motor to drum (`i`),
    /// `falls` the number of rope parts carrying the hook (1 = direct).
    pub fn new(
        mass_kg: f64,
        drum_radius_m: f64,
        ratio: f64,
        falls: u32,
        efficiency: f64,
    ) -> Result<Self, InputError> {
        check_hoist(mass_kg, drum_radius_m, ratio, efficiency)?;
        if falls == 0 {
            return Err(InputError::NotPositive {
                field: "falls",
                value: 0.0,
            });
        }
        Ok(Self {
            mass_kg,
            drum_radius_m,
            ratio,
            falls,
            efficiency,
        })
    }

    pub fn mass_kg(&self) -> f64 {
        self.mass_kg
    }

    /// Hook travel in metres per radian of motor shaft: `r / (falls i)`.
    pub fn metres_per_rad(&self) -> f64 {
        self.drum_radius_m / (f64::from(self.falls) * self.ratio)
    }

    /// Hook speed in m/s at shaft speed `omega_rad_s` (positive raises).
    pub fn hook_speed_m_s(&self, omega_rad_s: f64) -> f64 {
        omega_rad_s * self.metres_per_rad()
    }

    /// Magnitude of the weight torque on the motor shaft: `m g r / (falls i)`.
    pub fn static_torque_nm(&self) -> f64 {
        self.mass_kg * GRAVITY * self.metres_per_rad()
    }

    pub fn raising_torque_nm(&self) -> f64 {
        self.static_torque_nm() / self.efficiency
    }

    pub fn lowering_torque_nm(&self) -> f64 {
        self.static_torque_nm() * self.efficiency
    }

    /// Loss torque opposing upward motion: `T_static (1/eta - 1)`.
    pub fn friction_up_nm(&self) -> f64 {
        self.static_torque_nm() * (1.0 / self.efficiency - 1.0)
    }

    /// Loss torque opposing downward motion: `T_static (1 - eta)`.
    pub fn friction_down_nm(&self) -> f64 {
        self.static_torque_nm() * (1.0 - self.efficiency)
    }

    /// Hook mass as inertia on the motor shaft: `m (r / (falls i))^2`.
    pub fn reflected_inertia_kg_m2(&self) -> f64 {
        self.mass_kg * self.metres_per_rad().powi(2)
    }
}

/// Rotating shaft with the stiction logic of a holding brake and direction-dependent losses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shaft {
    inertia_kg_m2: f64,
    pub omega_rad_s: f64,
    pub angle_rad: f64,
}

impl Shaft {
    pub fn new(inertia_kg_m2: f64) -> Result<Self, InputError> {
        error::positive("inertia_kg_m2", inertia_kg_m2)?;
        Ok(Self {
            inertia_kg_m2,
            omega_rad_s: 0.0,
            angle_rad: 0.0,
        })
    }

    pub fn inertia_kg_m2(&self) -> f64 {
        self.inertia_kg_m2
    }

    /// Changes the inertia, for a rotor that is coupled or uncoupled while the shaft turns.
    /// The speed is kept (the added rotor is taken to be at shaft speed).
    pub fn set_inertia(&mut self, inertia_kg_m2: f64) -> Result<(), InputError> {
        error::positive("inertia_kg_m2", inertia_kg_m2)?;
        self.inertia_kg_m2 = inertia_kg_m2;
        Ok(())
    }

    /// Advance by `dt_s` under `drive_nm` (motor plus weight, signed). `resist_pos_nm` opposes
    /// motion in the positive sense and `resist_neg_nm` in the negative sense (losses plus
    /// brake torque, both not negative). At rest the shaft stays put while the drive torque is
    /// inside `-resist_neg ..= resist_pos`. Constant torque gives the exact kinematic
    /// result (trapezoid position update).
    pub fn step(&mut self, dt_s: f64, drive_nm: f64, resist_pos_nm: f64, resist_neg_nm: f64) {
        let omega = self.omega_rad_s;
        let (net, sign) = if omega > 0.0 || (omega == 0.0 && drive_nm > resist_pos_nm) {
            (drive_nm - resist_pos_nm, 1.0)
        } else if omega < 0.0 || (omega == 0.0 && drive_nm < -resist_neg_nm) {
            (drive_nm + resist_neg_nm, -1.0)
        } else {
            return;
        };
        let mut next = omega + net / self.inertia_kg_m2 * dt_s;
        if next * sign < 0.0 {
            // The resisting torque stopped the shaft inside this step.
            next = 0.0;
        }
        self.angle_rad += 0.5 * (omega + next) * dt_s;
        self.omega_rad_s = next;
    }
}
