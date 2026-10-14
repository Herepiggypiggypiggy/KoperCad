//! Terminal diagrams (WS-11 data) and cable diagrams (WS-12 data) as
//! generated reports: the rows come from `novo_terminals::screen` and
//! `novo_cables::cable_screens`, so the drawer, the generated pages and the
//! spreadsheet always show the same thing.

use novo_core::{DeviceKind, EntityKind, Project};

use crate::order::tag_cmp;
use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// Everything but free texts: the connectivity, structure, part and cable
/// records the WS-11 and WS-12 screens are built from, plus the project
/// rules (terminal joins, potentials from terminal names).
fn model_dependencies() -> Dependencies {
    use EntityKind::*;
    Dependencies::on(&[
        Page, Device, Symbol, Junction, Wire, Potential, Terminal, Cable, Part,
    ])
    .with_info()
}

/// Terminal diagrams: one section per terminal strip (natural tag order),
/// one row per terminal with what is wired to it inside the cabinet and in
/// the field, the cable cores, jumper group, pages and how it is built.
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalDiagram;

impl TerminalDiagram {
    pub const ID: &'static str = "terminals";
}

impl Report for TerminalDiagram {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Terminal diagram".into()
    }

    fn dependencies(&self) -> Dependencies {
        // The strip view reads wires, symbols, cables and the project rules.
        model_dependencies()
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let mut strips: Vec<_> = project
            .devices()
            .filter(|d| d.kind == DeviceKind::TerminalStrip)
            .collect();
        strips.sort_by(|a, b| tag_cmp(&a.tag, &b.tag).then(a.id.cmp(&b.id)));
        let sections = strips
            .into_iter()
            .map(|strip| {
                let screen = novo_terminals::screen(project, strip.id);
                let tag = strip.tag.to_string();
                let mut section = Section::new(
                    &tag,
                    &tag,
                    vec![
                        Column::new("internal", "Internal", 40.0),
                        Column::new("terminal", "Terminal", 14.0),
                        Column::new("kind", "Kind", 18.0),
                        Column::new("external", "External", 40.0),
                        Column::new("cable", "Cable", 25.0),
                        Column::new("jumper", "Jumper", 10.0).right(),
                        Column::new("pages", "Pages", 15.0),
                        Column::new("build", "Build", 30.0),
                        Column::new("spare", "Spare", 10.0),
                    ],
                );
                for row in screen.rows {
                    section.push(
                        vec![
                            Cell::text(row.internal.join(", ")),
                            Cell::text(row.terminal),
                            Cell::text(row.kind),
                            Cell::text(row.external.join(", ")),
                            Cell::text(row.cables.join(", ")),
                            row.jumper.map(i64::from).into(),
                            Cell::text(row.pages.join(", ")),
                            Cell::text(row.build),
                            Cell::text(if row.spare { "spare" } else { "" }),
                        ],
                        vec![strip.id.into()],
                    );
                }
                section
            })
            .collect();
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections,
        }
    }
}

/// Cable diagrams: an `overview` section (one row per cable), then one
/// section per cable with every core and what it lands on at the cabinet
/// end and the field end.
#[derive(Debug, Clone, Copy, Default)]
pub struct CableDiagram;

impl CableDiagram {
    pub const ID: &'static str = "cables";
}

impl Report for CableDiagram {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Cable diagram".into()
    }

    fn dependencies(&self) -> Dependencies {
        // Core ends read wires, symbols, devices, terminals and potentials.
        model_dependencies()
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let mut overview = Section::new(
            "overview",
            "Overview",
            vec![
                Column::new("tag", "Cable", 25.0),
                Column::new("type", "Type", 20.0),
                Column::new("length_m", "Length (m)", 12.0).right(),
                Column::new("from", "From", 18.0),
                Column::new("to", "To", 18.0),
                Column::new("used", "Used", 10.0).right(),
                Column::new("cores", "Cores", 10.0).right(),
                Column::new("spare", "Spare", 10.0).right(),
            ],
        );
        // Built once: the overview rows are derived from the same screens.
        let screens = novo_cables::cable_screens(project);
        for row in screens.iter().map(novo_cables::OverviewRow::of) {
            overview.push(
                vec![
                    Cell::text(row.tag),
                    Cell::text(row.type_name),
                    row.length_m.into(),
                    Cell::text(row.from),
                    Cell::text(row.to),
                    Cell::Int(row.used as i64),
                    Cell::Int(row.cores as i64),
                    Cell::Int(row.spare as i64),
                ],
                vec![row.cable.into()],
            );
        }
        let mut sections = vec![overview];
        for screen in screens {
            let mut title = format!("{} · {}", screen.tag, screen.type_name);
            if !screen.from.is_empty() || !screen.to.is_empty() {
                title.push_str(&format!(" · {} → {}", screen.from, screen.to));
            }
            if let Some(length) = screen.length_m {
                title.push_str(&format!(" · {length} m"));
            }
            let mut section = Section::new(
                &screen.tag,
                &title,
                vec![
                    Column::new("core", "Core", 10.0),
                    Column::new("cross_section_mm2", "mm²", 8.0).right(),
                    Column::new("number", "Wire no.", 12.0),
                    Column::new("potential", "Potential", 12.0),
                    Column::new("from_location", "From", 12.0),
                    Column::new("from", "Cabinet end", 35.0),
                    Column::new("to_location", "To", 12.0),
                    Column::new("to", "Field end", 35.0),
                    Column::new("note", "Note", 12.0),
                ],
            );
            for row in screen.rows {
                let [from, to] = row.ends;
                let end = |end: &Option<novo_cables::CoreEnd>| {
                    end.as_ref().map_or((Cell::Empty, Cell::Empty), |e| {
                        (Cell::text(e.location.clone()), Cell::text(e.target.clone()))
                    })
                };
                let ((from_location, from), (to_location, to)) = (end(&from), end(&to));
                let note = match (row.shield, row.spare) {
                    (true, _) => "shield",
                    (false, true) => "spare",
                    _ => "",
                };
                let mut sources: Vec<novo_core::AnyId> = vec![screen.cable.into()];
                sources.extend(row.wire.map(novo_core::AnyId::from));
                section.push(
                    vec![
                        Cell::text(row.core),
                        row.cross_section_mm2.into(),
                        Cell::from(row.wire_number),
                        Cell::from(row.potential),
                        from_location,
                        from,
                        to_location,
                        to,
                        Cell::text(note),
                    ],
                    sources,
                );
            }
            sections.push(section);
        }
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections,
        }
    }
}
