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
}

impl Study {
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
                let CurveSpec::Table { rated_a, points } = &d.curve else {
                    return Err(format!(
                        "{} has a named curve that needs the protection model",
                        d.label
                    ));
                };
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
