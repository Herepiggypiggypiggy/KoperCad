//! Cable schedule: one flat table of every core of every cable, for the
//! installer and for Excel (cable, type, length, core, from, to).

use std::collections::HashMap;

use novo_core::{CableId, EntityKind, Project};
use novo_routing::WireRules;
use novo_routing::model::{Connection, route_connections};

use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// The cable schedule: one row per core, cables in tag order. The ends are
/// the same cabinet and field designations the cable diagram shows
/// ([`novo_cables::cable_screens`]), so both always agree. A cable without
/// cores still gets one row so it is not lost from the schedule. Length is
/// the cable's own `length_m`; when that is not set and the cable's devices
/// are placed in one enclosure, the longest routed core (`novo-routing`,
/// through the ducts with slack), rounded up to 0.1 m and noted `calc`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CableSchedule;

impl CableSchedule {
    pub const ID: &'static str = "cableschedule";
}

/// Cable lengths calculated from the cabinet layout, for cables whose own
/// length is not set: the longest routed core, in metres rounded up to 0.1.
fn layout_lengths(project: &Project) -> HashMap<CableId, f64> {
    let mut connections = Vec::new();
    let mut owner = Vec::new();
    for cable in project.cables().filter(|c| c.length_m.is_none()) {
        for core in cable.cores.iter().filter(|c| !c.shield) {
            let Some(wire) = core.wire.and_then(|w| project.wire(w)) else {
                continue;
            };
            if let (Some(from), Some(to)) = (wire.from.as_pin(), wire.to.as_pin()) {
                connections.push(Connection {
                    id: connections.len() as u64,
                    from,
                    to,
                    diameter_mm: None,
                });
                owner.push(cable.id);
            }
        }
    }
    if connections.is_empty()
        || project
            .all::<novo_core::CabinetPlacement>()
            .next()
            .is_none()
    {
        return HashMap::new();
    }
    let routed = route_connections(project, &connections, &WireRules::of(project).routing);
    let mut lengths: HashMap<CableId, f64> = HashMap::new();
    for (id, route) in routed.routes {
        let metres = (route.length_mm / 100.0).ceil() / 10.0;
        let longest = lengths.entry(owner[id as usize]).or_insert(0.0);
        *longest = longest.max(metres);
    }
    lengths
}

impl Report for CableSchedule {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Cable schedule".into()
    }

    fn dependencies(&self) -> Dependencies {
        use EntityKind::*;
        crate::diagrams::model_dependencies().with_kinds(&[Rail, CabinetPlacement, Structure])
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
        let calculated = layout_lengths(project);
        for screen in novo_cables::cable_screens(project) {
            let calc = screen.length_m.is_none() && calculated.contains_key(&screen.cable);
            let length = screen
                .length_m
                .or_else(|| calculated.get(&screen.cable).copied());
            let target = |end: &Option<novo_cables::CoreEnd>| {
                Cell::text(end.as_ref().map_or(String::new(), |e| e.target.clone()))
            };
            if screen.rows.is_empty() {
                section.push(
                    vec![
                        Cell::text(screen.tag.clone()),
                        Cell::text(screen.type_name.clone()),
                        length.into(),
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
                let mut note: Vec<&str> = Vec::new();
                if row.shield {
                    note.push("shield");
                } else if row.spare {
                    note.push("spare");
                }
                if calc {
                    note.push("calc");
                }
                let mut sources: Vec<novo_core::AnyId> = vec![screen.cable.into()];
                sources.extend(row.wire.map(novo_core::AnyId::from));
                section.push(
                    vec![
                        Cell::text(screen.tag.clone()),
                        Cell::text(screen.type_name.clone()),
                        length.into(),
                        Cell::text(row.core.clone()),
                        row.cross_section_mm2.into(),
                        Cell::from(row.wire_number.clone()),
                        target(&row.ends[0]),
                        target(&row.ends[1]),
                        Cell::text(note.join(", ")),
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
