//! Plug connection lists: for every plug or socket, each pin with the wire
//! on it and where the wire goes.

use std::collections::HashSet;

use novo_core::{
    AnyId, DeviceKind, Endpoint, EntityKind, PinRef, Project, SymbolRole, WireId, natural_cmp,
};

use crate::order::tag_cmp;
use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// Plug connection lists: one section per connector device ([`DeviceKind::Connector`],
/// natural tag order), one row per pin and wire, pins in natural order. A pin
/// with no wire is listed once, marked `free`, so the list shows the whole
/// plug. `To` is the pin the wire reaches, followed through junctions; a
/// branch reaching several pins gives one row per destination.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlugList;

impl PlugList {
    pub const ID: &'static str = "plugs";
}

/// The pins reached from `start` over `wire`, through junctions only.
fn far_pins(project: &Project, wire: WireId, start: Endpoint) -> Vec<PinRef> {
    let mut pins = Vec::new();
    let mut seen_wires: HashSet<WireId> = HashSet::from([wire]);
    let mut seen_junctions = HashSet::new();
    let mut open = Vec::new();
    let far = |w: WireId, from: Endpoint| {
        project
            .wire(w)
            .map(|w| if w.from == from { w.to } else { w.from })
    };
    open.extend(far(wire, start));
    while let Some(end) = open.pop() {
        match end {
            Endpoint::Pin { symbol, pin } => pins.push(PinRef { symbol, pin }),
            Endpoint::Free { .. } => {}
            Endpoint::Junction { junction } => {
                if !seen_junctions.insert(junction) {
                    continue;
                }
                for w in project.wires_at(end).collect::<Vec<_>>() {
                    if seen_wires.insert(w) {
                        open.extend(far(w, end));
                    }
                }
            }
        }
    }
    pins.sort();
    pins.dedup();
    pins
}

impl Report for PlugList {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Plug connection list".into()
    }

    fn dependencies(&self) -> Dependencies {
        use EntityKind::*;
        Dependencies::on(&[Page, Device, Symbol, Junction, Wire, Terminal, Cable]).with_info()
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let mut plugs: Vec<_> = project
            .devices()
            .filter(|d| d.kind == DeviceKind::Connector)
            .collect();
        plugs.sort_by(|a, b| tag_cmp(&a.tag, &b.tag).then(a.id.cmp(&b.id)));
        let sections = plugs
            .into_iter()
            .map(|plug| {
                let tag = plug.tag.to_string();
                let mut section = Section::new(
                    &tag,
                    &tag,
                    vec![
                        Column::new("pin", "Pin", 8.0),
                        Column::new("number", "Wire no.", 12.0),
                        Column::new("color", "Colour", 10.0),
                        Column::new("cross_section_mm2", "mm²", 8.0).right(),
                        Column::new("to", "To", 38.0),
                        Column::new("cable", "Cable", 22.0),
                        Column::new("note", "Note", 8.0),
                    ],
                );
                // Pins of every symbol drawing this device.
                let mut pins: Vec<(String, PinRef)> = project
                    .symbols_of_device(plug.id)
                    .into_iter()
                    .filter(|s| matches!(s.role, SymbolRole::Device { .. }))
                    .flat_map(|s| {
                        s.pins
                            .iter()
                            .enumerate()
                            .map(|(i, p)| (p.name.clone(), PinRef::new(s.id, i as u16)))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                pins.sort_by(|a, b| natural_cmp(&a.0, &b.0).then(a.1.cmp(&b.1)));
                let mut listed: HashSet<String> = HashSet::new();
                for (name, pin) in pins {
                    let end = Endpoint::from(pin);
                    let wires: Vec<WireId> = project.wires_at(end).collect();
                    if wires.is_empty() {
                        // A pin drawn twice is one pin: list it once.
                        if listed.insert(name.clone()) {
                            section.push(
                                vec![
                                    Cell::text(name),
                                    Cell::Empty,
                                    Cell::Empty,
                                    Cell::Empty,
                                    Cell::Empty,
                                    Cell::Empty,
                                    Cell::text("free"),
                                ],
                                vec![plug.id.into()],
                            );
                        }
                        continue;
                    }
                    listed.insert(name.clone());
                    for wire_id in wires {
                        let Some(wire) = project.wire(wire_id) else {
                            continue;
                        };
                        let mut destinations: Vec<String> = far_pins(project, wire_id, end)
                            .into_iter()
                            .filter_map(|p| project.pin_designation(p))
                            .collect();
                        destinations.sort_by(|a, b| natural_cmp(a, b));
                        destinations.dedup();
                        if destinations.is_empty() {
                            destinations.push(String::new());
                        }
                        let cable = project.core_of_wire(wire_id).and_then(|(cable, core)| {
                            let cable = project.cable(cable)?;
                            Some((cable, cable.cores.get(core)?))
                        });
                        let section_mm2 = wire
                            .cross_section_mm2
                            .or_else(|| cable.and_then(|(_, c)| c.cross_section_mm2));
                        for to in destinations {
                            section.push(
                                vec![
                                    Cell::text(name.clone()),
                                    Cell::from(wire.number.clone()),
                                    Cell::from(wire.color.clone()),
                                    section_mm2.into(),
                                    Cell::text(to),
                                    Cell::text(cable.map_or(String::new(), |(c, core)| {
                                        format!("{} {}", c.tag, core.name)
                                    })),
                                    Cell::Empty,
                                ],
                                vec![plug.id.into(), AnyId::from(wire_id)],
                            );
                        }
                    }
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
