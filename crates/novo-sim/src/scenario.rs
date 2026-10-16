//! Scenarios: a fault or a load change at a time, and what the protection does
//! about it.
//!
//! A [`Scenario`] is a small JSON file (it lives in the project folder next to
//! the drawing, one per question: "short circuit at the end of cable W12",
//! "start the pump with a warm relay", "close the breaker onto a fault").
//! [`run`] plays it against a [`CircuitModel`], the part that knows the
//! network, and the [`ProtectiveDevice`]s:
//!
//! 1. the model says which current each device carries, given the faults, the
//!    load currents and the devices already open;
//! 2. every closed device works out when its curve (with the heat it has
//!    already collected) will trip at that current;
//! 3. time jumps to the earliest of the next scheduled action, the next trip
//!    and the next contact interruption; heat is integrated over the jump;
//! 4. the action happens, the model is asked again.
//!
//! A trip is a decision; the contacts part `opening_time` later and a fault
//! current is only extinguished at its next natural zero, so the let-through
//! and the clearing time are those of the real asymmetrical waveform.
//!
//! The circuit model is a trait because two implementations exist: the
//! analog solver of this crate (any topology, full impedance network) and
//! [`RadialNetwork`], the quick feeder-tree model below, which is exact for a
//! radial IEC 60909 positive-sequence calculation and is what the tests and
//! the sample scenarios run on.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::fault::{FaultCurrent, FaultKind};
use crate::protection::{ProtectiveDevice, ThermalMemory};
use crate::timing::{DigitalChannel, Level, TimingDiagram, format_time};
use crate::trace::{Trace, TraceSet};

/// File format revision of [`Scenario`].
pub const SCENARIO_FORMAT: u32 = 1;

/// One thing that happens at a time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Action {
    /// A fault begins at `node`. `fault_ohm` is the resistance of the fault
    /// itself (arc, earth contact); `inception_deg` the voltage angle it
    /// starts at, the worst case for the DC offset when absent.
    Fault {
        id: String,
        node: String,
        kind: FaultKind,
        #[serde(default)]
        fault_ohm: f64,
        #[serde(default)]
        inception_deg: Option<f64>,
    },
    /// The fault goes away by itself (a flashover, a removed tool).
    ClearFault { id: String },
    /// The load current at a node becomes `amps` (rms, per phase); zero
    /// switches it off. A motor start is two of these, see
    /// [`Scenario::motor_start`].
    Load { node: String, amps: f64 },
    /// A device is switched open by hand or by the control.
    Open { device: String },
    /// A device is closed, onto whatever is then still connected.
    Close { device: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioEvent {
    /// Seconds from the start of the run.
    pub t: f64,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub format: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Seconds simulated.
    pub duration: f64,
    pub events: Vec<ScenarioEvent>,
}

impl Scenario {
    pub fn new(name: impl Into<String>, duration: f64) -> Self {
        Self {
            format: SCENARIO_FORMAT,
            name: name.into(),
            description: String::new(),
            duration,
            events: Vec::new(),
        }
    }

    pub fn at(mut self, t: f64, action: Action) -> Self {
        self.events.push(ScenarioEvent { t, action });
        self
    }

    /// A direct-on-line motor start: `inrush_a` for `start_s` seconds, then
    /// the running current.
    pub fn motor_start(self, t: f64, node: &str, inrush_a: f64, start_s: f64, run_a: f64) -> Self {
        self.at(
            t,
            Action::Load {
                node: node.into(),
                amps: inrush_a,
            },
        )
        .at(
            t + start_s,
            Action::Load {
                node: node.into(),
                amps: run_a,
            },
        )
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a scenario always serializes")
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let scenario: Scenario = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if scenario.format > SCENARIO_FORMAT {
            return Err(format!(
                "scenario format {} is newer than this program understands ({SCENARIO_FORMAT})",
                scenario.format
            ));
        }
        Ok(scenario)
    }

    /// Mistakes that would make a run meaningless, as plain sentences.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !(self.duration.is_finite() && self.duration > 0.0) {
            out.push("The duration must be longer than zero.".into());
        }
        let mut ids = BTreeSet::new();
        for e in &self.events {
            if !e.t.is_finite() || e.t < 0.0 || e.t > self.duration {
                out.push(format!(
                    "An action at {} s lies outside the run of {} s.",
                    e.t, self.duration
                ));
            }
            if let Action::Fault { id, .. } = &e.action
                && !ids.insert(id.clone())
            {
                out.push(format!("Fault {id} is started twice."));
            }
        }
        for e in &self.events {
            if let Action::ClearFault { id } = &e.action
                && !ids.contains(id)
            {
                out.push(format!("Fault {id} is cleared but never started."));
            }
        }
        out
    }
}

/// A fault that is currently on the network.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveFault {
    pub id: String,
    pub node: String,
    pub kind: FaultKind,
    pub fault_ohm: f64,
    /// Voltage angle at inception in radians.
    pub theta: f64,
    pub t_start: f64,
}

/// Everything the network needs to know to say what flows.
#[derive(Debug, Clone, Copy)]
pub struct Conditions<'a> {
    pub faults: &'a [ActiveFault],
    /// Load current per node in A.
    pub loads: &'a BTreeMap<String, f64>,
    /// Devices whose contacts have parted and whose current is interrupted.
    pub open: &'a BTreeSet<String>,
}

/// The network as the scenario runner sees it.
pub trait CircuitModel {
    /// rms current in A through each protective device (by key); devices not
    /// in the map carry nothing.
    fn device_currents(&self, c: &Conditions) -> BTreeMap<String, f64>;

    /// The prospective short-circuit current of an active fault, `None` when
    /// it is no short circuit (an open phase) or is cut off from the source.
    fn fault_current(&self, fault: &ActiveFault, c: &Conditions) -> Option<FaultCurrent>;

    /// The fault feeding `device` with the highest current, if any.
    fn fault_through(&self, device: &str, c: &Conditions) -> Option<(String, FaultCurrent)>;

    /// Devices between `node` and the source, nearest first.
    fn upstream_devices(&self, node: &str) -> Vec<String>;
}

/// A moment in the run, in words and with its parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub t: f64,
    pub device: Option<String>,
    pub kind: EventKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    FaultStart,
    FaultCleared,
    LoadChange,
    /// A device decided to trip on a short-circuit current.
    TripShortCircuit,
    /// A device decided to trip on an overload (heat built up).
    TripOverload,
    /// The contacts of a device have parted and the current is zero.
    Interrupted,
    OpenedByHand,
    Closed,
}

/// What became of one fault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaultOutcome {
    pub fault: String,
    pub node: String,
    pub kind: FaultKind,
    pub t_start: f64,
    /// Prospective symmetrical current Ik'' (A) and peak ip (A) at the start.
    pub ik: f64,
    pub peak: f64,
    /// Devices that tripped on this fault with the time of the decision.
    pub tripped: Vec<(String, f64)>,
    /// When the current stopped, `None` if it still flowed at the end.
    pub cleared_at: Option<f64>,
    /// The last device to interrupt it.
    pub cleared_by: Option<String>,
    /// The device nearest the fault upstream: the one that should open.
    pub expected: Option<String>,
    /// Only the expected device tripped.
    pub selective: bool,
    /// ∫i²dt in A²s from the start to the interruption.
    pub let_through_i2t: Option<f64>,
    /// The prospective current at the start, `None` for an open phase.
    pub start_current: Option<FaultCurrent>,
}

impl FaultOutcome {
    fn fault_current_at_start(&self) -> Option<FaultCurrent> {
        self.start_current
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioResult {
    pub duration: f64,
    pub events: Vec<TimelineEvent>,
    pub faults: Vec<FaultOutcome>,
    /// Device currents (rms steps) and fault current waveforms.
    pub traces: TraceSet,
    pub timing: TimingDiagram,
    /// Highest heat each device reached (1 = tripped).
    pub peak_heat: BTreeMap<String, f64>,
    /// Things the user should know about the run (it was cut short).
    pub warnings: Vec<String>,
}

impl ScenarioResult {
    /// The timing diagram with every trace added as an analog row.
    pub fn diagram(&self) -> TimingDiagram {
        let mut d = self.timing.clone();
        d.analog = self.traces.traces.clone();
        d
    }

    /// First trip time of a device in the run.
    pub fn trip_time(&self, device: &str) -> Option<f64> {
        self.events
            .iter()
            .find(|e| {
                e.device.as_deref() == Some(device)
                    && matches!(
                        e.kind,
                        EventKind::TripShortCircuit | EventKind::TripOverload
                    )
            })
            .map(|e| e.t)
    }
}

const EPS: f64 = 1e-12;

/// Plays a scenario. Devices are matched to the model by key.
pub fn run(
    model: &dyn CircuitModel,
    devices: &[ProtectiveDevice],
    scenario: &Scenario,
) -> ScenarioResult {
    let mut script: Vec<&ScenarioEvent> = scenario.events.iter().collect();
    script.sort_by(|a, b| a.t.total_cmp(&b.t));
    let mut next_script = 0;

    let mut faults: Vec<ActiveFault> = Vec::new();
    let mut loads: BTreeMap<String, f64> = BTreeMap::new();
    let mut open: BTreeSet<String> = BTreeSet::new();
    let mut tripped: BTreeSet<String> = BTreeSet::new();
    let mut pending: Vec<(String, f64)> = Vec::new();
    let mut heat: Vec<ThermalMemory> = devices
        .iter()
        .map(|d| ThermalMemory {
            heat: d.initial_heat.min(1.0),
        })
        .collect();
    let mut peak_heat: BTreeMap<String, f64> = devices
        .iter()
        .map(|d| (d.key.clone(), d.initial_heat.min(1.0)))
        .collect();

    let mut events: Vec<TimelineEvent> = Vec::new();
    let mut outcomes: Vec<FaultOutcome> = Vec::new();
    let mut current_traces: Vec<Trace> = devices
        .iter()
        .map(|d| Trace::new(format!("I {}", d.label), "A"))
        .collect();
    let mut channels: Vec<DigitalChannel> = Vec::new();
    let mut device_channel: BTreeMap<String, usize> = BTreeMap::new();
    for d in devices {
        device_channel.insert(d.key.clone(), channels.len());
        let mut ch = DigitalChannel::new(d.label.clone(), "Protection");
        ch.set(0.0, Level::High);
        channels.push(ch);
    }
    let mut fault_channel: BTreeMap<String, usize> = BTreeMap::new();
    // Interrupt time per fault of the device that finally cleared it.
    let mut last_interrupt: BTreeMap<String, (String, f64)> = BTreeMap::new();

    let mut t = 0.0_f64;
    let mut guard = 0usize;
    let mut warnings: Vec<String> = Vec::new();
    loop {
        guard += 1;
        if guard > 200_000 {
            warnings.push(format!(
                "The run stopped at {} s: too many events (a circuit that keeps tripping and closing?)",
                format_time(t)
            ));
            break;
        }
        let cond = Conditions {
            faults: &faults,
            loads: &loads,
            open: &open,
        };
        let currents = model.device_currents(&cond);
        let current_of = |key: &str| currents.get(key).copied().unwrap_or(0.0);
        for (i, d) in devices.iter().enumerate() {
            current_traces[i].push_step(t, current_of(&d.key));
        }

        // A fault whose source is gone has been cleared.
        for f in &faults {
            let Some(o) = outcomes.iter_mut().find(|o| o.fault == f.id) else {
                continue;
            };
            if o.cleared_at.is_none()
                && f.kind != FaultKind::OpenPhase
                && model.fault_current(f, &cond).is_none()
            {
                o.cleared_at = Some(t);
                if let Some((dev, ti)) = last_interrupt.get(&f.id) {
                    o.cleared_by = Some(dev.clone());
                    if let Some(fc) = o.fault_current_at_start() {
                        o.let_through_i2t =
                            Some(fc.let_through(f.theta, (ti - f.t_start).max(0.0)));
                    }
                }
                if let Some(&c) = fault_channel.get(&f.id) {
                    channels[c].set(t, Level::Low);
                }
                events.push(TimelineEvent {
                    t,
                    device: o.cleared_by.clone(),
                    kind: EventKind::FaultCleared,
                    text: format!("Fault {} cleared", f.id),
                });
            }
        }

        let mut next = scenario.duration;
        if let Some(e) = script.get(next_script) {
            next = next.min(e.t.max(t));
        }
        for (_, ti) in &pending {
            next = next.min(ti.max(t));
        }
        let mut trips_at: Vec<(usize, f64)> = Vec::new();
        for (i, d) in devices.iter().enumerate() {
            if tripped.contains(&d.key) || open.contains(&d.key) {
                continue;
            }
            let tt = t + heat[i].time_to_trip(d, current_of(&d.key));
            if tt.is_finite() {
                trips_at.push((i, tt));
                next = next.min(tt);
            }
        }

        let dt = (next - t).max(0.0);
        if dt > 0.0 {
            for (i, d) in devices.iter().enumerate() {
                if !tripped.contains(&d.key) {
                    heat[i].advance(d, current_of(&d.key), dt);
                }
                let e = peak_heat.entry(d.key.clone()).or_default();
                *e = e.max(heat[i].heat);
            }
        }
        t = next;

        // 1. contacts that have parted
        let mut done = Vec::new();
        pending.retain(|(key, ti)| {
            if *ti <= t + EPS {
                done.push((key.clone(), *ti));
                false
            } else {
                true
            }
        });
        let mut changed = !done.is_empty();
        for (key, ti) in done {
            // The faults this device cuts off: fed before it opened, not after.
            let before = Conditions {
                faults: &faults,
                loads: &loads,
                open: &open,
            };
            let fed: Vec<bool> = faults
                .iter()
                .map(|f| model.fault_current(f, &before).is_some())
                .collect();
            open.insert(key.clone());
            let after = Conditions {
                faults: &faults,
                loads: &loads,
                open: &open,
            };
            if let Some(&c) = device_channel.get(&key) {
                channels[c].set(ti, Level::Low);
            }
            for (f, was_fed) in faults.iter().zip(fed) {
                if was_fed && model.fault_current(f, &after).is_none() {
                    last_interrupt.insert(f.id.clone(), (key.clone(), ti));
                }
            }
            events.push(TimelineEvent {
                t: ti,
                device: Some(key.clone()),
                kind: EventKind::Interrupted,
                text: format!("{} has interrupted the current", label_of(devices, &key)),
            });
        }

        // 2. scheduled actions
        while let Some(e) = script.get(next_script) {
            if e.t > t + EPS {
                break;
            }
            next_script += 1;
            changed = true;
            apply(
                &e.action,
                t,
                model,
                devices,
                &mut faults,
                &mut loads,
                &mut open,
                &mut tripped,
                &mut heat,
                &mut events,
                &mut outcomes,
                &mut channels,
                &mut fault_channel,
                &device_channel,
            );
        }

        // 3. devices that have used up their time
        for (i, tt) in trips_at {
            let d = &devices[i];
            if tt > t + EPS || tripped.contains(&d.key) || open.contains(&d.key) {
                continue;
            }
            changed = true;
            heat[i].heat = 1.0;
            *peak_heat.entry(d.key.clone()).or_default() = 1.0;
            tripped.insert(d.key.clone());
            let cond = Conditions {
                faults: &faults,
                loads: &loads,
                open: &open,
            };
            let through = model.fault_through(&d.key, &cond);
            let opens = t + d.opening_time;
            let interrupt = match &through {
                Some((id, fc)) => {
                    let f = faults.iter().find(|f| &f.id == id).expect("active fault");
                    f.t_start + fc.interrupt_at(opens - f.t_start, f.theta)
                }
                None => opens,
            };
            pending.push((d.key.clone(), interrupt));
            if let Some(&c) = device_channel.get(&d.key) {
                channels[c].set(t, Level::Alarm);
            }
            let amps = current_of(&d.key);
            let (kind, what) = match &through {
                Some((id, _)) => (EventKind::TripShortCircuit, format!("short circuit {id}")),
                None => (EventKind::TripOverload, "overload".to_string()),
            };
            if let Some((id, _)) = &through
                && let Some(o) = outcomes.iter_mut().find(|o| &o.fault == id)
            {
                o.tripped.push((d.key.clone(), t));
            }
            events.push(TimelineEvent {
                t,
                device: Some(d.key.clone()),
                kind,
                text: format!("{} trips on {} at {:.0} A", d.label, what, amps),
            });
        }

        if !changed && t >= scenario.duration - EPS {
            break;
        }
    }

    // Selectivity and waveforms.
    let mut traces = TraceSet::default();
    for tr in current_traces {
        traces.add(tr);
    }
    for o in &mut outcomes {
        let expected = model.upstream_devices(&o.node).into_iter().next();
        o.selective = expected.is_some()
            && o.cleared_by == expected
            && o.tripped.iter().all(|(d, _)| Some(d) == expected.as_ref());
        o.expected = expected;
        if let (Some(fc), Some(f)) = (
            o.fault_current_at_start(),
            scenario.events.iter().find_map(|e| match &e.action {
                Action::Fault {
                    id, inception_deg, ..
                } if *id == o.fault => Some(*inception_deg),
                _ => None,
            }),
        ) {
            let theta = f.map_or(fc.worst_case_angle(), f64::to_radians);
            let end = o
                .cleared_at
                .unwrap_or((o.t_start + 10.0 / fc.frequency).min(scenario.duration));
            let per_cycle = if end - o.t_start > 2.0 { 16 } else { 64 };
            traces.add(fc.waveform(
                &format!("i fault {}", o.fault),
                theta,
                o.t_start,
                end,
                scenario.duration,
                per_cycle,
            ));
        }
    }
    events.sort_by(|a, b| a.t.total_cmp(&b.t));
    let mut timing = TimingDiagram::new(scenario.duration);
    for &c in fault_channel.values() {
        timing.channels.push(channels[c].clone());
    }
    for &c in device_channel.values() {
        timing.channels.push(channels[c].clone());
    }
    ScenarioResult {
        duration: scenario.duration,
        events,
        faults: outcomes,
        traces,
        timing,
        peak_heat,
        warnings,
    }
}

fn label_of(devices: &[ProtectiveDevice], key: &str) -> String {
    devices
        .iter()
        .find(|d| d.key == key)
        .map_or_else(|| key.to_string(), |d| d.label.clone())
}

#[allow(clippy::too_many_arguments)]
fn apply(
    action: &Action,
    t: f64,
    model: &dyn CircuitModel,
    devices: &[ProtectiveDevice],
    faults: &mut Vec<ActiveFault>,
    loads: &mut BTreeMap<String, f64>,
    open: &mut BTreeSet<String>,
    tripped: &mut BTreeSet<String>,
    heat: &mut [ThermalMemory],
    events: &mut Vec<TimelineEvent>,
    outcomes: &mut Vec<FaultOutcome>,
    channels: &mut Vec<DigitalChannel>,
    fault_channel: &mut BTreeMap<String, usize>,
    device_channel: &BTreeMap<String, usize>,
) {
    match action {
        Action::Fault {
            id,
            node,
            kind,
            fault_ohm,
            inception_deg,
        } => {
            let mut fault = ActiveFault {
                id: id.clone(),
                node: node.clone(),
                kind: *kind,
                fault_ohm: *fault_ohm,
                theta: 0.0,
                t_start: t,
            };
            faults.push(fault.clone());
            let cond = Conditions {
                faults,
                loads: &BTreeMap::new(),
                open,
            };
            let fc = model.fault_current(&fault, &cond);
            fault.theta = match (inception_deg, &fc) {
                (Some(d), _) => d.to_radians(),
                (None, Some(fc)) => fc.worst_case_angle(),
                _ => 0.0,
            };
            if let Some(f) = faults.iter_mut().find(|f| f.id == *id) {
                f.theta = fault.theta;
            }
            let mut ch = DigitalChannel::new(format!("Fault {id}"), "Faults");
            ch.set(0.0, Level::Low);
            ch.set(t, Level::Alarm);
            fault_channel.insert(id.clone(), channels.len());
            channels.push(ch);
            outcomes.push(FaultOutcome {
                fault: id.clone(),
                node: node.clone(),
                kind: *kind,
                t_start: t,
                ik: fc.map_or(0.0, |c| c.ik),
                peak: fc.map_or(0.0, |c| c.peak()),
                tripped: Vec::new(),
                cleared_at: None,
                cleared_by: None,
                expected: None,
                selective: false,
                let_through_i2t: None,
                start_current: fc,
            });
            events.push(TimelineEvent {
                t,
                device: None,
                kind: EventKind::FaultStart,
                text: match fc {
                    Some(fc) => format!(
                        "{} at {node} ({id}): Ik'' {:.0} A, peak {:.0} A",
                        kind.label(),
                        fc.ik,
                        fc.peak()
                    ),
                    None => format!("{} at {node} ({id})", kind.label()),
                },
            });
        }
        Action::ClearFault { id } => {
            if let Some(pos) = faults.iter().position(|f| f.id == *id) {
                faults.remove(pos);
                if let Some(&c) = fault_channel.get(id) {
                    channels[c].set(t, Level::Low);
                }
                if let Some(o) = outcomes.iter_mut().find(|o| o.fault == *id)
                    && o.cleared_at.is_none()
                {
                    o.cleared_at = Some(t);
                }
                events.push(TimelineEvent {
                    t,
                    device: None,
                    kind: EventKind::FaultCleared,
                    text: format!("Fault {id} removed"),
                });
            }
        }
        Action::Load { node, amps } => {
            loads.insert(node.clone(), *amps);
            events.push(TimelineEvent {
                t,
                device: None,
                kind: EventKind::LoadChange,
                text: format!("Load at {node} is now {amps:.1} A"),
            });
        }
        Action::Open { device } => {
            if open.insert(device.clone()) {
                if let Some(&c) = device_channel.get(device) {
                    channels[c].set(t, Level::Low);
                }
                events.push(TimelineEvent {
                    t,
                    device: Some(device.clone()),
                    kind: EventKind::OpenedByHand,
                    text: format!("{} opened", label_of(devices, device)),
                });
            }
        }
        Action::Close { device } => {
            open.remove(device);
            tripped.remove(device);
            if let Some(i) = devices.iter().position(|d| d.key == *device) {
                // A breaker that tripped is reset, its heat stays.
                heat[i].heat = heat[i].heat.min(1.0);
            }
            if let Some(&c) = device_channel.get(device) {
                channels[c].set(t, Level::High);
            }
            events.push(TimelineEvent {
                t,
                device: Some(device.clone()),
                kind: EventKind::Closed,
                text: format!("{} closed", label_of(devices, device)),
            });
        }
    }
}
