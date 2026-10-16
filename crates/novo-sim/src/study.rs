//! A fault study: the network, its protective devices and the scenarios to
//! run, in one JSON file (`<project>.faults.json` next to the project file).
//!
//! The study is what the Fault scenarios window edits and runs. Today the
//! network and the curves are written down in the file (the window makes an
//! example to start from); they are the *input* of the engine, not a copy of
//! the drawing. Reading them from the drawing's cables, breakers and parts is
//! the job of the protection model of novo-checks, which will produce the same
//! [`NetworkSpec`] and [`CurveSpec::Generic`] entries.

use serde::{Deserialize, Serialize};

use crate::fault::FaultKind;
use crate::feeder::{Impedance, RadialNetwork};
use crate::protection::{Band, ProtectiveDevice, TableCurve};
use crate::scenario::{Action, Scenario, ScenarioResult, run};

pub const STUDY_FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceSpec {
    pub busbar: String,
    /// Line-to-line voltage in V.
    pub un: f64,
    pub frequency: f64,
    /// Voltage factor c (1.05 for maximum, 0.95 for minimum currents).
    pub c: f64,
    /// Source impedance seen from the busbar, ohm.
    pub r: f64,
    pub x: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeSpec {
    pub name: String,
    pub parent: String,
    /// Positive-sequence impedance of the segment in ohm.
    pub r: f64,
    pub x: f64,
    /// Phase plus return conductor; twice the segment's when absent.
    #[serde(default)]
    pub loop_r: Option<f64>,
    #[serde(default)]
    pub loop_x: Option<f64>,
    /// Key of the protective device at the head of the segment.
    #[serde(default)]
    pub device: Option<String>,
}

/// Where a device's time-current curve comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CurveSpec {
    /// A printed curve: `(multiple of rated, t_min s, t_max s)` points.
    Table {
        rated_a: f64,
        points: Vec<(f64, f64, f64)>,
    },
    /// A curve by name, for the protection model to resolve (`"C"`, `"gG"`,
    /// `"class 10"`). The runner cannot evaluate it itself.
    Generic { name: String, rated_a: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceSpec {
    pub key: String,
    pub label: String,
    pub curve: CurveSpec,
    #[serde(default = "typical")]
    pub band: Band,
    #[serde(default = "ten_ms")]
    pub opening_time_s: f64,
    #[serde(default = "ten_minutes")]
    pub cool_tau_s: f64,
    #[serde(default)]
    pub initial_heat: f64,
}

fn typical() -> Band {
    Band::Typical
}
fn ten_ms() -> f64 {
    0.010
}
fn ten_minutes() -> f64 {
    600.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Study {
    pub format: u32,
    pub name: String,
    pub source: SourceSpec,
    pub nodes: Vec<NodeSpec>,
    pub devices: Vec<DeviceSpec>,
    pub scenarios: Vec<Scenario>,
    /// Faults the person added from the schematic editor (right-click >
    /// Add fault). Each one becomes a scenario when the study runs, see
    /// [`Study::prepared`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faults: Vec<ProjectFault>,
}

/// What a fault added from the drawing does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "what", rename_all = "snake_case")]
pub enum FaultWhat {
    /// A short circuit or open phase at the target.
    Short { kind: FaultKind },
    /// The target device opens by itself (a stuck or failed contact that
    /// drops out).
    DeviceOpens,
    /// The target device closes by itself (a contact stuck closed, welded).
    DeviceCloses,
}

impl FaultWhat {
    pub fn label(&self) -> String {
        match self {
            FaultWhat::Short { kind } => kind.label().to_owned(),
            FaultWhat::DeviceOpens => "Device opens by itself".to_owned(),
            FaultWhat::DeviceCloses => "Device closes by itself".to_owned(),
        }
    }
}

/// One fault of the project: where (a device tag, a wire or a terminal name
/// from the drawing), what, and when.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFault {
    /// `F1`, `F2`, ...: unique in the study.
    pub id: String,
    /// The name the person picked in the drawing: `-Q1`, `-X1:3`, `L1`.
    pub target: String,
    pub what: FaultWhat,
    /// Resistance of the fault itself (arc, earth contact), ohm.
    #[serde(default)]
    pub fault_ohm: f64,
    /// Seconds from the start of the run.
    #[serde(default = "default_at")]
    pub at_s: f64,
}

fn default_at() -> f64 {
    0.1
}

/// Impedance of a stub segment hung on the busbar for a target the study
/// does not list (ohm).
const STUB_R: f64 = 0.01;
const STUB_X: f64 = 0.005;

/// A study ready to run: the project faults are scenarios, targets the
/// network lacks are stub nodes, and `warnings` says what was assumed.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub study: Study,
    pub warnings: Vec<String>,
}

fn norm(name: &str) -> String {
    name.trim().trim_start_matches('-').to_lowercase()
}

impl Study {
    /// An empty study for a project that has none yet: a 400 V / 50 Hz
    /// source (the example's), no nodes, no devices.
    pub fn empty(name: &str) -> Self {
        let mut s = Study::example();
        s.name = name.to_owned();
        s.nodes.clear();
        s.devices.clear();
        s.scenarios.clear();
        s.faults.clear();
        s
    }

    /// The next free fault id (`F1`, `F2`, ...).
    pub fn next_fault_id(&self) -> String {
        (1..)
            .map(|n| format!("F{n}"))
            .find(|id| !self.faults.iter().any(|f| &f.id == id))
            .expect("an unused id exists")
    }

    /// Adds a fault and returns its id.
    pub fn add_fault(
        &mut self,
        target: &str,
        what: FaultWhat,
        fault_ohm: f64,
        at_s: f64,
    ) -> String {
        let id = self.next_fault_id();
        self.faults.push(ProjectFault {
            id: id.clone(),
            target: target.trim().to_owned(),
            what,
            fault_ohm,
            at_s,
        });
        id
    }

    /// Removes a fault by id; whether it existed.
    pub fn remove_fault(&mut self, id: &str) -> bool {
        let before = self.faults.len();
        self.faults.retain(|f| f.id != id);
        self.faults.len() != before
    }

    fn node_for(&self, target: &str) -> Option<String> {
        let t = norm(target);
        self.nodes
            .iter()
            .find(|n| norm(&n.name) == t)
            .or_else(|| {
                self.nodes
                    .iter()
                    .find(|n| n.device.as_deref().is_some_and(|d| norm(d) == t))
            })
            .map(|n| n.name.clone())
            .or_else(|| (norm(&self.source.busbar) == t).then(|| self.source.busbar.clone()))
    }

    fn device_for(&self, target: &str) -> Option<String> {
        let t = norm(target);
        self.devices
            .iter()
            .find(|d| norm(&d.key) == t)
            .or_else(|| {
                self.devices.iter().find(|d| {
                    d.label
                        .to_lowercase()
                        .split_whitespace()
                        .any(|w| w.trim_start_matches('-') == t)
                })
            })
            .map(|d| d.key.clone())
    }

    /// The study with the project faults turned into scenarios (after the
    /// hand-written ones). A fault whose target is not a node of the network
    /// gets a stub node on the busbar, and the warning says so; a device
    /// fault on an unknown device is left out with a warning.
    pub fn prepared(&self) -> Prepared {
        let mut study = self.clone();
        let mut warnings = Vec::new();
        for f in &self.faults {
            let name = format!("{}: {} at {}", f.id, f.what.label(), f.target);
            let mut scenario = match &f.what {
                FaultWhat::Short { kind } => {
                    let node = match study.node_for(&f.target) {
                        Some(n) => n,
                        None => {
                            let node = f.target.trim().to_owned();
                            study.nodes.push(NodeSpec {
                                name: node.clone(),
                                parent: study.source.busbar.clone(),
                                r: STUB_R,
                                x: STUB_X,
                                loop_r: None,
                                loop_x: None,
                                device: None,
                            });
                            warnings.push(format!(
                                "{}: {} is not in the study's network; it is assumed to hang on \
                                 {} through {STUB_R} + j{STUB_X} ohm with no protection. Add it \
                                 as a node in the study file for real numbers.",
                                f.id, f.target, study.source.busbar
                            ));
                            node
                        }
                    };
                    let tail = if *kind == FaultKind::OpenPhase {
                        60.0
                    } else {
                        0.5
                    };
                    Scenario::new(name, f.at_s + tail).at(
                        f.at_s,
                        Action::Fault {
                            id: f.id.clone(),
                            node,
                            kind: *kind,
                            fault_ohm: f.fault_ohm,
                            inception_deg: None,
                        },
                    )
                }
                FaultWhat::DeviceOpens | FaultWhat::DeviceCloses => {
                    let Some(device) = study.device_for(&f.target) else {
                        warnings.push(format!(
                            "{}: {} is not a protective device of the study, so \"{}\" was \
                             left out.",
                            f.id,
                            f.target,
                            f.what.label()
                        ));
                        continue;
                    };
                    let action = if f.what == FaultWhat::DeviceOpens {
                        Action::Open { device }
                    } else {
                        Action::Close { device }
                    };
                    Scenario::new(name, f.at_s + 1.0).at(f.at_s, action)
                }
            };
            scenario.description = format!("Added from the drawing: {}.", f.what.label());
            study.scenarios.push(scenario);
        }
        Prepared { study, warnings }
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a study always serializes")
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let study: Study = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if study.format > STUDY_FORMAT {
            return Err(format!(
                "study format {} is newer than this program understands ({STUDY_FORMAT})",
                study.format
            ));
        }
        Ok(study)
    }

    /// The network, or a sentence saying what is wrong with the file.
    pub fn network(&self) -> Result<RadialNetwork, String> {
        let s = &self.source;
        let positive = |v: f64| v.is_finite() && v > 0.0;
        if !positive(s.un) || !positive(s.frequency) || !positive(s.c) {
            return Err(
                "The source needs a voltage, frequency and voltage factor above zero".into(),
            );
        }
        if !(s.r.is_finite() && s.x.is_finite() && s.r >= 0.0 && s.x >= 0.0 && s.r + s.x > 0.0) {
            return Err("The source impedance must be zero or more, and not zero in total".into());
        }
        let mut net = RadialNetwork::new(&s.busbar, s.un, s.frequency, Impedance::new(s.r, s.x))
            .with_voltage_factor(s.c);
        let mut known = vec![s.busbar.as_str()];
        for n in &self.nodes {
            if !known.contains(&n.parent.as_str()) {
                return Err(format!(
                    "{} hangs on {}, which is not above it in the list",
                    n.name, n.parent
                ));
            }
            let finite = |v: f64| v.is_finite() && v >= 0.0;
            if !finite(n.r)
                || !finite(n.x)
                || !n.loop_r.is_none_or(finite)
                || !n.loop_x.is_none_or(finite)
            {
                return Err(format!(
                    "{} has an impedance that is negative or not a number",
                    n.name
                ));
            }
            let z = Impedance::new(n.r, n.x);
            let zl = Impedance::new(n.loop_r.unwrap_or(2.0 * n.r), n.loop_x.unwrap_or(2.0 * n.x));
            net = net.segment_loop(&n.name, &n.parent, z, zl, n.device.as_deref());
            known.push(&n.name);
        }
        Ok(net)
    }

    /// The protective devices; fails on a curve that only the protection
    /// model can resolve.
    pub fn devices(&self) -> Result<Vec<ProtectiveDevice>, String> {
        self.devices
            .iter()
            .map(|d| {
                let positive = |v: f64| v.is_finite() && v > 0.0;
                if !positive(d.opening_time_s + 1e-300)
                    || d.opening_time_s < 0.0
                    || !positive(d.cool_tau_s)
                    || !d.initial_heat.is_finite()
                {
                    return Err(format!(
                        "{} has an opening time, cooling time or starting heat that is not valid",
                        d.label
                    ));
                }
                let CurveSpec::Table { rated_a, points } = &d.curve else {
                    return Err(format!(
                        "{} has a named curve that needs the protection model",
                        d.label
                    ));
                };
                if !positive(*rated_a)
                    || points.is_empty()
                    || points
                        .iter()
                        .any(|p| !(positive(p.0) && positive(p.1) && positive(p.2) && p.2 >= p.1))
                {
                    return Err(format!(
                        "{} needs a rated current above zero and curve points with positive \
                         multiples and times (latest time not before earliest)",
                        d.label
                    ));
                }
                let mut dev = ProtectiveDevice::new(
                    d.key.clone(),
                    d.label.clone(),
                    TableCurve::new(*rated_a, points.clone()),
                )
                .with_band(d.band)
                .with_opening_time(d.opening_time_s)
                .with_initial_heat(d.initial_heat);
                dev.cool_tau = d.cool_tau_s;
                Ok(dev)
            })
            .collect()
    }

    pub fn run(&self, scenario: usize) -> Result<ScenarioResult, String> {
        let s = self.scenarios.get(scenario).ok_or("No such scenario")?;
        let problems = s.problems();
        if let Some(p) = problems.first() {
            return Err(p.clone());
        }
        Ok(run(&self.network()?, &self.devices()?, s))
    }

    /// A feeder with a fuse, a motor starter and a pump branch, and four
    /// scenarios that show what the window does: a short circuit at the pump,
    /// the same one with the breaker as the only protection, a motor start,
    /// and an open phase.
    pub fn example() -> Self {
        let table = |rated: f64, pts: &[(f64, f64, f64)]| CurveSpec::Table {
            rated_a: rated,
            points: pts.to_vec(),
        };
        let fault = |id: &str, node: &str, kind| Action::Fault {
            id: id.into(),
            node: node.into(),
            kind,
            fault_ohm: 0.0,
            inception_deg: None,
        };
        Study {
            faults: Vec::new(),
            format: STUDY_FORMAT,
            name: "Pump feeder (example)".into(),
            source: SourceSpec {
                busbar: "Busbar".into(),
                un: 400.0,
                frequency: 50.0,
                c: 1.05,
                r: 0.002,
                x: 0.009,
            },
            nodes: vec![
                NodeSpec {
                    name: "Panel".into(),
                    parent: "Busbar".into(),
                    r: 0.004,
                    x: 0.002,
                    loop_r: None,
                    loop_x: None,
                    device: Some("Q1".into()),
                },
                NodeSpec {
                    name: "Pump".into(),
                    parent: "Panel".into(),
                    r: 0.060,
                    x: 0.004,
                    loop_r: None,
                    loop_x: None,
                    device: Some("F2".into()),
                },
                NodeSpec {
                    name: "Motor".into(),
                    parent: "Pump".into(),
                    r: 0.001,
                    x: 0.0,
                    loop_r: None,
                    loop_x: None,
                    device: Some("K3".into()),
                },
            ],
            devices: vec![
                DeviceSpec {
                    key: "Q1".into(),
                    label: "-Q1 gG 100 A".into(),
                    curve: table(
                        100.0,
                        &[
                            (1.6, 3600.0, 7200.0),
                            (2.5, 100.0, 300.0),
                            (5.0, 2.0, 8.0),
                            (10.0, 0.2, 0.6),
                            (50.0, 0.02, 0.06),
                        ],
                    ),
                    band: Band::Typical,
                    opening_time_s: 0.0,
                    cool_tau_s: 600.0,
                    initial_heat: 0.0,
                },
                DeviceSpec {
                    key: "F2".into(),
                    label: "-F2 C 16 A (example curve)".into(),
                    curve: table(
                        16.0,
                        &[
                            (1.45, 60.0, 3600.0),
                            (2.55, 1.0, 120.0),
                            (5.0, 0.02, 10.0),
                            (10.0, 0.005, 0.02),
                            (50.0, 0.004, 0.01),
                        ],
                    ),
                    band: Band::Typical,
                    opening_time_s: 0.010,
                    cool_tau_s: 600.0,
                    initial_heat: 0.0,
                },
                DeviceSpec {
                    key: "K3".into(),
                    label: "-K3 overload relay class 10".into(),
                    curve: table(
                        10.0,
                        &[
                            (1.2, 600.0, 7200.0),
                            (1.5, 120.0, 480.0),
                            (7.2, 4.0, 10.0),
                            (20.0, 4.0, 10.0),
                        ],
                    ),
                    band: Band::Typical,
                    opening_time_s: 0.05,
                    cool_tau_s: 600.0,
                    initial_heat: 0.0,
                },
            ],
            scenarios: vec![
                {
                    let mut s = Scenario::new("Short circuit at the pump", 0.5)
                        .at(0.1, fault("SC1", "Pump", FaultKind::ThreePhase));
                    s.description = "Three-phase fault at the pump terminals; does the pump breaker clear it alone?".into();
                    s
                },
                {
                    let mut s = Scenario::new("Phase to neutral at the pump", 0.5)
                        .at(0.1, fault("SC1", "Pump", FaultKind::PhaseNeutral));
                    s.description = "The lowest fault current on a TN-S system: is the magnetic trip still reached?".into();
                    s
                },
                {
                    let mut s =
                        Scenario::new("Pump start", 30.0).motor_start(1.0, "Motor", 49.0, 2.0, 7.0);
                    s.description = "Direct-on-line start at seven times the running current; breaker and relay ride through.".into();
                    s
                },
                {
                    let mut s = Scenario::new("Open phase while running", 900.0)
                        .at(
                            0.0,
                            Action::Load {
                                node: "Motor".into(),
                                amps: 8.0,
                            },
                        )
                        .at(10.0, fault("OP", "Motor", FaultKind::OpenPhase));
                    s.description = "A phase conductor opens; the motor draws 1.7 times more on the other two and the relay trips; the breaker never sees an overload.".into();
                    s
                },
            ],
        }
    }
}
