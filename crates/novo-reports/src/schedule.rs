//! Cable schedule: one flat table of every core of every cable, for the
//! installer and for Excel (cable, type, length, core, from, to).

use novo_core::Project;

use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// The cable schedule: one row per core, cables in tag order. The ends are
/// the same cabinet and field designations the cable diagram shows
/// ([`novo_cables::cable_screens`]), so both always agree. A cable without
/// cores still gets one row so it is not lost from the schedule. Length is
/// the cable's own `length_m`, empty until it is known.
#[derive(Debug, Clone, Copy, Default)]
pub struct CableSchedule;

impl CableSchedule {
    pub const ID: &'static str = "cableschedule";
}

impl Report for CableSchedule {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Cable schedule".into()
    }

    fn dependencies(&self) -> Dependencies {
        crate::diagrams::model_dependencies()
    }

    fn proposed_by_default(&self) -> bool {
        // The cable diagram already carries the overview.
        false
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let mut section = Section::new(
            "schedule",
            "",
            vec![
                Column::new("cable", "Cable", 20.0),
                Column::new("type", "Type", 18.0),
                Column::new("length_m", "Length (m)", 10.0).right(),
                Column::new("core", "Core", 8.0),
                Column::new("cross_section_mm2", "mm²", 7.0).right(),
                Column::new("number", "Wire no.", 10.0),
                Column::new("from", "From", 32.0),
                Column::new("to", "To", 32.0),
                Column::new("note", "Note", 8.0),
            ],
        );
        for screen in novo_cables::cable_screens(project) {
            let target = |end: &Option<novo_cables::CoreEnd>| {
                Cell::text(end.as_ref().map_or(String::new(), |e| e.target.clone()))
            };
            if screen.rows.is_empty() {
                section.push(
                    vec![
                        Cell::text(screen.tag.clone()),
                        Cell::text(screen.type_name.clone()),
                        screen.length_m.into(),
                        Cell::Empty,
                        Cell::Empty,
                        Cell::Empty,
                        Cell::text(screen.from.clone()),
                        Cell::text(screen.to.clone()),
                        Cell::Empty,
                    ],
                    vec![screen.cable.into()],
                );
            }
            for row in &screen.rows {
                let note = match (row.shield, row.spare) {
                    (true, _) => "shield",
                    (false, true) => "spare",
                    _ => "",
                };
                let mut sources: Vec<novo_core::AnyId> = vec![screen.cable.into()];
                sources.extend(row.wire.map(novo_core::AnyId::from));
                section.push(
                    vec![
                        Cell::text(screen.tag.clone()),
                        Cell::text(screen.type_name.clone()),
                        screen.length_m.into(),
                        Cell::text(row.core.clone()),
                        row.cross_section_mm2.into(),
                        Cell::from(row.wire_number.clone()),
                        target(&row.ends[0]),
                        target(&row.ends[1]),
                        Cell::text(note),
                    ],
                    sources,
                );
            }
        }
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections: vec![section],
        }
    }
}
