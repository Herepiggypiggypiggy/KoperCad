//! A complete contactor hoist: sequencer, pole-change motor, brake, gravity load and, for the
//! fine-lift families, a second motor (lanes L2a and L2d). This is what the co-simulation loop
//! (L5a) steps once per tick.
//!
//! Families covered by the same engine, chosen by what is plugged in:
//! * pole-change motor with a rectifier-fed spring brake (the default; the rectifier kind and
//!   its faults come from [`crate::brake_system`]);
//! * pole-change conical-rotor motor, where the brake is lifted by the motor's own field;
//! * either of those plus a fine-lift motor on the same drum ([`FineLift`]).
//!
//! The supply is part of the input ([`Feeds::supply`]) and of the faults the user can inject
//! ([`HoistFaults`]): low voltage, a lost phase, a worn brake, a rectifier fault.

use crate::brake::{BrakeState, SpringBrake};
use crate::brake_system::{
    BrakeInput, BrakeSource, BrakeSystem, Rectifier, RectifierFault, RectifierKind,
};
use crate::error::InputError;
use crate::mechanics::{Hoist, Shaft};
use crate::motor::{Direction, PoleChangeMotor, Winding, WindingSel};
use crate::sequencer::{Command, Energised, Sequencer};
use crate::supply::{self, Phase, Supply};

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

/// Everything the circuit tells the hoist for one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feeds {
    /// The pole-change motor: winding and direction, `None` when no winding contactor is closed.
    pub main: Option<Energised>,
    /// The fine-lift motor, if the hoist has one: the direction it is fed in.
    pub fine_lift: Option<Direction>,
    /// Main contactor closed. False drops everything.
    pub mains_on: bool,
    /// Used only with [`BrakeFeed::Separate`].
    pub brake_coil_fed: bool,
    /// What the circuit says about the terminals (a phase open in the drawing).
    pub supply: Supply,
    /// The drawing has the motor humming on two phases: it is fed, but the circuit could not
    /// say which winding or which way. The last winding and direction are kept.
    pub humming: bool,
}

impl Feeds {
    /// The simple case: only the main motor, healthy supply.
    pub fn simple(main: Option<Energised>, mains_on: bool, brake_coil_fed: bool) -> Self {
        Self {
            main,
            fine_lift: None,
            mains_on,
            brake_coil_fed,
            supply: Supply::HEALTHY,
            humming: false,
        }
    }
}

/// Faults and wear the user can put on a hoist to see what the circuit and the mechanics do.
/// All off by default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoistFaults {
    /// Line voltage over rated, on top of what the circuit says.
    pub voltage_ratio: f64,
    /// A phase open, on top of what the circuit says.
    pub lost_phase: Option<Phase>,
    /// Brake delays take this many times as long.
    pub brake_delay_scale: f64,
    /// Brake holding torque is this share of rated (worn lining).
    pub brake_torque_scale: f64,
    /// Rectifier fault, for a rectifier brake.
    pub rectifier: RectifierFault,
}

impl HoistFaults {
    pub const NONE: Self = Self {
        voltage_ratio: 1.0,
        lost_phase: None,
        brake_delay_scale: 1.0,
        brake_torque_scale: 1.0,
        rectifier: RectifierFault::None,
    };

    pub fn is_none(&self) -> bool {
        *self == Self::NONE
    }
}

impl Default for HoistFaults {
    fn default() -> Self {
        Self::NONE
    }
}

/// Things the user should look at, derived each tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Conditions {
    /// A motor has been fed for a while and its brake has not opened.
    pub fed_brake_closed: bool,
    /// The load is sliding down through a closed brake.
    pub brake_slipping: bool,
    /// A phase is open (drawing or injected).
    pub phase_lost: bool,
    /// Supply below 90 % of rated.
    pub low_voltage: bool,
    /// The main motor and the fine-lift motor are fed at the same time.
    pub both_motors_fed: bool,
}

impl Conditions {
    pub fn any(&self) -> bool {
        !self.messages().is_empty()
    }

    /// Plain-language lines for the window.
    pub fn messages(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.fed_brake_closed {
            out.push("Motor is fed but the brake stays closed: the motor pushes against it");
        }
        if self.brake_slipping {
            out.push("The load slides down through the closed brake (too little brake torque)");
        }
        if self.phase_lost {
            out.push("A phase is open");
        }
        if self.low_voltage {
            out.push("Supply voltage is low (below 90 % of rated)");
        }
        if self.both_motors_fed {
            out.push("Main motor and fine-lift motor are fed together: they fight each other");
        }
        out
    }
}

/// Seconds a fed motor may wait for its brake before it counts as a condition.
pub const FED_BRAKE_CLOSED_AFTER_S: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoistSnapshot {
    pub hook_height_m: f64,
    pub hook_speed_m_s: f64,
    pub shaft_rpm: f64,
    pub motor_torque_nm: f64,
    /// Brake torque on the main motor's shaft in this tick (start of tick).
    pub brake_torque_nm: f64,
    pub brake_state: BrakeState,
    pub energised: Option<Energised>,
    /// Share of the rated excitation on the main brake in this tick.
    pub brake_excitation: f64,
    /// The fine-lift motor is coupled (fed) in this tick.
    pub fine_engaged: bool,
    /// Fine-lift motor torque on the main shaft, N m (0 without one).
    pub fine_torque_nm: f64,
    pub fine_brake_state: Option<BrakeState>,
    /// The supply as the motors saw it.
    pub supply: Supply,
    pub conditions: Conditions,
}

/// A second, small motor on the same drum for slow, fine positioning (for example the fine-lift
/// motor of a rope hoist). A model: the motor is coupled to the main shaft through a fixed
/// speed ratio while it is fed and uncoupled (no torque, no inertia, its brake irrelevant)
/// while it is not. Real gear arrangements differ and are not modelled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FineLift {
    winding: Winding,
    /// Fine motor speed over main shaft speed (greater than 1: the fine motor turns faster).
    ratio: f64,
    rotor_inertia_kg_m2: f64,
    brake: BrakeSystem,
    releases_main_brake: bool,
}

impl FineLift {
    /// `releases_main_brake`: feeding the fine-lift motor also lifts the main brake (otherwise
    /// the main brake holds the drum and the fine motor cannot move it; the user states which
    /// is true for the hoist).
    pub fn new(
        winding: Winding,
        ratio: f64,
        rotor_inertia_kg_m2: f64,
        brake: BrakeSystem,
        releases_main_brake: bool,
    ) -> Result<Self, InputError> {
        crate::error::positive("fine-lift ratio", ratio)?;
        crate::error::non_negative("fine-lift rotor_inertia_kg_m2", rotor_inertia_kg_m2)?;
        Ok(Self {
            winding,
            ratio,
            rotor_inertia_kg_m2,
            brake,
            releases_main_brake,
        })
    }

    pub fn winding(&self) -> &Winding {
        &self.winding
    }

    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    pub fn releases_main_brake(&self) -> bool {
        self.releases_main_brake
    }

    pub fn brake(&self) -> &BrakeSystem {
        &self.brake
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoSpeedHoistUnit {
    motor: PoleChangeMotor,
    hoist: Hoist,
    sequencer: Sequencer,
    brake: BrakeSystem,
    brake_feed: BrakeFeed,
    shaft: Shaft,
    base_inertia: f64,
    fine: Option<FineLift>,
    faults: HoistFaults,
    /// What was fed on the last step (from the sequencer or from the circuit).
    last_energised: Option<Energised>,
    /// How long the main motor or the fine motor has been fed with its brake closed.
    fed_closed_s: f64,
}

impl TwoSpeedHoistUnit {
    /// A pole-change motor with a bridge-rectified spring brake (the original configuration).
    pub fn new(
        motor: PoleChangeMotor,
        hoist: Hoist,
        sequencer: Sequencer,
        brake: SpringBrake,
        brake_feed: BrakeFeed,
    ) -> Result<Self, InputError> {
        // A plain bridge with no fault gives full excitation whenever it is fed.
        let rect = Rectifier::new(RectifierKind::Bridge)?;
        Self::with_system(
            motor,
            hoist,
            sequencer,
            BrakeSystem::new(brake, BrakeSource::Rectifier(rect)),
            brake_feed,
        )
    }

    /// Like [`new`](Self::new) with any brake system (conical or a chosen rectifier).
    pub fn with_system(
        motor: PoleChangeMotor,
        hoist: Hoist,
        sequencer: Sequencer,
        brake: BrakeSystem,
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
            base_inertia: inertia,
            fine: None,
            faults: HoistFaults::NONE,
            last_energised: None,
            fed_closed_s: 0.0,
        })
    }

    /// Adds a fine-lift motor.
    pub fn with_fine_lift(mut self, fine: FineLift) -> Self {
        self.fine = Some(fine);
        self
    }

    pub fn fine_lift(&self) -> Option<&FineLift> {
        self.fine.as_ref()
    }

    pub fn shaft(&self) -> &Shaft {
        &self.shaft
    }

    pub fn brake(&self) -> &SpringBrake {
        &self.brake.brake
    }

    pub fn brake_system(&self) -> &BrakeSystem {
        &self.brake
    }

    pub fn faults(&self) -> &HoistFaults {
        &self.faults
    }

    /// Puts faults on the hoist (replacing the previous ones). Takes effect from the next tick.
    pub fn set_faults(&mut self, faults: HoistFaults) -> Result<(), InputError> {
        // Validate the numbers first so a refused set leaves the unit unchanged.
        Supply::new(faults.voltage_ratio, faults.lost_phase)?;
        let mut main = self.brake;
        main.brake
            .set_wear(faults.brake_delay_scale, faults.brake_torque_scale)?;
        if let BrakeSource::Rectifier(r) = main.source_mut() {
            r.set_fault(faults.rectifier)?;
        }
        let mut fine = self.fine;
        if let Some(f) = fine.as_mut() {
            f.brake
                .brake
                .set_wear(faults.brake_delay_scale, faults.brake_torque_scale)?;
            if let BrakeSource::Rectifier(r) = f.brake.source_mut() {
                r.set_fault(faults.rectifier)?;
            }
        }
        self.brake = main;
        self.fine = fine;
        self.faults = faults;
        Ok(())
    }

    pub fn step(&mut self, input: HoistInput, dt_s: f64) -> HoistSnapshot {
        let command = if input.mains_on {
            input.command
        } else {
            Command::OFF
        };
        let energised = self.sequencer.step(command, dt_s);
        self.step_energised(energised, input.mains_on, input.brake_coil_fed, dt_s)
    }

    /// Like [`step`](Self::step) but the winding and direction come from the circuit: the
    /// schematic's contactors decide what is fed, so the internal sequencer is bypassed
    /// (the L5a loop uses this). `energised` is `None` when no winding contactor is closed.
    pub fn step_energised(
        &mut self,
        energised: Option<Energised>,
        mains_on: bool,
        brake_coil_fed: bool,
        dt_s: f64,
    ) -> HoistSnapshot {
        self.step_feeds(Feeds::simple(energised, mains_on, brake_coil_fed), dt_s)
    }

    /// One tick with everything the circuit says.
    pub fn step_feeds(&mut self, feeds: Feeds, dt_s: f64) -> HoistSnapshot {
        let mains_on = feeds.mains_on;
        let mut main = if mains_on { feeds.main } else { None };
        let fine_dir = if mains_on && self.fine.is_some() {
            feeds.fine_lift
        } else {
            None
        };
        // Two phases from the drawing: fed, humming, same winding and way as before.
        if mains_on && feeds.humming && main.is_none() && fine_dir.is_none() {
            main = Some(self.last_energised.unwrap_or(Energised {
                winding: WindingSel::Fine,
                direction: Direction::Positive,
            }));
        }
        self.last_energised = main;

        // The supply the motors see: the circuit's and the injected fault, combined.
        let lost = self.faults.lost_phase.or(feeds.supply.lost_phase());
        let ratio = (self.faults.voltage_ratio * feeds.supply.voltage_ratio()).min(1.5);
        let supply = Supply::new(ratio, lost).unwrap_or(Supply::HEALTHY);

        let fine_fed = fine_dir.is_some();
        let fine_forces_main = fine_fed && self.fine.is_some_and(|f| f.releases_main_brake);
        let rectifier_fed = match self.brake_feed {
            BrakeFeed::MotorTerminals => main.is_some() || fine_forces_main,
            BrakeFeed::Separate => mains_on && feeds.brake_coil_fed,
        };

        // Brake torques of this tick are the ones at its start; the coil acts on the next tick.
        let main_brake_torque = self.brake.holding_torque_nm();
        let fine_brake_torque = match &self.fine {
            Some(f) if fine_fed => f.brake.holding_torque_nm() * f.ratio,
            _ => 0.0,
        };
        self.brake.step(
            BrakeInput {
                rectifier_fed,
                winding: main
                    .map(|e| e.winding)
                    .or(fine_forces_main.then_some(WindingSel::Coarse)),
                supply,
            },
            dt_s,
        );
        if let Some(f) = self.fine.as_mut() {
            f.brake.step(
                BrakeInput {
                    rectifier_fed: fine_fed,
                    winding: fine_fed.then_some(WindingSel::Coarse),
                    supply,
                },
                dt_s,
            );
        }

        // Couple or uncouple the fine rotor.
        let inertia = self.base_inertia
            + match &self.fine {
                Some(f) if fine_fed => f.rotor_inertia_kg_m2 * f.ratio * f.ratio,
                _ => 0.0,
            };
        if (inertia - self.shaft.inertia_kg_m2()).abs() > 0.0 {
            // `inertia` is a sum of positive numbers; the error case cannot occur.
            let _ = self.shaft.set_inertia(inertia);
        }

        let omega = self.shaft.omega_rad_s;
        let main_torque = main.map_or(0.0, |e| {
            supply::torque_nm(self.motor.winding(e.winding), e.direction, omega, &supply)
        });
        let fine_torque = match (&self.fine, fine_dir) {
            (Some(f), Some(d)) => {
                supply::torque_nm(&f.winding, d, omega * f.ratio, &supply) * f.ratio
            }
            _ => 0.0,
        };
        let drive = main_torque + fine_torque - self.hoist.static_torque_nm();
        let resist = main_brake_torque + fine_brake_torque;
        self.shaft.step(
            dt_s,
            drive,
            self.hoist.friction_up_nm() + resist,
            self.hoist.friction_down_nm() + resist,
        );

        // Conditions.
        let fed = main.is_some() || fine_fed;
        let closed = self.brake.state() == BrakeState::Applied
            || (fine_fed
                && self
                    .fine
                    .is_some_and(|f| f.brake.state() == BrakeState::Applied));
        if fed && closed && self.shaft.omega_rad_s.abs() < 1e-3 {
            self.fed_closed_s += dt_s;
        } else {
            self.fed_closed_s = 0.0;
        }
        let conditions = Conditions {
            fed_brake_closed: self.fed_closed_s >= FED_BRAKE_CLOSED_AFTER_S,
            brake_slipping: !fed
                && self.shaft.omega_rad_s < -1e-3
                && self.brake.state() == BrakeState::Applied,
            phase_lost: supply.phase_lost(),
            low_voltage: supply.voltage_ratio() < 0.9,
            both_motors_fed: main.is_some() && fine_fed,
        };

        HoistSnapshot {
            hook_height_m: self.shaft.angle_rad * self.hoist.metres_per_rad(),
            hook_speed_m_s: self.hoist.hook_speed_m_s(self.shaft.omega_rad_s),
            shaft_rpm: self.shaft.omega_rad_s * 60.0 / std::f64::consts::TAU,
            motor_torque_nm: main_torque,
            brake_torque_nm: main_brake_torque,
            brake_state: self.brake.state(),
            energised: main,
            brake_excitation: self.brake.excitation(),
            fine_engaged: fine_fed,
            fine_torque_nm: fine_torque,
            fine_brake_state: self.fine.as_ref().map(|f| f.brake.state()),
            supply,
            conditions,
        }
    }

    /// Which winding was fed on the last step, whether the internal sequencer or the circuit
    /// decided it.
    pub fn winding(&self) -> Option<WindingSel> {
        self.last_energised.map(|e| e.winding)
    }

    /// Where the brake rectifier gets its supply.
    pub fn brake_feed(&self) -> BrakeFeed {
        self.brake_feed
    }
}
