//! Autopilot: proposes the wire data the drawing already implies.
//!
//! A fresh drawing has wires without a cross-section, a colour or a number.
//! The program knows enough to fill most of it in: the protective device
//! behind a wire says how much current it may carry, the ampacity tables say
//! which section carries that, the voltage drop limit says whether a long
//! run needs more, and the standard says which colour a protective earth, a
//! neutral or a control wire has. [`plan`] works that out **without changing
//! anything** and gives every choice a [`Proposal`] with the reason in one
//! sentence, so the person can accept row by row or all at once; [`apply`]
//! writes the accepted rows as the caller's one undo step.
//!
//! # What is exact and what is not
//!
//! Each proposal says how far to trust it ([`Basis`]):
//!
//! - [`Basis::Rule`]: a rule of the standard (protective earth is green and
//!   yellow, a protective conductor follows Table 54.2).
//! - [`Basis::Calculated`]: calculated from licensed ampacity tables.
//! - [`Basis::Planning`]: calculated from the built-in ampacity tables, which
//!   are planning values until the user loads the licensed ones. This is
//!   what a sized wire is today.
//! - [`Basis::Assumed`]: the drawing gives too little (no protective device,
//!   no voltage on the net), so a minimum or a common choice is proposed.
//!
//! Ferrules and strip lengths follow from the section on the Bench output
//! tabs, so they are not separate rows. A proposal is a starting point for a
//! designer, never a certificate.

use std::collections::{BTreeMap, BTreeSet};

use novo_checks::calc::cable::{Loaded, Method};
use novo_checks::calc::drop::{Run, Supply};
use novo_checks::calc::pe::min_pe_section;
use novo_checks::calc::tables::TableSet;
use novo_checks::calc_basis::{
    Circuit, WireBasis, drop_limit_percent, supply_voltage_v, wire_basis,
};
use novo_core::{DeviceId, PotentialKind, Project, Result, SymbolRole, Tx, Wire, WireId};

use crate::wire_defaults::unset_wires;
use crate::wire_rules::{ColourClass, WireRules};
use crate::wirecut::WireCutJob;
use crate::{is_estimate, wire_cut_jobs};

/// Sections below the first table row, with the most current each is
/// allowed to carry in a panel. Not from a standard's table: deliberately
/// low planning caps so a thin control wire is never overloaded.
const SMALL: [(f64, f64); 2] = [(0.75, 4.0), (1.0, 6.0)];

/// How far to trust a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// A rule of the standard.
    Rule,
    /// Calculated from licensed tables.
    Calculated,
    /// Calculated from the built-in planning tables.
    Planning,
    /// A common choice because the drawing gives too little.
    Assumed,
}

impl Basis {
    pub fn label(self) -> &'static str {
        match self {
            Basis::Rule => "standard rule",
            Basis::Calculated => "calculated",
            Basis::Planning => "planning value",
            Basis::Assumed => "assumed",
        }
    }
}

/// What a proposal sets.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// Cross-section in mm².
    Size(f64),
    /// Colour code (`BK`, `GNYE`).
    Colour(String),
    /// Wire number.
    Number(String),
    /// Put these wires in one cable of this type (`4G1.5`). Written last,
    /// when the sizes are in, from the cable proposal of that moment.
    Cable(String),
    /// Mark where wires from outside enter this enclosure's plate (mm, plate
    /// coordinates).
    EntryPoint {
        plate: novo_core::StructureId,
        x: f64,
        y: f64,
    },
}

impl Change {
    pub fn kind(&self) -> &'static str {
        match self {
            Change::Size(_) => "Size",
            Change::Colour(_) => "Colour",
            Change::Number(_) => "Number",
            Change::Cable(_) => "Cable",
            Change::EntryPoint { .. } => "Entry point",
        }
    }

    /// The value as shown in the review list.
    pub fn value(&self) -> String {
        match self {
            Change::Size(s) => format!("{} mm²", fmt(*s)),
            Change::Colour(c) | Change::Number(c) | Change::Cable(c) => c.clone(),
            Change::EntryPoint { x, y, .. } => format!("x {} mm, y {} mm", fmt(*x), fmt(*y)),
        }
    }
}

/// One suggested change with its reason.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    /// Position in the plan; stable while the plan lives.
    pub id: usize,
    pub wires: Vec<WireId>,
    /// Which connection: number, or "unnumbered", and the pins it joins.
    pub label: String,
    pub change: Change,
    pub reason: String,
    pub basis: Basis,
}

/// The whole plan.
#[derive(Debug, Clone, Default)]
pub struct Autopilot {
    pub proposals: Vec<Proposal>,
    /// Connections that could not be sized, with the reason.
    pub unresolved: Vec<String>,
    /// What Autopilot cannot do for the drawing, said plainly: lengths still
    /// estimated because the devices are not placed or no duct is drawn.
    pub left_to_you: Vec<String>,
    /// Which ampacity tables the sizes were read from.
    pub tables: String,
    pub planning_values: bool,
    revision: u64,
}

impl Autopilot {
    pub fn is_empty(&self) -> bool {
        self.proposals.is_empty()
    }

    /// Proposals of one kind.
    pub fn count(&self, kind: &str) -> usize {
        self.proposals
            .iter()
            .filter(|p| p.change.kind() == kind)
            .count()
    }

    /// Adds proposals made elsewhere (numbers, from novo-numbering), giving
    /// them the next ids.
    pub fn extend(&mut self, extra: impl IntoIterator<Item = Proposal>) {
        for mut p in extra {
            p.id = self.proposals.len();
            self.proposals.push(p);
        }
    }

    /// Writes the proposals with these ids (all when `accepted` is `None`).
    /// Call it in one transaction: one undo. Refused when the drawing
    /// changed since the plan was made. Returns `(sizes, colours, numbers, cables, entry points)`.
    pub fn apply(
        &self,
        tx: &mut Tx<'_>,
        accepted: Option<&BTreeSet<usize>>,
    ) -> Result<(usize, usize, usize, usize, usize)> {
        if tx.project().revision() != self.revision {
            return Err(novo_core::ModelError::Refused {
                what: "Autopilot".into(),
                reason: "the drawing changed since the review list was made".into(),
            });
        }
        let mut counts = (0, 0, 0, 0, 0);
        for p in &self.proposals {
            if accepted.is_some_and(|set| !set.contains(&p.id)) {
                continue;
            }
            for wire in &p.wires {
                match &p.change {
                    Change::Size(s) => {
                        tx.update::<Wire>(*wire, |w| w.cross_section_mm2 = Some(*s))?
                    }
                    Change::Colour(c) => tx.update::<Wire>(*wire, |w| w.color = Some(c.clone()))?,
                    Change::Number(n) => {
                        tx.update::<Wire>(*wire, |w| w.number = Some(n.clone()))?
                    }
                    Change::Cable(_) | Change::EntryPoint { .. } => {}
                }
            }
            // A change that is not about wires.
            if let Change::EntryPoint { plate, x, y } = &p.change {
                novo_routing::entry::set_entry_point(
                    tx,
                    *plate,
                    Some(novo_routing::Point::new(*x, *y)),
                )?;
            }
            match p.change {
                Change::Size(_) => counts.0 += 1,
                Change::Colour(_) => counts.1 += 1,
                Change::Number(_) => counts.2 += 1,
                Change::Cable(_) => {}
                Change::EntryPoint { .. } => counts.4 += 1,
            }
        }
        // Cables last: their type follows the sizes just written.
        let wanted: Vec<BTreeSet<WireId>> = self
            .proposals
            .iter()
            .filter(|p| matches!(p.change, Change::Cable(_)))
            .filter(|p| accepted.is_none_or(|set| set.contains(&p.id)))
            .map(|p| p.wires.iter().copied().collect())
            .collect();
        if !wanted.is_empty() {
            let now: Vec<_> = novo_cables::propose::propose_cables(tx.project())
                .into_iter()
                .filter(|c| wanted.contains(&c.wires.iter().copied().collect::<BTreeSet<_>>()))
                .collect();
            counts.3 = novo_cables::propose::accept_proposals(tx, &now, None)?.len();
        }
        Ok(counts)
    }
}

/// `1.5`, `16`: at most two decimals.
fn fmt(v: f64) -> String {
    format!("{}", (v * 100.0).round() / 100.0)
}

/// The project revision a plan was made on.
impl Autopilot {
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

/// Works out sizes and colours for the wires that have none. Never changes
/// the project. `jobs` is the bench cut list (it carries the routed lengths
/// the voltage drop needs).
pub fn plan(project: &Project, tables: &TableSet, jobs: &[WireCutJob]) -> Autopilot {
    let netlist = project.netlist();
    let basis = wire_basis(project, &netlist);
    let unset = unset_wires(project);
    let no_size: BTreeSet<WireId> = unset.no_size.iter().copied().collect();
    let no_colour: BTreeSet<WireId> = unset.no_colour.iter().copied().collect();
    let limit = drop_limit_percent(project);
    let rules = WireRules::of(project);
    let supply_v = supply_voltage_v(project);
    let mut out = Autopilot {
        tables: tables.disclosure(),
        planning_values: !tables.fully_licensed(),
        revision: project.revision(),
        ..Autopilot::default()
    };

    // Line sections known so far, by class (control = true): the wires that
    // have one, then the proposals. Neutral and earth follow these.
    let mut lines: Vec<Line> = Vec::new();
    let follows = |k: Option<PotentialKind>| {
        matches!(
            k,
            Some(
                PotentialKind::ProtectiveEarth | PotentialKind::Neutral | PotentialKind::DcNegative
            )
        )
    };
    for job in jobs {
        if let Some((s, b)) = job
            .cross_section_mm2
            .zip(job.wires.first().and_then(|w| basis.get(w)))
            && !follows(b.kind)
        {
            lines.push(Line::new(project, job, s, is_control(b), false));
        }
    }

    // First everything but neutral and earth, then those from the result.
    let mut followers: Vec<&WireCutJob> = Vec::new();
    for job in jobs {
        let wires: Vec<WireId> = job
            .wires
            .iter()
            .copied()
            .filter(|w| no_size.contains(w))
            .collect();
        let Some(first) = wires.first() else { continue };
        let Some(b) = basis.get(first) else { continue };
        if follows(b.kind) {
            followers.push(job);
            continue;
        }
        let length = (job.length_mm > 0.0).then(|| job.length_mm / 1000.0);
        match size_wire(b, length, is_estimate(&job.basis), tables, supply_v, limit) {
            Ok(s) => {
                let (section, reason) = with_minimum(s.section, s.reason, is_control(b), &rules);
                lines.push(Line::new(project, job, section, is_control(b), true));
                out.proposals.push(Proposal {
                    id: out.proposals.len(),
                    wires,
                    label: job_label(job),
                    change: Change::Size(section),
                    reason,
                    basis: s.basis,
                });
            }
            Err(why) => out.unresolved.push(format!("{}: {why}", job_label(job))),
        }
    }
    // The largest line wire (of one class) on the same devices as the
    // follower; the whole project when its devices have none.
    let largest = |control: Option<bool>, devices: &BTreeSet<DeviceId>| {
        let pick = |same_circuit: bool| {
            lines
                .iter()
                .filter(|l| control.is_none_or(|want| l.control == want))
                .filter(|l| !same_circuit || !l.devices.is_disjoint(devices))
                .max_by(|a, b| a.section.total_cmp(&b.section))
                .map(|l| (l.section, l.label.clone(), l.proposed, same_circuit))
        };
        pick(true).or_else(|| pick(false))
    };
    for job in followers {
        let wires: Vec<WireId> = job
            .wires
            .iter()
            .copied()
            .filter(|w| no_size.contains(w))
            .collect();
        let Some(b) = wires.first().and_then(|w| basis.get(w)) else {
            continue;
        };
        let earth = b.kind == Some(PotentialKind::ProtectiveEarth);
        let control = is_control(b);
        let devices = job_devices(project, job);
        // Earth follows the power conductors; neutral the conductors of its own class.
        let reference = if earth {
            largest(Some(false), &devices).or_else(|| largest(None, &devices))
        } else {
            largest(Some(control), &devices).or_else(|| largest(None, &devices))
        };
        let (section, reason, basis_kind) = match reference {
            Some((phase, from, proposed, same)) if earth => {
                let want = min_pe_section(phase).unwrap_or(phase);
                (
                    listed_at_least(tables, want),
                    format!(
                        "protective conductor sized from the largest power wire {} ({} mm², {from}{}): the same up to 16 mm², 16 mm² up to 35, half above (IEC 60364-5-54 Table 54.2)",
                        scope(same),
                        fmt(phase),
                        if proposed { ", itself a proposal" } else { "" }
                    ),
                    if proposed || !same { Basis::Planning } else { Basis::Rule },
                )
            }
            Some((phase, from, proposed, same)) => (
                phase,
                format!(
                    "the neutral or 0 V conductor takes the section of the largest line wire {} ({} mm², {from}{}), IEC 60364-5-52 §524.2",
                    scope(same),
                    fmt(phase),
                    if proposed { ", itself a proposal" } else { "" }
                ),
                if proposed || !same { Basis::Planning } else { Basis::Rule },
            ),
            None => (
                if control { SMALL[0].0 } else { 1.5 },
                "no line wire has a size yet, so the smallest size is proposed; it is not re-checked until the others are sized".to_owned(),
                Basis::Assumed,
            ),
        };
        let (section, reason) = with_minimum(section, reason, control, &rules);
        out.proposals.push(Proposal {
            id: out.proposals.len(),
            wires,
            label: job_label(job),
            change: Change::Size(section),
            reason,
            basis: basis_kind,
        });
    }

    for job in jobs {
        let wires: Vec<WireId> = job
            .wires
            .iter()
            .copied()
            .filter(|w| no_colour.contains(w))
            .collect();
        let Some(b) = wires.first().and_then(|w| basis.get(w)) else {
            continue;
        };
        let (code, reason, basis_kind) = colour_for(b, &rules);
        out.proposals.push(Proposal {
            id: out.proposals.len(),
            wires,
            label: job_label(job),
            change: Change::Colour(code),
            reason,
            basis: basis_kind,
        });
    }
    let (mut feeds, mut field, mut unplaced) = (0, 0, 0);
    let mut need_entry: BTreeSet<novo_core::StructureId> = BTreeSet::new();
    for job in jobs.iter().filter(|j| is_estimate(&j.basis)) {
        let (a, b) = (&job.end_a.designation, &job.end_b.designation);
        if a.ends_with("(feed)") || b.ends_with("(feed)") {
            feeds += 1;
            for pin in [job.end_a.pin, job.end_b.pin] {
                if let Some(plate) = placed_plate(project, pin)
                    && novo_routing::entry::entry_point(project, plate).is_none()
                {
                    need_entry.insert(plate);
                }
            }
        } else if location_of(a) != location_of(b) {
            field += 1;
        } else {
            unplaced += 1;
        }
    }
    let mut left = |n: usize, what: &str| {
        if n > 0 {
            out.left_to_you
                .push(format!("{n} wire lengths are estimates: {what}"));
        }
    };
    left(
        unplaced,
        "a device is not placed on the mounting plate, or its two ends are on different plates. Use Cabinet > Place all; the lengths of placed devices are routed.",
    );
    left(
        field,
        "the wire goes to a device in another location. Put the wires in a cable (see the cable rows) and accept the cable rows above, then type Length (m) in the cable's properties: that figure counts as stated and the wire is no longer an estimate. Until then it is the page distance plus service loops.",
    );
    left(
        feeds,
        "the wire comes in from outside the drawing (a supply lead with no device at the other end). Set the cabinet entry and the wire length outside it (Cabinet > Routes): the lead is then routed to the entry and its length uses your figure.",
    );
    for plate in need_entry {
        let Some(structure) = project.get::<novo_core::Structure>(plate) else {
            continue;
        };
        let Some(size) = structure.plate else {
            continue;
        };
        out.proposals.push(Proposal {
            id: out.proposals.len(),
            wires: Vec::new(),
            label: format!("entry point of {}", structure.tag),
            change: Change::EntryPoint {
                plate,
                x: size.width / 2.0,
                y: size.height,
            },
            reason: "supply leads come in from outside and the plate has no entry point: the bottom middle of the plate is where glands usually are. Move it to where yours is".to_owned(),
            basis: Basis::Assumed,
        });
    }
    for c in novo_cables::propose::propose_cables(project) {
        let name = c.cable_type.name();
        out.proposals.push(Proposal {
            id: out.proposals.len(),
            wires: c.wires.clone(),
            label: format!("cable {}, {} wires", c.tag, c.wires.len()),
            change: Change::Cable(name.clone()),
            reason: format!(
                "{} wires run together from {} to {} and leave the cabinet location: one {name} cable, its type chosen from the conductors and the largest wire size when you accept",
                c.wires.len(),
                c.ends[0].location.join(" "),
                c.ends[1].location.join(" "),
            ),
            basis: Basis::Planning,
        });
    }
    out
}

/// Convenience: the cut list for the project's saved ferrule standard.
pub fn plan_for(project: &Project, tables: &TableSet) -> Autopilot {
    plan(project, tables, &wire_cut_jobs(project))
}

/// A wire with a known section, as a reference for neutral and earth.
struct Line {
    section: f64,
    control: bool,
    label: String,
    proposed: bool,
    /// The devices at its two ends.
    devices: BTreeSet<DeviceId>,
}

impl Line {
    fn new(
        project: &Project,
        job: &WireCutJob,
        section: f64,
        control: bool,
        proposed: bool,
    ) -> Self {
        Line {
            section,
            control,
            label: job_label(job),
            proposed,
            devices: job_devices(project, job),
        }
    }
}

/// The devices a connection ends on (terminals do not count: a strip joins
/// many circuits).
fn job_devices(project: &Project, job: &WireCutJob) -> BTreeSet<DeviceId> {
    [job.end_a.pin, job.end_b.pin]
        .into_iter()
        .filter_map(|pin| match &project.symbol(pin.symbol)?.role {
            SymbolRole::Device { device, .. } => Some(*device),
            _ => None,
        })
        .collect()
}

fn scope(same_circuit: bool) -> &'static str {
    if same_circuit {
        "on the same device"
    } else {
        "in the project (none on the same device)"
    }
}

/// The enclosure whose plate a pin's device is placed on.
fn placed_plate(project: &Project, pin: novo_core::PinRef) -> Option<novo_core::StructureId> {
    let device = match &project.symbol(pin.symbol)?.role {
        SymbolRole::Device { device, .. } => *device,
        SymbolRole::Terminal { .. } => {
            let symbol = project.symbol(pin.symbol)?;
            project.terminal(symbol.terminal_of_pin(pin.pin)?)?.strip
        }
        _ => return None,
    };
    project
        .all::<novo_core::CabinetPlacement>()
        .find(|p| p.device == device)
        .map(|p| p.structure)
}

/// The `+LOCATION` of a designation (`=A1+CAB-X1:1` gives `CAB`).
fn location_of(designation: &str) -> &str {
    designation
        .split_once('+')
        .map(|(_, rest)| rest.split('-').next().unwrap_or(rest))
        .unwrap_or("")
}

fn job_label(job: &WireCutJob) -> String {
    let number = if job.number.trim().is_empty() {
        "unnumbered".to_owned()
    } else {
        format!("wire {}", job.number)
    };
    format!(
        "{number}, {} to {}",
        job.end_a.designation, job.end_b.designation
    )
}

/// The smallest listed section (including the small ones) at or above `want`.
fn listed_at_least(tables: &TableSet, want: f64) -> f64 {
    SMALL
        .iter()
        .map(|(s, _)| *s)
        .chain(tables.sections().iter().copied())
        .find(|s| *s + 1e-9 >= want)
        .unwrap_or(want)
}

struct Sized {
    section: f64,
    reason: String,
    basis: Basis,
}

/// Whether the wire belongs to a control circuit (60 V or less).
fn is_control(b: &WireBasis) -> bool {
    match b.circuit {
        Circuit::Power => false,
        Circuit::Control => true,
        Circuit::Unknown => {
            b.potential_voltage.is_some_and(|v| v <= 60.0)
                || (b.kind == Some(PotentialKind::Signal) && b.potential_voltage.is_none())
        }
    }
}

fn size_wire(
    b: &WireBasis,
    length_m: Option<f64>,
    estimate: bool,
    tables: &TableSet,
    supply_v: f64,
    limit_percent: f64,
) -> std::result::Result<Sized, String> {
    let control = is_control(b);
    let class = if control { "control" } else { "power" };
    let neighbour_max = b.neighbours.iter().max_by(|a, c| a.amps.total_cmp(&c.amps));
    let Some((current, source)) = b
        .protecting
        .as_ref()
        .map(|p| (p.rated_a, format!("{} ({} A) protects it", p.tag, fmt(p.rated_a))))
        .or_else(|| {
            neighbour_max.map(|n| {
                let all: Vec<String> = b
                    .neighbours
                    .iter()
                    .map(|r| format!("{} {} {} A", r.tag, r.what, fmt(r.amps)))
                    .collect();
                (
                    n.amps,
                    format!(
                        "no rating is stated on a device in front of it, so the rated devices on its circuit set the current ({}); the highest is used",
                        all.join(", ")
                    ),
                )
            })
        })
        .or_else(|| {
            b.load_a
                .map(|a| (a, format!("its stated load is {} A", fmt(a))))
        })
    else {
        let section = if control { SMALL[0].0 } else { 1.5 };
        return Ok(Sized {
            section,
            reason: format!(
                "no rated device is connected to this wire and no load current is stated, so the smallest {class} size is proposed; set a rating on the fuse or breaker, or load_current_a, to have it calculated"
            ),
            basis: Basis::Assumed,
        });
    };
    let ambient = tables
        .ambient_factor(b.ambient_c, b.insulation)
        .ok_or_else(|| format!("ambient {} °C is outside the table", fmt(b.ambient_c)))?;
    let grouping = tables.grouping_factor(b.grouped_circuits);
    let derating = ambient * grouping;
    let method = b.method.unwrap_or(Method::C);
    let voltage = b.potential_voltage.unwrap_or(supply_v);
    let three = b.kind == Some(PotentialKind::Line) && voltage >= 380.0;
    let loaded = if three { Loaded::Three } else { Loaded::Two };

    // (section, ampacity before derating, from the tables)
    let mut ladder: Vec<(f64, f64, bool)> = Vec::new();
    if control {
        ladder.extend(SMALL.iter().map(|(s, cap)| (*s, *cap, false)));
    }
    for s in tables.sections() {
        if let Some(iz0) = tables.ampacity(*s, b.insulation, method, loaded) {
            ladder.push((*s, iz0, true));
        }
    }
    let mut at = ladder
        .iter()
        .position(|(_, iz0, _)| *iz0 * derating + 1e-9 >= current)
        .ok_or_else(|| {
            format!(
                "{} A is more than the largest listed section carries; size it by hand",
                fmt(current)
            )
        })?;

    let supply = if three {
        Supply::ThreePhase
    } else {
        Supply::TwoWire
    };
    let drop_current = b.load_a.unwrap_or(current);
    let at_what = if b.load_a.is_some() {
        ""
    } else {
        ", at the device rating"
    };
    // A control wire on a net with no stated voltage: the supply voltage
    // would make the drop far too small, so the drop is not checked.
    let voltage_known = b.potential_voltage.is_some() || !control;
    let mut drop_note = if voltage_known {
        "length not known, voltage drop not checked".to_owned()
    } else {
        "voltage of the net unknown, voltage drop not checked".to_owned()
    };
    if let (Some(len), true) = (length_m, voltage_known) {
        loop {
            let (s, _, _) = ladder[at];
            let drop = Run::new(len, s, drop_current, supply, voltage)
                .drop_percent()
                .ok_or_else(|| {
                    format!(
                        "the voltage drop of {} mm² over {len:.1} m could not be calculated",
                        fmt(s)
                    )
                })?;
            if drop <= limit_percent + 1e-9 {
                drop_note = format!(
                    "voltage drop {:.2} % of {} % over {:.1} m{}{at_what}",
                    drop,
                    fmt(limit_percent),
                    len,
                    if estimate { " (estimated length)" } else { "" }
                );
                break;
            }
            if at + 1 >= ladder.len() {
                return Err(format!(
                    "the voltage drop is {:.2} % at the largest listed section ({} mm²) over {:.1} m, above the {} % limit: shorten the run, raise the voltage or size it by hand",
                    drop,
                    fmt(s),
                    len,
                    fmt(limit_percent)
                ));
            }
            at += 1;
        }
    }
    let (section, iz0, from_table) = ladder[at];
    let mut reason = format!("{source}; {} mm² carries ", fmt(section));
    if from_table {
        reason.push_str(&format!(
            "{} A (method {}{}, {}, {} °C, grouping {})",
            fmt(iz0 * derating),
            method.label().split(' ').next().unwrap_or(""),
            if b.method.is_none() { " assumed" } else { "" },
            match b.insulation {
                novo_checks::calc::Insulation::Pvc => "PVC",
                novo_checks::calc::Insulation::Xlpe => "XLPE",
            },
            fmt(b.ambient_c),
            b.grouped_circuits
        ));
    } else {
        reason.push_str(&format!(
            "{} A (a low planning cap, below the first table row)",
            fmt(iz0 * derating)
        ));
    }
    reason.push_str(&format!(", enough for {} A; {drop_note}", fmt(current)));
    Ok(Sized {
        section,
        reason,
        basis: if !from_table || !tables.fully_licensed() {
            Basis::Planning
        } else {
            Basis::Calculated
        },
    })
}

/// Which colour class a wire belongs to, following the same branches as
/// [`colour_of`].
fn colour_class(b: &WireBasis) -> ColourClass {
    let v = b.potential_voltage;
    match b.kind {
        Some(PotentialKind::ProtectiveEarth) => ColourClass::ProtectiveEarth,
        Some(PotentialKind::Pen) => ColourClass::Pen,
        Some(PotentialKind::Neutral) => ColourClass::Neutral,
        Some(PotentialKind::DcNegative) => ColourClass::DcNegative,
        Some(PotentialKind::Line) if v.is_some_and(|v| v >= 380.0) => ColourClass::AcPower,
        Some(PotentialKind::Line) => match b.circuit {
            Circuit::Power => ColourClass::AcPower,
            Circuit::Control => ColourClass::AcControl,
            Circuit::Unknown => match b.protecting.as_ref().map(|p| p.rated_a) {
                Some(a) if a <= 6.0 => ColourClass::AcControl,
                _ => ColourClass::AcPower,
            },
        },
        Some(PotentialKind::DcPositive | PotentialKind::Signal) if v.is_some_and(|v| v > 60.0) => {
            if b.kind == Some(PotentialKind::Signal) {
                ColourClass::AcControl
            } else {
                ColourClass::DcPower
            }
        }
        Some(PotentialKind::DcPositive | PotentialKind::Signal) => ColourClass::DcControl,
        None => ColourClass::Unknown,
    }
}

/// The colour under the project's rules: the house colour of the class when
/// the project states one, else IEC 60204-1.
fn colour_for(b: &WireBasis, rules: &WireRules) -> (String, String, Basis) {
    let (code, reason, basis) = colour_of(b);
    let class = colour_class(b);
    match rules.house_colour(class) {
        Some(house) => (
            house.to_owned(),
            format!(
                "house rule (Wire rules setting): {} wires are {house}; IEC 60204-1 would give {code}",
                class.label().to_lowercase()
            ),
            // A rule of the project is as good as a rule gets, unless the
            // class itself was a guess.
            if basis == Basis::Assumed {
                Basis::Assumed
            } else {
                Basis::Rule
            },
        ),
        None => (code.to_owned(), reason, basis),
    }
}

/// Raises a proposed section to the project's minimum for its class.
fn with_minimum(section: f64, reason: String, control: bool, rules: &WireRules) -> (f64, String) {
    match rules.min_mm2(control) {
        Some(min) if section + 1e-9 < min => (
            min,
            format!(
                "{reason}; raised to {} mm², the house minimum for {} wires (Wire rules setting)",
                fmt(min),
                if control { "control" } else { "power" }
            ),
        ),
        _ => (section, reason),
    }
}

/// Colour per IEC 60204-1 §13.2 and IEC 60757 codes.
fn colour_of(b: &WireBasis) -> (&'static str, String, Basis) {
    let v = b.potential_voltage;
    match b.kind {
        Some(PotentialKind::ProtectiveEarth) => (
            "GNYE",
            "protective conductors are green and yellow (IEC 60204-1 §13.2.2)".into(),
            Basis::Rule,
        ),
        Some(PotentialKind::Pen) => (
            "GNYE",
            "PEN conductor: green and yellow with light blue marks at the ends; the marks are made by hand".into(),
            Basis::Rule,
        ),
        Some(PotentialKind::Neutral) => (
            "BU",
            "the neutral conductor is light blue (IEC 60204-1 §13.2.3)".into(),
            Basis::Rule,
        ),
        Some(PotentialKind::DcNegative) => (
            "BU",
            "0 V / DC minus is blue (IEC 60204-1 §13.2.4)".into(),
            Basis::Rule,
        ),
        Some(PotentialKind::Line) if v.is_some_and(|v| v >= 380.0) => (
            "BK",
            "AC power circuits are black (IEC 60204-1 §13.2.4)".into(),
            Basis::Rule,
        ),
        Some(PotentialKind::Line) => match b.circuit {
            Circuit::Power => (
                "BK",
                "AC power circuit (a motor or main contactor is on it): black (IEC 60204-1 §13.2.4)".into(),
                Basis::Rule,
            ),
            Circuit::Control => (
                "RD",
                "AC control circuit (only operators, coils and contacts on it): red (IEC 60204-1 §13.2.4)".into(),
                Basis::Rule,
            ),
            Circuit::Unknown => match b.protecting.as_ref().map(|p| p.rated_a) {
                Some(a) if a > 6.0 => (
                    "BK",
                    "single-phase line wire protected above 6 A, taken as power: black (IEC 60204-1 §13.2.4)".into(),
                    Basis::Assumed,
                ),
                Some(_) => (
                    "RD",
                    "single-phase line wire protected at 6 A or less, taken as AC control: red (IEC 60204-1 §13.2.4). If it is a power wire, change it to black".into(),
                    Basis::Assumed,
                ),
                None => (
                    "BK",
                    "single-phase line wire with nothing to tell power from control, black assumed (IEC 60204-1 §13.2.4)".into(),
                    Basis::Assumed,
                ),
            },
        },
        Some(PotentialKind::DcPositive | PotentialKind::Signal) if v.is_some_and(|v| v > 60.0) => {
            if b.kind == Some(PotentialKind::Signal) {
                (
                    "RD",
                    "control circuit above 60 V, taken as AC: red (IEC 60204-1 §13.2.4)".into(),
                    Basis::Assumed,
                )
            } else {
                (
                    "BK",
                    "DC power circuit: black (IEC 60204-1 §13.2.4)".into(),
                    Basis::Rule,
                )
            }
        }
        Some(PotentialKind::DcPositive | PotentialKind::Signal) => (
            "BU",
            "DC control circuit: blue (IEC 60204-1 §13.2.4)".into(),
            if v.is_some() { Basis::Rule } else { Basis::Assumed },
        ),
        None => (
            "BK",
            "this wire's net carries no potential, so black (power) is assumed; name the potential to get the colour of the standard".into(),
            Basis::Assumed,
        ),
    }
}

/// Proposals gathered under the protective device that limits their wires,
/// so a reviewer accepts a circuit instead of dozens of lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// "-F2 (5 A)", "Not behind a protective device", "Cabinet".
    pub title: String,
    /// Proposal ids, in plan order.
    pub ids: Vec<usize>,
    /// What the group proposes in a line: "1.5 mm² · BK, BN, GY · 6 wire numbers".
    pub summary: String,
}

/// A drawn wire that contradicts the project's house wire rules.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleBreak {
    pub wire: WireId,
    pub message: String,
}

/// Wires that already have a colour or size the project's house rules do not
/// allow: a colour other than the house colour of its class, a section under
/// the house minimum. Empty under IEC 60204-1 (the project states no rules of
/// its own), and a wire without a value is not a break: the Autopilot fills
/// those in. Read only.
pub fn rule_breaks(project: &Project) -> Vec<RuleBreak> {
    let rules = WireRules::of(project);
    if rules.preset != crate::wire_rules::Preset::House {
        return Vec::new();
    }
    let basis = wire_basis(project, &project.netlist());
    let mut out = Vec::new();
    for item in crate::wires::items(project, true) {
        for wire in item.wires {
            let Some(b) = basis.get(&wire.id) else {
                continue;
            };
            let class = colour_class(b);
            let name = match wire.number.as_deref().map(str::trim) {
                Some(n) if !n.is_empty() => format!("Wire {n}"),
                _ => "A wire without a number".to_owned(),
            };
            let have = wire
                .color
                .as_deref()
                .and_then(crate::wire_rules::normal_code);
            // A class the Autopilot had to guess is no ground for a warning.
            let sure_class = colour_of(b).2 != Basis::Assumed;
            if let (Some(have), Some(house)) = (have, rules.house_colour(class))
                && sure_class
                && have != house
            {
                out.push(RuleBreak {
                    wire: wire.id,
                    message: format!(
                        "{name} ({} circuit) is {have}; the project's house rule gives {house}",
                        class.label()
                    ),
                });
            }
            let control = is_control(b);
            let smallest = crate::wirecut::sections(project, &[wire])
                .into_iter()
                .reduce(f64::min);
            // Power or control is a guess when neither the circuit nor a voltage says.
            let sure_kind = b.circuit != Circuit::Unknown || b.potential_voltage.is_some();
            if let (Some(have), Some(min)) = (smallest, rules.min_mm2(control))
                && sure_kind
                && have + 1e-9 < min
            {
                out.push(RuleBreak {
                    wire: wire.id,
                    message: format!(
                        "{name} is {} mm², under the house minimum of {} mm² for {} wires",
                        fmt(have),
                        fmt(min),
                        if control { "control" } else { "power" }
                    ),
                });
            }
        }
    }
    out
}

/// Groups the plan by protective device. The wire's protective device is the
/// one `wire_basis` finds (the highest-rated one in front of it); proposals
/// that touch no wire (cabinet entry points) go in "Cabinet".
pub fn groups(project: &Project, plan: &Autopilot) -> Vec<Group> {
    let basis = wire_basis(project, &project.netlist());
    let mut map: BTreeMap<(u8, String), Vec<usize>> = BTreeMap::new();
    for p in &plan.proposals {
        let key = match p.wires.first() {
            None => (2, "Cabinet".to_owned()),
            Some(w) => match basis.get(w).and_then(|b| b.protecting.as_ref()) {
                Some(f) => {
                    // "-F2" of "=A1+CAB-F2": the plant and place are on the page.
                    let short = f.tag.rfind('-').map_or(f.tag.as_str(), |i| &f.tag[i..]);
                    (0, format!("{short} ({} A)", fmt(f.rated_a)))
                }
                None => (1, "Not behind a protective device".to_owned()),
            },
        };
        map.entry(key).or_default().push(p.id);
    }
    map.into_iter()
        .map(|((_, title), ids)| {
            let mut sizes: BTreeSet<String> = BTreeSet::new();
            let mut colours: BTreeSet<String> = BTreeSet::new();
            let (mut numbers, mut other) = (0usize, 0usize);
            for p in plan.proposals.iter().filter(|p| ids.contains(&p.id)) {
                match &p.change {
                    Change::Size(s) => {
                        sizes.insert(format!("{} mm²", fmt(*s)));
                    }
                    Change::Colour(c) => {
                        colours.insert(c.clone());
                    }
                    Change::Number(_) => numbers += 1,
                    _ => other += 1,
                }
            }
            let mut parts: Vec<String> = Vec::new();
            if !sizes.is_empty() {
                parts.push(sizes.into_iter().collect::<Vec<_>>().join(", "));
            }
            if !colours.is_empty() {
                parts.push(colours.into_iter().collect::<Vec<_>>().join(", "));
            }
            if numbers > 0 {
                parts.push(format!("{numbers} wire numbers"));
            }
            if other > 0 {
                parts.push(format!("{other} other"));
            }
            Group {
                title,
                ids,
                summary: parts.join(" · "),
            }
        })
        .collect()
}

/// The proposals as a map from wire to what is proposed, for tests and
/// for checking a plan against the model.
pub fn by_wire(plan: &Autopilot) -> BTreeMap<WireId, Vec<&Proposal>> {
    let mut map: BTreeMap<WireId, Vec<&Proposal>> = BTreeMap::new();
    for p in &plan.proposals {
        for w in &p.wires {
            map.entry(*w).or_default().push(p);
        }
    }
    map
}

/// How much of what the workshop pack needs is in place.
///
/// Every wire has four things to settle (number, cross-section, colour, a
/// routed length) and the drawing needs a mounting plate to route on; each
/// is one check. `done` of `total` is the meter. Estimates count as not done,
/// so the meter only reaches 100 % when every length comes from a routed wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ready {
    pub done: usize,
    pub total: usize,
    /// Wires to cut the meter counts (four checks each, one more for the plate).
    pub wires: usize,
}

impl Ready {
    /// Whole percent, rounded down so 100 means nothing is missing.
    pub fn percent(&self) -> u32 {
        if self.total == 0 {
            return 0;
        }
        (self.done * 100 / self.total) as u32
    }

    pub fn is_complete(&self) -> bool {
        self.total > 0 && self.done == self.total
    }

    /// What the percentage counts, for the hover text.
    pub fn explanation(&self) -> String {
        format!(
            "{} of {} checks pass: {} with four checks each (number, cross-section, colour, a known length: routed or stated by you), plus one for the mounting plate. Estimated lengths do not count, and without a plate no length check passes.",
            self.done,
            self.total,
            crate::count_text(self.wires, "wires to cut")
        )
    }
}

/// The meter for a cut list. `has_plate` is whether a mounting plate exists.
pub fn ready(jobs: &[WireCutJob], has_plate: bool) -> Ready {
    if jobs.is_empty() {
        return Ready::default();
    }
    let mut r = Ready {
        done: usize::from(has_plate),
        total: 1,
        wires: jobs.len(),
    };
    for job in jobs {
        r.total += 4;
        r.done += usize::from(!job.number.trim().is_empty());
        r.done += usize::from(job.cross_section_mm2.is_some());
        r.done += usize::from(!job.colour.trim().is_empty());
        r.done += usize::from(has_plate && !is_estimate(&job.basis));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_checks::calc::Insulation;
    use novo_checks::calc_basis::Protecting;
    use novo_core::DeviceId;

    fn basis(rated: f64, kind: PotentialKind, voltage: f64) -> WireBasis {
        WireBasis {
            protecting: Some(Protecting {
                device: DeviceId::from(1),
                tag: "F1".into(),
                rated_a: rated,
            }),
            neighbours: Vec::new(),
            circuit: Circuit::Power,
            load_a: None,
            insulation: Insulation::Pvc,
            method: Some(Method::C),
            ambient_c: 30.0,
            grouped_circuits: 1,
            kind: Some(kind),
            potential_voltage: Some(voltage),
        }
    }

    fn size(b: &WireBasis, length: Option<f64>) -> Sized {
        size_wire(b, length, false, &TableSet::builtin(), 400.0, 5.0).unwrap()
    }

    /// IEC 60364-5-52 Annex B, copper PVC, method C, three loaded
    /// conductors: 1.5 mm² 17.5 A, 2.5 mm² 24 A, 4 mm² 32 A, 6 mm² 41 A.
    #[test]
    fn three_phase_sizes_follow_the_table_at_the_steps() {
        for (current, expect) in [
            (6.0, 1.5),
            (17.0, 1.5),
            (18.0, 2.5),
            (24.0, 2.5),
            (25.0, 4.0),
            (32.0, 4.0),
            (33.0, 6.0),
        ] {
            let got = size(&basis(current, PotentialKind::Line, 400.0), None);
            assert_eq!(got.section, expect, "{current} A");
        }
    }

    /// The table column follows the loaded conductors: single-phase and DC
    /// (two) read the two-conductor column, three-phase the three-conductor
    /// one. Method C PVC at 1.5 mm²: 19.5 A and 17.5 A in the built-in
    /// planning table (unverified against the standard, see the table
    /// disclosure shown with every plan).
    #[test]
    fn the_table_column_follows_the_number_of_loaded_conductors() {
        let one_phase = basis(19.0, PotentialKind::Line, 230.0);
        assert_eq!(size(&one_phase, None).section, 1.5, "19 A, two loaded");
        let three = basis(19.0, PotentialKind::Line, 400.0);
        assert_eq!(size(&three, None).section, 2.5, "19 A, three loaded");
        let dc = basis(19.0, PotentialKind::DcPositive, 24.0);
        assert_eq!(size(&dc, None).section, 1.5, "19 A, two loaded (DC)");
    }

    #[test]
    fn a_warm_ambient_and_grouping_ask_for_more_copper() {
        let mut b = basis(13.0, PotentialKind::Line, 400.0);
        assert_eq!(size(&b, None).section, 1.5);
        b.ambient_c = 50.0; // k1 = 0.71 for PVC: 17.5 A becomes 12.4 A
        assert_eq!(size(&b, None).section, 2.5);
        b.ambient_c = 30.0;
        b.grouped_circuits = 4; // k2 = 0.65: 17.5 A becomes 11.4 A
        assert_eq!(size(&b, None).section, 2.5);
    }

    /// 16 A over 100 m at 400 V: the 1.5 mm² carries it but drops 9.5 %, and
    /// 2.5 mm² still drops 5.7 % of the 5 % allowed; 4 mm² gives 3.6 %.
    #[test]
    fn a_long_run_is_sized_up_for_the_voltage_drop() {
        let b = basis(16.0, PotentialKind::Line, 400.0);
        assert_eq!(size(&b, None).section, 1.5);
        let long = size(&b, Some(100.0));
        assert_eq!(long.section, 4.0);
        assert!(long.reason.contains("voltage drop 3.57"), "{}", long.reason);
    }

    /// 6 A at 24 V DC over 10 m: 1 mm² would drop 10 %, 2.5 mm² gives 4.1 %.
    #[test]
    fn a_thin_control_wire_is_sized_up_when_the_run_is_long() {
        let mut b = basis(6.0, PotentialKind::DcPositive, 24.0);
        b.circuit = Circuit::Control;
        assert_eq!(size(&b, None).section, 1.0);
        assert_eq!(size(&b, Some(10.0)).section, 2.5);
        let mut small = basis(3.0, PotentialKind::DcPositive, 24.0);
        small.circuit = Circuit::Control;
        assert_eq!(size(&small, None).section, 0.75);
    }

    /// 16 A at 24 V DC over 600 m: even the largest listed section drops far too much.
    #[test]
    fn a_run_that_no_listed_section_can_serve_is_left_to_the_designer() {
        let mut b = basis(16.0, PotentialKind::DcPositive, 24.0);
        b.circuit = Circuit::Control;
        let why = size_wire(&b, Some(600.0), false, &TableSet::builtin(), 400.0, 5.0)
            .err()
            .expect("unresolved");
        assert!(why.contains("largest listed section"), "{why}");
        assert!(why.contains("120 mm²"), "{why}");
    }

    /// A long signal wire on a net with no voltage must not be judged against
    /// the 400 V supply (that would make the drop 17 times too small).
    #[test]
    fn a_control_wire_with_unknown_voltage_skips_the_drop_check_and_says_so() {
        let mut b = basis(3.0, PotentialKind::Signal, 0.0);
        b.potential_voltage = None;
        b.circuit = Circuit::Control;
        let got = size(&b, Some(200.0));
        assert_eq!(got.section, 0.75);
        assert!(
            got.reason.contains("voltage of the net unknown"),
            "{}",
            got.reason
        );
    }

    #[test]
    fn without_any_current_the_smallest_size_is_proposed_and_called_assumed() {
        let mut b = basis(6.0, PotentialKind::Line, 400.0);
        b.protecting = None;
        let got = size(&b, None);
        assert_eq!((got.section, got.basis), (1.5, Basis::Assumed));
        b.circuit = Circuit::Control;
        assert_eq!(size(&b, None).section, 0.75);
    }

    #[test]
    fn calculated_sizes_are_planning_values_while_the_tables_are_unverified() {
        let got = size(&basis(10.0, PotentialKind::Line, 400.0), None);
        assert_eq!(got.basis, Basis::Planning);
        assert!(
            got.reason.contains("F1 (10 A) protects it"),
            "{}",
            got.reason
        );
        assert!(got.reason.contains("method C"), "{}", got.reason);
    }

    #[test]
    fn a_stated_method_is_not_called_assumed() {
        let mut b = basis(10.0, PotentialKind::Line, 400.0);
        assert!(size(&b, None).reason.contains("method C,"));
        b.method = None;
        assert!(size(&b, None).reason.contains("method C assumed"));
    }

    #[test]
    fn colours_follow_iec_60204_1() {
        let code = |k, v, circuit| {
            let mut b = basis(10.0, k, v);
            b.circuit = circuit;
            colour_of(&b).0
        };
        assert_eq!(
            code(PotentialKind::ProtectiveEarth, 0.0, Circuit::Unknown),
            "GNYE"
        );
        assert_eq!(code(PotentialKind::Neutral, 230.0, Circuit::Power), "BU");
        assert_eq!(code(PotentialKind::Line, 400.0, Circuit::Power), "BK");
        assert_eq!(code(PotentialKind::Line, 230.0, Circuit::Control), "RD");
        assert_eq!(
            code(PotentialKind::DcPositive, 24.0, Circuit::Control),
            "BU"
        );
        assert_eq!(
            code(PotentialKind::DcNegative, 24.0, Circuit::Control),
            "BU"
        );
    }

    #[test]
    fn the_meter_counts_four_checks_per_wire_and_the_plate() {
        assert_eq!(ready(&[], true), Ready::default());
        assert_eq!(
            Ready {
                done: 5,
                total: 9,
                wires: 2
            }
            .percent(),
            55
        );
        assert_eq!(
            Ready {
                done: 8,
                total: 9,
                wires: 2
            }
            .percent(),
            88,
            "never rounds up to 100"
        );
        assert!(
            Ready {
                done: 9,
                total: 9,
                wires: 2
            }
            .is_complete()
        );
    }
}
