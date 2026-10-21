//! Wire list: every connection from device pin to device pin.

use std::collections::{BTreeMap, HashMap};

use novo_core::{
    AnyId, Endpoint, EntityKind, PageId, PinRef, Project, SymbolRole, Wire, natural_cmp,
};

use crate::lengths::{self, Connection as Measured};
use crate::order::{join_distinct, location_text};
use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// The wire list's section title with its count. A connection is one row,
/// whatever number of drawn wires and junctions it passes. The supply and PE
/// leads that have a feed at one end are not here; the workshop reports
/// ([`crate::WireCut`]) list them.
pub(crate) const WIRE_LIST_COUNT_TITLE: &str =
    "{n} connections between device pins (supply and PE leads: see Wire production)";

/// The wire list: one row per connection between two device or terminal
/// pins, the way a wireman reads it.
///
/// Drawn wires are joined into connections at everything that is not a
/// physical connection point: junctions, interruption points (by name,
/// across pages) and potential feeds. A connection reaching more than two
/// pins (a T-junction) is listed as a daisy chain in drawing order (page,
/// then top to bottom, then left to right), because a clamp takes the wires
/// one by one. Wires that reach fewer than two pins (a supply feed into a
/// fuse) are not wiring work and are left out.
///
/// Columns: wire number, from, to (`=A1+CAB-K1:13` designations), the
/// net's potential(s), colour, cross-section (the wire's own, else its
/// cable core's), cable and core, and the `page.column` of the from pin.
/// Connections sort by the reading order of their first pin: page, then
/// left to right, then top to bottom.
///
/// The last two columns are the cut length in millimetres and how it was
/// found. A wire between two devices placed on one mounting plate is routed
/// through that plate's ducts by `novo-routing` (`routed`; `free air` when
/// no duct reaches both, which the cabinet checks flag); anything else is an
/// `estimate` from the schematic distance of the two pins. Lengths include a
/// service loop at each end and are rounded up to a whole millimetre; see
/// the `lengths` module for the estimate rules.
#[derive(Debug, Clone, Copy, Default)]
pub struct WireList;

impl WireList {
    pub const ID: &'static str = "wires";
}

/// Where wires join without a physical clamp.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Joint<'a> {
    End(Endpoint),
    Interruption(&'a str),
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

impl Report for WireList {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Wire list".into()
    }

    fn dependencies(&self) -> Dependencies {
        use EntityKind::*;
        // The netlist (potentials) also reads the project rules.
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
        WireList::build(project, false)
    }
}

impl WireList {
    /// The wire list's table. With `with_feeds` it also holds the supply and
    /// PE leads that end on a potential feed, as the workshop reports (cut
    /// list, labels, wire production) list them; the wire list itself does
    /// not.
    pub(crate) fn build(project: &Project, with_feeds: bool) -> ReportOutput {
        let netlist = project.netlist();
        let connections = items(project, with_feeds);
        let mut section = Section::new(
            "wires",
            "",
            vec![
                Column::new("number", "Wire no.", 15.0),
                Column::new("from", "From", 35.0),
                Column::new("to", "To", 35.0),
                Column::new("potential", "Potential", 15.0),
                Column::new("color", "Colour", 12.0),
                Column::new("cross_section_mm2", "mm²", 10.0).right(),
                Column::new("cable", "Cable", 25.0),
                Column::new("location", "Location", 15.0),
                Column::new("length_mm", "Cut length mm", 14.0).right(),
                Column::new("length_basis", "Length basis", 12.0),
            ],
        );
        // The wire's own cross-section, else its cable core's.
        let cross_sections = |members: &[&Wire]| -> Vec<f64> {
            let mut sections: Vec<f64> = Vec::new();
            for wire in members {
                let core = project.core_of_wire(wire.id).and_then(|(cable, core)| {
                    let cable = project.cable(cable)?;
                    cable.cores.get(core)
                });
                if let Some(value) = wire
                    .cross_section_mm2
                    .or(core.and_then(|c| c.cross_section_mm2))
                    && !sections.iter().any(|s| s.to_bits() == value.to_bits())
                {
                    sections.push(value);
                }
            }
            sections.sort_by(f64::total_cmp);
            sections
        };
        let measured: Vec<Measured> = connections
            .iter()
            .map(|c| Measured {
                from: c.from,
                to: c.to,
                cross_section_mm2: cross_sections(&c.wires).last().copied(),
                cable_mm: lengths::cable_mm(project, &c.wires),
            })
            .collect();
        let lengths = lengths::cut_lengths(project, &measured);
        for (connection, length) in connections.into_iter().zip(lengths) {
            let members: Vec<&Wire> = connection.wires.clone();
            let mut numbers: Vec<&str> =
                members.iter().filter_map(|w| w.number.as_deref()).collect();
            numbers.sort_by(|a, b| natural_cmp(a, b));
            let mut cores = Vec::new();
            for wire in &members {
                let core = project.core_of_wire(wire.id).and_then(|(cable, core)| {
                    let cable = project.cable(cable)?;
                    Some((cable, cable.cores.get(core)?))
                });
                if let Some((cable, core)) = core {
                    cores.push(format!("{} {}", cable.tag, core.name));
                }
            }
            let sections = cross_sections(&members);
            let cross_section = match sections.as_slice() {
                [] => Cell::Empty,
                [single] => Cell::Number(*single),
                many => Cell::text(join_distinct(many.iter().map(f64::to_string))),
            };
            let potential = netlist
                .net_of(connection.from)
                .map(|net| {
                    join_distinct(
                        netlist
                            .net(net)
                            .potentials
                            .iter()
                            .filter_map(|&p| project.potential(p))
                            .map(|p| p.name.as_str()),
                    )
                })
                .unwrap_or_default();
            let location = project.symbol(connection.from.symbol).and_then(|symbol| {
                let mut location = project.location_of(symbol.id)?;
                let page = project.page(symbol.page)?;
                let x = project.endpoint_position(connection.from.into())?.x;
                location.column = page.format.column_at(x);
                Some(location_text(&location))
            });
            let designation = |pin| crate::wirecut::end_designation(project, pin);
            section.push(
                vec![
                    Cell::text(join_distinct(numbers)),
                    Cell::text(designation(connection.from)),
                    Cell::text(designation(connection.to)),
                    Cell::text(potential),
                    Cell::text(join_distinct(
                        members.iter().filter_map(|w| w.color.as_deref()),
                    )),
                    cross_section,
                    Cell::text(join_distinct(cores)),
                    location.into(),
                    Cell::Number(length.mm),
                    Cell::text(length.basis.text()),
                ],
                members.iter().map(|w| AnyId::from(w.id)).collect(),
            );
        }
        // Says what the rows are, so the count can be compared with the
        // other workshop reports (see [`WIRE_LIST_COUNT_TITLE`]).
        if !with_feeds {
            section.title = WIRE_LIST_COUNT_TITLE.replace("{n}", &section.rows.len().to_string());
        }
        ReportOutput {
            report: Self::ID.into(),
            title: Report::title(&WireList),
            revision: project.revision(),
            sections: vec![section],
        }
    }
}

/// One physical connection of the wire list: two pins and the drawn wires
/// that make it (more than one when it passes junctions or interruption
/// points). Shared by the wire list and the wire production reports.
pub(crate) struct Item<'a> {
    pub from: PinRef,
    pub to: PinRef,
    pub wires: Vec<&'a Wire>,
}

/// The connections in reading order: page, left to right, top to bottom.
///
/// With `with_feeds`, a potential feed counts as the far end of a wire: the
/// supply lead into a fuse and the PE wire to the motor are wires to cut,
/// although only one of their ends is a device pin. The wire list leaves
/// them out; the workshop reports do not.
pub(crate) fn items(project: &Project, with_feeds: bool) -> Vec<Item<'_>> {
    let page_index: HashMap<PageId, usize> = project
        .pages_in_order()
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id, i))
        .collect();
    let mut wires: Vec<&Wire> = project.wires().collect();
    wires.sort_by_key(|w| w.id);

    // Physical pins stay apart; everything else joins its wires.
    let joint = |end: Endpoint| -> Option<Joint<'_>> {
        match end {
            Endpoint::Junction { .. } => Some(Joint::End(end)),
            // An open end joins nothing.
            Endpoint::Free { .. } => None,
            Endpoint::Pin { symbol, .. } => match &project.symbol(symbol)?.role {
                SymbolRole::Device { .. } | SymbolRole::Terminal { .. } => None,
                SymbolRole::Interruption { name } => Some(Joint::Interruption(name)),
                SymbolRole::PotentialFeed { .. } | SymbolRole::Unassigned => Some(Joint::End(end)),
            },
        }
    };
    let mut parent: Vec<usize> = (0..wires.len()).collect();
    let mut first_at: HashMap<Joint<'_>, usize> = HashMap::new();
    for (i, wire) in wires.iter().enumerate() {
        for end in wire.ends() {
            if let Some(j) = joint(end) {
                let other = *first_at.entry(j).or_insert(i);
                let (a, b) = (find(&mut parent, i), find(&mut parent, other));
                parent[a] = b;
            }
        }
    }
    let mut components: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..wires.len() {
        let root = find(&mut parent, i);
        components.entry(root).or_default().push(i);
    }

    // Pin order key: page, then top to bottom, then left to right.
    let pin_key = |pin: PinRef| {
        let symbol = project.symbol(pin.symbol);
        let page = symbol.map_or(usize::MAX, |s| {
            page_index.get(&s.page).copied().unwrap_or(usize::MAX)
        });
        let at = project
            .endpoint_position(pin.into())
            .unwrap_or(novo_core::Point::new(0.0, 0.0));
        (page, at)
    };

    struct Reading<'a> {
        reading: (usize, f64, f64),
        item: Item<'a>,
    }
    let mut connections: Vec<Reading<'_>> = Vec::new();
    for members in components.into_values() {
        let mut pins: Vec<PinRef> = members
            .iter()
            .flat_map(|&i| wires[i].ends())
            .filter(|&end| joint(end).is_none())
            .filter_map(Endpoint::as_pin)
            .collect();
        pins.sort();
        pins.dedup();
        // Each feed attaches to the first device pin it reaches without
        // passing another feed; two feeds in one net never pair.
        let mut feeds: Vec<(PinRef, PinRef)> = Vec::new();
        if with_feeds && !pins.is_empty() {
            let is_feed = |p: PinRef| {
                project
                    .symbol(p.symbol)
                    .is_some_and(|s| matches!(s.role, SymbolRole::PotentialFeed { .. }))
            };
            let mut at_joint: HashMap<Joint<'_>, Vec<usize>> = HashMap::new();
            for &i in &members {
                for end in wires[i].ends() {
                    if let Some(j) = joint(end) {
                        at_joint.entry(j).or_default().push(i);
                    }
                }
            }
            let mut feed_pins: Vec<PinRef> = members
                .iter()
                .flat_map(|&i| wires[i].ends())
                .filter_map(Endpoint::as_pin)
                .filter(|&p| joint(p.into()).is_some() && is_feed(p))
                .collect();
            feed_pins.sort();
            feed_pins.dedup();
            for feed in feed_pins {
                let start = Joint::End(Endpoint::from(feed));
                let mut seen = vec![start.clone()];
                let mut queue = vec![start];
                let mut reached: Vec<PinRef> = Vec::new();
                while let Some(at) = queue.pop() {
                    for &i in at_joint.get(&at).into_iter().flatten() {
                        for end in wires[i].ends() {
                            match joint(end) {
                                None => reached.extend(end.as_pin()),
                                // Another feed ends the walk.
                                Some(_) if end.as_pin().is_some_and(is_feed) => {}
                                Some(j) if !seen.contains(&j) => {
                                    seen.push(j.clone());
                                    queue.push(j);
                                }
                                Some(_) => {}
                            }
                        }
                    }
                }
                reached.sort();
                reached.dedup();
                if let Some(&pin) = reached.first() {
                    feeds.push((pin, feed));
                }
            }
        }
        if pins.len() + feeds.len() < 2 {
            continue;
        }
        pins.sort_by(|&a, &b| {
            let ((pa, ka), (pb, kb)) = (pin_key(a), pin_key(b));
            pa.cmp(&pb)
                .then(ka.y.total_cmp(&kb.y))
                .then(ka.x.total_cmp(&kb.x))
                .then(a.cmp(&b))
        });
        let (page, first) = pin_key(pins[0]);
        let mut pairs: Vec<(PinRef, PinRef)> = pins.windows(2).map(|p| (p[0], p[1])).collect();
        pairs.extend(feeds);
        for (from, to) in pairs {
            connections.push(Reading {
                reading: (page, first.x, first.y),
                item: Item {
                    from,
                    to,
                    wires: members.iter().map(|&i| wires[i]).collect(),
                },
            });
        }
    }
    // Stable sort: a chain keeps its order.
    connections.sort_by(|a, b| {
        a.reading
            .0
            .cmp(&b.reading.0)
            .then(a.reading.1.total_cmp(&b.reading.1))
            .then(a.reading.2.total_cmp(&b.reading.2))
    });
    connections.into_iter().map(|r| r.item).collect()
}
