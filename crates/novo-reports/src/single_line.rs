//! The single-line diagram as a generated page: the power distribution of
//! the project drawn from the supply down to its loads, with the cable size
//! and length, the protection rating, the load current and the voltage drop
//! on every branch.
//!
//! The model and every number come from [`novo_checks::single_line`]; this
//! file lays them out. The report has one section per drawn sheet. A sheet
//! prints as the drawing ([`sheet_layout`]); the section's table is the same
//! boxes as a schedule, so the Reports workspace, Excel and CSV list exactly
//! what the sheet draws, and a schedule longer than a page continues as an
//! ordinary table.
//!
//! A wide distribution is split over sheets by branch: each sheet repeats
//! the supply, and a branch that is too wide for one sheet is split below
//! its first fitting level, with the boxes above it repeated and marked
//! "continued".
//!
//! # Reading the figures
//!
//! A figure without a mark is stated in the project. `~` marks a current
//! calculated from a power or from the loads below, `?` an assumed input and
//! `*` a cross-section proposed by the Autopilot and not applied. Cable
//! ratings (`Iz`) and voltage drops (`ΔU`) are always calculated; with the
//! built-in ampacity tables they are planning values.

use std::collections::BTreeMap;

use novo_checks::calc::circuit::Status;
use novo_checks::calc::tables::TableSet;
use novo_checks::single_line::{Figure, Kind, Link, Node, Origin, SingleLine, fmt, single_line};
use novo_core::{AnyId, EntityKind, Project, WireId};

use crate::autopilot::Change;
use crate::page::{Line, PageLayout, PageSetup, TextBox};
use crate::report::{Dependencies, Report};
use crate::table::{Align, Cell, Column, ReportOutput, Section};

/// Leaf columns on a sheet.
const COLUMNS_PER_SHEET: usize = 7;
/// Distance between two columns, mm.
const PITCH_MM: f64 = 50.0;
/// Width of a box, mm.
const BOX_W: f64 = 44.0;
/// Height of a box, mm.
const BOX_H: f64 = 17.0;
/// Width of the supply box, mm.
const SUPPLY_W: f64 = 64.0;
/// Height of the run between two levels, mm.
const LINK_H: f64 = 21.0;
/// Character height inside a box, mm.
const TEXT_MM: f64 = 2.3;
/// Character height of the legend, mm.
const SMALL_MM: f64 = 2.6;
/// Line pitch of the legend, mm.
const LEGEND_PITCH: f64 = 3.9;
/// Smallest scale a crowded sheet is shrunk to, and the largest a sparse one
/// is enlarged to.
const MIN_SCALE: f64 = 0.55;
const MAX_SCALE: f64 = 1.6;

/// The single-line diagram report.
#[derive(Debug, Clone, Copy, Default)]
pub struct SingleLineDiagram;

impl SingleLineDiagram {
    pub const ID: &'static str = "singleline";
}

/// The diagram of a project, with the Autopilot's cross-sections for the
/// wires that have none.
pub fn build(project: &Project) -> SingleLine {
    let tables = TableSet::builtin();
    let netlist = project.netlist();
    let plain = single_line(project, &netlist, &tables, &BTreeMap::new());
    let lacks_section = plain.nodes.iter().any(|n| {
        n.link
            .as_ref()
            .is_some_and(|l| l.segments.iter().any(|s| s.section_mm2.is_missing()))
    });
    if !lacks_section {
        return plain;
    }
    let plan = crate::autopilot::plan_for(project, &tables);
    let proposed: BTreeMap<WireId, f64> = plan
        .proposals
        .iter()
        .filter_map(|p| match p.change {
            Change::Size(s) => Some((p, s)),
            _ => None,
        })
        .flat_map(|(p, s)| p.wires.iter().map(move |w| (*w, s)))
        .collect();
    if proposed.is_empty() {
        return plain;
    }
    single_line(project, &netlist, &tables, &proposed)
}

/// What one sheet draws.
#[derive(Debug, Clone, PartialEq)]
struct Plan {
    /// A branch: boxes repeated from above, then the subtree.
    items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
struct Item {
    /// Ancestors below the supply, outermost first, repeated from another
    /// sheet.
    chain: Vec<usize>,
    root: usize,
    /// The boxes the schedule lists for this item.
    rows: Vec<usize>,
}

fn subtree(sl: &SingleLine, root: usize, out: &mut Vec<usize>) {
    out.push(root);
    for &c in sl.children(root) {
        subtree(sl, c, out);
    }
}

fn width(sl: &SingleLine, node: usize) -> usize {
    let kids = sl.children(node);
    if kids.is_empty() {
        1
    } else {
        kids.iter().map(|c| width(sl, *c)).sum()
    }
}

fn split(sl: &SingleLine, node: usize, chain: &[usize], out: &mut Vec<Item>) {
    if width(sl, node) <= COLUMNS_PER_SHEET {
        let mut rows = Vec::new();
        subtree(sl, node, &mut rows);
        out.push(Item {
            chain: chain.to_vec(),
            root: node,
            rows,
        });
        return;
    }
    let mut below = chain.to_vec();
    below.push(node);
    let first = out.len();
    for &c in sl.children(node) {
        split(sl, c, &below, out);
    }
    // The wide box itself is listed with its first branch.
    if let Some(item) = out.get_mut(first) {
        item.rows.insert(0, node);
    }
}

fn plan(sl: &SingleLine) -> Vec<Plan> {
    let mut items = Vec::new();
    for &c in sl.children(0) {
        split(sl, c, &[], &mut items);
    }
    let mut sheets: Vec<Plan> = Vec::new();
    let mut used = 0;
    for item in items {
        let w = width(sl, item.root);
        match sheets.last_mut() {
            Some(sheet) if used + w <= COLUMNS_PER_SHEET => {
                sheet.items.push(item);
                used += w;
            }
            _ => {
                sheets.push(Plan { items: vec![item] });
                used = w;
            }
        }
    }
    if sheets.is_empty() {
        sheets.push(Plan { items: Vec::new() });
    }
    sheets
}

/// `-F1` from `=A1+CAB-F1`.
fn short_tag(tag: &str) -> &str {
    tag.rfind('-').map_or(tag, |i| &tag[i..])
}

/// `=A1+CAB` from `=A1+CAB-F1`.
fn place_of(tag: &str) -> &str {
    tag.rfind('-').map_or("", |i| &tag[..i])
}

fn mark(origin: Origin) -> &'static str {
    match origin {
        Origin::Stated => "",
        Origin::Calculated => "~",
        Origin::Proposed => "*",
        Origin::Assumed => "?",
        Origin::Missing => "",
    }
}

fn amps(f: Figure) -> String {
    match f.value {
        Some(v) => format!("{} A{}", fmt(v), mark(f.origin)),
        None => "n/a".to_owned(),
    }
}

fn status_word(status: Status) -> &'static str {
    match status {
        Status::Pass => "ok",
        Status::Fail => "FAIL",
        Status::Warning => "check",
        Status::Unknown => "incomplete",
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn number(f: Figure) -> Cell {
    f.value.map_or(Cell::Empty, |v| Cell::Number(round2(v)))
}

fn columns() -> Vec<Column> {
    // The verdict and what was assumed come before the long texts: on a
    // 1366 px window the first columns up to "Assumed" fit.
    vec![
        Column::new("tag", "Device", 17.5),
        Column::new("type", "Type", 11.0),
        Column::new("in", "In A", 5.0).right(),
        Column::new("curve", "Curve", 5.0),
        Column::new("icu", "Icu kA", 5.0).right(),
        Column::new("ik", "Ik max kA", 7.0).right(),
        Column::new("ib", "Ib A", 6.0).right(),
        Column::new("section", "mm²", 5.0).right(),
        Column::new("length", "m", 5.0).right(),
        Column::new("iz", "Iz A", 6.0).right(),
        Column::new("du", "ΔU %", 6.0).right(),
        Column::new("du_total", "ΔU Σ %", 6.5).right(),
        Column::new("check", "Check", 14.0),
        Column::new("why", "Why", 26.0),
        Column::new("assumed", "Assumed", 10.0),
        Column::new("ib_basis", "Ib basis", 28.0),
        Column::new("cable", "Cable", 8.0),
    ]
}

/// The basis of a current for the table: the notes and the drawing's legend
/// carry the no-diversity remark and the advice in brackets.
fn basis_short(basis: &str) -> String {
    basis
        .split(" (")
        .next()
        .unwrap_or_default()
        .replace(", no diversity", "")
}

/// What a run's Iz and ΔU assumed, for the schedule's "Assumed" column.
fn assumed_text(link: &Link) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if !link.iz_defaults.is_empty() {
        parts.push("Iz ?");
    }
    if link.drop_pf_assumed {
        parts.push("ΔU ?");
    }
    parts.join(", ")
}

fn cable_text(link: &Link) -> String {
    let mut tags: Vec<&str> = link
        .segments
        .iter()
        .filter_map(|s| s.cable.as_deref())
        .map(short_tag)
        .collect();
    tags.dedup();
    tags.join(", ")
}

fn row(sl: &SingleLine, node: &Node) -> (Vec<Cell>, Vec<AnyId>) {
    let link = node.link.as_ref();
    let mut why: Vec<String> = Vec::new();
    if let Some(l) = link {
        why.extend(l.reasons.iter().cloned());
    }
    if let Some(v) = &node.verdict
        && !v.failing.is_empty()
    {
        why.push(format!("Calculations: {}", v.failing.join(", ")));
    }
    if let Some(v) = &node.verdict
        && v.status == Status::Unknown
        && v.failing.is_empty()
    {
        why.push(if v.missing.is_empty() {
            "the calculation lacks an input".to_owned()
        } else {
            format!(
                "missing: {}",
                v.missing
                    .iter()
                    .map(|m| m
                        .trim_end_matches("_ka")
                        .trim_end_matches("_a")
                        .replace('_', " "))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
    }
    let check = match (
        link.map(|l| l.status),
        node.verdict.as_ref().map(|v| v.status),
    ) {
        (Some(Status::Fail), _) | (_, Some(Status::Fail)) => Status::Fail,
        (Some(Status::Unknown), _) => Status::Unknown,
        (_, Some(Status::Warning)) | (Some(Status::Warning), _) => Status::Warning,
        (_, Some(Status::Unknown)) if node.kind.is_protective() => Status::Unknown,
        _ => Status::Pass,
    };
    let _ = sl;
    let cells = vec![
        Cell::text(node.tag.clone()),
        Cell::text(node.kind.short_label()),
        number(node.rated_a),
        Cell::text(
            node.characteristic
                .as_ref()
                .map(|(c, o)| format!("{c}{}", mark(*o)))
                .unwrap_or_default(),
        ),
        number(node.breaking_ka),
        node.verdict
            .as_ref()
            .and_then(|v| v.ik_max_ka)
            .map_or(Cell::Empty, |k| Cell::Number(round2(k))),
        number(node.current_a),
        link.map_or(Cell::Empty, |l| number(l.section_mm2)),
        link.map_or(Cell::Empty, |l| number(l.length_m)),
        link.map_or(Cell::Empty, |l| number(l.iz_a)),
        link.map_or(Cell::Empty, |l| number(l.drop_percent)),
        link.map_or(Cell::Empty, |l| number(l.drop_total_percent)),
        Cell::text(if link.is_some() || node.verdict.is_some() {
            status_word(check)
        } else {
            ""
        }),
        Cell::text(why.join("; ")),
        Cell::text(link.map(assumed_text).unwrap_or_default()),
        Cell::text(basis_short(&node.current_basis)),
        Cell::text(link.map(cable_text).unwrap_or_default()),
    ];
    let mut sources: Vec<AnyId> = Vec::new();
    if let Some(d) = node.device {
        sources.push(AnyId::Device(d));
    }
    if let Some(l) = link {
        sources.extend(l.segments.iter().map(|s| AnyId::Wire(s.wire)));
    }
    (cells, sources)
}

impl Report for SingleLineDiagram {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Single-line diagram".into()
    }

    fn dependencies(&self) -> Dependencies {
        use EntityKind::*;
        Dependencies::on(&[
            Page, Device, Symbol, Junction, Wire, Potential, Terminal, Cable, Part,
        ])
        .with_info()
    }

    /// Drawn on request: a project without a power distribution has no
    /// diagram to propose.
    fn proposed_by_default(&self) -> bool {
        false
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let sl = build(project);
        let sheets = plan(&sl);
        let mut sections = Vec::new();
        if !sl.is_empty() {
            let count = sheets.len();
            for (i, sheet) in sheets.iter().enumerate() {
                let title = if count > 1 {
                    format!("Sheet {} of {}", i + 1, count)
                } else {
                    String::new()
                };
                let mut section = Section::new(&format!("sheet{}", i + 1), &title, columns());
                if i == 0 {
                    let (cells, sources) = row(&sl, &sl.nodes[0]);
                    section.push(cells, sources);
                }
                for item in &sheet.items {
                    for &n in &item.rows {
                        let (cells, sources) = row(&sl, &sl.nodes[n]);
                        section.push(cells, sources);
                    }
                }
                sections.push(section);
            }
        }
        ReportOutput {
            report: Self::ID.to_owned(),
            title: self.title(),
            revision: project.revision(),
            sections,
        }
    }
}

// ---------------------------------------------------------------------------
// Drawing

struct Canvas {
    layout: PageLayout,
    /// Scale and offset from natural mm to the sheet.
    scale: f64,
    ox: f64,
    oy: f64,
    /// The right edge of the usable area in natural mm: a text box never
    /// reaches past it.
    right: f64,
}

impl Canvas {
    fn x(&self, v: f64) -> f64 {
        self.ox + v * self.scale
    }

    fn y(&self, v: f64) -> f64 {
        self.oy + v * self.scale
    }

    fn line(&mut self, a: (f64, f64), b: (f64, f64), heavy: bool) {
        self.layout.lines.push(Line {
            from: (self.x(a.0), self.y(a.1)),
            to: (self.x(b.0), self.y(b.1)),
            heavy,
        });
    }

    #[allow(clippy::too_many_arguments)] // a text box is a position, a size, the words and a style
    fn text(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        text: &str,
        mm: f64,
        align: Align,
        bold: bool,
    ) {
        if text.is_empty() {
            return;
        }
        let w = w.min(self.right - x).max(0.0);
        self.layout.texts.push(TextBox {
            x: self.x(x),
            y: self.y(y),
            width: w * self.scale,
            height: h * self.scale,
            text: text.to_owned(),
            text_mm: mm * self.scale,
            align,
            bold,
        });
    }

    /// A box with its top-left corner at `(x, y)`.
    fn frame(&mut self, x: f64, y: f64, w: f64, h: f64, heavy: bool) {
        self.line((x, y), (x + w, y), heavy);
        self.line((x + w, y), (x + w, y + h), heavy);
        self.line((x + w, y + h), (x, y + h), heavy);
        self.line((x, y + h), (x, y), heavy);
    }
}

fn load_line(node: &Node) -> String {
    match node.kind {
        Kind::Supply => String::new(),
        k if k.is_protective() => {
            let mut s = format!("In {}", amps(node.rated_a));
            if let Some((c, o)) = &node.characteristic {
                s.push_str(&format!(" {c}{}", mark(*o)));
            }
            if let Some(icu) = node.breaking_ka.value {
                s.push_str(&format!("  Icu {} kA", fmt(icu)));
            }
            s
        }
        Kind::OverloadRelay => format!("Ir {}", amps(node.rated_a)),
        Kind::Contactor if node.rated_a.value.is_some() => {
            format!("AC-3 {}", amps(node.rated_a))
        }
        k if k.is_load() => {
            let mut s = String::new();
            if let Some(p) = node.power_kw.value {
                s.push_str(&format!("{} kW  ", fmt(p)));
            }
            if node.rated_a.value.is_some() {
                s.push_str(&format!("In {}", amps(node.rated_a)));
            }
            s.trim_end().to_owned()
        }
        _ => String::new(),
    }
}

fn current_line(node: &Node) -> String {
    let mut s = match node.current_a.value {
        Some(v) => format!("Ib {} A{}", fmt(v), mark(node.current_a.origin)),
        None => "Ib n/a".to_owned(),
    };
    if node.loads_without_current > 0 {
        s.push_str(&format!(" (+{} without)", node.loads_without_current));
    }
    if let Some(ik) = node.verdict.as_ref().and_then(|v| v.ik_max_ka) {
        s.push_str(&format!("  Ik ≤ {} kA", fmt(ik)));
    }
    s
}

impl Canvas {
    /// A closed polygon approximating a circle.
    fn circle(&mut self, cx: f64, cy: f64, r: f64) {
        const SIDES: usize = 20;
        let point = |i: usize| {
            let a = std::f64::consts::TAU * i as f64 / SIDES as f64;
            (cx + r * a.cos(), cy + r * a.sin())
        };
        for i in 0..SIDES {
            self.line(point(i), point(i + 1), false);
        }
    }

    fn rect(&mut self, cx: f64, cy: f64, w: f64, h: f64) {
        self.frame(cx - w / 2.0, cy - h / 2.0, w, h, false);
    }

    /// The IEC-style symbol of a box, about 10 mm square, centred on
    /// `(cx, cy)`. Simplified: enough to tell a fuse from a breaker from a
    /// motor at a glance, not the full library drawing.
    fn symbol(&mut self, kind: Kind, cx: f64, cy: f64) {
        let lead = |c: &mut Canvas| {
            c.line((cx, cy - 5.0), (cx, cy - 2.0), false);
            c.line((cx, cy + 2.0), (cx, cy + 5.0), false);
        };
        match kind {
            Kind::Fuse => {
                self.rect(cx, cy, 3.6, 8.0);
                self.line((cx, cy - 5.0), (cx, cy + 5.0), false);
            }
            Kind::FuseSwitch => {
                self.rect(cx + 1.5, cy, 3.0, 6.0);
                self.line((cx + 1.5, cy - 5.0), (cx + 1.5, cy - 3.0), false);
                self.line((cx + 1.5, cy + 3.0), (cx + 1.5, cy + 5.0), false);
                self.line((cx - 3.0, cy + 4.0), (cx - 3.0, cy - 3.0), false);
            }
            Kind::Disconnector
            | Kind::Contactor
            | Kind::Breaker
            | Kind::Rcd
            | Kind::MotorProtection
            | Kind::MotorCircuitProtector => {
                self.line((cx, cy + 5.0), (cx, cy + 1.5), false);
                self.line((cx, cy + 1.5), (cx - 3.5, cy - 3.0), false);
                self.line((cx, cy - 1.0), (cx, cy - 5.0), false);
                match kind {
                    Kind::Breaker
                    | Kind::Rcd
                    | Kind::MotorProtection
                    | Kind::MotorCircuitProtector => {
                        self.line((cx - 4.5, cy - 2.2), (cx - 2.5, cy - 4.2), false);
                        self.line((cx - 4.5, cy - 4.2), (cx - 2.5, cy - 2.2), false);
                    }
                    Kind::Contactor => {
                        self.line((cx - 1.2, cy - 0.5), (cx + 1.8, cy - 0.5), false);
                    }
                    _ => {}
                }
            }
            Kind::OverloadRelay => {
                lead(self);
                self.rect(cx, cy, 7.0, 4.0);
                self.line((cx - 2.5, cy + 1.0), (cx - 1.0, cy - 1.0), false);
                self.line((cx - 1.0, cy - 1.0), (cx + 1.0, cy + 1.0), false);
                self.line((cx + 1.0, cy + 1.0), (cx + 2.5, cy - 1.0), false);
            }
            Kind::Motor => {
                self.circle(cx, cy, 4.6);
                self.layout.texts.push(TextBox {
                    x: self.x(cx - 3.0),
                    y: self.y(cy - 2.0),
                    width: 6.0 * self.scale,
                    height: 4.0 * self.scale,
                    text: "M".to_owned(),
                    text_mm: TEXT_MM * self.scale,
                    align: Align::Center,
                    bold: true,
                });
            }
            Kind::Heater => {
                lead(self);
                self.rect(cx, cy, 3.6, 4.0);
            }
            Kind::Lamp => {
                self.circle(cx, cy, 4.2);
                self.line((cx - 3.0, cy - 3.0), (cx + 3.0, cy + 3.0), false);
                self.line((cx - 3.0, cy + 3.0), (cx + 3.0, cy - 3.0), false);
            }
            Kind::Sounder => {
                self.rect(cx - 2.0, cy, 3.0, 4.0);
                self.line((cx - 0.5, cy - 2.0), (cx + 3.5, cy - 4.5), false);
                self.line((cx - 0.5, cy + 2.0), (cx + 3.5, cy + 4.5), false);
                self.line((cx + 3.5, cy - 4.5), (cx + 3.5, cy + 4.5), false);
            }
            Kind::Drive | Kind::SoftStarter | Kind::PowerSupply => {
                self.rect(cx, cy, 9.0, 8.0);
                self.line((cx - 4.5, cy + 4.0), (cx + 4.5, cy - 4.0), false);
            }
            Kind::Transformer => {
                self.circle(cx, cy - 2.2, 3.0);
                self.circle(cx, cy + 2.2, 3.0);
            }
            Kind::Supply => {
                self.circle(cx, cy, 4.6);
                self.line((cx - 3.0, cy), (cx - 1.5, cy - 2.0), false);
                self.line((cx - 1.5, cy - 2.0), (cx, cy), false);
                self.line((cx, cy), (cx + 1.5, cy + 2.0), false);
                self.line((cx + 1.5, cy + 2.0), (cx + 3.0, cy), false);
            }
        }
    }
}

fn draw_box(c: &mut Canvas, sl: &SingleLine, node: &Node, cx: f64, y: f64, continued: bool) {
    let w = if node.kind == Kind::Supply {
        SUPPLY_W
    } else {
        BOX_W
    };
    let tw = w - 15.0;
    let x = cx - w / 2.0;
    c.frame(x, y, w, BOX_H, node.fails());
    c.symbol(node.kind, x + w - 8.0, y + BOX_H / 2.0);
    let line_h = 3.5;
    let pad = 1.2;
    let (top, second) = if node.kind == Kind::Supply {
        (format!("{} V supply", fmt(sl.voltage_v)), node.tag.clone())
    } else {
        (
            format!("{}  {}", short_tag(&node.tag), node.kind.short_label()),
            if continued {
                "continued from the sheet before".to_owned()
            } else if node.description.is_empty() {
                place_of(&node.tag).to_owned()
            } else {
                node.description.clone()
            },
        )
    };
    c.text(
        x + pad,
        y + 0.6,
        tw,
        line_h,
        &top,
        TEXT_MM,
        Align::Left,
        true,
    );
    c.text(
        x + pad,
        y + 0.6 + line_h,
        tw,
        line_h,
        &second,
        SMALL_MM,
        Align::Left,
        false,
    );
    if node.kind == Kind::Supply {
        let ik = match sl.supply_ik_ka.value {
            Some(k) => format!("Ik max {} kA", fmt(k)),
            None => "Ik max n/a".to_owned(),
        };
        c.text(
            x + pad,
            y + 0.6 + 2.0 * line_h,
            tw,
            line_h,
            &ik,
            TEXT_MM,
            Align::Left,
            false,
        );
        c.text(
            x + pad,
            y + 0.6 + 3.0 * line_h,
            tw,
            line_h,
            &current_line(node),
            TEXT_MM,
            Align::Left,
            false,
        );
        return;
    }
    if continued {
        return;
    }
    c.text(
        x + pad,
        y + 0.6 + 2.0 * line_h,
        tw,
        line_h,
        &load_line(node),
        TEXT_MM,
        Align::Left,
        false,
    );
    c.text(
        x + pad,
        y + 0.6 + 3.0 * line_h,
        tw,
        line_h,
        &current_line(node),
        TEXT_MM,
        Align::Left,
        false,
    );
}

fn link_lines(link: &Link) -> Vec<String> {
    let mut out = Vec::new();
    let section = match link.section_mm2.value {
        Some(s) => format!(
            "{}x{} mm²{}",
            link.poles.max(1),
            fmt(s),
            mark(link.section_mm2.origin)
        ),
        None => "no cross-section".to_owned(),
    };
    let cable = cable_text(link);
    out.push(if cable.is_empty() {
        section
    } else {
        format!("{cable} {section}")
    });
    let length = link
        .length_m
        .value
        .map_or("no length".to_owned(), |l| format!("{} m", fmt(l)));
    out.push(match link.iz_a.value {
        Some(iz) => format!("{length}  Iz {} A{}", fmt(iz), mark(link.iz_a.origin)),
        None => length,
    });
    if let Some(d) = link.drop_percent.value {
        let mut s = format!("ΔU {} %{}", fmt(d), mark(link.drop_percent.origin));
        if let Some(t) = link.drop_total_percent.value
            && (t - d).abs() > 0.005
        {
            s.push_str(&format!(
                " Σ {} %{}",
                fmt(t),
                mark(link.drop_total_percent.origin)
            ));
        }
        if !link.total_complete {
            s.push_str(" +");
        }
        out.push(s);
    }
    if let Some(first) = link.inline.first() {
        out.push(format!(
            "via {}",
            short_tag(first.split(' ').next().unwrap_or(""))
        ));
    }
    out
}

struct Placed {
    cx: f64,
    level: usize,
}

/// Lays the subtree of `node` out: leaves take the next column, a parent
/// sits over its children.
fn place(
    sl: &SingleLine,
    node: usize,
    level: usize,
    next: &mut f64,
    out: &mut BTreeMap<usize, Placed>,
) -> f64 {
    let kids = sl.children(node);
    let cx = if kids.is_empty() {
        let c = *next + 0.5;
        *next += 1.0;
        c
    } else {
        let xs: Vec<f64> = kids
            .iter()
            .map(|k| place(sl, *k, level + 1, next, out))
            .collect();
        (xs[0] + xs[xs.len() - 1]) / 2.0
    };
    out.insert(node, Placed { cx, level });
    cx
}

fn level_y(level: usize) -> f64 {
    level as f64 * (BOX_H + LINK_H)
}

/// Draws the run from a parent's bottom to a child's top, with its label.
fn draw_run(
    c: &mut Canvas,
    from_x: f64,
    from_level: usize,
    to_x: f64,
    to_level: usize,
    link: Option<&Link>,
) {
    let top = level_y(from_level) + BOX_H;
    let bottom = level_y(to_level);
    let mid = top + LINK_H / 2.0;
    let heavy = link.is_some_and(|l| l.status == Status::Fail);
    if (from_x - to_x).abs() < 0.01 {
        c.line((to_x, top), (to_x, bottom), heavy);
    } else {
        c.line((from_x, top), (from_x, mid), false);
        c.line((from_x, mid), (to_x, mid), false);
        c.line((to_x, mid), (to_x, bottom), heavy);
    }
    let Some(link) = link else { return };
    let lines = link_lines(link);
    let h = 2.9;
    let straight = (from_x - to_x).abs() < 0.01;
    let y0 = if straight { top + 1.0 } else { mid + 0.5 };
    for (i, text) in lines.iter().enumerate() {
        c.text(
            to_x + 1.2,
            y0 + h * i as f64,
            PITCH_MM - 3.0,
            h,
            text,
            SMALL_MM,
            Align::Left,
            false,
        );
    }
    if link.status == Status::Fail {
        let n = lines.len() as f64;
        let why = format!("FAIL {}", link.fail_codes.join(", "));
        let why = why.as_str();
        c.text(
            to_x + 1.2,
            y0 + h * n,
            PITCH_MM - 3.0,
            h,
            why,
            SMALL_MM,
            Align::Left,
            true,
        );
    }
}

/// `text` broken at spaces into lines of at most `max` characters.
fn wrap(text: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > max {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn legend(sl: &SingleLine) -> Vec<String> {
    let mut out = vec![
        "No mark: stated.  ~ calculated from a power or from the loads below (no diversity).  ? assumed.  * proposed by the Autopilot, not applied.  Heavy outline: a check fails.".to_owned(),
        format!(
            "Iz (cable rating) and ΔU (voltage drop, Σ from the supply, + = a wire without data) are calculated; limit {} % {}.",
            fmt(sl.drop_limit_percent),
            if sl.drop_limit_stated { "stated" } else { "default" }
        ),
    ];
    if !sl.missing.is_empty() {
        out.push(format!(
            "Missing inputs ({}): {}{}",
            sl.missing.len(),
            sl.missing[..sl.missing.len().min(2)].join("; "),
            if sl.missing.len() > 2 { "; ..." } else { "" }
        ));
    }
    out.push(
        "Ik ≤ is the stated supply short-circuit current carried down unchanged: the cable impedance is not included, so the real value is lower."
            .to_owned(),
    );
    for note in &sl.notes {
        if note.starts_with("Iz assumes")
            || note.starts_with("ΔU is calculated at power factor 1")
            || note.starts_with("Lamps and sounders are drawn")
            || note.starts_with("Not drawn, lamps and sounders")
            || note.starts_with("A load on one line conductor")
        {
            out.push(note.clone());
        }
    }
    if sl.planning_values {
        out.push(
            "Ampacity tables are planning values, not checked against the standard: verify in your licensed copy."
                .to_owned(),
        );
    }
    out
}

/// The drawing of sheet `index` (0-based) of the project's single-line
/// diagram, inside the usable area of `setup`. `None` when the diagram has no
/// such sheet.
pub fn sheet_layout(project: &Project, index: usize, setup: &PageSetup) -> Option<PageLayout> {
    let sl = build(project);
    sheet_layout_of(&sl, index, setup)
}

/// As [`sheet_layout`] for a diagram already built.
pub fn sheet_layout_of(sl: &SingleLine, index: usize, setup: &PageSetup) -> Option<PageLayout> {
    if sl.is_empty() {
        return None;
    }
    let sheets = plan(sl);
    let sheet = sheets.get(index)?;
    let (width, height) = setup.format.size_mm();
    let (x0, y0, area_w, area_h) = setup.table_area();
    let mut layout = PageLayout {
        width,
        height,
        ..PageLayout::default()
    };
    let mut heading = "Single-line diagram".to_owned();
    if sheets.len() > 1 {
        heading.push_str(&format!(" · sheet {} of {}", index + 1, sheets.len()));
    }
    layout.texts.push(TextBox {
        x: x0,
        y: setup.margins.top,
        width: area_w,
        height: setup.heading_mm,
        text: heading,
        text_mm: setup.text_mm * 1.4,
        align: Align::Left,
        bold: true,
    });

    // Natural positions of every item, columns laid side by side.
    let mut placed: BTreeMap<usize, Placed> = BTreeMap::new();
    let mut chains: Vec<(Vec<usize>, usize, f64)> = Vec::new();
    let mut next = 0.0;
    let mut deepest = 0usize;
    for item in &sheet.items {
        let before = next;
        let base = item.chain.len() + 1;
        let cx = place(sl, item.root, base, &mut next, &mut placed);
        chains.push((item.chain.clone(), item.root, cx));
        let _ = before;
    }
    for p in placed.values() {
        deepest = deepest.max(p.level);
    }
    let columns = next.max(1.0);
    let natural_w = columns * PITCH_MM;
    // About 0.5 of the character height per character; wrap to the area.
    let legend_chars = (area_w / (SMALL_MM * 0.5)) as usize;
    let legend_lines: Vec<String> = legend(sl)
        .iter()
        .flat_map(|l| wrap(l, legend_chars))
        .collect();
    let legend_h = legend_lines.len() as f64 * LEGEND_PITCH + 2.0;
    let natural_h = level_y(deepest) + BOX_H;
    let avail_h = (area_h - legend_h).max(10.0);
    let scale = (area_w / natural_w)
        .min(avail_h / natural_h)
        .clamp(MIN_SCALE, MAX_SCALE);
    let ox = x0 + ((area_w - natural_w * scale) / 2.0).max(0.0);
    let mut c = Canvas {
        layout,
        scale,
        ox,
        oy: y0,
        right: (x0 + area_w - ox) / scale,
    };

    // Supply over the middle of the sheet's branches.
    let first = chains.first().map_or(0.5, |ch| ch.2);
    let last = chains.last().map_or(0.5, |ch| ch.2);
    let supply_x = (first + last) / 2.0 * PITCH_MM;
    draw_box(&mut c, sl, &sl.nodes[0], supply_x, level_y(0), false);

    // Each item: its repeated chain, then the subtree.
    let mut roots: Vec<(f64, usize, Option<&Link>)> = Vec::new();
    for (chain, root, cx) in &chains {
        let cx = cx * PITCH_MM;
        for (i, &n) in chain.iter().enumerate() {
            let level = i + 1;
            draw_box(&mut c, sl, &sl.nodes[n], cx, level_y(level), true);
            if i > 0 {
                draw_run(&mut c, cx, level - 1, cx, level, sl.nodes[n].link.as_ref());
            }
        }
        let top_level = chain.len() + 1;
        let first_node = chain.first().copied().unwrap_or(*root);
        roots.push((cx, top_level, sl.nodes[first_node].link.as_ref()));
        if !chain.is_empty() {
            draw_run(
                &mut c,
                cx,
                chain.len(),
                cx,
                top_level,
                sl.nodes[*root].link.as_ref(),
            );
        }
    }
    // The bus under the supply.
    let bus_y = level_y(0) + BOX_H + LINK_H / 2.0;
    if let (Some(a), Some(b)) = (roots.first(), roots.last()) {
        c.line((supply_x, level_y(0) + BOX_H), (supply_x, bus_y), false);
        c.line((a.0, bus_y), (b.0, bus_y), false);
    }
    for (cx, _level, link) in &roots {
        c.line(
            (*cx, bus_y),
            (*cx, level_y(1)),
            link.is_some_and(|l| l.status == Status::Fail),
        );
        if let Some(link) = link {
            let lines = link_lines(link);
            for (i, text) in lines.iter().enumerate() {
                c.text(
                    *cx + 1.2,
                    bus_y + 0.5 + 2.9 * i as f64,
                    PITCH_MM - 3.0,
                    2.9,
                    text,
                    SMALL_MM,
                    Align::Left,
                    false,
                );
            }
        }
    }

    // The subtrees.
    for (chain, root, _) in &chains {
        let mut stack = vec![*root];
        while let Some(n) = stack.pop() {
            let Some(p) = placed.get(&n) else { continue };
            let cx = p.cx * PITCH_MM;
            let node = &sl.nodes[n];
            draw_box(&mut c, sl, node, cx, level_y(p.level), false);
            if n != *root
                && let Some(parent) = node.parent.and_then(|pp| placed.get(&pp))
            {
                draw_run(
                    &mut c,
                    parent.cx * PITCH_MM,
                    parent.level,
                    cx,
                    p.level,
                    node.link.as_ref(),
                );
            }
            stack.extend(sl.children(n));
        }
        let _ = chain;
    }

    // Legend along the bottom of the area.
    let bottom = y0 + area_h - legend_h;
    for (i, text) in legend_lines.iter().enumerate() {
        c.layout.texts.push(TextBox {
            x: x0,
            y: bottom + 1.0 + LEGEND_PITCH * i as f64,
            width: area_w,
            height: LEGEND_PITCH,
            text: text.clone(),
            text_mm: SMALL_MM,
            align: Align::Left,
            bold: false,
        });
    }
    Some(c.layout)
}

/// The number of drawn sheets of the diagram.
pub fn sheet_count(sl: &SingleLine) -> usize {
    if sl.is_empty() { 0 } else { plan(sl).len() }
}

/// Every sheet of the diagram drawn inside `setup`.
pub fn layouts(sl: &SingleLine, setup: &PageSetup) -> Vec<PageLayout> {
    (0..sheet_count(sl))
        .filter_map(|i| sheet_layout_of(sl, i, setup))
        .collect()
}
