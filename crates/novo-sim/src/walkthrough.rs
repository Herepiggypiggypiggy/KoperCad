//! Walk-throughs: scripted scenarios for the live circuit (lane F1, design
//! `docs/design/fault-injection.md`).
//!
//! A [`Walkthrough`] is a short JSON script of what an operator does and what goes wrong, in
//! time: press START at 1 s, weld the contactor's contact at 3 s, press STOP at 5 s, and what
//! must be true afterwards ("the motor must be stopped"). [`run`] plays it twice side by side
//! against the same drawing: once with a healthy plant and once with the injected
//! [`Defect`]s, with the same operator inputs. Where the two differ is the effect of the
//! defect, and the report lists it step by step with the defect that started it; the
//! expectations are checked against the faulted plant.
//!
//! The same step list is what a [`Recorder`] collects from a live session, so a hand-made test
//! of the plant can be saved and replayed.
//!
//! This is a rehearsal of wiring and logic, not a certified safety function: no failure rates,
//! no diagnostic coverage, no response times of real devices beyond the timers drawn.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{Endpoint, Netlist, PotentialId, Project, SymbolId};
use novo_machine::hoist_loop::HoistLoop;
use serde::{Deserialize, Serialize};

use crate::devices::{DeviceStates, MotorRun, device_states};
use crate::inject::{
    Defect, DefectSet, Prepared, device_by_text, full_pin_name, pin_endpoints, poles_of_device,
};
use crate::machine::{
    self, HoistDefect, HookIs, MAX_MACHINE_SECONDS, MachineSide, PlantPort, check_hook,
    describe_hook, symbols_by_tag,
};
use crate::pins::{operate_poles, pole_of_pin};
use crate::{Clock, Direction, Live, PoleState, Settled, is_coil, rest_state, settle_timed};

/// File format revision of [`Walkthrough`].
pub const WALKTHROUGH_FORMAT: u32 = 1;

/// Seconds of simulated time per settle step while a timer is counting.
const TICK: f64 = 0.05;

/// The longest a walk-through runs, in seconds. A typo such as `t = 1e6` in a hand-edited file
/// must not freeze the program.
pub const MAX_SECONDS: f64 = 3600.0;

/// What a motor must be doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotorIs {
    Stopped,
    /// Turning in either direction.
    Running,
    Forward,
    Reverse,
    /// Fed on two phases only: hums, does not turn.
    PhaseLoss,
    Fault,
}

impl MotorIs {
    fn matches(self, run: MotorRun) -> bool {
        matches!(
            (self, run),
            (MotorIs::Stopped, MotorRun::Stopped)
                | (MotorIs::PhaseLoss, MotorRun::PhaseLoss)
                | (MotorIs::Fault, MotorRun::Fault)
                | (MotorIs::Running, MotorRun::Running { .. })
                | (
                    MotorIs::Forward,
                    MotorRun::Running {
                        direction: Direction::Forward,
                        ..
                    }
                )
                | (
                    MotorIs::Reverse,
                    MotorRun::Running {
                        direction: Direction::Reverse,
                        ..
                    }
                )
        )
    }
}

fn motor_text(run: MotorRun) -> &'static str {
    match run {
        MotorRun::Stopped => "stopped",
        MotorRun::Running {
            direction: Direction::Forward,
            ..
        } => "running forward",
        MotorRun::Running {
            direction: Direction::Reverse,
            ..
        } => "running in reverse",
        MotorRun::PhaseLoss => "humming on two phases",
        MotorRun::Fault => "in fault (short or two speeds at once)",
    }
}

/// What a net must carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetIs {
    Dead,
    /// Any one potential.
    Live,
    /// Two potentials meet.
    Shorted,
    /// This potential (`L1`, `+24V`).
    Carries(String),
}

/// A statement about the faulted plant at one moment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "what", rename_all = "snake_case")]
pub enum Check {
    /// The coil of this device is on (`on: true`) or off.
    Coil {
        device: String,
        on: bool,
    },
    /// The lamp is lit or dark.
    Lamp {
        device: String,
        lit: bool,
    },
    Motor {
        device: String,
        is: MotorIs,
    },
    /// What the net at `tag:pin` carries.
    Net {
        at: String,
        is: NetIs,
    },
    /// Where a hoist's hook is, or what the hoist reports (needs a hoist in the project).
    Hook {
        hoist: String,
        #[serde(flatten)]
        is: HookIs,
    },
}

/// One thing that happens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Action {
    /// The operator holds (`operated: true`) or lets go of a hand device: its poles go to the
    /// opposite of their rest state and back.
    Operate {
        device: String,
        /// One contact of the device, by its pin (`-A1:3` is the third output of a receiver);
        /// all contacts of the device when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pin: Option<String>,
        operated: bool,
    },
    /// A defect begins.
    Inject { with: Defect },
    /// One defect is repaired.
    Repair { with: Defect },
    /// All defects are repaired.
    RepairAll,
    /// Something goes wrong with a hoist (a worn brake, an open phase, a sagging supply).
    Machine { hoist: String, with: HoistDefect },
    /// Every machine defect of this hoist is repaired.
    MachineRepair { hoist: String },
    /// The faulted plant must now satisfy this.
    Expect { check: Check },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// Seconds from the start of the run.
    pub t: f64,
    #[serde(flatten)]
    pub action: Action,
    /// What the person is checking (shown in the report).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Walkthrough {
    pub format: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Seconds to keep running after the last step.
    #[serde(default)]
    pub tail: f64,
    pub steps: Vec<Step>,
}

/// Why a walk-through file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    Syntax(String),
    TooNew(u32),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Syntax(e) => write!(f, "not a walk-through file: {e}"),
            LoadError::TooNew(v) => write!(
                f,
                "format {v} is newer than this program reads ({WALKTHROUGH_FORMAT})"
            ),
        }
    }
}

impl std::error::Error for LoadError {}

impl Walkthrough {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            format: WALKTHROUGH_FORMAT,
            name: name.into(),
            description: String::new(),
            tail: 0.0,
            steps: Vec::new(),
        }
    }

    pub fn at(mut self, t: f64, action: Action) -> Self {
        self.steps.push(Step {
            t,
            action,
            note: String::new(),
        });
        self
    }

    /// Adds a step with a note.
    pub fn noted(mut self, t: f64, action: Action, note: &str) -> Self {
        self.steps.push(Step {
            t,
            action,
            note: note.to_owned(),
        });
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a walk-through always serializes")
    }

    pub fn from_json(text: &str) -> Result<Self, LoadError> {
        let w: Walkthrough =
            serde_json::from_str(text).map_err(|e| LoadError::Syntax(e.to_string()))?;
        if w.format > WALKTHROUGH_FORMAT {
            return Err(LoadError::TooNew(w.format));
        }
        Ok(w)
    }
}

/// Collects the operator's moves and the defects of a live session into a [`Walkthrough`].
#[derive(Debug, Clone, Default)]
pub struct Recorder {
    steps: Vec<Step>,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Notes an action at `t` seconds after the recording started.
    pub fn note(&mut self, t: f64, action: Action) {
        // Times only go forward; a late call keeps the order it came in.
        let t = self.steps.last().map_or(t, |s| t.max(s.t));
        self.steps.push(Step {
            t,
            action,
            note: String::new(),
        });
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Ends the recording. `tail` is how long the plant ran after the last move.
    pub fn finish(self, name: &str, tail: f64) -> Walkthrough {
        Walkthrough {
            steps: self.steps,
            tail: tail.max(0.0),
            ..Walkthrough::new(name)
        }
    }
}

/// Whether a step passed its check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// The step is not a check.
    Done,
    Pass,
    Fail,
}

/// What one step did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepReport {
    pub t: f64,
    /// The step in words: "Press -S2", "Inject Contact welded -K1".
    pub label: String,
    pub note: String,
    pub verdict: Verdict,
    /// Why a check failed, or what could not be done.
    pub detail: String,
    /// What now differs from the healthy plant that did not before this step, and why: for an
    /// injected defect these are its effects.
    pub appeared: Vec<String>,
    /// What differed before and does not any more.
    pub gone: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub name: String,
    pub steps: Vec<StepReport>,
    pub passed: usize,
    pub failed: usize,
    /// Defects whose names lead nowhere in the drawing.
    pub problems: Vec<String>,
    /// Seconds simulated.
    pub duration: f64,
    /// How the faulted plant differs from the healthy one at the end.
    pub end_differences: Vec<String>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.failed == 0 && self.problems.is_empty()
    }

    /// A printable report for the walk-through of a safety review or a commissioning rehearsal.
    pub fn to_markdown(&self) -> String {
        let mut out = format!("# Walk-through: {}\n\n", self.name);
        out.push_str(&format!(
            "{} checks passed, {} failed, {:.1} s simulated.\n\n",
            self.passed, self.failed, self.duration
        ));
        out.push_str(
            "This is a rehearsal of the drawn wiring and logic. It is not a safety function and \
             not a compliance result.\n\n",
        );
        for p in &self.problems {
            out.push_str(&format!("- **Not applied:** {p}\n"));
        }
        out.push_str("| Time | Step | Result | What changed |\n| --- | --- | --- | --- |\n");
        for s in &self.steps {
            let result = match s.verdict {
                Verdict::Done => String::new(),
                Verdict::Pass => "pass".to_owned(),
                Verdict::Fail => format!("FAIL: {}", s.detail),
            };
            let mut changed = s.appeared.join("; ");
            if !s.gone.is_empty() {
                if !changed.is_empty() {
                    changed.push_str("; ");
                }
                changed.push_str(&format!("back to normal: {}", s.gone.join("; ")));
            }
            let label = if s.note.is_empty() {
                s.label.clone()
            } else {
                format!("{} ({})", s.label, s.note)
            };
            out.push_str(&format!(
                "| {:.2} s | {label} | {result} | {changed} |\n",
                s.t
            ));
        }
        if !self.end_differences.is_empty() {
            out.push_str("\nAt the end the faulted plant differs from a healthy one:\n\n");
            for d in &self.end_differences {
                out.push_str(&format!("- {d}\n"));
            }
        }
        out
    }
}

/// One plant side: its clock and which coils were on.
struct Side {
    clock: Clock,
    held: BTreeSet<SymbolId>,
    settled: Option<Settled>,
}

impl Side {
    fn new() -> Self {
        Self {
            clock: Clock::new(),
            held: BTreeSet::new(),
            settled: None,
        }
    }
}

/// Plays `walkthrough` against `project`. The project is not changed. Steps that name a hoist
/// fail here (use [`run_with`] to give the project's hoists).
pub fn run(project: &Project, walkthrough: &Walkthrough) -> Report {
    run_with(project, walkthrough, None)
}

type Machines = (MachineSide, MachineSide);

/// One tick of the hoists on both plants: each settles its circuit once per tick, with the
/// limit switch contacts the hoists wrote.
#[allow(clippy::too_many_arguments)]
fn machine_tick(
    project: &Project,
    by_tag: &BTreeMap<String, Vec<SymbolId>>,
    healthy_net: &Netlist,
    prepared: &Prepared,
    manual: &PoleState,
    forced: &BTreeSet<SymbolId>,
    clean: &mut Side,
    faulty: &mut Side,
    m: &mut Machines,
) {
    {
        let state = machine::with_contacts(project, manual, &m.0.contacts);
        let MachineSide { lp, contacts, .. } = &mut m.0;
        let mut port = PlantPort::new(project, healthy_net, by_tag, state, contacts, |st, dt| {
            clean.clock.advance(dt);
            let s = settle_timed(
                project,
                healthy_net,
                st,
                forced,
                &clean.held,
                &mut clean.clock,
            );
            clean.held = s.energized.clone();
            clean.settled = Some(s.clone());
            s
        });
        lp.step(&mut port);
    }
    let working = prepared.project(project);
    let state = machine::with_contacts(working, manual, &m.1.contacts);
    let MachineSide { lp, contacts, .. } = &mut m.1;
    let mut port = PlantPort::new(
        working,
        prepared.netlist(),
        by_tag,
        state,
        contacts,
        |st, dt| {
            faulty.clock.advance(dt);
            let s = prepared.settle(project, st, forced, &faulty.held, &mut faulty.clock);
            faulty.held = s.energized.clone();
            faulty.settled = Some(s.clone());
            s
        },
    );
    lp.step(&mut port);
}

/// [`run`] with the project's hoists: `hoists` makes a fresh loop for each plant. Steps can
/// then put machine defects on a hoist and ask where its hook is.
pub fn run_with(
    project: &Project,
    walkthrough: &Walkthrough,
    hoists: Option<&dyn Fn() -> HoistLoop>,
) -> Report {
    let healthy_net = project.netlist();
    let by_tag = symbols_by_tag(project);
    let mut machine: Option<Machines> =
        hoists.map(|make| (MachineSide::new(make()), MachineSide::new(make())));
    let rest = rest_state(project);
    let mut manual = rest.clone();
    let mut defects = DefectSet::new();
    let mut prepared = defects.prepare(project);
    let mut problems: Vec<String> = Vec::new();
    let mut clean = Side::new();
    let mut faulty = Side::new();
    let forced = BTreeSet::new();

    // Settle both sides with the operator's current inputs.
    let settle = |manual: &PoleState,
                  machine: Option<&Machines>,
                  prepared: &Prepared,
                  clean: &mut Side,
                  faulty: &mut Side| {
        let (manual_clean, manual_faulty) = match machine {
            Some(m) => (
                machine::with_contacts(project, manual, &m.0.contacts),
                machine::with_contacts(prepared.project(project), manual, &m.1.contacts),
            ),
            None => (manual.clone(), manual.clone()),
        };
        let (manual, manual_f) = (&manual_clean, &manual_faulty);
        let s = settle_timed(
            project,
            &healthy_net,
            manual,
            &forced,
            &clean.held,
            &mut clean.clock,
        );
        clean.held = s.energized.clone();
        clean.settled = Some(s);
        let s = prepared.settle(project, manual_f, &forced, &faulty.held, &mut faulty.clock);
        faulty.held = s.energized.clone();
        faulty.settled = Some(s);
    };
    settle(
        &manual,
        machine.as_ref(),
        &prepared,
        &mut clean,
        &mut faulty,
    );
    let diffs = |clean: &Side, prepared: &Prepared, faulty: &Side, machine: Option<&Machines>| {
        let mut out = differences(project, &healthy_net, clean, prepared, faulty);
        if let Some(m) = machine {
            machine::differences(&m.0, &m.1, &mut out);
        }
        out
    };

    let mut steps: Vec<&Step> = walkthrough.steps.iter().collect();
    steps.sort_by(|a, b| a.t.total_cmp(&b.t));
    let longest = steps.last().map_or(0.0, |s| s.t);
    if longest > MAX_SECONDS || walkthrough.tail > MAX_SECONDS {
        problems.push(format!(
            "the script runs longer than {MAX_SECONDS} s: times and the tail are cut to that"
        ));
    }
    let mut end = longest.clamp(0.0, MAX_SECONDS) + walkthrough.tail.clamp(0.0, MAX_SECONDS);
    if machine.is_some() && end > MAX_MACHINE_SECONDS {
        problems.push(format!(
            "a run with a hoist in it stops at {MAX_MACHINE_SECONDS} s (every tick settles the circuit)"
        ));
        end = MAX_MACHINE_SECONDS;
    }
    let mut now = 0.0_f64;
    // Plays to `target` seconds: tick by tick with hoists, else straight to the next event.
    macro_rules! advance {
        ($target:expr) => {{
            let target: f64 = $target;
            if let Some(m) = machine.as_mut() {
                while now + 1e-9 < target {
                    machine_tick(
                        project,
                        &by_tag,
                        &healthy_net,
                        &prepared,
                        &manual,
                        &forced,
                        &mut clean,
                        &mut faulty,
                        m,
                    );
                    now += m.0.dt();
                }
            } else {
                while now < target {
                    let dt = next_dt(&clean, &faulty, target - now);
                    clean.clock.advance(dt);
                    faulty.clock.advance(dt);
                    now += dt;
                    settle(&manual, None, &prepared, &mut clean, &mut faulty);
                }
            }
        }};
    }

    let mut before = diffs(&clean, &prepared, &faulty, machine.as_ref());
    let mut reports = Vec::with_capacity(steps.len());
    let (mut passed, mut failed) = (0, 0);

    for step in steps {
        let target = step
            .t
            .clamp(0.0, if machine.is_some() { end } else { MAX_SECONDS });
        advance!(target);
        let mut detail = String::new();
        let mut verdict = Verdict::Done;
        let label = match &step.action {
            Action::Operate {
                device,
                pin,
                operated,
            } => {
                let poles = match pin {
                    Some(pin) => pole_of_pin(
                        project,
                        &full_pin_name(project, &format!("{}:{pin}", device.trim())),
                    )
                    .map(|p| vec![p]),
                    None => poles_of_device(project, device),
                };
                match poles {
                    Ok(poles) => operate_poles(&mut manual, &rest, &poles, *operated),
                    Err(e) => {
                        detail = e.to_string();
                        verdict = Verdict::Fail;
                    }
                }
                let verb = if *operated { "Operate" } else { "Release" };
                match pin {
                    Some(pin) => format!("{verb} {}:{pin}", device.trim()),
                    None => format!("{verb} {}", device.trim()),
                }
            }
            Action::Inject { with } => {
                defects.inject(with.clone());
                format!("Inject: {}", with.label())
            }
            Action::Repair { with } => {
                if !defects.clear(with) {
                    detail = "that defect was not present".to_owned();
                    verdict = Verdict::Fail;
                }
                format!("Repair: {}", with.label())
            }
            Action::RepairAll => {
                defects.clear_all();
                "Repair everything".to_owned()
            }
            Action::Machine { hoist, with } => {
                match machine.as_mut() {
                    Some(m) => {
                        if let Err(e) = m.1.inject(hoist, *with) {
                            detail = e;
                            verdict = Verdict::Fail;
                        }
                    }
                    None => {
                        detail = "this project has no hoist".to_owned();
                        verdict = Verdict::Fail;
                    }
                }
                format!("Hoist {hoist}: {}", with.label())
            }
            Action::MachineRepair { hoist } => {
                match machine.as_mut() {
                    Some(m) => match m.1.repair(hoist) {
                        Ok(true) => {}
                        Ok(false) => {
                            detail = "that hoist had no defect".to_owned();
                            verdict = Verdict::Fail;
                        }
                        Err(e) => {
                            detail = e;
                            verdict = Verdict::Fail;
                        }
                    },
                    None => {
                        detail = "this project has no hoist".to_owned();
                        verdict = Verdict::Fail;
                    }
                }
                format!("Repair hoist {hoist}")
            }
            Action::Expect { check } => {
                let (ok, why) = match (check, machine.as_ref()) {
                    (Check::Hook { hoist, is }, Some(m)) => check_hook(&m.1, hoist, is),
                    (Check::Hook { .. }, None) => (false, "this project has no hoist".to_owned()),
                    _ => evaluate(project, &prepared, &faulty, check),
                };
                verdict = if ok { Verdict::Pass } else { Verdict::Fail };
                if !ok {
                    detail = why;
                }
                describe_check(check)
            }
        };
        if matches!(
            step.action,
            Action::Inject { .. } | Action::Repair { .. } | Action::RepairAll
        ) {
            prepared = defects.prepare(project);
            for p in &prepared.problems {
                if !problems.contains(p) {
                    problems.push(p.clone());
                }
            }
        }
        settle(
            &manual,
            machine.as_ref(),
            &prepared,
            &mut clean,
            &mut faulty,
        );
        match verdict {
            Verdict::Pass => passed += 1,
            Verdict::Fail => failed += 1,
            Verdict::Done => {}
        }
        let after = diffs(&clean, &prepared, &faulty, machine.as_ref());
        reports.push(StepReport {
            t: step.t,
            label,
            note: step.note.clone(),
            verdict,
            detail,
            appeared: after
                .iter()
                .filter(|d| !before.contains(*d))
                .cloned()
                .collect(),
            gone: before
                .iter()
                .filter(|d| !after.contains(*d))
                .cloned()
                .collect(),
        });
        before = after;
    }
    advance!(end);
    let end_differences: Vec<String> = diffs(&clean, &prepared, &faulty, machine.as_ref())
        .into_iter()
        .collect();
    Report {
        name: walkthrough.name.clone(),
        steps: reports,
        passed,
        failed,
        problems,
        duration: now,
        end_differences,
    }
}

/// The next time step: fine while a timer counts, else straight to the next event, since
/// nothing changes by itself.
fn next_dt(clean: &Side, faulty: &Side, left: f64) -> f64 {
    let counting = [clean, faulty]
        .iter()
        .any(|s| s.settled.as_ref().is_none_or(|x| x.timing));
    if counting { TICK.min(left) } else { left }
}

fn describe_check(check: &Check) -> String {
    match check {
        Check::Coil { device, on } => {
            format!(
                "Expect coil {} {}",
                device.trim(),
                if *on { "on" } else { "off" }
            )
        }
        Check::Lamp { device, lit } => format!(
            "Expect lamp {} {}",
            device.trim(),
            if *lit { "lit" } else { "dark" }
        ),
        Check::Motor { device, is } => {
            format!("Expect motor {} {is:?}", device.trim()).to_lowercase()
        }
        Check::Net { at, is } => format!("Expect {at} {}", net_is_text(is)),
        Check::Hook { hoist, is } => describe_hook(hoist, is),
    }
}

fn net_is_text(is: &NetIs) -> String {
    match is {
        NetIs::Dead => "dead".to_owned(),
        NetIs::Live => "live".to_owned(),
        NetIs::Shorted => "shorted".to_owned(),
        NetIs::Carries(p) => format!("at {p}"),
    }
}

/// Checks one statement against the faulted plant.
fn evaluate(project: &Project, prepared: &Prepared, side: &Side, check: &Check) -> (bool, String) {
    let Some(settled) = side.settled.as_ref() else {
        return (false, "nothing has been solved yet".to_owned());
    };
    let working = prepared.project(project);
    let netlist = prepared.netlist();
    match check {
        Check::Coil { device, on } => {
            let id = match device_by_text(project, device) {
                Ok(id) => id,
                Err(e) => return (false, e.to_string()),
            };
            let coils: Vec<_> = project
                .symbols_of_device(id)
                .into_iter()
                .filter(|s| is_coil(s))
                .collect();
            if coils.is_empty() {
                return (false, format!("{} has no coil", device.trim()));
            }
            let actual = coils.iter().any(|c| settled.energized.contains(&c.id));
            (
                actual == *on,
                format!(
                    "{} coil is {}, expected {}",
                    device.trim(),
                    on_off(actual),
                    on_off(*on)
                ),
            )
        }
        Check::Lamp { device, lit } => {
            let id = match device_by_text(project, device) {
                Ok(id) => id,
                Err(e) => return (false, e.to_string()),
            };
            let states = device_states(working, netlist, settled);
            let mine: Vec<bool> = project
                .symbols_of_device(id)
                .into_iter()
                .filter_map(|s| states.lamps.get(&s.id).copied())
                .collect();
            if mine.is_empty() {
                return (false, format!("{} is not a lamp", device.trim()));
            }
            let actual = mine.iter().any(|&l| l);
            (
                actual == *lit,
                format!(
                    "{} is {}, expected {}",
                    device.trim(),
                    if actual { "lit" } else { "dark" },
                    if *lit { "lit" } else { "dark" }
                ),
            )
        }
        Check::Motor { device, is } => {
            let id = match device_by_text(project, device) {
                Ok(id) => id,
                Err(e) => return (false, e.to_string()),
            };
            let states = device_states(working, netlist, settled);
            let Some(run) = project
                .symbols_of_device(id)
                .into_iter()
                .find_map(|s| states.motors.get(&s.id).copied())
            else {
                return (false, format!("{} is not a motor", device.trim()));
            };
            (
                is.matches(run),
                format!(
                    "{} is {}, expected {}",
                    device.trim(),
                    motor_text(run),
                    format!("{is:?}").to_lowercase()
                ),
            )
        }
        Check::Hook { .. } => (
            false,
            "a hook is checked against the hoists, not the circuit".to_owned(),
        ),
        Check::Net { at, is } => {
            let ends = match pin_endpoints(project, at) {
                Ok(e) => e,
                Err(e) => return (false, e),
            };
            let live = ends
                .iter()
                .map(|&e| settled.solution.of(netlist, e).clone())
                .fold(Live::Dead, |acc, l| match (&acc, &l) {
                    (Live::Dead, _) => l,
                    _ => acc,
                });
            let ok = match (is, &live) {
                (NetIs::Dead, Live::Dead) => true,
                (NetIs::Live, Live::Carries(_)) => true,
                (NetIs::Shorted, Live::Shorted(_)) => true,
                (NetIs::Carries(name), Live::Carries(p)) => {
                    potential_name(project, *p).is_some_and(|n| n.eq_ignore_ascii_case(name.trim()))
                }
                _ => false,
            };
            (
                ok,
                format!(
                    "{at} is {}, expected {}",
                    live_text(project, &live),
                    net_is_text(is)
                ),
            )
        }
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

fn potential_name(project: &Project, id: PotentialId) -> Option<String> {
    project.potential(id).map(|p| p.name.clone())
}

fn live_text(project: &Project, live: &Live) -> String {
    match live {
        Live::Dead => "dead".to_owned(),
        Live::Carries(p) => format!(
            "carrying {}",
            potential_name(project, *p).unwrap_or_else(|| "a potential".to_owned())
        ),
        Live::Shorted(ps) => format!(
            "shorted ({})",
            ps.iter()
                .filter_map(|p| potential_name(project, *p))
                .collect::<Vec<_>>()
                .join(" and ")
        ),
    }
}

/// How the faulted plant differs from the healthy one, as sentences, in a stable order.
fn differences(
    project: &Project,
    healthy_net: &Netlist,
    clean: &Side,
    prepared: &Prepared,
    faulty: &Side,
) -> BTreeSet<String> {
    match (clean.settled.as_ref(), faulty.settled.as_ref()) {
        (Some(h), Some(f)) => differences_of(project, healthy_net, h, prepared, f),
        _ => BTreeSet::new(),
    }
}

/// How a faulted solution differs from the healthy one of the same inputs, as sentences in a
/// stable order: coils, lamps and motors that behave otherwise and nets that are shorted only
/// because of a defect. The live view uses this for its "what is different" list.
pub fn differences_of(
    project: &Project,
    healthy_net: &Netlist,
    h: &Settled,
    prepared: &Prepared,
    f: &Settled,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let tag = |s: SymbolId| -> String {
        project
            .symbol(s)
            .and_then(|sym| sym.device())
            .and_then(|d| project.device(d))
            .map_or_else(|| "?".to_owned(), |d| d.tag.to_string())
    };
    for coil in project.symbols().filter(|s| is_coil(s)) {
        let (a, b) = (
            h.energized.contains(&coil.id),
            f.energized.contains(&coil.id),
        );
        if a != b {
            out.insert(format!(
                "coil {} is {} (a healthy plant: {})",
                tag(coil.id),
                on_off(b),
                on_off(a)
            ));
        }
    }
    let hs = device_states(project, healthy_net, h);
    let fs = device_states(prepared.project(project), prepared.netlist(), f);
    for (id, a) in &hs.lamps {
        if let Some(b) = fs.lamps.get(id)
            && a != b
        {
            out.insert(format!(
                "lamp {} is {} (a healthy plant: {})",
                tag(*id),
                if *b { "lit" } else { "dark" },
                if *a { "lit" } else { "dark" }
            ));
        }
    }
    motor_differences(&hs, &fs, &tag, &mut out);
    shorted_differences(project, healthy_net, h, prepared, f, &mut out);
    out
}

fn motor_differences(
    hs: &DeviceStates,
    fs: &DeviceStates,
    tag: &dyn Fn(SymbolId) -> String,
    out: &mut BTreeSet<String>,
) {
    for (id, a) in &hs.motors {
        let Some(b) = fs.motors.get(id) else {
            continue;
        };
        let differs = match (a, b) {
            (MotorRun::Running { direction: d1, .. }, MotorRun::Running { direction: d2, .. }) => {
                d1 != d2
            }
            _ => std::mem::discriminant(a) != std::mem::discriminant(b),
        };
        if differs {
            out.insert(format!(
                "motor {} is {} (a healthy plant: {})",
                tag(*id),
                motor_text(*b),
                motor_text(*a)
            ));
        }
    }
}

/// Nets that are shorted now and were not in the healthy plant, named by one pin on them.
fn shorted_differences(
    project: &Project,
    healthy_net: &Netlist,
    h: &Settled,
    prepared: &Prepared,
    f: &Settled,
    out: &mut BTreeSet<String>,
) {
    let working = prepared.project(project);
    let net = prepared.netlist();
    for n in net.nets() {
        let Live::Shorted(ps) = f.solution.net(n.id) else {
            continue;
        };
        // Was one of the same endpoints shorted before?
        let before = n
            .endpoints
            .iter()
            .any(|&e| matches!(h.solution.of(healthy_net, e), Live::Shorted(_)));
        if before {
            continue;
        }
        let names: Vec<String> = ps
            .iter()
            .filter_map(|p| potential_name(project, *p))
            .collect();
        let at = n
            .endpoints
            .iter()
            .find_map(|&e| pin_name(working, e))
            .unwrap_or_else(|| "a conductor".to_owned());
        out.insert(format!("short between {} at {at}", names.join(" and ")));
    }
}

/// `tag:pin` of an endpoint on a device symbol.
fn pin_name(project: &Project, end: Endpoint) -> Option<String> {
    let pin = end.as_pin()?;
    let sym = project.symbol(pin.symbol)?;
    let device = project.device(sym.device()?)?;
    Some(format!(
        "{}:{}",
        device.tag,
        sym.pins.get(usize::from(pin.pin))?.name
    ))
}
