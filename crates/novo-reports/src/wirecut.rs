//! Wire production: the cut list a wire-processing machine or a wireman
//! works from, with end processing, labels, wire stock, ferrules and the
//! duct fill the lengths were routed with.

use std::collections::BTreeMap;

use novo_core::{
    AnyId, Device, DeviceId, EntityKind, PinRef, Project, Rail, StructureId, SymbolRole, WireId,
};
use novo_core::{Value, natural_cmp};

use crate::CsvOptions;
use crate::bench::FerruleStandard;
use crate::lengths::{self, Connection as Measured};
use crate::order::join_distinct;
use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};
use crate::wires::items;

/// Part property that gives the length a clamp or connector needs stripped.
pub const STRIP_LENGTH_KEY: &str = "strip_length_mm";
/// Wire property that turns the ferrule off (`none`) or names the one to use.
pub const FERRULE_KEY: &str = "ferrule";

/// Where a strip length came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripBasis {
    /// The part of the device the wire ends on gives it.
    Part,
    /// A typical value for the cross-section (see [`typical_strip_mm`]).
    Typical,
    /// The cross-section of the wire is unknown, so no strip length is
    /// proposed.
    Unknown,
}

impl StripBasis {
    pub fn text(self) -> &'static str {
        match self {
            StripBasis::Part => "part",
            StripBasis::Typical => "typical",
            StripBasis::Unknown => "unknown",
        }
    }
}

/// Typical strip length for a conductor of the given cross-section, which is
/// also the ferrule's contact length. A planning value in line with the usual
/// sleeve ranges (8 mm up to 1 mm2, 10 mm to 4 mm2, 12 mm to 16 mm2, 16 mm
/// to 35 mm2, 20 mm to 50 mm2, 24 mm above): the clamp's own data wins, and
/// a part can give it in [`STRIP_LENGTH_KEY`].
pub fn typical_strip_mm(cross_section_mm2: f64) -> f64 {
    match cross_section_mm2 {
        s if s <= 1.0 => 8.0,
        s if s <= 4.0 => 10.0,
        s if s <= 16.0 => 12.0,
        s if s <= 35.0 => 16.0,
        s if s <= 50.0 => 20.0,
        _ => 24.0,
    }
}

/// One end of a wire to be produced.
#[derive(Debug, Clone, PartialEq)]
pub struct WireEnd {
    pub pin: PinRef,
    /// `=A1+CAB-K1:13`.
    pub designation: String,
    /// Length to strip; `None` when the cross-section is unknown.
    pub strip_mm: Option<f64>,
    pub strip_basis: StripBasis,
    /// Ferrule designation (`A1.5-10`: cross-section and contact length), or
    /// `None` when the wire is marked without one or its size is unknown.
    pub ferrule: Option<String>,
    /// Label text for the flag at this end: the wire number and where the
    /// other end goes.
    pub label: String,
}

/// One wire to cut, in production order.
#[derive(Debug, Clone, PartialEq)]
pub struct WireCutJob {
    /// The drawn wires that make this connection.
    pub wires: Vec<WireId>,
    pub number: String,
    pub cross_section_mm2: Option<f64>,
    pub colour: String,
    /// The wire's routed length with the service loops, in whole millimetres
    /// (the wire list's cut length).
    pub length_mm: f64,
    /// What the machine cuts: the routed length plus the termination
    /// allowance at both ends, the same rule as the cut list.
    pub cut_mm: f64,
    /// How the length was found: `routed`, `routed, no depth`, `free air`
    /// or `estimate`.
    pub basis: String,
    pub end_a: WireEnd,
    pub end_b: WireEnd,
    /// The ducts the wire lies in, in order; empty when it is not routed.
    pub ducts: Vec<u64>,
}

fn device_of(project: &Project, pin: PinRef) -> Option<&Device> {
    let symbol = project.symbol(pin.symbol)?;
    let id: DeviceId = match &symbol.role {
        SymbolRole::Device { device, .. } => *device,
        SymbolRole::Terminal { .. } => project.terminal(symbol.terminal_of_pin(pin.pin)?)?.strip,
        _ => return None,
    };
    project.device(id)
}

fn number_property(project: &Project, device: &Device, key: &str) -> Option<f64> {
    device.parts.iter().find_map(|part| {
        match project.get::<novo_core::Part>(*part)?.properties.get(key)? {
            Value::Float(v) if v.is_finite() && *v > 0.0 => Some(*v),
            Value::Int(v) if *v > 0 => Some(*v as f64),
            _ => None,
        }
    })
}

fn number_text(v: f64) -> String {
    let t = format!("{v:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Where a wire end goes: `=A1+CAB-K1:13`, or the potential's name with
/// `(feed)` for the supply side of a potential feed.
fn end_designation(project: &Project, pin: PinRef) -> String {
    if let Some(SymbolRole::PotentialFeed { potential }) =
        project.symbol(pin.symbol).map(|s| &s.role)
        && let Some(p) = project.potential(*potential)
    {
        return format!("{} (feed)", p.name);
    }
    project.pin_designation(pin).unwrap_or_default()
}

fn end(
    project: &Project,
    pin: PinRef,
    other: PinRef,
    section: Option<f64>,
    ferrule_override: Option<&str>,
    number: &str,
    standard: &FerruleStandard,
) -> WireEnd {
    let designation = end_designation(project, pin);
    let from_part =
        device_of(project, pin).and_then(|d| number_property(project, d, STRIP_LENGTH_KEY));
    let (strip_mm, strip_basis) = match (from_part, section) {
        (Some(v), _) => (Some(v), StripBasis::Part),
        (None, Some(s)) => (
            Some(standard.length_mm(s).unwrap_or_else(|| typical_strip_mm(s))),
            StripBasis::Typical,
        ),
        (None, None) => (None, StripBasis::Unknown),
    };
    let ferrule = match ferrule_override {
        Some(text) if text.eq_ignore_ascii_case("none") => None,
        Some(text) => Some(text.to_owned()),
        None => section
            .zip(strip_mm)
            .map(|(s, l)| format!("A{}-{}", number_text(s), number_text(l))),
    };
    let to = end_designation(project, other);
    let label = match (number.is_empty(), to.is_empty()) {
        (false, false) => format!("{number} > {to}"),
        (false, true) => number.to_owned(),
        _ => to,
    };
    WireEnd {
        pin,
        designation,
        strip_mm,
        strip_basis,
        ferrule,
        label,
    }
}

/// Every connection as a wire to produce, ordered the way a machine batches
/// them: cross-section (smallest first, unknown last), colour, then longest
/// first, then wire number.
pub fn wire_cut_jobs(project: &Project) -> Vec<WireCutJob> {
    wire_cut_jobs_with(project, &FerruleStandard::of(project))
}

/// [`wire_cut_jobs`] with the ferrule standard given instead of the
/// project's: the strip length of a wire end whose part does not give one is
/// the standard's contact length for the cross-section.
pub fn wire_cut_jobs_with(project: &Project, standard: &FerruleStandard) -> Vec<WireCutJob> {
    produce(project, standard).0
}

type Plans = Vec<(StructureId, novo_routing::Plan)>;

fn produce(project: &Project, standard: &FerruleStandard) -> (Vec<WireCutJob>, Plans) {
    let items = items(project, true);
    let measured: Vec<Measured> = items
        .iter()
        .map(|i| Measured {
            from: i.from,
            to: i.to,
            cross_section_mm2: sections(project, &i.wires).last().copied(),
        })
        .collect();
    let lengths::Measurement { lengths, plans } = lengths::measure(project, &measured);
    let cut_list = crate::CutList::of(project);
    let mut jobs: Vec<WireCutJob> = items
        .iter()
        .zip(lengths)
        .map(|(item, length)| {
            let section = sections(project, &item.wires).last().copied();
            let mut numbers: Vec<&str> = item
                .wires
                .iter()
                .filter_map(|w| w.number.as_deref())
                .collect();
            numbers.sort_by(|a, b| natural_cmp(a, b));
            let number = join_distinct(numbers);
            let ferrule = item
                .wires
                .iter()
                .find_map(|w| match w.properties.get(FERRULE_KEY) {
                    Some(Value::Text(t)) => Some(t.as_str()),
                    _ => None,
                });
            WireCutJob {
                wires: item.wires.iter().map(|w| w.id).collect(),
                cross_section_mm2: section,
                colour: join_distinct(item.wires.iter().filter_map(|w| w.color.as_deref())),
                length_mm: length.mm,
                cut_mm: length.mm + 2.0 * cut_list.end_allowance_mm(section),
                basis: length.basis.text().to_owned(),
                end_a: end(
                    project, item.from, item.to, section, ferrule, &number, standard,
                ),
                end_b: end(
                    project, item.to, item.from, section, ferrule, &number, standard,
                ),
                ducts: length.ducts,
                number,
            }
        })
        .collect();
    jobs.sort_by(|a, b| {
        let key = |j: &WireCutJob| j.cross_section_mm2.unwrap_or(f64::INFINITY);
        key(a)
            .total_cmp(&key(b))
            .then_with(|| a.colour.cmp(&b.colour))
            .then_with(|| b.cut_mm.total_cmp(&a.cut_mm))
            .then_with(|| natural_cmp(&a.number, &b.number))
    });
    (jobs, plans)
}

/// The cross-sections of a connection's wires: the wire's own, else its
/// cable core's, smallest first.
pub(crate) fn sections(project: &Project, wires: &[&novo_core::Wire]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for wire in wires {
        let core = project.core_of_wire(wire.id).and_then(|(cable, core)| {
            let cable = project.cable(cable)?;
            cable.cores.get(core)
        });
        if let Some(value) = wire
            .cross_section_mm2
            .or(core.and_then(|c| c.cross_section_mm2))
            && !out.iter().any(|s| s.to_bits() == value.to_bits())
        {
            out.push(value);
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

/// Columns of the machine job file, in order. One row per wire; the
/// machine-specific formats (named when the workshop's machine is known) are
/// column maps over this same data.
pub const JOB_COLUMNS: [&str; 16] = [
    "No",
    "Wire no.",
    "Qty",
    "mm2",
    "Colour",
    "Cut mm",
    "Wire mm",
    "Strip A mm",
    "Ferrule A",
    "Label A",
    "Strip B mm",
    "Ferrule B",
    "Label B",
    "From",
    "To",
    "Length basis",
];

fn job_cells(n: usize, j: &WireCutJob) -> Vec<Cell> {
    vec![
        Cell::Int(n as i64),
        Cell::text(j.number.clone()),
        Cell::Int(1),
        j.cross_section_mm2.into(),
        Cell::text(j.colour.clone()),
        Cell::Number(j.cut_mm),
        Cell::Number(j.length_mm),
        j.end_a.strip_mm.into(),
        j.end_a.ferrule.clone().into(),
        Cell::text(j.end_a.label.clone()),
        j.end_b.strip_mm.into(),
        j.end_b.ferrule.clone().into(),
        Cell::text(j.end_b.label.clone()),
        Cell::text(j.end_a.designation.clone()),
        Cell::text(j.end_b.designation.clone()),
        Cell::text(j.basis.clone()),
    ]
}

/// The generic wire-cut job file as CSV: one row per wire under
/// [`JOB_COLUMNS`], in production order. Use `;` as the delimiter for a
/// machine tool that expects decimal commas.
pub fn wire_cut_csv(jobs: &[WireCutJob], options: &CsvOptions) -> String {
    let mut section = Section::new(
        "cuts",
        "",
        JOB_COLUMNS
            .iter()
            .map(|h| Column::new(h, h, 10.0))
            .collect(),
    );
    for (i, j) in jobs.iter().enumerate() {
        section.push(job_cells(i + 1, j), Vec::new());
    }
    crate::to_csv(
        &ReportOutput {
            report: WireCut::ID.into(),
            title: "Wire cut".into(),
            revision: 0,
            sections: vec![section],
        },
        options,
    )
}

/// The wire production report (`wirecut`): the job list, the wire stock to pull, the
/// ferrules to order and how full the ducts are.
#[derive(Debug, Clone, Copy, Default)]
pub struct WireCut;

impl WireCut {
    pub const ID: &'static str = "wirecut";
}

impl Report for WireCut {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Wire production".into()
    }

    fn proposed_by_default(&self) -> bool {
        false
    }

    fn dependencies(&self) -> Dependencies {
        use EntityKind::*;
        Dependencies::on(&[
            Page,
            Device,
            Symbol,
            Junction,
            Wire,
            Potential,
            Terminal,
            Cable,
            CabinetPlacement,
            Rail,
            Part,
            Structure,
        ])
        .with_info()
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let (jobs, plans) = produce(project, &FerruleStandard::of(project));
        if jobs.is_empty() {
            return ReportOutput {
                report: Self::ID.into(),
                title: self.title(),
                revision: project.revision(),
                sections: Vec::new(),
            };
        }
        let columns = vec![
            Column::new("no", "No", 6.0).right(),
            Column::new("number", "Wire no.", 15.0),
            Column::new("qty", "Qty", 6.0).right(),
            Column::new("mm2", "mm²", 8.0).right(),
            Column::new("colour", "Colour", 10.0),
            Column::new("cut", "Cut mm", 10.0).right(),
            Column::new("length", "Wire mm", 10.0).right(),
            Column::new("strip_a", "Strip A", 8.0).right(),
            Column::new("ferrule_a", "Ferrule A", 12.0),
            Column::new("label_a", "Label A", 30.0),
            Column::new("strip_b", "Strip B", 8.0).right(),
            Column::new("ferrule_b", "Ferrule B", 12.0),
            Column::new("label_b", "Label B", 30.0),
            Column::new("from", "From", 25.0),
            Column::new("to", "To", 25.0),
            Column::new("basis", "Length basis", 14.0),
        ];
        let mut cuts = Section::new("cuts", "Wires to cut", columns);
        for (i, j) in jobs.iter().enumerate() {
            cuts.push(
                job_cells(i + 1, j),
                j.wires.iter().map(|w| AnyId::from(*w)).collect(),
            );
        }

        // Stock per cross-section and colour: pieces and metres, as cut.
        let mut stock: BTreeMap<(u64, String), (usize, f64)> = BTreeMap::new();
        for j in &jobs {
            let key = (
                j.cross_section_mm2.map_or(u64::MAX, |s| s.to_bits()),
                j.colour.clone(),
            );
            let e = stock.entry(key).or_default();
            e.0 += 1;
            e.1 += j.cut_mm;
        }
        let mut stock_rows: Vec<_> = stock.into_iter().collect();
        stock_rows.sort_by(|a, b| {
            let s = |k: u64| {
                if k == u64::MAX {
                    f64::INFINITY
                } else {
                    f64::from_bits(k)
                }
            };
            s(a.0.0).total_cmp(&s(b.0.0)).then(a.0.1.cmp(&b.0.1))
        });
        let mut stock_section = Section::new(
            "stock",
            "Wire stock",
            vec![
                Column::new("mm2", "mm²", 10.0).right(),
                Column::new("colour", "Colour", 12.0),
                Column::new("pieces", "Pieces", 10.0).right(),
                Column::new("metres", "Metres", 12.0).right(),
            ],
        );
        for ((bits, colour), (pieces, mm)) in stock_rows {
            let section = (bits != u64::MAX).then(|| f64::from_bits(bits));
            stock_section.push(
                vec![
                    section.into(),
                    Cell::text(colour),
                    Cell::Int(pieces as i64),
                    Cell::Number((mm / 1000.0 * 100.0).ceil() / 100.0),
                ],
                Vec::new(),
            );
        }

        let mut ferrules: BTreeMap<String, i64> = BTreeMap::new();
        for j in &jobs {
            for f in [&j.end_a.ferrule, &j.end_b.ferrule].into_iter().flatten() {
                *ferrules.entry(f.clone()).or_default() += 1;
            }
        }
        let mut ferrule_section = Section::new(
            "ferrules",
            "Ferrules",
            vec![
                Column::new("ferrule", "Ferrule", 14.0),
                Column::new("qty", "Qty", 10.0).right(),
            ],
        );
        for (f, n) in ferrules {
            ferrule_section.push(vec![Cell::text(f), Cell::Int(n)], Vec::new());
        }

        // Duct fill after routing, with the wires that lie in each duct.
        let mut ducts = Section::new(
            "ducts",
            "Duct fill",
            vec![
                Column::new("duct", "Duct", 24.0),
                Column::new("wires", "Wires", 8.0).right(),
                Column::new("used", "Used mm²", 12.0).right(),
                Column::new("capacity", "Allowed mm²", 12.0).right(),
                Column::new("fill", "Fill %", 8.0).right(),
                Column::new("status", "Status", 12.0),
                Column::new("numbers", "Wire nos.", 40.0),
            ],
        );
        for (structure, plan) in &plans {
            let tag = project
                .get::<novo_core::Structure>(*structure)
                .map(|s| s.tag.to_string())
                .unwrap_or_default();
            for fill in &plan.fill {
                let rail = project.all::<Rail>().find(|r| r.id.raw() == fill.duct);
                let name = rail.map_or(format!("{tag} duct"), |r| {
                    format!("{tag} duct at {:.0}, {:.0}", r.x, r.y)
                });
                let sized = fill.capacity_mm2 > 0.0;
                let mut numbers: Vec<&str> = jobs
                    .iter()
                    .filter(|j| j.ducts.contains(&fill.duct))
                    .map(|j| j.number.as_str())
                    .collect();
                numbers.sort_by(|a, b| natural_cmp(a, b));
                let status = if !sized {
                    "unsized"
                } else if fill.ratio > 1.0 {
                    "overfull"
                } else {
                    "ok"
                };
                ducts.push(
                    vec![
                        Cell::text(name),
                        Cell::Int(fill.jobs.len() as i64),
                        Cell::Number((fill.used_mm2 * 10.0).round() / 10.0),
                        sized
                            .then(|| (fill.capacity_mm2 * 10.0).round() / 10.0)
                            .into(),
                        sized.then(|| (fill.ratio * 1000.0).round() / 10.0).into(),
                        Cell::text(status),
                        Cell::text(join_distinct(numbers)),
                    ],
                    rail.map(|r| AnyId::from(r.id)).into_iter().collect(),
                );
            }
        }

        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections: vec![cuts, stock_section, ferrule_section, ducts],
        }
    }
}
