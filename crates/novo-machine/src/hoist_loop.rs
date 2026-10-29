//! The fixed-step loop over hoist modules (lane L5a).
//!
//! One tick, in this order, always the same, so a run is deterministic:
//! 1. limit switches are written from the hook heights of the previous tick,
//! 2. the circuit settles by one step,
//! 3. each hoist reads what the circuit feeds its motor and brake coil,
//! 4. the engine of each hoist advances by the step.
//!
//! Exact by definition: the order of the tick, the sensor rule (a limit contact is closed
//! below its entered height and open at or above it, for an upper limit). Model: everything
//! inside `novo_drive` (see `novo_drive::motor::MOTOR_MODEL_LEAVES_OUT`).

use novo_drive::InputError;
use novo_drive::hoist_unit::{BrakeFeed, Feeds, HoistSnapshot, TwoSpeedHoistUnit};
use novo_drive::sequencer::Energised;
use novo_drive::supply::{Phase, Supply};

use crate::bindings::SensorBinding;
use crate::cosim::StepConfig;
use crate::port::{CircuitPort, MotorFeed};

/// Which way a limit switch protects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    /// Opens when the hook reaches or passes the height going up.
    Upper,
    /// Opens when the hook reaches or passes the height going down.
    Lower,
}

/// A limit switch bound to a schematic device. The contact is normally closed (it conducts
/// until the limit is reached); the circuit decides what that cuts, for example only the
/// coarse contactor chain.
#[derive(Debug, Clone, PartialEq)]
pub struct LimitSwitch {
    pub binding: SensorBinding,
    pub kind: LimitKind,
}

impl LimitSwitch {
    /// Conducting state at a hook height in metres (`binding.actuate_at_mm` is the limit).
    pub fn closed_at(&self, height_m: f64) -> bool {
        let at_m = self.binding.actuate_at_mm / 1000.0;
        match self.kind {
            LimitKind::Upper => height_m < at_m,
            LimitKind::Lower => height_m > at_m,
        }
    }
}

/// Where one hoist is attached to the schematic.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HoistBinding {
    /// Device tag of the motor symbol.
    pub motor: String,
    /// Device tag of the fine-lift motor symbol, for a hoist with a second motor.
    pub fine_motor: Option<String>,
    /// Device tag of the overload relay contact: normally closed, opens when the relay trips.
    pub overload_contact: Option<String>,
    /// Device tag whose energised state feeds the brake coil (used with
    /// `BrakeFeed::Separate`; ignored with `BrakeFeed::MotorTerminals`).
    pub brake_feed: Option<String>,
    /// Device tag of the main contactor coil. `None` means mains is always on.
    pub mains: Option<String>,
    pub limits: Vec<LimitSwitch>,
}

/// A hoist bound to something the circuit does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingFinding {
    pub module: String,
    pub message: String,
}

fn missing(module: &str, what: &str, tag: &str) -> BindingFinding {
    BindingFinding {
        module: module.to_owned(),
        message: format!("{what} {tag} is not in the circuit"),
    }
}

/// A hoist module: engine plus bindings. Bindings are per instance, so two identical hoists
/// on one crab bind to different devices.
#[derive(Debug)]
pub struct HoistModule {
    pub id: String,
    pub unit: TwoSpeedHoistUnit,
    pub binding: HoistBinding,
    height_m: f64,
    snapshot: Option<HoistSnapshot>,
}

impl HoistModule {
    pub fn new(id: impl Into<String>, unit: TwoSpeedHoistUnit, binding: HoistBinding) -> Self {
        Self {
            id: id.into(),
            unit,
            binding,
            height_m: 0.0,
            snapshot: None,
        }
    }

    /// Hook height in metres, 0 at the start position (positive is up).
    pub fn height_m(&self) -> f64 {
        self.height_m
    }

    /// The last tick's numbers, `None` before the first tick.
    pub fn snapshot(&self) -> Option<&HoistSnapshot> {
        self.snapshot.as_ref()
    }
}

/// The loop over any number of hoists (cost is linear in the module count).
pub struct HoistLoop {
    pub config: StepConfig,
    pub hoists: Vec<HoistModule>,
    time_s: f64,
}

impl HoistLoop {
    /// The step is validated (see [`StepConfig::validate`]).
    pub fn new(config: StepConfig) -> Result<Self, InputError> {
        config.validate()?;
        Ok(Self {
            config,
            hoists: Vec::new(),
            time_s: 0.0,
        })
    }

    pub fn time_s(&self) -> f64 {
        self.time_s
    }

    pub fn hoist(&self, id: &str) -> Option<&HoistModule> {
        self.hoists.iter().find(|h| h.id == id)
    }

    /// Binding problems to show the user. A hoist bound to a device the circuit does not know
    /// would otherwise run as "never fed" or "brake held" without a word.
    pub fn check_bindings(&self, circuit: &impl CircuitPort) -> Vec<BindingFinding> {
        let mut out = Vec::new();
        for h in &self.hoists {
            if !circuit.knows_device(&h.binding.motor) {
                out.push(missing(&h.id, "motor", &h.binding.motor));
            }
            match (&h.binding.fine_motor, h.unit.fine_lift()) {
                (Some(tag), Some(_)) if !circuit.knows_device(tag) => {
                    out.push(missing(&h.id, "fine-lift motor", tag))
                }
                (None, Some(_)) => out.push(BindingFinding {
                    module: h.id.clone(),
                    message: "the hoist has a fine-lift motor but no fine-lift motor device is \
                              bound, so it is never fed"
                        .into(),
                }),
                (Some(tag), None) => out.push(BindingFinding {
                    module: h.id.clone(),
                    message: format!(
                        "fine-lift motor {tag} is bound but the fine-lift data is not complete"
                    ),
                }),
                _ => {}
            }
            if let Some(tag) = &h.binding.mains
                && !circuit.knows_device(tag)
            {
                out.push(missing(&h.id, "main contactor", tag));
            }
            match &h.binding.brake_feed {
                Some(tag) if !circuit.knows_device(tag) => {
                    out.push(missing(&h.id, "brake feed", tag))
                }
                None if h.unit.brake_feed() == BrakeFeed::Separate => out.push(BindingFinding {
                    module: h.id.clone(),
                    message: "the brake is fed separately but no brake feed device is bound, \
                              so the brake stays applied"
                        .into(),
                }),
                _ => {}
            }
            if let Some(tag) = &h.binding.overload_contact
                && !circuit.knows_device(tag)
            {
                out.push(missing(&h.id, "overload relay contact", tag));
            }
            for limit in &h.binding.limits {
                if !circuit.knows_device(&limit.binding.device_tag) {
                    out.push(missing(&h.id, "limit switch", &limit.binding.device_tag));
                }
            }
        }
        out
    }

    /// One tick.
    pub fn step(&mut self, circuit: &mut impl CircuitPort) {
        let dt = self.config.dt_s;
        for h in &self.hoists {
            for limit in &h.binding.limits {
                circuit.set_contact(&limit.binding.device_tag, limit.closed_at(h.height_m));
            }
            if let Some(tag) = &h.binding.overload_contact {
                circuit.set_contact(tag, !h.unit.overload_tripped());
            }
        }
        circuit.settle(dt);
        for h in &mut self.hoists {
            let energised = match circuit.motor_feed(&h.binding.motor) {
                MotorFeed::Off => None,
                MotorFeed::Fed { winding, direction } => Some(Energised { winding, direction }),
            };
            let mains_on = h
                .binding
                .mains
                .as_deref()
                .is_none_or(|tag| circuit.is_energised(tag));
            let brake_fed = h
                .binding
                .brake_feed
                .as_deref()
                .is_some_and(|tag| circuit.is_energised(tag));
            let fine_lift =
                h.binding
                    .fine_motor
                    .as_deref()
                    .and_then(|tag| match circuit.motor_feed(tag) {
                        MotorFeed::Fed { direction, .. } => Some(direction),
                        MotorFeed::Off => None,
                    });
            let humming = circuit.motor_humming(&h.binding.motor)
                || h.binding
                    .fine_motor
                    .as_deref()
                    .is_some_and(|tag| circuit.motor_humming(tag));
            // The drawing does not say which phase is open: take L3, the one the brake
            // rectifier (on L1 and L2) does not use. An injected fault can pick another.
            let supply = if humming {
                Supply::HEALTHY.with_phase_lost(Phase::L3)
            } else {
                Supply::HEALTHY
            };
            let snap = h.unit.step_feeds(
                Feeds {
                    main: energised,
                    fine_lift,
                    mains_on,
                    brake_coil_fed: brake_fed,
                    supply,
                    humming,
                },
                dt,
            );
            h.height_m = snap.hook_height_m;
            h.snapshot = Some(snap);
        }
        self.time_s += dt;
    }

    /// Runs `ticks` ticks.
    pub fn run(&mut self, circuit: &mut impl CircuitPort, ticks: usize) {
        for _ in 0..ticks {
            self.step(circuit);
        }
    }
}
