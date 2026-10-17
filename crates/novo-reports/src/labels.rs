//! Labels for label printers: wire, terminal, device and cable labels,
//! generated from the same data as the other reports.

use novo_core::{DeviceKind, EntityKind, Project};

use crate::order::tag_cmp;
use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};
use crate::{CsvOptions, WireList, to_csv};

/// The most text lines a label carries.
pub const LABEL_LINES: usize = 3;

/// Title of the wire labels, with the count: one label per end, two for each
/// wire to cut (the supply and PE leads included).
pub(crate) const WIRE_LABELS_COUNT_TITLE: &str =
    "Wire labels · {n} (one per wire end: two for each wire to cut)";

/// The labels of a project: one table per kind, one row per label, the text
/// in up to [`LABEL_LINES`] lines.
///
/// - `wires`: one label per end of each wire to cut, for a sleeve or flag:
///   the wire number, then where the other end goes (`=A1+CAB-K1:13`), then
///   this end. The wires are the wire list's connections plus the supply and
///   PE leads (the far end of a lead is the potential's name with `(feed)`),
///   the same wires as wire production. A connection that is a daisy chain
///   gives a label per pair.
/// - `terminals`: strip tag and terminal name, strip order.
/// - `devices`: tag and description, natural tag order.
/// - `cables`: tag, description and core count.
///
/// A kind with no labels has no table. Rows keep the objects they describe. The report is exported on demand
/// and is not proposed as a page.
#[derive(Debug, Clone, Copy, Default)]
pub struct Labels;

impl Labels {
    pub const ID: &'static str = "labels";
    /// Section keys, in the order they are listed.
    pub const KINDS: [&'static str; 4] = ["wires", "terminals", "devices", "cables"];
}

fn section(key: &str, title: &str) -> Section {
    Section::new(
        key,
        title,
        (1..=LABEL_LINES)
            .map(|i| Column::new(&format!("line{i}"), &format!("Line {i}"), 30.0))
            .collect(),
    )
}

fn lines(items: &[String]) -> Vec<Cell> {
    (0..LABEL_LINES)
        .map(|i| Cell::text(items.get(i).cloned().unwrap_or_default()))
        .collect()
}

impl Report for Labels {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Labels".into()
    }

    fn dependencies(&self) -> Dependencies {
        let mut deps = WireList.dependencies();
        deps.kinds.extend([EntityKind::Terminal, EntityKind::Cable]);
        deps
    }

    fn proposed_by_default(&self) -> bool {
        false
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        // Wires: both ends of every wire-list row.
        let mut wires = section("wires", "Wire labels");
        let list = WireList::build(project, true);
        if let Some(table) = list.sections.first() {
            let at = |row: &crate::table::Row, key: &str| {
                table
                    .column(key)
                    .and_then(|i| row.cells.get(i))
                    .map(ToString::to_string)
                    .unwrap_or_default()
            };
            for row in &table.rows {
                let (number, from, to) = (at(row, "number"), at(row, "from"), at(row, "to"));
                for (near, far) in [(&from, &to), (&to, &from)] {
                    wires.push(
                        lines(&[number.clone(), far.clone(), near.clone()]),
                        row.sources.clone(),
                    );
                }
            }
        }

        wires.title = WIRE_LABELS_COUNT_TITLE.replace("{n}", &wires.rows.len().to_string());

        // Terminals: strips in tag order, terminals in strip order.
        let mut terminals = section("terminals", "Terminal labels");
        let mut strips: Vec<_> = project
            .devices()
            .filter(|d| d.kind == DeviceKind::TerminalStrip)
            .collect();
        strips.sort_by(|a, b| tag_cmp(&a.tag, &b.tag).then(a.id.cmp(&b.id)));
        for strip in strips {
            for terminal in project.terminals_of_strip(strip.id) {
                terminals.push(
                    lines(&[strip.tag.to_string(), terminal.name.clone()]),
                    vec![terminal.id.into()],
                );
            }
        }

        // Devices.
        let mut devices = section("devices", "Device labels");
        let mut all: Vec<_> = project.devices().collect();
        all.sort_by(|a, b| tag_cmp(&a.tag, &b.tag).then(a.id.cmp(&b.id)));
        for device in all {
            devices.push(
                lines(&[device.tag.to_string(), device.description.clone()]),
                vec![device.id.into()],
            );
        }

        // Cables.
        let mut cables = section("cables", "Cable labels");
        let mut all: Vec<_> = project.cables().collect();
        all.sort_by(|a, b| tag_cmp(&a.tag, &b.tag).then(a.id.cmp(&b.id)));
        for cable in all {
            let cores = cable.cores.iter().filter(|c| !c.shield).count();
            let count = if cores == 0 {
                String::new()
            } else {
                format!("{cores} cores")
            };
            cables.push(
                lines(&[cable.tag.to_string(), cable.description.clone(), count]),
                vec![cable.id.into()],
            );
        }

        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            // A kind without labels has no table, like the other reports.
            sections: [wires, terminals, devices, cables]
                .into_iter()
                .filter(|s| !s.rows.is_empty())
                .collect(),
        }
    }
}

/// How labels are written to CSV for a label printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelLayout {
    /// A header row, comma separated, one column per line (RFC 4180, with a
    /// byte order mark for Excel): for any software that reads CSV.
    #[default]
    Generic,
    /// The layout Brother P-touch Editor imports as a database: no header,
    /// tab separated, UTF-8 without byte order mark, one label per row, one
    /// field per text line.
    Brother,
}

impl LabelLayout {
    pub fn parse(name: &str) -> Option<LabelLayout> {
        match name.to_ascii_lowercase().as_str() {
            "generic" => Some(LabelLayout::Generic),
            "brother" | "ptouch" | "p-touch" => Some(LabelLayout::Brother),
            _ => None,
        }
    }
}

/// The labels of the given kinds (`wires`, `terminals`, `devices`,
/// `cables`; all when empty) as CSV in the given layout. Kinds are written
/// in [`Labels::KINDS`] order, one after another; the generic layout puts
/// each kind's title on a line of its own, the Brother layout writes the
/// rows only, so choose one kind per file there.
pub fn labels_csv(output: &ReportOutput, kinds: &[&str], layout: LabelLayout) -> String {
    let wanted = |key: &str| kinds.is_empty() || kinds.contains(&key);
    let sections: Vec<Section> = output
        .sections
        .iter()
        .filter(|s| wanted(&s.key))
        .cloned()
        .collect();
    match layout {
        LabelLayout::Generic => to_csv(
            &ReportOutput {
                sections,
                ..output.clone()
            },
            &CsvOptions::default(),
        ),
        LabelLayout::Brother => {
            let mut out = String::new();
            for section in &sections {
                for row in &section.rows {
                    let fields: Vec<String> = row
                        .cells
                        .iter()
                        .map(|c| c.to_string().replace(['\t', '\r', '\n'], " "))
                        .collect();
                    out.push_str(&fields.join("\t"));
                    out.push_str("\r\n");
                }
            }
            out
        }
    }
}
