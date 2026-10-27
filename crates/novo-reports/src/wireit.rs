//! Wire-it: the order to run the wires by hand, one connection at a time, and
//! what each step needs at the bench.
//!
//! A pure function of the project. It reads the wire production jobs
//! ([`wire_cut_jobs`]: lengths, strip, ferrules, labels, the ducts each wire
//! lies in) and puts them in a bench sequence:
//!
//! 1. Protective earth, then power (mains, motor, 2.5 mm2 and up), then the
//!    24 V supply, then signals. Heavy, stiff wires go in first while the
//!    ducts are empty; the thin wires are laid last on top.
//! 2. Inside one class the wires of one duct track stay together, so a duct
//!    is filled in one visit. Tracks run shortest first (by their shortest
//!    wire), unrouted wires and cable cores after the routed ones.
//! 3. Inside a track: short before long, then by wire number.
//!
//! After each step the plan says how full every duct the wire lies in is,
//! counted the way the routing counts it (wire area against the duct's
//! allowed fill), so the last step of a duct agrees with the route screen.
//!
//! The classes are a model of the usual practice, not a reading of the
//! standard: a wire is power when its net carries a line or neutral
//! potential, a voltage above 60 V, or when it is 2.5 mm2 or more. The order
//! of a step never depends on what is ticked, so a half-wired cabinet
//! resumes where it stopped.
//!
//! Progress is a text property on the drawn wires of the step ([`DONE_KEY`]):
//! the keys of the connections run so far, `;`-separated. A step is keyed by
//! its two pins, not by its wires, because a T-junction is one drawn net and
//! several connections to run. It saves with the project, travels in a copy
//! and undoes with it; a key whose connection no longer exists is counted by
//! [`orphaned`] and left alone.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use novo_core::{PotentialKind, Project, Value, WireId, natural_cmp};

use crate::lengths::RunPath;
use crate::wirecut::{WireCutJob, duct_capacities_mm2, duct_names, wire_cut_jobs};

/// The wire property that lists the connections run (see [`WireItStep::key`]).
pub const DONE_KEY: &str = "wireit.done";

/// Basis texts of a length that is not a measurement of the finished wire.
const WEAK_BASES: [&str; 4] = ["estimate", "free air", "routed, no depth", "door, no gap"];

/// Which pass of the bench sequence a wire belongs to, in the order the
/// passes run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WireClass {
    /// Protective earth (green-yellow, or a PE potential).
    Earth,
    /// Mains and motor wiring.
    Power,
    /// The 24 V (extra-low voltage) supply: DC potentials.
    Supply,
    /// Control and signal wires.
    Signal,
}

impl WireClass {
    pub fn name(self) -> &'static str {
        match self {
            WireClass::Earth => "Protective earth",
            WireClass::Power => "Power",
            WireClass::Supply => "24 V supply",
            WireClass::Signal => "Signals",
        }
    }
}

/// One duct a wire lies in, and how full it is once the wire is in.
#[derive(Debug, Clone, PartialEq)]
pub struct DuctStep {
    pub duct: u64,
    /// `D2`.
    pub name: String,
    /// Fill of the allowed cross-section after this wire, `0.63` for 63 %;
    /// `None` when the duct has no size, so its fill cannot be told.
    pub fill_after: Option<f64>,
    /// And before it.
    pub fill_before: Option<f64>,
    /// Wires in the duct after this one (this one included).
    pub wires_after: usize,
}

impl DuctStep {
    /// `D2 now 63 %`, or `D2 (no size set)`.
    pub fn text(&self) -> String {
        match self.fill_after {
            Some(f) => format!("{} now {:.0} %", self.name, f * 100.0),
            None => format!("{} (no size set)", self.name),
        }
    }

    /// Over the allowed fill after this wire.
    pub fn over(&self) -> bool {
        self.fill_after.is_some_and(|f| f > 1.0)
    }
}

/// One connection to run.
#[derive(Debug, Clone, PartialEq)]
pub struct WireItStep {
    /// 1-based position in the sequence.
    pub number: usize,
    /// The connection's identity, the same after a reorder or a rename:
    /// its two pins, `12.0>15.2`.
    pub key: String,
    /// The drawn wires that make the connection. A T-junction is one set of
    /// drawn wires and several steps.
    pub wires: Vec<WireId>,
    pub wire_number: String,
    pub class: WireClass,
    pub colour: String,
    pub cross_section_mm2: Option<f64>,
    /// `=A1+CAB-K1:13`.
    pub from: String,
    pub to: String,
    /// What the machine or the knife cuts, end allowances included.
    pub cut_mm: f64,
    /// The routed length without the allowances.
    pub length_mm: f64,
    /// What each end adds to the routed length.
    pub allowance_mm: f64,
    pub strip_from_mm: Option<f64>,
    pub strip_to_mm: Option<f64>,
    /// Ferrule or lug for each end, `None` when nothing is proposed.
    pub fitting_from: Option<String>,
    pub fitting_to: Option<String>,
    /// What is written on the label flag at each end.
    pub label_from: String,
    pub label_to: String,
    /// How the length was found (`routed`, `estimate`, ...).
    pub basis: String,
    /// `D1 > D3`, `in cable` or `not routed`.
    pub track: String,
    /// The ducts the wire lies in, with the fill each reaches.
    pub ducts: Vec<DuctStep>,
    /// The wire is a core of a cable.
    pub cable_core: bool,
    /// The tag of that cable, empty for a hookup wire.
    pub cable_tag: String,
    /// Where the connection runs on its plate; `None` when it is not routed
    /// on one (a wire to another cabinet, an unplaced device).
    pub path: Option<RunPath>,
}

impl WireItStep {
    /// The length is a planning figure, not a measurement of the finished
    /// wire.
    pub fn estimated(&self) -> bool {
        WEAK_BASES.contains(&self.basis.as_str())
    }

    /// Millimetres of hookup wire to cut for this step: none for a core,
    /// which is run with its cable and never cut from wire stock.
    pub fn cut_to_do_mm(&self) -> f64 {
        if self.cable_core { 0.0 } else { self.cut_mm }
    }

    /// What the bench does: `cut 1250 mm` for a wire, `run cable =A1-W1,
    /// 12020 mm` for a core.
    pub fn run_or_cut_text(&self) -> String {
        let mm = novo_layout::drill::mm(self.cut_mm);
        if self.cable_core {
            let cable = if self.cable_tag.is_empty() {
                "its cable"
            } else {
                &self.cable_tag
            };
            format!("run cable {cable}, {mm} mm")
        } else {
            format!(
                "cut {}{mm} mm",
                if self.estimated() { "about " } else { "" }
            )
        }
    }

    /// `1.5 mm2 black`.
    pub fn kind_text(&self) -> String {
        let size = self
            .cross_section_mm2
            .map(|s| format!("{} mm2", trim(s)))
            .unwrap_or_else(|| "size not set".to_owned());
        if self.colour.is_empty() {
            size
        } else {
            format!("{size} {}", self.colour)
        }
    }

    /// `strip 10 mm`, `strip 8 / 10 mm` when the ends differ; empty when the
    /// size is unknown.
    pub fn strip_text(&self) -> String {
        match (self.strip_from_mm, self.strip_to_mm) {
            (None, None) => String::new(),
            (Some(a), Some(b)) if (a - b).abs() < 0.05 => format!("strip {} mm", trim(a)),
            (a, b) => format!(
                "strip {} / {} mm",
                a.map_or("?".to_owned(), trim),
                b.map_or("?".to_owned(), trim)
            ),
        }
    }

    /// The ferrule or lug of each end: one text when both agree.
    pub fn fitting_text(&self) -> String {
        match (&self.fitting_from, &self.fitting_to) {
            (None, None) => "no ferrule proposed".to_owned(),
            (Some(a), Some(b)) if a == b => a.clone(),
            (a, b) => format!(
                "{} / {}",
                a.as_deref().unwrap_or("none"),
                b.as_deref().unwrap_or("none")
            ),
        }
    }

    /// `D1 now 40 %, D3 now 63 %`; the track in words when no duct is used.
    pub fn fill_text(&self) -> String {
        if self.ducts.is_empty() {
            return match self.track.as_str() {
                "in cable" => "cable core: no duct fill".to_owned(),
                _ => "not routed through a duct".to_owned(),
            };
        }
        let text = self
            .ducts
            .iter()
            .map(DuctStep::text)
            .collect::<Vec<_>>()
            .join(", ");
        if self.cross_section_mm2.is_none() {
            format!("{text} (size not set, default diameter)")
        } else {
            text
        }
    }

    /// Any duct the wire lies in is over its allowed fill after this wire.
    pub fn over_filled(&self) -> bool {
        self.ducts.iter().any(DuctStep::over)
    }
}

/// The whole sequence.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WireItPlan {
    pub steps: Vec<WireItStep>,
}

impl WireItPlan {
    /// The step that has the drawn wire `wire`.
    pub fn step_of(&self, wire: WireId) -> Option<&WireItStep> {
        self.steps.iter().find(|s| s.wires.contains(&wire))
    }

    /// The steps of one class, in order.
    pub fn of_class(&self, class: WireClass) -> impl Iterator<Item = &WireItStep> {
        self.steps.iter().filter(move |s| s.class == class)
    }

    /// Metres of wire to cut for every step not yet done.
    pub fn cut_m_left(&self, project: &Project) -> f64 {
        self.steps
            .iter()
            .filter(|s| !is_done(project, s))
            .map(|s| s.cut_to_do_mm())
            .sum::<f64>()
            / 1000.0
    }
}

/// How far the wiring is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    /// Index into the steps of the first step not done.
    pub next: Option<usize>,
}

impl Progress {
    pub fn complete(&self) -> bool {
        self.total > 0 && self.done == self.total
    }
}

fn keys_on(project: &Project, wire: WireId) -> Vec<String> {
    match project.wire(wire).and_then(|w| w.properties.get(DONE_KEY)) {
        Some(Value::Text(t)) => t
            .split(';')
            .filter(|k| !k.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Whether the connection of the step is marked run on any of its drawn
/// wires.
pub fn is_done(project: &Project, step: &WireItStep) -> bool {
    step.wires
        .iter()
        .any(|w| keys_on(project, *w).contains(&step.key))
}

/// Marks the step run (`on`) or not yet run, on every drawn wire of the
/// connection. Other connections of the same drawn wires are untouched.
pub fn set_done(tx: &mut novo_core::Tx<'_>, step: &WireItStep, on: bool) -> novo_core::Result<()> {
    for w in &step.wires {
        let mut keys = keys_on(tx.project(), *w);
        keys.retain(|k| *k != step.key);
        if on {
            keys.push(step.key.clone());
            keys.sort();
        }
        let value = (!keys.is_empty()).then(|| Value::Text(keys.join(";")));
        tx.set_property(*w, DONE_KEY, value)?;
    }
    Ok(())
}

/// Counts what is ticked.
pub fn progress(project: &Project, plan: &WireItPlan) -> Progress {
    let mut done = 0;
    let mut next = None;
    for (i, s) in plan.steps.iter().enumerate() {
        if is_done(project, s) {
            done += 1;
        } else if next.is_none() {
            next = Some(i);
        }
    }
    Progress {
        done,
        total: plan.steps.len(),
        next,
    }
}

/// Ticks kept on wires that no step of the plan matches any more (a
/// connection deleted or rewired after it was run). They stay on the wires;
/// the sheet says how many there are.
pub fn orphaned(project: &Project, plan: &WireItPlan) -> usize {
    let known: BTreeSet<&str> = plan.steps.iter().map(|s| s.key.as_str()).collect();
    let mut stray = BTreeSet::new();
    for w in project.wires() {
        for k in keys_on(project, w.id) {
            if !known.contains(k.as_str()) {
                stray.insert(k);
            }
        }
    }
    stray.len()
}

/// The sequence of the project, shared per project revision (it routes the
/// project, see [`wire_cut_jobs`]).
pub fn wire_it_plan(project: &Project) -> Arc<WireItPlan> {
    struct Cached(Arc<WireItPlan>);
    project.derived(|p| Cached(Arc::new(build(p)))).0.clone()
}

fn trim(v: f64) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}").trim_end_matches('0').to_owned()
    }
}

fn class_of(project: &Project, job: &WireCutJob) -> WireClass {
    use novo_checks::standards::Colour;
    let net = project.netlist();
    let potential = job
        .wires
        .iter()
        .filter_map(|id| project.wire(*id))
        .find_map(|w| {
            net.potential_of(w.from)
                .or_else(|| net.potential_of(w.to))
                .and_then(|p| project.potential(p))
        });
    classify(
        potential.map(|p| (p.kind, p.voltage)),
        Colour::parse(&job.colour) == Some(Colour::GreenYellow),
        job.cross_section_mm2,
    )
}

/// The pass of a wire from what the drawing says about it: the potential of
/// its net (kind and nominal voltage), whether it is green-yellow and its
/// size. A green-yellow wire is protective earth whatever net it was drawn
/// on (a check flags the mismatch); the net decides next; the size is the
/// fallback for a net with no potential.
fn classify(
    potential: Option<(PotentialKind, Option<f64>)>,
    green_yellow: bool,
    section_mm2: Option<f64>,
) -> WireClass {
    if green_yellow {
        return WireClass::Earth;
    }
    let high = |v: Option<f64>| v.is_some_and(|v| v > 60.0);
    match potential {
        Some((PotentialKind::ProtectiveEarth | PotentialKind::Pen, _)) => WireClass::Earth,
        Some((PotentialKind::Line | PotentialKind::Neutral, _)) => WireClass::Power,
        Some((PotentialKind::DcPositive | PotentialKind::DcNegative, v)) => {
            if high(v) {
                WireClass::Power
            } else {
                WireClass::Supply
            }
        }
        Some((PotentialKind::Signal, v)) if high(v) => WireClass::Power,
        _ if section_mm2.is_some_and(|s| s >= 2.5) => WireClass::Power,
        _ => WireClass::Signal,
    }
}

/// Where a job sorts inside its class: routed tracks first (by bundle),
/// then cable cores, then wires in no duct.
fn track_rank(job: &WireCutJob) -> u8 {
    if job.bundle.is_some() {
        0
    } else if job.cable.is_some() {
        1
    } else {
        2
    }
}

fn step_key(job: &WireCutJob) -> String {
    let pin = |p: novo_core::PinRef| format!("{}.{}", p.symbol.raw(), p.pin);
    let (a, b) = (pin(job.end_a.pin), pin(job.end_b.pin));
    if a <= b {
        format!("{a}>{b}")
    } else {
        format!("{b}>{a}")
    }
}

fn area_mm2(section: Option<f64>) -> f64 {
    let d = novo_routing::model::wire_diameter_mm(section)
        .unwrap_or(novo_routing::DEFAULT_WIRE_DIAMETER_MM);
    std::f64::consts::FRAC_PI_4 * d * d
}

fn build(project: &Project) -> WireItPlan {
    let jobs = wire_cut_jobs(project);
    let capacity = duct_capacities_mm2(project);
    let names = duct_names(project);
    let classes: Vec<WireClass> = jobs.iter().map(|j| class_of(project, j)).collect();

    // The shortest wire of each (class, track): tracks run shortest first.
    let mut shortest: BTreeMap<(WireClass, Option<u32>, u8), f64> = BTreeMap::new();
    let key_of = |i: usize| (classes[i], jobs[i].bundle, track_rank(&jobs[i]));
    for (i, job) in jobs.iter().enumerate() {
        let e = shortest.entry(key_of(i)).or_insert(f64::INFINITY);
        *e = e.min(job.cut_mm);
    }
    let mut order: Vec<usize> = (0..jobs.len()).collect();
    order.sort_by(|&a, &b| {
        let (ka, kb) = (key_of(a), key_of(b));
        ka.0.cmp(&kb.0)
            .then(ka.2.cmp(&kb.2))
            .then_with(|| shortest[&ka].total_cmp(&shortest[&kb]))
            .then(ka.1.cmp(&kb.1))
            .then_with(|| jobs[a].cut_mm.total_cmp(&jobs[b].cut_mm))
            .then_with(|| natural_cmp(&jobs[a].number, &jobs[b].number))
            .then_with(|| jobs[a].wires.cmp(&jobs[b].wires))
    });

    let mut used: BTreeMap<u64, f64> = BTreeMap::new();
    let mut count: BTreeMap<u64, usize> = BTreeMap::new();
    let mut steps = Vec::with_capacity(jobs.len());
    for (n, &i) in order.iter().enumerate() {
        let j = &jobs[i];
        let area = area_mm2(j.cross_section_mm2);
        let mut seen = BTreeSet::new();
        let mut ducts = Vec::new();
        for &d in &j.ducts {
            if !seen.insert(d) {
                continue;
            }
            let cap = capacity.get(&d).copied().unwrap_or(0.0);
            let before = used.get(&d).copied().unwrap_or(0.0);
            let after = before + area;
            used.insert(d, after);
            let c = count.entry(d).or_insert(0);
            *c += 1;
            let ratio = |v: f64| (cap > 0.0).then(|| v / cap);
            ducts.push(DuctStep {
                duct: d,
                name: names
                    .get(&d)
                    .cloned()
                    .unwrap_or_else(|| "duct ?".to_owned()),
                fill_before: ratio(before),
                fill_after: ratio(after),
                wires_after: *c,
            });
        }
        steps.push(WireItStep {
            number: n + 1,
            key: step_key(j),
            wires: j.wires.clone(),
            wire_number: j.number.clone(),
            class: classes[i],
            colour: j.colour.clone(),
            cross_section_mm2: j.cross_section_mm2,
            from: j.end_a.designation.clone(),
            to: j.end_b.designation.clone(),
            cut_mm: j.cut_mm,
            length_mm: j.length_mm,
            allowance_mm: ((j.cut_mm - j.length_mm) / 2.0).max(0.0),
            strip_from_mm: j.end_a.strip_mm,
            strip_to_mm: j.end_b.strip_mm,
            fitting_from: j.end_a.sleeve_text(),
            fitting_to: j.end_b.sleeve_text(),
            label_from: j.end_a.label.clone(),
            label_to: j.end_b.label.clone(),
            basis: j.basis.clone(),
            track: j.track.clone(),
            ducts,
            cable_core: j.cable.is_some(),
            cable_tag: j
                .cable
                .and_then(|c| project.cable(c))
                .map(|c| c.tag.to_string())
                .unwrap_or_default(),
            path: j.path.clone(),
        });
    }
    WireItPlan { steps }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PotentialKind::*;

    #[test]
    fn green_yellow_is_earth_on_any_net() {
        assert_eq!(classify(None, true, Some(1.5)), WireClass::Earth);
        assert_eq!(
            classify(Some((Line, None)), true, Some(4.0)),
            WireClass::Earth
        );
    }

    #[test]
    fn the_net_decides_before_the_size() {
        assert_eq!(
            classify(Some((ProtectiveEarth, None)), false, None),
            WireClass::Earth
        );
        assert_eq!(classify(Some((Pen, None)), false, None), WireClass::Earth);
        assert_eq!(
            classify(Some((Line, None)), false, Some(0.5)),
            WireClass::Power
        );
        assert_eq!(
            classify(Some((Neutral, None)), false, Some(0.5)),
            WireClass::Power
        );
        assert_eq!(
            classify(Some((DcPositive, Some(24.0))), false, Some(4.0)),
            WireClass::Supply
        );
        assert_eq!(
            classify(Some((DcNegative, None)), false, Some(1.5)),
            WireClass::Supply
        );
        assert_eq!(
            classify(Some((DcPositive, Some(110.0))), false, None),
            WireClass::Power,
            "a high DC voltage is not the extra-low supply"
        );
        assert_eq!(
            classify(Some((Signal, Some(230.0))), false, Some(0.75)),
            WireClass::Power
        );
        assert_eq!(
            classify(Some((Signal, None)), false, Some(0.75)),
            WireClass::Signal
        );
    }

    #[test]
    fn with_no_potential_the_size_decides() {
        assert_eq!(classify(None, false, Some(2.5)), WireClass::Power);
        assert_eq!(classify(None, false, Some(10.0)), WireClass::Power);
        assert_eq!(classify(None, false, Some(1.5)), WireClass::Signal);
        assert_eq!(classify(None, false, None), WireClass::Signal);
    }

    #[test]
    fn passes_run_in_the_order_earth_power_supply_signals() {
        assert!(WireClass::Earth < WireClass::Power);
        assert!(WireClass::Power < WireClass::Supply);
        assert!(WireClass::Supply < WireClass::Signal);
    }

    #[test]
    fn numbers_read_without_trailing_zeros() {
        assert_eq!(trim(1.5), "1.5");
        assert_eq!(trim(10.0), "10");
        assert_eq!(trim(0.75), "0.75");
        assert_eq!(trim(2.504), "2.5");
    }
}
