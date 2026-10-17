//! Safety functions of the project: sensor, logic and actuator blocks bound
//! to project devices, their stored form and the check that raises Problems
//! when the achieved performance level is below the required one.
//!
//! **Calculation aid, not a certification.** The numbers come from
//! `novo-safety` (ISO 13849-1 simplified method; PFHd of categories 2 to 4 is
//! a closed-form model, not Table K.1). A part states its reliability
//! itself or takes it from the bound device's properties
//! [`B10D_CYCLES`] and [`MTTFD_YEARS`] (a device's own property, else the
//! first of its parts that has it). A part with neither is reported, and the
//! function is not rated until it is filled in.
//!
//! The functions live in the project information as one JSON text under
//! [`FUNCTIONS_KEY`], so they save, undo and travel with the project.

use std::collections::BTreeSet;

use novo_core::{AnyId, DeviceId, DeviceTag, Project, Value};
use novo_safety::{
    Category, Combined, Component, Frequency, PerformanceLevel, Probability, Severity, Subsystem,
    SubsystemResult, combine_series, mttfd_from_b10d, operations_per_year, required_pl,
};
use serde::{Deserialize, Serialize};

use crate::{Context, Problem, ProblemKind};

/// Project information key holding the JSON list of safety functions.
pub const FUNCTIONS_KEY: &str = "safety.functions";
/// Device or part property: B10d in cycles.
pub const B10D_CYCLES: &str = "b10d_cycles";
/// Device or part property: MTTFd in years.
pub const MTTFD_YEARS: &str = "mttfd_years";

/// The role of a block in the chain from detection to action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Sensor,
    Logic,
    Actuator,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Sensor => "Sensor",
            Role::Logic => "Logic",
            Role::Actuator => "Actuator",
        }
    }
}

/// How a part's reliability is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Basis {
    /// MTTFd in years.
    Mttfd,
    /// B10d in cycles; MTTFd follows from the duty.
    B10d,
}

/// One part of a channel, optionally bound to a project device by its tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartRef {
    pub name: String,
    /// Tag text of the bound device (`-S1`); `None` for a part not in the project.
    #[serde(default)]
    pub device: Option<String>,
    pub basis: Basis,
    /// Reliability value; `None` takes it from the bound device. Zero or
    /// less means an own value was chosen and is not typed yet.
    #[serde(default)]
    pub value: Option<f64>,
}

impl PartRef {
    pub fn new(name: impl Into<String>) -> Self {
        PartRef {
            name: name.into(),
            device: None,
            basis: Basis::B10d,
            value: None,
        }
    }
}

/// A sensor, logic or actuator block: one subsystem of the function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub role: Role,
    pub name: String,
    /// Category label: `B`, `1`, `2`, `3` or `4`.
    pub category: String,
    pub dc_percent: f64,
    pub ccf: u32,
    pub channel1: Vec<PartRef>,
    #[serde(default)]
    pub channel2: Vec<PartRef>,
}

impl Block {
    pub fn new(role: Role, name: impl Into<String>) -> Self {
        Block {
            role,
            name: name.into(),
            category: "3".into(),
            dc_percent: 90.0,
            ccf: 65,
            channel1: vec![PartRef::new("Part 1")],
            channel2: Vec::new(),
        }
    }

    fn category(&self) -> Category {
        match self.category.as_str() {
            "B" => Category::B,
            "1" => Category::C1,
            "2" => Category::C2,
            "4" => Category::C4,
            _ => Category::C3,
        }
    }
}

/// One safety function with its risk graph, duty and blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafetyFunction {
    pub name: String,
    /// `S1` or `S2`.
    pub severity: String,
    /// `F1` or `F2`.
    pub frequency: String,
    /// `P1` or `P2`.
    pub avoidance: String,
    pub days: f64,
    pub hours: f64,
    pub cycle_s: f64,
    pub blocks: Vec<Block>,
}

impl SafetyFunction {
    /// A new function with an E-stop, a safety relay and contactors.
    pub fn new(name: impl Into<String>) -> Self {
        SafetyFunction {
            name: name.into(),
            severity: "S2".into(),
            frequency: "F1".into(),
            avoidance: "P1".into(),
            days: 220.0,
            hours: 16.0,
            cycle_s: 600.0,
            blocks: vec![
                Block::new(Role::Sensor, "E-stop"),
                Block::new(Role::Logic, "Safety relay"),
                Block::new(Role::Actuator, "Contactors"),
            ],
        }
    }

    pub fn required(&self) -> PerformanceLevel {
        let s = if self.severity == "S1" {
            Severity::S1
        } else {
            Severity::S2
        };
        let f = if self.frequency == "F2" {
            Frequency::F2
        } else {
            Frequency::F1
        };
        let p = if self.avoidance == "P2" {
            Probability::P2
        } else {
            Probability::P1
        };
        required_pl(s, f, p)
    }

    pub fn nop(&self) -> f64 {
        operations_per_year(self.days, self.hours, self.cycle_s)
    }
}

/// Reads the stored functions; an empty or unreadable value gives none.
pub fn functions(project: &Project) -> Vec<SafetyFunction> {
    match project.info().properties.get(FUNCTIONS_KEY) {
        Some(Value::Text(t)) => serde_json::from_str(t).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Whether a value is stored under [`FUNCTIONS_KEY`] that cannot be read
/// back (hand-edited or from a newer version). [`functions`] then gives none.
pub fn unreadable(project: &Project) -> bool {
    match project.info().properties.get(FUNCTIONS_KEY) {
        Some(Value::Text(t)) => serde_json::from_str::<Vec<SafetyFunction>>(t).is_err(),
        Some(_) => true,
        None => false,
    }
}

/// The stored form of `list`; `None` (remove the property) when empty.
pub fn encode(list: &[SafetyFunction]) -> Option<Value> {
    (!list.is_empty()).then(|| Value::Text(serde_json::to_string(list).unwrap_or_default()))
}

/// The devices carrying exactly the tag a part is bound to.
pub fn devices_with_tag(project: &Project, tag: &str) -> Vec<DeviceId> {
    DeviceTag::parse(tag)
        .map(|t| project.devices_by_tag(&t))
        .unwrap_or_default()
}

/// Finds the device a bound tag names: an exact tag match, and only when
/// exactly one device has it.
pub fn find_device(project: &Project, tag: &str) -> Option<DeviceId> {
    match devices_with_tag(project, tag).as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// B10d or MTTFd from a device or, failing that, its parts.
fn device_data(project: &Project, device: DeviceId, basis: Basis) -> Option<f64> {
    let key = match basis {
        Basis::B10d => B10D_CYCLES,
        Basis::Mttfd => MTTFD_YEARS,
    };
    let dev = project.device(device)?;
    let num = |v: &Value| match v {
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        Value::Text(t) => t.trim().replace(',', ".").parse().ok(),
        Value::Bool(_) => None,
    };
    dev.properties
        .get(key)
        .and_then(num)
        .or_else(|| {
            dev.parts
                .iter()
                .filter_map(|p| project.part(*p))
                .find_map(|p| p.properties.get(key).and_then(num))
        })
        .filter(|v| *v > 0.0)
}

/// What a part resolves to.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolved {
    Component(Component),
    /// Bound tag matches no device (0) or several (n).
    NoDevice(String, usize),
    /// No value given and none on the device.
    NoData,
    /// "Own value" was chosen but no positive number has been typed yet.
    NotEntered,
}

/// Resolves one part to a component for the engine.
pub fn resolve(project: &Project, part: &PartRef, nop: f64) -> Resolved {
    let device = match part.device.as_deref().filter(|t| !t.trim().is_empty()) {
        Some(tag) => match find_device(project, tag) {
            Some(id) => Some(id),
            None => {
                return Resolved::NoDevice(tag.to_owned(), devices_with_tag(project, tag).len());
            }
        },
        None => None,
    };
    // `Some(v)` is the part's own value; zero or less means "own value"
    // was chosen and nothing is typed yet, which never falls back to the
    // device or to a placeholder.
    let value = match part.value {
        Some(v) if v > 0.0 => Some(v),
        Some(_) => return Resolved::NotEntered,
        None => device.and_then(|d| device_data(project, d, part.basis)),
    };
    match value {
        None => Resolved::NoData,
        Some(v) => Resolved::Component(match part.basis {
            Basis::Mttfd => Component::new(&part.name, v),
            Basis::B10d => Component::new(&part.name, mttfd_from_b10d(v, nop)),
        }),
    }
}

/// The rating of one block.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockRating {
    pub name: String,
    pub role: Role,
    /// `None` while a part has no data or a missing device.
    pub result: Option<SubsystemResult>,
    /// What stops the rating or needs a look, one sentence each.
    pub binding: Vec<String>,
    /// The devices bound in this block.
    pub devices: Vec<DeviceId>,
}

/// The rating of one function.
#[derive(Debug, Clone, PartialEq)]
pub struct Rating {
    pub name: String,
    pub required: PerformanceLevel,
    pub blocks: Vec<BlockRating>,
    /// Series total; `None` while a block cannot be rated or none exists.
    pub total: Option<Combined>,
}

impl Rating {
    /// `Some(true)` met, `Some(false)` not met, `None` not rated yet.
    pub fn met(&self) -> Option<bool> {
        self.total.as_ref().map(|t| t.pl >= self.required)
    }
}

fn rate_block(project: &Project, block: &Block, nop: f64) -> BlockRating {
    let mut binding = Vec::new();
    let mut devices = BTreeSet::new();
    let sane = nop.is_finite()
        && nop > 0.0
        && (0.0..=100.0).contains(&block.dc_percent)
        && block.ccf <= 100;
    if !sane {
        binding.push(format!(
            "{} block \"{}\" has a duty, DCavg or CCF value out of range, so it is not rated",
            block.role.label(),
            block.name
        ));
    }
    let mut channel = |parts: &[PartRef]| -> Vec<Component> {
        let mut out = Vec::new();
        for p in parts {
            if let Some(d) = p.device.as_deref().and_then(|t| find_device(project, t)) {
                devices.insert(d);
            }
            match resolve(project, p, nop) {
                Resolved::Component(c) => out.push(c),
                Resolved::NoDevice(tag, 0) => binding.push(format!(
                    "{} part \"{}\" is bound to {tag}, which is not a device in the project",
                    block.role.label(),
                    p.name
                )),
                Resolved::NoDevice(tag, n) => binding.push(format!(
                    "{} part \"{}\" is bound to {tag}, which matches {n} devices",
                    block.role.label(),
                    p.name
                )),
                Resolved::NotEntered => binding.push(format!(
                    "{} part \"{}\" has an own value that is not entered yet",
                    block.role.label(),
                    p.name
                )),
                Resolved::NoData => binding.push(format!(
                    "{} part \"{}\" has no B10d or MTTFd{}",
                    block.role.label(),
                    p.name,
                    p.device
                        .as_deref()
                        .map(|t| format!(" on {t}"))
                        .unwrap_or_default()
                )),
            }
        }
        out
    };
    let c1 = channel(&block.channel1);
    let c2 = channel(&block.channel2);
    let complete = binding.is_empty() && !c1.is_empty();
    if block.channel1.is_empty() {
        binding.push(format!(
            "{} block \"{}\" has no parts",
            block.role.label(),
            block.name
        ));
    }
    let result = complete.then(|| {
        Subsystem {
            name: block.name.clone(),
            category: block.category(),
            channel1: c1,
            channel2: c2,
            dc_avg_percent: block.dc_percent,
            ccf_score: block.ccf,
        }
        .calculate()
    });
    BlockRating {
        name: block.name.clone(),
        role: block.role,
        result,
        binding,
        devices: devices.into_iter().collect(),
    }
}

/// Rates one function against the project.
pub fn rate(project: &Project, f: &SafetyFunction) -> Rating {
    let nop = f.nop();
    let blocks: Vec<BlockRating> = f
        .blocks
        .iter()
        .map(|b| rate_block(project, b, nop))
        .collect();
    let results: Option<Vec<SubsystemResult>> = blocks.iter().map(|b| b.result.clone()).collect();
    let total = results
        .filter(|r| !r.is_empty())
        .map(|r| combine_series(&r));
    Rating {
        name: f.name.clone(),
        required: f.required(),
        blocks,
        total,
    }
}

/// Raises the safety Problems; silent when the project has no functions.
pub(crate) fn run(cx: &Context<'_>, out: &mut Vec<Problem>) {
    if unreadable(cx.project) {
        out.push(Problem::new(
            ProblemKind::SafetyBinding,
            Vec::new(),
            format!("The stored safety functions ({FUNCTIONS_KEY}) cannot be read and are ignored"),
        ));
    }
    for f in functions(cx.project) {
        let rating = rate(cx.project, &f);
        let mut devices: Vec<AnyId> = Vec::new();
        for b in &rating.blocks {
            devices.extend(b.devices.iter().map(|d| AnyId::from(*d)));
            for text in &b.binding {
                out.push(Problem::new(
                    ProblemKind::SafetyBinding,
                    b.devices.iter().map(|d| AnyId::from(*d)).collect(),
                    format!("Safety function \"{}\": {text}", f.name),
                ));
            }
            if let Some(r) = &b.result {
                for p in &r.problems {
                    out.push(Problem::new(
                        ProblemKind::SafetyBinding,
                        b.devices.iter().map(|d| AnyId::from(*d)).collect(),
                        format!("Safety function \"{}\", {}: {}", f.name, b.name, p.message),
                    ));
                }
            }
        }
        if let (Some(total), Some(false)) = (&rating.total, rating.met()) {
            out.push(Problem::new(
                ProblemKind::SafetyLevelNotMet,
                devices,
                format!(
                    "Safety function \"{}\" achieves {} but requires {} (PFHd {:.2e} /h, model, not exact)",
                    f.name, total.pl, rating.required, total.pfhd
                ),
            ));
        }
    }
}
