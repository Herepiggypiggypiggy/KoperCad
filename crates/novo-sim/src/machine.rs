//! Machine-level defects for walk-throughs (lane F1b, design `docs/design/fault-injection.md`).
//!
//! The circuit defects of [`crate::inject`] change the wiring. These change the machine the
//! wiring drives: a brake that is worn or slow, a rectifier that lost a diode, a phase that is
//! open or a supply that sags. A walk-through can put them on a hoist and then ask where the
//! hook is, whether it moves and which conditions the hoist reports.
//!
//! The numbers come from the hoist engine (`novo-drive`) and are a model: an averaged brake and
//! motor, not a measured drive. Nothing here is a safety function.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{Netlist, Project, SymbolId};
use novo_drive::brake_system::RectifierFault;
use novo_drive::hoist_unit::HoistFaults;
use novo_drive::motor::{Direction, WindingSel};
use novo_drive::supply::Phase;
use novo_machine::hoist_loop::HoistLoop;
use novo_machine::port::{CircuitPort, MotorFeed};
use serde::{Deserialize, Serialize};

use crate::devices::{DeviceStates, MotorRun, device_states};
use crate::inject::poles_of_device;
use crate::{PoleState, Settled};

/// The longest a run with a hoist in it may be, in seconds (every tick settles the circuit).
pub const MAX_MACHINE_SECONDS: f64 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseName {
    L1,
    L2,
    L3,
}

impl PhaseName {
    fn phase(self) -> Phase {
        match self {
            PhaseName::L1 => Phase::L1,
            PhaseName::L2 => Phase::L2,
            PhaseName::L3 => Phase::L3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "fault", rename_all = "snake_case")]
pub enum RectifierDefect {
    OpenDiode,
    ShortedDiode,
    OpenOutput,
    NoBoost,
    LowOutput { share: f64 },
}

/// Something wrong with a hoist (not with its wiring).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HoistDefect {
    /// A supply phase is open at the motor.
    LostPhase { phase: PhaseName },
    /// The supply is `ratio` of rated (0.9 is 90 %).
    Supply { ratio: f64 },
    /// The brake takes this many times as long to open and close.
    BrakeDelay { times: f64 },
    /// The brake holds this share of its rated torque (worn lining).
    BrakeLining { share: f64 },
    /// The brake rectifier is faulty (rectifier brakes only).
    Rectifier(RectifierDefect),
}

impl HoistDefect {
    pub fn label(&self) -> String {
        match self {
            HoistDefect::LostPhase { phase } => format!("phase {phase:?} open at the motor"),
            HoistDefect::Supply { ratio } => format!("supply at {:.0} %", ratio * 100.0),
            HoistDefect::BrakeDelay { times } => format!("brake {times}x slower"),
            HoistDefect::BrakeLining { share } => {
                format!("brake lining holds {:.0} %", share * 100.0)
            }
            HoistDefect::Rectifier(r) => match r {
                RectifierDefect::OpenDiode => "brake rectifier: one diode open".to_owned(),
                RectifierDefect::ShortedDiode => "brake rectifier: diode shorted".to_owned(),
                RectifierDefect::OpenOutput => "brake coil circuit open".to_owned(),
                RectifierDefect::NoBoost => "brake rectifier: no over-excitation".to_owned(),
                RectifierDefect::LowOutput { share } => {
                    format!("brake rectifier output at {:.0} %", share * 100.0)
                }
            },
        }
    }

    fn apply(&self, f: &mut HoistFaults) {
        match *self {
            HoistDefect::LostPhase { phase } => f.lost_phase = Some(phase.phase()),
            HoistDefect::Supply { ratio } => f.voltage_ratio = ratio,
            HoistDefect::BrakeDelay { times } => f.brake_delay_scale = times,
            HoistDefect::BrakeLining { share } => f.brake_torque_scale = share,
            HoistDefect::Rectifier(r) => {
                f.rectifier = match r {
                    RectifierDefect::OpenDiode => RectifierFault::OpenDiode,
                    RectifierDefect::ShortedDiode => RectifierFault::ShortedDiode,
                    RectifierDefect::OpenOutput => RectifierFault::OpenOutput,
                    RectifierDefect::NoBoost => RectifierFault::NoBoost,
                    RectifierDefect::LowOutput { share } => RectifierFault::LowOutput(share),
                }
            }
        }
    }
}

/// What the hoist reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionName {
    FedBrakeClosed,
    BrakeSlipping,
    PhaseLost,
    LowVoltage,
    BothMotorsFed,
}

/// A statement about a hoist's hook.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum HookIs {
    /// At least this high, metres above the start.
    Above { m: f64 },
    /// At most this high.
    Below { m: f64 },
    /// Not moving (under 1 mm/s).
    Standing,
    /// Moving.
    Moving,
    /// The hoist reports this condition.
    Reporting { condition: ConditionName },
    /// The hoist reports nothing wrong.
    Clear,
}

fn condition_on(name: ConditionName, c: &novo_drive::hoist_unit::Conditions) -> bool {
    match name {
        ConditionName::FedBrakeClosed => c.fed_brake_closed,
        ConditionName::BrakeSlipping => c.brake_slipping,
        ConditionName::PhaseLost => c.phase_lost,
        ConditionName::LowVoltage => c.low_voltage,
        ConditionName::BothMotorsFed => c.both_motors_fed,
    }
}

/// One plant's hoists and the machine defects on them.
pub(crate) struct MachineSide {
    pub lp: HoistLoop,
    /// Limit switch contacts the loop wrote last, by device tag (`true` = conducting).
    pub contacts: BTreeMap<String, bool>,
    defects: BTreeMap<String, Vec<HoistDefect>>,
}

impl MachineSide {
    pub fn new(lp: HoistLoop) -> Self {
        Self {
            lp,
            contacts: BTreeMap::new(),
            defects: BTreeMap::new(),
        }
    }

    pub fn dt(&self) -> f64 {
        self.lp.config.dt_s
    }

    fn push(&mut self, hoist: &str) -> Result<(), String> {
        let mut faults = HoistFaults::NONE;
        for d in self.defects.get(hoist).into_iter().flatten() {
            d.apply(&mut faults);
        }
        let module = self
            .lp
            .hoists
            .iter_mut()
            .find(|h| h.id == hoist)
            .ok_or_else(|| format!("no hoist {hoist} in the project"))?;
        module
            .unit
            .set_faults(faults)
            .map_err(|e| format!("{hoist}: {e}"))
    }

    pub fn inject(&mut self, hoist: &str, d: HoistDefect) -> Result<(), String> {
        let list = self.defects.entry(hoist.to_owned()).or_default();
        list.push(d);
        let result = self.push(hoist);
        if let (Err(_), Some(list)) = (&result, self.defects.get_mut(hoist)) {
            list.pop();
        }
        result
    }

    pub fn repair(&mut self, hoist: &str) -> Result<bool, String> {
        let had = self.defects.remove(hoist).is_some_and(|l| !l.is_empty());
        self.push(hoist)?;
        Ok(had)
    }

    pub fn ids(&self) -> Vec<String> {
        self.lp.hoists.iter().map(|h| h.id.clone()).collect()
    }
}

/// Checks a hook statement on the faulted side; the text says what was found.
pub(crate) fn check_hook(side: &MachineSide, hoist: &str, is: &HookIs) -> (bool, String) {
    let Some(module) = side.lp.hoist(hoist) else {
        return (false, format!("no hoist {hoist} in the project"));
    };
    let h = module.height_m();
    let speed = module.snapshot().map_or(0.0, |s| s.hook_speed_m_s);
    let conditions = module.snapshot().map(|s| s.conditions).unwrap_or_default();
    match *is {
        HookIs::Above { m } => (
            h >= m,
            format!("hook of {hoist} is at {h:.2} m, expected at least {m:.2} m"),
        ),
        HookIs::Below { m } => (
            h <= m,
            format!("hook of {hoist} is at {h:.2} m, expected at most {m:.2} m"),
        ),
        HookIs::Standing => (
            speed.abs() < 0.001,
            format!("hook of {hoist} moves at {speed:.3} m/s, expected standing"),
        ),
        HookIs::Moving => (
            speed.abs() >= 0.001,
            format!("hook of {hoist} moves at {speed:.3} m/s, expected moving"),
        ),
        HookIs::Reporting { condition } => (
            condition_on(condition, &conditions),
            format!(
                "{hoist} reports: {}",
                if conditions.any() {
                    conditions.messages().join("; ")
                } else {
                    "nothing wrong".to_owned()
                }
            ),
        ),
        HookIs::Clear => (
            !conditions.any(),
            format!("{hoist} reports: {}", conditions.messages().join("; ")),
        ),
    }
}

pub(crate) fn describe_hook(hoist: &str, is: &HookIs) -> String {
    match is {
        HookIs::Above { m } => format!("Expect hook of {hoist} at least {m:.2} m"),
        HookIs::Below { m } => format!("Expect hook of {hoist} at most {m:.2} m"),
        HookIs::Standing => format!("Expect hook of {hoist} standing"),
        HookIs::Moving => format!("Expect hook of {hoist} moving"),
        HookIs::Reporting { condition } => {
            format!(
                "Expect {hoist} to report {}",
                format!("{condition:?}").to_lowercase()
            )
        }
        HookIs::Clear => format!("Expect {hoist} to report nothing wrong"),
    }
}

/// How the faulted hoists differ from the healthy ones, as sentences. The hook position itself
/// changes every tick, so only a clear gap and the reported conditions are listed.
pub(crate) fn differences(clean: &MachineSide, faulty: &MachineSide, out: &mut BTreeSet<String>) {
    for id in faulty.ids() {
        let (Some(a), Some(b)) = (clean.lp.hoist(&id), faulty.lp.hoist(&id)) else {
            continue;
        };
        let gap = b.height_m() - a.height_m();
        if gap.abs() >= 0.05 {
            out.insert(format!(
                "hook of {id} is {:.2} m {} than a healthy hoist's",
                gap.abs(),
                if gap > 0.0 { "higher" } else { "lower" }
            ));
        }
        let had = a.snapshot().map(|s| s.conditions).unwrap_or_default();
        let has = b.snapshot().map(|s| s.conditions).unwrap_or_default();
        if has.any() {
            for m in has.messages() {
                if !had.messages().contains(&m) {
                    out.insert(format!("{id}: {m}"));
                }
            }
        }
    }
}

/// The circuit as the hoist loop sees it, for one plant of a walk-through. `settle_with` gets
/// the pole state (the operator's inputs with the limit switches written over them) and the
/// step, and returns the settled circuit; the plant keeps its own clock and held coils.
pub(crate) struct PlantPort<'a, F: FnMut(&PoleState, f64) -> Settled> {
    pub project: &'a Project,
    pub net: &'a Netlist,
    pub by_tag: &'a BTreeMap<String, Vec<SymbolId>>,
    pub state: PoleState,
    pub contacts: &'a mut BTreeMap<String, bool>,
    pub settle_with: F,
    states: DeviceStates,
    coils: BTreeSet<SymbolId>,
}

impl<'a, F: FnMut(&PoleState, f64) -> Settled> PlantPort<'a, F> {
    pub fn new(
        project: &'a Project,
        net: &'a Netlist,
        by_tag: &'a BTreeMap<String, Vec<SymbolId>>,
        state: PoleState,
        contacts: &'a mut BTreeMap<String, bool>,
        settle_with: F,
    ) -> Self {
        Self {
            project,
            net,
            by_tag,
            state,
            contacts,
            settle_with,
            states: DeviceStates::default(),
            coils: BTreeSet::new(),
        }
    }
}

impl<F: FnMut(&PoleState, f64) -> Settled> CircuitPort for PlantPort<'_, F> {
    fn settle(&mut self, dt_s: f64) {
        let settled = (self.settle_with)(&self.state, dt_s);
        self.states = device_states(self.project, self.net, &settled);
        self.coils = settled.energized;
    }

    fn motor_feed(&self, motor: &str) -> MotorFeed {
        let run = self
            .by_tag
            .get(motor)
            .into_iter()
            .flatten()
            .find_map(|id| self.states.motors.get(id));
        match run {
            // The first winding set of a two-speed motor symbol is the slow (fine) one.
            Some(MotorRun::Running { direction, speed }) => MotorFeed::Fed {
                winding: if *speed == 0 {
                    WindingSel::Fine
                } else {
                    WindingSel::Coarse
                },
                direction: match direction {
                    crate::Direction::Forward => Direction::Positive,
                    crate::Direction::Reverse => Direction::Negative,
                },
            },
            _ => MotorFeed::Off,
        }
    }

    fn is_energised(&self, device: &str) -> bool {
        self.by_tag.get(device).into_iter().flatten().any(|id| {
            self.coils.contains(id) || self.states.lamps.get(id).copied().unwrap_or(false)
        })
    }

    fn set_contact(&mut self, device: &str, closed: bool) {
        self.contacts.insert(device.to_owned(), closed);
        if let Ok(poles) = poles_of_device(self.project, device) {
            for p in poles {
                self.state.set(p.symbol, p.group, closed);
            }
        }
    }

    fn knows_device(&self, device: &str) -> bool {
        self.by_tag.contains_key(device)
    }

    fn motor_humming(&self, motor: &str) -> bool {
        self.by_tag
            .get(motor)
            .into_iter()
            .flatten()
            .any(|id| matches!(self.states.motors.get(id), Some(MotorRun::PhaseLoss)))
    }
}

/// Symbols by full device tag.
pub(crate) fn symbols_by_tag(project: &Project) -> BTreeMap<String, Vec<SymbolId>> {
    let mut by_tag: BTreeMap<String, Vec<SymbolId>> = BTreeMap::new();
    for symbol in project.symbols() {
        if let Some(device) = symbol.device().and_then(|d| project.device(d)) {
            by_tag
                .entry(device.tag.to_string())
                .or_default()
                .push(symbol.id);
        }
    }
    by_tag
}

/// The operator's pole state with the limit switch contacts the hoists last wrote over it.
pub(crate) fn with_contacts(
    project: &Project,
    manual: &PoleState,
    contacts: &BTreeMap<String, bool>,
) -> PoleState {
    let mut state = manual.clone();
    for (tag, closed) in contacts {
        if let Ok(poles) = poles_of_device(project, tag) {
            for p in poles {
                state.set(p.symbol, p.group, *closed);
            }
        }
    }
    state
}
