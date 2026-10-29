//! The hand drilling plan: what a workshop that drills and cuts by hand
//! needs at the bench for each plate (or door, wall, extra plate): the datum
//! every measurement starts from, the tools to lay out, and the holes and
//! cut-outs grouped by tool in a check-off order, each with its distance
//! from the datum and from the nearest edge.
//!
//! The holes are the numbers of the drill table ([`crate::drill`]) and the
//! plate drawing, so a hole has one number on every sheet. On top of the
//! table the plan adds the fixing holes of the wiring ducts (a planning
//! rule, listed apart) and says for every row where its position comes from
//! ([`Basis`]): maker data, typed in by the user, a planning rule, or the
//! layout model. Nothing here is a measurement of the real plate.
//!
//! [`DrillPlan::pages`] lays the plan out on A4 portrait pages as plain
//! marks (the same ones as the plate drawing), so the PDF writer only draws.

use std::collections::HashMap;

use crate::drill::{CutoutShape, DrillTable, RAIL_HOLE_DIA_MM, Surface, mm};
use crate::plate_drawing::{HAlign, Layer, Marks, VAlign, sheet_size, sheet_surfaces};
use crate::state::CabinetLayout;
use crate::workpack::{duct_fixings, half_mm};

/// Part key: where a part's `hole_pattern` and `cutout` came from. `maker`
/// for data read from a maker's file, `typed` for values the user entered;
/// anything else (or no key) is [`Basis::PartData`].
pub const BASIS_KEY: &str = "drill_basis";

/// Diameter assumed for a duct fixing hole (M4 clearance, medium series).
/// A planning rule: the duct maker's instructions win.
pub const DUCT_HOLE_DIA_MM: f64 = 4.5;
/// Holes wider than this get a smaller pilot hole first (planning rule).
pub const PILOT_ABOVE_MM: f64 = 8.0;
/// Size of that pilot hole.
pub const PILOT_DIA_MM: f64 = 4.0;
/// A rectangular cut-out starts with a hole this wide in a corner (planning
/// rule), for the jigsaw or nibbler to enter.
pub const CORNER_HOLE_MM: f64 = 10.0;

/// Where a row's position or size comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Basis {
    /// The part's hole pattern or cut-out was read from the maker's data.
    Maker,
    /// The user typed the hole pattern or cut-out into the part.
    Typed,
    /// The part has the data, but it does not say who put it there.
    PartData,
    /// A planning rule of KoperCAD (rail screws, duct fixings), not data.
    Rule,
    /// An opening the user entered in the project.
    Entered,
}

impl Basis {
    /// Short text for the table.
    pub fn label(self) -> &'static str {
        match self {
            Basis::Maker => "maker data",
            Basis::Typed => "typed in",
            Basis::PartData => "part data",
            Basis::Rule => "planning rule",
            Basis::Entered => "project opening",
        }
    }

    /// Reads a part's [`BASIS_KEY`] text.
    pub fn from_text(text: Option<&str>) -> Basis {
        match text.map(|t| t.trim().to_lowercase()).as_deref() {
            Some("maker") => Basis::Maker,
            Some("typed") | Some("user") => Basis::Typed,
            _ => Basis::PartData,
        }
    }
}

/// One line of the tool list.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolRow {
    pub tool: String,
    pub for_what: String,
    pub count: usize,
}

/// One hole or cut-out to machine.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// `7` for a hole of the drill table, `C2` for a cut-out, `D3` for a
    /// duct fixing.
    pub label: String,
    /// Distance from the datum, mm, x to the right and y down.
    pub x: f64,
    pub y: f64,
    /// The same point from the nearest edges: `12.5 from left, 40 from top`.
    pub edges: String,
    pub source: String,
    pub basis: Basis,
}

/// Steps that share a tool, drilled one after the other.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub heading: String,
    pub steps: Vec<Step>,
}

/// The plan of one surface.
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub surface: Surface,
    pub size: (f64, f64),
    pub datum: String,
    pub tools: Vec<ToolRow>,
    pub groups: Vec<Group>,
    /// Plate features for the sketch: holes (x, y, dia), cut-outs.
    pub sketch_holes: Vec<(f64, f64, f64)>,
    pub sketch_cutouts: Vec<(CutoutShape, f64, f64)>,
    pub sketch_rails: Vec<((f64, f64), (f64, f64))>,
}

impl Sheet {
    /// Number of holes and cut-outs on the sheet.
    pub fn steps(&self) -> usize {
        self.groups.iter().map(|g| g.steps.len()).sum()
    }
}

/// The whole plan: one sheet per surface that has work.
#[derive(Debug, Clone, PartialEq)]
pub struct DrillPlan {
    pub project: String,
    pub title: String,
    pub sheets: Vec<Sheet>,
    /// Part data that could not be read, as the drill table reports it.
    pub problems: Vec<String>,
}

fn edges_text(x: f64, y: f64, (w, h): (f64, f64)) -> String {
    let (hn, hv) = if x <= w - x {
        ("left", x)
    } else {
        ("right", w - x)
    };
    let (vn, vv) = if y <= h - y {
        ("top", y)
    } else {
        ("bottom", h - y)
    };
    format!("{} from {hn}, {} from {vn}", mm(hv), mm(vv))
}

fn datum_text(surface: &Surface, (w, h): (f64, f64)) -> String {
    let side = match surface {
        Surface::Plate => {
            "mounting plate, seen from the front (the side the devices are mounted on)"
        }
        Surface::Door => "door, seen from the outside",
        Surface::Named(_) => "surface, seen from the outside",
    };
    format!(
        "Datum 0 / 0 is the top left corner of the {side}. X is measured to the right, Y downward, \
         both in mm from the datum. The sheet is {} x {} mm.",
        mm(w),
        mm(h)
    )
}

fn drill_tool(dia: f64) -> String {
    format!("Drill dia {} mm", mm(dia))
}

fn cutout_tool(shape: CutoutShape) -> String {
    match shape {
        CutoutShape::Round { dia } => format!("Hole saw or step drill dia {}", mm(half_mm(dia))),
        CutoutShape::Rect { w, h } => {
            format!("Punch or jigsaw {} x {}", mm(half_mm(w)), mm(half_mm(h)))
        }
    }
}

fn same(a: &CutoutShape, b: &CutoutShape) -> bool {
    match (a, b) {
        (CutoutShape::Rect { w, h }, CutoutShape::Rect { w: w2, h: h2 }) => {
            (w - w2).abs() < 0.01 && (h - h2).abs() < 0.01
        }
        (CutoutShape::Round { dia }, CutoutShape::Round { dia: d2 }) => (dia - d2).abs() < 0.01,
        _ => false,
    }
}

/// Builds the plan of a layout from its drill table.
///
/// `basis` gives the origin of a device's data by the device tag that the
/// table shows as the row's source; rails and openings are known from the
/// layout, and any other source is [`Basis::PartData`]. Duct fixing holes
/// are added when `ducts` is set.
pub fn plan(
    project: &str,
    title: &str,
    layout: &CabinetLayout,
    table: &DrillTable,
    basis: &HashMap<String, Basis>,
    ducts: bool,
) -> DrillPlan {
    let rails: Vec<String> = (1..=layout.rails.len())
        .map(|i| format!("Rail {i}"))
        .collect();
    let openings: Vec<String> = layout
        .openings
        .iter()
        .map(|o| format!("Opening {}", o.name))
        .collect();
    let basis_of = |source: &str| -> Basis {
        if rails.iter().any(|r| r == source) {
            Basis::Rule
        } else if openings.iter().any(|o| o == source) {
            Basis::Entered
        } else {
            basis.get(source).copied().unwrap_or(Basis::PartData)
        }
    };

    let mut sheets = Vec::new();
    for surface in sheet_surfaces(table) {
        let size = sheet_size(layout, &surface);
        let (holes, cutouts) = table.on(&surface);
        let mut groups: Vec<Group> = Vec::new();
        let mut tools: Vec<ToolRow> = Vec::new();
        let mut sketch_holes = Vec::new();
        let mut sketch_cutouts = Vec::new();

        // Holes arrive grouped by diameter, smallest first.
        for h in &holes {
            sketch_holes.push((h.x, h.y, h.dia));
            let step = Step {
                label: h.no.to_string(),
                x: h.x,
                y: h.y,
                edges: edges_text(h.x, h.y, size),
                source: h.source.clone(),
                basis: basis_of(&h.source),
            };
            match groups.last_mut() {
                Some(g) if g.heading == drill_tool(h.dia) => g.steps.push(step),
                _ => groups.push(Group {
                    heading: drill_tool(h.dia),
                    steps: vec![step],
                }),
            }
        }
        for g in &groups {
            let dia: f64 = g
                .heading
                .trim_start_matches("Drill dia ")
                .trim_end_matches(" mm")
                .parse()
                .unwrap_or(0.0);
            let mut for_what = if g.steps.iter().all(|s| s.source.starts_with("Rail ")) {
                "DIN rail screws".to_owned()
            } else {
                "device screws".to_owned()
            };
            if dia > PILOT_ABOVE_MM {
                for_what.push_str(&format!(
                    "; pilot with dia {} mm first (planning rule)",
                    mm(PILOT_DIA_MM)
                ));
            }
            tools.push(ToolRow {
                tool: g.heading.clone(),
                for_what,
                count: g.steps.len(),
            });
        }

        // Duct fixings: the plate only, apart from the drill table.
        if ducts && surface == Surface::Plate && !layout.ducts.is_empty() {
            let mut ds: Vec<_> = layout.ducts.iter().collect();
            ds.sort_by(|a, b| {
                a.y.total_cmp(&b.y)
                    .then(a.x.total_cmp(&b.x))
                    .then(a.id.cmp(&b.id))
            });
            let mut steps = Vec::new();
            for (i, d) in ds.iter().enumerate() {
                for at in duct_fixings(d.length) {
                    let (x, y) = if d.vertical {
                        (d.x, d.y + at)
                    } else {
                        (d.x + at, d.y)
                    };
                    let (x, y) = ((x * 100.0).round() / 100.0, (y * 100.0).round() / 100.0);
                    sketch_holes.push((x, y, DUCT_HOLE_DIA_MM));
                    steps.push(Step {
                        label: format!("D{}", steps.len() + 1),
                        x,
                        y,
                        edges: edges_text(x, y, size),
                        source: format!("Duct {}", i + 1),
                        basis: Basis::Rule,
                    });
                }
            }
            if !steps.is_empty() {
                let heading = format!(
                    "Duct fixing, dia {} mm (planning rule)",
                    mm(DUCT_HOLE_DIA_MM)
                );
                tools.push(ToolRow {
                    tool: drill_tool(DUCT_HOLE_DIA_MM),
                    for_what: "duct fixing, on the duct centre line; the duct maker's fixing wins"
                        .to_owned(),
                    count: steps.len(),
                });
                groups.push(Group { heading, steps });
            }
        }

        // Cut-outs last, one group per size.
        let mut cut_groups: Vec<(CutoutShape, Group)> = Vec::new();
        for c in &cutouts {
            sketch_cutouts.push((c.shape, c.x, c.y));
            let step = Step {
                label: format!("C{}", c.no),
                x: c.x,
                y: c.y,
                edges: edges_text(c.x, c.y, size),
                source: c.source.clone(),
                basis: basis_of(&c.source),
            };
            match cut_groups.iter_mut().find(|(s, _)| same(s, &c.shape)) {
                Some((_, g)) => g.steps.push(step),
                None => cut_groups.push((
                    c.shape,
                    Group {
                        heading: format!("Cut-out {}", c.shape.describe()),
                        steps: vec![step],
                    },
                )),
            }
        }
        for (shape, g) in cut_groups {
            let mut for_what = "cut-out".to_owned();
            if matches!(shape, CutoutShape::Rect { .. }) {
                for_what.push_str(&format!(
                    "; start with a dia {} mm hole in a corner (planning rule)",
                    mm(CORNER_HOLE_MM)
                ));
            }
            tools.push(ToolRow {
                tool: cutout_tool(shape),
                for_what,
                count: g.steps.len(),
            });
            groups.push(g);
        }

        if groups.is_empty() {
            continue;
        }
        let sketch_rails = if surface == Surface::Plate {
            layout
                .rails
                .iter()
                .map(|r| {
                    if r.vertical {
                        ((r.x, r.y), (r.x, r.y + r.length))
                    } else {
                        ((r.x, r.y), (r.x + r.length, r.y))
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        sheets.push(Sheet {
            surface: surface.clone(),
            size,
            datum: datum_text(&surface, size),
            tools,
            groups,
            sketch_holes,
            sketch_cutouts,
            sketch_rails,
        });
    }
    DrillPlan {
        project: project.to_owned(),
        title: title.to_owned(),
        sheets,
        problems: table.problems.clone(),
    }
}

// ---- Pages -------------------------------------------------------------

/// A4 portrait, mm.
pub const PAGE_MM: (f64, f64) = (210.0, 297.0);
const MARGIN: f64 = 12.0;
const BODY: f64 = 2.7;
const ROW: f64 = 5.6;
/// Column x offsets from the left margin and widths of the step table:
/// tick, no, X, Y, from edges, source, basis.
const COLS: [(f64, f64); 7] = [
    (0.0, 9.0),
    (9.0, 13.0),
    (22.0, 19.0),
    (41.0, 19.0),
    (60.0, 54.0),
    (114.0, 46.0),
    (160.0, 26.0),
];
const CONTENT_W: f64 = PAGE_MM.0 - 2.0 * MARGIN;

/// One printed page.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanPage {
    pub marks: Marks,
}

/// Cuts text to what fits `width` mm at `height` mm text.
fn fit(text: &str, width: f64, height: f64) -> String {
    let max = ((width - 2.0) / (height * 0.55)).floor().max(1.0) as usize;
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut s: String = text.chars().take(max.saturating_sub(2)).collect();
        s.push_str("..");
        s
    }
}

/// Word wrap to `width` mm.
fn wrap(text: &str, width: f64, height: f64) -> Vec<String> {
    let max = (width / (height * 0.55)).floor().max(8.0) as usize;
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
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

struct Flow<'a> {
    plan: &'a DrillPlan,
    pages: Vec<Marks>,
    y: f64,
    heading: String,
}

impl<'a> Flow<'a> {
    fn marks(&mut self) -> &mut Marks {
        self.pages.last_mut().expect("a page is open")
    }

    fn text(&mut self, x: f64, y: f64, h: f64, text: impl Into<String>, bold: bool) {
        self.marks().text(
            Layer::Text,
            (x, y),
            h,
            0.0,
            (HAlign::Left, VAlign::Middle),
            text,
            bold,
        );
    }

    fn new_page(&mut self) {
        self.pages.push(Marks::default());
        let heading = self.heading.clone();
        let project = self.plan.project.clone();
        self.text(MARGIN, MARGIN + 3.0, 4.2, heading, true);
        self.text(
            MARGIN,
            MARGIN + 9.0,
            2.4,
            format!("{project}  |  KoperCAD drill plan"),
            false,
        );
        self.marks().line(
            Layer::Table,
            (MARGIN, MARGIN + 12.0),
            (PAGE_MM.0 - MARGIN, MARGIN + 12.0),
        );
        self.y = MARGIN + 17.0;
    }

    fn room(&self) -> f64 {
        PAGE_MM.1 - MARGIN - 8.0 - self.y
    }

    /// Starts a new page when fewer than `h` mm are left.
    fn ensure(&mut self, h: f64) {
        if self.room() < h {
            self.new_page();
        }
    }

    fn wrapped(&mut self, text: &str, tick: bool) {
        let w = CONTENT_W - if tick { 7.0 } else { 0.0 };
        let ls = wrap(text, w, BODY);
        for (i, l) in ls.iter().enumerate() {
            self.ensure(ROW);
            let y = self.y + ROW / 2.0;
            if tick && i == 0 {
                self.marks().rect(
                    Layer::Table,
                    (MARGIN + 1.0, y - 1.7),
                    (MARGIN + 4.4, y + 1.7),
                );
            }
            let x = MARGIN + if tick { 7.0 } else { 0.0 };
            self.text(x, y, BODY, l.clone(), false);
            self.y += ROW;
        }
    }

    fn title(&mut self, text: &str) {
        self.ensure(ROW * 3.0);
        self.y += 2.0;
        self.text(MARGIN, self.y + ROW / 2.0, 3.4, text, true);
        self.y += ROW + 1.0;
    }

    /// A ruled table row with `cells` in the given columns.
    fn row(&mut self, cols: &[(f64, f64)], cells: &[String], bold: bool, shade: bool) {
        self.ensure(ROW);
        let (top, bottom) = (self.y, self.y + ROW);
        let m = self.marks();
        if shade {
            m.rect(Layer::Table, (MARGIN, top), (MARGIN + CONTENT_W, bottom));
        }
        m.line(Layer::Table, (MARGIN, bottom), (MARGIN + CONTENT_W, bottom));
        let mut texts = Vec::new();
        for ((x, w), cell) in cols.iter().zip(cells) {
            texts.push((MARGIN + x + 1.0, fit(cell, *w, BODY)));
        }
        for (x, t) in texts {
            self.text(x, bottom - ROW / 2.0, BODY, t, bold);
        }
        self.y = bottom;
    }
}

fn sketch(f: &mut Flow<'_>, sheet: &Sheet, x0: f64, y0: f64, bw: f64, bh: f64) {
    let (w, h) = sheet.size;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let s = ((bw - 10.0) / w).min((bh - 10.0) / h);
    let (ox, oy) = (x0 + 8.0, y0 + 6.0);
    let p = |x: f64, y: f64| (ox + x * s, oy + y * s);
    let m = f.marks();
    m.rect(Layer::Plate, p(0.0, 0.0), p(w, h));
    for (a, b) in &sheet.sketch_rails {
        m.line(Layer::Rails, p(a.0, a.1), p(b.0, b.1));
    }
    for (shape, x, y) in &sheet.sketch_cutouts {
        match shape {
            CutoutShape::Rect { w, h } => m.rect(
                Layer::Cutouts,
                p(x - w / 2.0, y - h / 2.0),
                p(x + w / 2.0, y + h / 2.0),
            ),
            CutoutShape::Round { dia } => m.circle(Layer::Cutouts, p(*x, *y), dia / 2.0 * s),
        }
    }
    for (x, y, dia) in &sheet.sketch_holes {
        m.circle(Layer::Holes, p(*x, *y), (dia / 2.0 * s).max(0.4));
    }
    // Datum cross and axes in the corner.
    let o = p(0.0, 0.0);
    m.line(Layer::Dimensions, (o.0 - 5.0, o.1), (o.0 + 12.0, o.1));
    m.line(Layer::Dimensions, (o.0, o.1 - 4.0), (o.0, o.1 + 12.0));
    m.text(
        Layer::Text,
        (o.0 + 13.0, o.1 - 1.0),
        2.4,
        0.0,
        (HAlign::Left, VAlign::Middle),
        "X",
        true,
    );
    m.text(
        Layer::Text,
        (o.0 - 4.0, o.1 + 14.0),
        2.4,
        0.0,
        (HAlign::Left, VAlign::Middle),
        "Y",
        true,
    );
    m.text(
        Layer::Text,
        (o.0 + 1.5, o.1 + 3.0),
        2.2,
        0.0,
        (HAlign::Left, VAlign::Middle),
        "0 / 0",
        true,
    );
    m.text(
        Layer::Text,
        (ox + w * s / 2.0, oy + h * s + 3.0),
        2.2,
        0.0,
        (HAlign::Center, VAlign::Middle),
        format!("{} mm (sketch, not to scale of the page)", mm(w)),
        false,
    );
}

impl DrillPlan {
    /// The plan on A4 portrait pages: for each sheet the datum with a
    /// sketch, the procedure, the tool list and the check-off table, then
    /// the notes on where the numbers come from. Deterministic.
    pub fn pages(&self) -> Vec<PlanPage> {
        let mut f = Flow {
            plan: self,
            pages: Vec::new(),
            y: 0.0,
            heading: String::new(),
        };
        for sheet in &self.sheets {
            f.heading = format!("Drill plan - {} - {}", self.title, sheet.surface.name());
            f.new_page();

            // Datum text and the sketch side by side.
            let top = f.y;
            let (bw, bh) = (78.0, 62.0);
            let text_w = CONTENT_W - bw - 4.0;
            let ls = wrap(&sheet.datum, text_w, BODY);
            for (i, l) in ls.iter().enumerate() {
                f.text(MARGIN, top + 3.0 + i as f64 * 4.4, BODY, l.clone(), false);
            }
            let counts = format!(
                "{} holes and cut-outs in {} tool groups.",
                sheet.steps(),
                sheet.groups.len()
            );
            f.text(
                MARGIN,
                top + 3.0 + ls.len() as f64 * 4.4 + 2.0,
                BODY,
                counts,
                true,
            );
            sketch(&mut f, sheet, MARGIN + text_w + 4.0, top, bw, bh);
            f.y = top + bh.max(ls.len() as f64 * 4.4 + 10.0);

            f.title("Before you start");
            let w = sheet.size;
            f.wrapped(
                &format!(
                    "Measure the real sheet: it must be {} x {} mm. Stop if it differs.",
                    mm(w.0),
                    mm(w.1)
                ),
                true,
            );
            f.wrapped(
                "Scribe the datum: a line along the top edge and one along the left edge. Measure every X and Y from them.",
                true,
            );
            f.wrapped(
                "Check rails, ducts and devices against the layout before you mark. The positions come from the layout model, not from a measurement.",
                true,
            );
            f.wrapped(
                "Mark and centre punch one group, drill it, then tick its boxes. Work through the groups in the order below.",
                true,
            );

            f.title("Tools");
            f.row(
                &[(0.0, 9.0), (9.0, 70.0), (79.0, 14.0), (93.0, 93.0)],
                &["".into(), "Tool".into(), "Count".into(), "For".into()],
                true,
                false,
            );
            for t in &sheet.tools {
                f.row(
                    &[(0.0, 9.0), (9.0, 70.0), (79.0, 14.0), (93.0, 93.0)],
                    &[
                        "".into(),
                        t.tool.clone(),
                        t.count.to_string(),
                        t.for_what.clone(),
                    ],
                    false,
                    false,
                );
                let y = f.y - ROW / 2.0;
                f.marks().rect(
                    Layer::Table,
                    (MARGIN + 2.0, y - 1.7),
                    (MARGIN + 5.4, y + 1.7),
                );
            }

            f.title("Check-off");
            let head =
                ["Done", "No", "X", "Y", "From edges (mm)", "Source", "Basis"].map(String::from);
            for g in &sheet.groups {
                f.ensure(ROW * 4.0);
                f.row(
                    &[(0.0, CONTENT_W)],
                    &[format!("{} - {} steps", g.heading, g.steps.len())],
                    true,
                    false,
                );
                f.row(&COLS, &head, true, false);
                for s in &g.steps {
                    f.row(
                        &COLS,
                        &[
                            "".into(),
                            s.label.clone(),
                            mm(s.x),
                            mm(s.y),
                            s.edges.clone(),
                            s.source.clone(),
                            s.basis.label().to_owned(),
                        ],
                        false,
                        false,
                    );
                    let y = f.y - ROW / 2.0;
                    f.marks().rect(
                        Layer::Table,
                        (MARGIN + 2.5, y - 1.7),
                        (MARGIN + 5.9, y + 1.7),
                    );
                }
            }

            f.title("Where the numbers come from");
            for note in notes(sheet, &self.problems) {
                f.wrapped(&note, false);
            }
        }
        let of = f.pages.len();
        let project = self.project.clone();
        for (i, marks) in f.pages.iter_mut().enumerate() {
            marks.text(
                Layer::Text,
                (PAGE_MM.0 - MARGIN, PAGE_MM.1 - MARGIN),
                2.4,
                0.0,
                (HAlign::Right, VAlign::Bottom),
                format!("{project}  |  page {} of {of}", i + 1),
                false,
            );
        }
        f.pages
            .into_iter()
            .map(|marks| PlanPage { marks })
            .collect()
    }
}

fn notes(sheet: &Sheet, problems: &[String]) -> Vec<String> {
    let mut out = vec![
        "Hole and cut-out offsets inside a device come from the part, as the Basis column says. \"maker data\" was read from a maker's file, \"typed in\" was entered by hand, \"part data\" is on the part without a recorded origin."
            .to_owned(),
        "Where the device sits on the plate is the layout of this project (a model), not a measurement of the real plate."
            .to_owned(),
        "\"planning rule\" rows (rail screws, duct fixings, pilot and corner holes) are KoperCAD defaults, not datasheet values; the maker's instructions win."
            .to_owned(),
    ];
    if sheet
        .groups
        .iter()
        .any(|g| g.heading.starts_with("Cut-out"))
    {
        out.push(
            "Cut-out sizes are rounded up to the next half millimetre for the tool, so the opening is never smaller than the part."
                .to_owned(),
        );
    }
    if sheet.tools.iter().any(|t| {
        t.tool.starts_with("Drill dia") && {
            let d: f64 = t
                .tool
                .trim_start_matches("Drill dia ")
                .trim_end_matches(" mm")
                .parse()
                .unwrap_or(0.0);
            (d - RAIL_HOLE_DIA_MM).abs() < 1e-9
        }
    }) {
        out.push(format!(
            "DIN rail screw holes are dia {} mm (M5 clearance), 10 mm in from each end and at most 200 mm apart.",
            mm(RAIL_HOLE_DIA_MM)
        ));
    }
    for p in problems {
        out.push(format!("Not read, not in this plan: {p}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drill::{DeviceFeatures, Hole, drill_table};
    use crate::{Duct, Margins, Placement, Plate, Rail, Size};

    fn layout() -> CabinetLayout {
        let mut l = CabinetLayout::new(Plate {
            width: 600.0,
            height: 800.0,
            margin: 10.0,
        });
        l.rails.push(Rail {
            id: 1,
            x: 10.0,
            y: 100.0,
            length: 580.0,
            vertical: false,
        });
        l.ducts.push(Duct {
            id: 3,
            x: 10.0,
            y: 300.0,
            length: 300.0,
            width: 30.0,
            vertical: false,
        });
        l.placements.push(Placement {
            id: 7,
            x: 100.0,
            y: 120.0,
            rotated: false,
            size: Size::from_part(Some(40.0), Some(80.0)),
            clearance: None,
            margins: Margins::default(),
            rail: Some(1),
        });
        l
    }

    fn feature() -> DeviceFeatures {
        DeviceFeatures {
            id: 7,
            label: "-Q1".into(),
            surface: Surface::Plate,
            holes: vec![
                Hole {
                    dx: 5.0,
                    dy: 5.0,
                    dia: 4.2,
                },
                Hole {
                    dx: 35.0,
                    dy: 75.0,
                    dia: 4.2,
                },
            ],
            cutout: None,
            problems: vec![],
        }
    }

    fn built(ducts: bool) -> DrillPlan {
        let l = layout();
        let f = vec![feature()];
        let t = drill_table(&l, &f);
        let mut b = HashMap::new();
        b.insert("-Q1".to_owned(), Basis::Maker);
        plan("Test", "=A1+CAB1", &l, &t, &b, ducts)
    }

    #[test]
    fn groups_follow_the_drill_table_numbers_by_tool() {
        let p = built(false);
        assert_eq!(p.sheets.len(), 1);
        let s = &p.sheets[0];
        let heads: Vec<_> = s.groups.iter().map(|g| g.heading.as_str()).collect();
        assert_eq!(heads, ["Drill dia 4.2 mm", "Drill dia 5.5 mm"]);
        assert_eq!(s.groups[0].steps[0].label, "1");
        assert_eq!(s.groups[0].steps[0].x, 105.0);
        assert_eq!(s.groups[0].steps[0].y, 125.0);
        assert_eq!(s.steps(), 2 + 4);
    }

    #[test]
    fn every_row_says_where_it_comes_from() {
        let p = built(true);
        let s = &p.sheets[0];
        assert_eq!(s.groups[0].steps[0].basis, Basis::Maker);
        let rail = s.groups.iter().find(|g| g.heading.contains("5.5")).unwrap();
        assert!(rail.steps.iter().all(|x| x.basis == Basis::Rule));
        let duct = s.groups.last().unwrap();
        assert!(duct.heading.starts_with("Duct fixing"));
        assert!(duct.steps.iter().all(|x| x.basis == Basis::Rule));
        // Fixings sit on the duct centre line, starting 25 mm in.
        assert_eq!((duct.steps[0].x, duct.steps[0].y), (35.0, 300.0));
    }

    #[test]
    fn edges_name_the_nearest_edge() {
        assert_eq!(
            edges_text(580.0, 50.0, (600.0, 800.0)),
            "20 from right, 50 from top"
        );
        assert_eq!(
            edges_text(5.0, 790.0, (600.0, 800.0)),
            "5 from left, 10 from bottom"
        );
    }

    #[test]
    fn big_holes_get_a_pilot_note_and_rect_cutouts_a_corner_hole() {
        let mut l = layout();
        l.openings.clear();
        let mut f = feature();
        f.holes.push(Hole {
            dx: 20.0,
            dy: 40.0,
            dia: 12.0,
        });
        f.cutout = Some(crate::drill::Cutout {
            shape: CutoutShape::Rect { w: 90.2, h: 60.0 },
            centre: None,
        });
        let t = drill_table(&l, &[f]);
        let p = plan("T", "X", &l, &t, &HashMap::new(), false);
        let tools = &p.sheets[0].tools;
        assert!(
            tools
                .iter()
                .any(|t| t.tool == "Drill dia 12 mm" && t.for_what.contains("pilot"))
        );
        let c = tools.iter().find(|t| t.tool.starts_with("Punch")).unwrap();
        assert_eq!(c.tool, "Punch or jigsaw 90.5 x 60");
        assert!(c.for_what.contains("corner"));
        // Unknown origin stays "part data", never promoted to maker data.
        assert_eq!(p.sheets[0].groups[0].steps[0].basis, Basis::PartData);
    }

    #[test]
    fn basis_text_is_read() {
        assert_eq!(Basis::from_text(Some(" Maker ")), Basis::Maker);
        assert_eq!(Basis::from_text(Some("typed")), Basis::Typed);
        assert_eq!(Basis::from_text(Some("who knows")), Basis::PartData);
        assert_eq!(Basis::from_text(None), Basis::PartData);
    }

    #[test]
    fn pages_are_stable_and_carry_the_numbers() {
        let p = built(true);
        let a = p.pages();
        assert_eq!(a, p.pages());
        assert!(!a.is_empty());
        let all: String = a
            .iter()
            .flat_map(|pg| pg.marks.texts.iter().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all.contains("Datum 0 / 0"));
        assert!(all.contains("Drill dia 4.2 mm"));
        assert!(all.contains("maker data"));
        assert!(all.contains("planning rule"));
        assert!(all.contains("page 1 of"));
        // Nothing runs off the page.
        for pg in &a {
            for t in &pg.marks.texts {
                assert!(t.pos.0 >= 0.0 && t.pos.0 <= PAGE_MM.0, "{t:?}");
                assert!(t.pos.1 >= 0.0 && t.pos.1 <= PAGE_MM.1, "{t:?}");
            }
        }
    }

    #[test]
    fn a_long_plan_flows_onto_more_pages() {
        let mut l = layout();
        for i in 0..80 {
            l.placements.push(Placement {
                id: 100 + i,
                x: 20.0 + (i % 10) as f64 * 50.0,
                y: 400.0 + (i / 10) as f64 * 40.0,
                rotated: false,
                size: Size::from_part(Some(40.0), Some(30.0)),
                clearance: None,
                margins: Margins::default(),
                rail: None,
            });
        }
        let features: Vec<_> = l
            .placements
            .iter()
            .map(|p| DeviceFeatures {
                id: p.id,
                label: format!("-K{}", p.id),
                surface: Surface::Plate,
                holes: vec![Hole {
                    dx: 5.0,
                    dy: 5.0,
                    dia: 4.2,
                }],
                cutout: None,
                problems: vec![],
            })
            .collect();
        let t = drill_table(&l, &features);
        let p = plan("T", "X", &l, &t, &HashMap::new(), true);
        assert!(p.pages().len() >= 3);
    }

    #[test]
    fn fit_and_wrap_keep_text_in_bounds() {
        assert_eq!(fit("short", 40.0, 2.7), "short");
        assert!(fit(&"x".repeat(100), 20.0, 2.7).ends_with(".."));
        for l in wrap(&"word ".repeat(60), 100.0, 2.7) {
            assert!(l.chars().count() as f64 * 2.7 * 0.55 <= 100.0 + 2.7);
        }
    }
}
