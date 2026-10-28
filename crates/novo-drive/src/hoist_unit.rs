//! A complete two-speed contactor hoist: sequencer, pole-change motor, rectifier-fed brake,
//! gravity load (lane L2a). This is what the co-simulation loop (L5a) steps once per tick.

use crate::brake::{BrakeState, SpringBrake};
use crate::error::InputError;
use crate::mechanics::{Hoist, Shaft};
use crate::motor::{PoleChangeMotor, WindingSel};
use crate::sequencer::{Command, Energised, Sequencer};

/// Where the brake rectifier gets its supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrakeFeed {
    /// From the motor terminals: the coil is fed whenever either winding is fed.
    MotorTerminals,
    /// From a separate contact: the schematic decides ([`HoistInput::brake_coil_fed`]).
    Separate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoistInput {
    pub command: Command,
    /// Main contactor closed (E-stop chain healthy). False drops everything.
    pub mains_on: bool,
    /// Used only with [`BrakeFeed::Separate`].
    pub brake_coil_fed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoistSnapshot {
    pub hook_height_m: f64,
    pub hook_speed_m_s: f64,
    pub shaft_rpm: f64,
    pub motor_torque_nm: f64,
    pub brake_torque_nm: f64,
    pub brake_state: BrakeState,
    pub energised: Option<Energised>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoSpeedHoistUnit {
    motor: PoleChangeMotor,
    hoist: Hoist,
    sequencer: Sequencer,
    brake: SpringBrake,
    brake_feed: BrakeFeed,
    shaft: Shaft,
}

impl TwoSpeedHoistUnit {
    pub fn new(
        motor: PoleChangeMotor,
        hoist: Hoist,
        sequencer: Sequencer,
        brake: SpringBrake,
        brake_feed: BrakeFeed,
    ) -> Result<Self, InputError> {
        let inertia = motor.rotor_inertia_kg_m2() + hoist.reflected_inertia_kg_m2();
        Ok(Self {
            motor,
            hoist,
            sequencer,
            brake,
            brake_feed,
            shaft: Shaft::new(inertia)?,
        })
    }

    pub fn shaft(&self) -> &Shaft {
        &self.shaft
    }

    pub fn brake(&self) -> &SpringBrake {
        &self.brake
    }

    pub fn step(&mut self, input: HoistInput, dt_s: f64) -> HoistSnapshot {
        let command = if input.mains_on {
            input.command
        } else {
            Command::OFF
        };
        let energised = self.sequencer.step(command, dt_s);
        let coil_fed = match self.brake_feed {
            BrakeFeed::MotorTerminals => energised.is_some(),
            BrakeFeed::Separate => input.mains_on && input.brake_coil_fed,
        };
        // The brake torque of this tick is the one at its start; the coil acts on the next tick.
        let brake_torque = self.brake.holding_torque_nm();
        self.brake.step(coil_fed, dt_s);

        let motor_torque = energised.map_or(0.0, |e| {
            self.motor
                .torque_nm(e.winding, e.direction, self.shaft.omega_rad_s)
        });
        let drive = motor_torque - self.hoist.static_torque_nm();
        self.shaft.step(
            dt_s,
            drive,
            self.hoist.friction_up_nm() + brake_torque,
            self.hoist.friction_down_nm() + brake_torque,
        );
        HoistSnapshot {
            hook_height_m: self.shaft.angle_rad * self.hoist.metres_per_rad(),
            hook_speed_m_s: self.hoist.hook_speed_m_s(self.shaft.omega_rad_s),
            shaft_rpm: self.shaft.omega_rad_s * 60.0 / std::f64::consts::TAU,
            motor_torque_nm: motor_torque,
            brake_torque_nm: brake_torque,
            brake_state: self.brake.state(),
            energised,
        }
    }

    /// Which winding is fed now.
    pub fn winding(&self) -> Option<WindingSel> {
        self.sequencer.energised().map(|e| e.winding)
    }
}
