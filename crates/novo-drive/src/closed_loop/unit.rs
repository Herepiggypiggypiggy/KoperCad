//! A complete closed-loop winch: drive, motor shaft, holding brake and hoist load, stepped as one
//! unit. For two winches on one load the mechanics are integrated by the caller (see
//! `novo-machine`), with [`ClosedLoopWinch::control_step`] and [`ClosedLoopWinch::shaft_mut`].

use super::controller::{
    ClInputs, ClOutputs, ClosedLoopConfig, ClosedLoopDrive, PreTorque, ShaftSense,
};
use crate::brake::{BrakeState, SpringBrake};
use crate::error::InputError;
use crate::mechanics::{Hoist, Shaft};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WinchSnapshot {
    pub hook_height_m: f64,
    pub hook_speed_m_s: f64,
    pub shaft_rpm: f64,
    pub drive: ClOutputs,
    pub brake_torque_nm: f64,
    pub brake_state: BrakeState,
    /// Torque the motor makes as a fraction of the weight torque on the shaft (0 at rest with
    /// the brake holding everything, 1 when the drive carries the whole load).
    pub drive_share: f64,
}

#[derive(Debug, Clone)]
pub struct ClosedLoopWinch {
    pub drive: ClosedLoopDrive,
    hoist: Hoist,
    brake: SpringBrake,
    shaft: Shaft,
    rotor_inertia_kg_m2: f64,
}

impl ClosedLoopWinch {
    pub fn new(
        cfg: ClosedLoopConfig,
        hoist: Hoist,
        brake: SpringBrake,
        rotor_inertia_kg_m2: f64,
    ) -> Result<Self, InputError> {
        crate::error::non_negative("rotor_inertia_kg_m2", rotor_inertia_kg_m2)?;
        let inertia = rotor_inertia_kg_m2 + hoist.reflected_inertia_kg_m2();
        Self::with_shaft_inertia(cfg, hoist, brake, rotor_inertia_kg_m2, inertia)
    }

    /// Like [`new`](Self::new) but with the total shaft inertia given, for a winch whose load
    /// is not the rigid hook mass of `hoist` (two winches on a rope-suspended beam: the load is
    /// integrated by the caller, `hoist` only supplies the rope geometry and the efficiency).
    pub fn with_shaft_inertia(
        cfg: ClosedLoopConfig,
        hoist: Hoist,
        brake: SpringBrake,
        rotor_inertia_kg_m2: f64,
        shaft_inertia_kg_m2: f64,
    ) -> Result<Self, InputError> {
        Ok(Self {
            drive: ClosedLoopDrive::new(cfg)?,
            hoist,
            brake,
            shaft: Shaft::new(shaft_inertia_kg_m2)?,
            rotor_inertia_kg_m2,
        })
    }

    pub fn hoist(&self) -> &Hoist {
        &self.hoist
    }

    pub fn brake(&self) -> &SpringBrake {
        &self.brake
    }

    pub fn shaft(&self) -> &Shaft {
        &self.shaft
    }

    pub fn shaft_mut(&mut self) -> &mut Shaft {
        &mut self.shaft
    }

    /// Motor rotor inertia in kg m2 (the load is added by the hoist).
    pub fn rotor_inertia_kg_m2(&self) -> f64 {
        self.rotor_inertia_kg_m2
    }

    /// The drive and the brake for one tick, without the mechanics. Returns the drive outputs
    /// and the brake torque available during the tick (the one at its start; the coil acts on
    /// the next tick).
    pub fn control_step(&mut self, inp: &ClInputs, dt_s: f64) -> (ClOutputs, f64) {
        let mut inp = *inp;
        if inp.load_cell_nm.is_none() && self.drive.cfg.pre_torque == PreTorque::LoadCell {
            inp.load_cell_nm = Some(self.hoist.static_torque_nm());
        }
        let out = self.drive.step(
            &inp,
            ShaftSense {
                angle_rad: self.shaft.angle_rad,
                omega_rad_s: self.shaft.omega_rad_s,
            },
            dt_s,
        );
        let brake_torque = self.brake.holding_torque_nm();
        self.brake
            .step(inp.brake_coil.unwrap_or(out.brake_release_cmd), dt_s);
        (out, brake_torque)
    }

    /// One tick with the hook mass hanging rigidly on the rope.
    pub fn step(&mut self, inp: &ClInputs, dt_s: f64) -> WinchSnapshot {
        let (out, brake_torque) = self.control_step(inp, dt_s);
        let weight = self.hoist.static_torque_nm();
        let drive_nm = out.torque_nm - weight;
        self.shaft.step(
            dt_s,
            drive_nm,
            self.hoist.friction_up_nm() + brake_torque,
            self.hoist.friction_down_nm() + brake_torque,
        );
        self.snapshot(out, brake_torque)
    }

    pub fn snapshot(&self, drive: ClOutputs, brake_torque_nm: f64) -> WinchSnapshot {
        WinchSnapshot {
            hook_height_m: self.shaft.angle_rad * self.hoist.metres_per_rad(),
            hook_speed_m_s: self.hoist.hook_speed_m_s(self.shaft.omega_rad_s),
            shaft_rpm: self.shaft.omega_rad_s * 60.0 / std::f64::consts::TAU,
            drive,
            brake_torque_nm,
            brake_state: self.brake.state(),
            drive_share: drive.torque_nm / self.hoist.static_torque_nm().max(1e-12),
        }
    }
}
