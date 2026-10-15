//! Cutting list: the wires of the wire list, grouped by cross-section and
//! colour, with the length to cut.

use std::cmp::Ordering;

use novo_core::Project;
use novo_core::natural_cmp;

use crate::WireList;
use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Row, Section};

/// The cutting list: one row per wire of the wire list, in one table per
/// cross-section and colour, so the workshop cuts one kind of wire at a
/// time. A summary table opens the report: wires and total length per
/// group, to order the wire by the metre.
///
/// The cut length is the wire list's length (the route plus a service loop
/// at each end, see [`WireList`]) plus an allowance at each end for the
/// termination: a ferrule below [`CutList::lug_from_mm2`], a cable lug from
/// there on. Within a table the longest wires come first, then by wire
/// number. A wire whose cross-section is unknown is listed last, with the
/// ferrule allowance.
///
/// Rows keep the model objects of the wire-list row they come from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutList {
    /// Stripped length and ferrule collar per end, in millimetres.
    pub ferrule_mm: f64,
    /// Stripped length and lug barrel per end, in millimetres.
    pub lug_mm: f64,
    /// Smallest cross-section (mm²) that gets a cable lug, not a ferrule.
    pub lug_from_mm2: f64,
}

impl Default for CutList {
    fn default() -> Self {
        CutList {
            ferrule_mm: 10.0,
            lug_mm: 20.0,
            lug_from_mm2: 25.0,
        }
    }
}

impl CutList {
    pub const ID: &'static str = "cutlist";
}

/// The largest cross-section named in a wire-list cell: a number, or a text
/// of several (`0.75, 1.5`) when a daisy chain mixes wires.
fn cross_section(cell: &Cell) -> Option<f64> {
    match cell {
        Cell::Number(v) => Some(*v),
        Cell::Int(v) => Some(*v as f64),
        Cell::Text(t) => t
            .split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .filter_map(|s| s.parse::<f64>().ok())
            .max_by(f64::total_cmp),
        Cell::Empty => None,
    }
}

fn text_of(cell: &Cell) -> String {
    cell.to_string()
}

fn number_of(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(v) => *v,
        Cell::Int(v) => *v as f64,
        _ => 0.0,
    }
}

struct Wire {
    number: String,
    from: String,
    to: String,
    cable: String,
    basis: String,
    colour: String,
    section: Option<f64>,
    length: f64,
    row: Row,
}

fn group_order(a: &Wire, b: &Wire) -> Ordering {
    match (a.section, b.section) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| natural_cmp(&a.colour, &b.colour))
}

fn group_title(w: &Wire) -> String {
    let mut parts = Vec::new();
    parts.push(match w.section {
        Some(s) => format!("{s} mm²"),
        None => "Size unknown".into(),
    });
    if !w.colour.is_empty() {
        parts.push(w.colour.clone());
    }
    parts.join(" ")
}

impl Report for CutList {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Cut list".into()
    }

    fn dependencies(&self) -> Dependencies {
        WireList.dependencies()
    }

    fn proposed_by_default(&self) -> bool {
        false
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let list = WireList.generate(project);
        let mut wires: Vec<Wire> = Vec::new();
        if let Some(section) = list.sections.first() {
            let col = |key: &str| section.column(key);
            let at = |row: &Row, key: &str| {
                col(key)
                    .and_then(|i| row.cells.get(i))
                    .cloned()
                    .unwrap_or(Cell::Empty)
            };
            for row in &section.rows {
                wires.push(Wire {
                    number: text_of(&at(row, "number")),
                    from: text_of(&at(row, "from")),
                    to: text_of(&at(row, "to")),
                    cable: text_of(&at(row, "cable")),
                    basis: text_of(&at(row, "length_basis")),
                    colour: text_of(&at(row, "color")),
                    section: cross_section(&at(row, "cross_section_mm2")),
                    length: number_of(&at(row, "length_mm")),
                    row: row.clone(),
                });
            }
        }
        // Group, then longest first, then by wire number and pins.
        wires.sort_by(|a, b| {
            group_order(a, b)
                .then(b.length.total_cmp(&a.length))
                .then_with(|| natural_cmp(&a.number, &b.number))
                .then_with(|| natural_cmp(&a.from, &b.from))
                .then_with(|| natural_cmp(&a.to, &b.to))
        });

        let allowance = |w: &Wire| {
            let end = match w.section {
                Some(s) if s >= self.lug_from_mm2 => self.lug_mm,
                _ => self.ferrule_mm,
            };
            2.0 * end
        };

        let mut summary = Section::new(
            "summary",
            "Summary",
            vec![
                Column::new("group", "Wire", 30.0),
                Column::new("wires", "Wires", 10.0).right(),
                Column::new("total_m", "Total m", 12.0).right(),
            ],
        );
        let mut tables: Vec<Section> = Vec::new();
        let mut start = 0;
        while start < wires.len() {
            let end = wires[start..]
                .iter()
                .position(|w| group_order(&wires[start], w).is_ne())
                .map_or(wires.len(), |n| start + n);
            let group = &wires[start..end];
            let title = group_title(&group[0]);
            let mut table = Section::new(
                &format!("group-{}", tables.len() + 1),
                &title,
                vec![
                    Column::new("number", "Wire no.", 14.0),
                    Column::new("from", "From", 30.0),
                    Column::new("to", "To", 30.0),
                    Column::new("cable", "Cable", 22.0),
                    Column::new("basis", "Length basis", 12.0),
                    Column::new("length_mm", "Wire mm", 10.0).right(),
                    Column::new("allowance_mm", "Ends mm", 10.0).right(),
                    Column::new("cut_mm", "Cut mm", 10.0).right(),
                ],
            );
            let mut total = 0.0;
            for w in group {
                let cut = w.length + allowance(w);
                total += cut;
                table.push(
                    vec![
                        Cell::text(&w.number),
                        Cell::text(&w.from),
                        Cell::text(&w.to),
                        Cell::text(&w.cable),
                        Cell::text(&w.basis),
                        Cell::Number(w.length),
                        Cell::Number(allowance(w)),
                        Cell::Number(cut),
                    ],
                    w.row.sources.clone(),
                );
            }
            summary.push(
                vec![
                    Cell::text(&title),
                    Cell::Int(group.len() as i64),
                    Cell::Number((total / 1000.0 * 100.0).round() / 100.0),
                ],
                Vec::new(),
            );
            tables.push(table);
            start = end;
        }
        let mut sections = Vec::new();
        if !tables.is_empty() {
            sections.push(summary);
            sections.extend(tables);
        }
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections,
        }
    }
}
