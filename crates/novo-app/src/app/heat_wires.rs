//! Heat of the wires lying in the cabinet's cable ducts, for the Heat view
//! and the 3D heat run.
//!
//! The pieces already exist elsewhere: the single-line diagram knows the
//! load current of every run and its wires, the routing engine knows which
//! ducts a wire lies in, and `novo_thermal::wires` turns current,
//! cross-section and length into watts. This module joins them: per duct,
//! the I²R heat of the power wires that lie in it, plus the sentences that
//! say what the figure rests on.
//!
//! Where the user finds it: Heat view and 3D heat (Simulate): the duct
//! cells carry the heat, the Heat panel's notes list the wattage and its
//! basis; the share of the rated current is Check > Electrical inputs.

use std::collections::{BTreeMap, BTreeSet};

use novo_checks::calc::tables::TableSet;
use novo_checks::single_line::{Origin, SingleLine, single_line};
use novo_core::{Project, StructureId, Value, WireId};
use novo_routing::{Duct, Point};
use novo_thermal::wires::{DuctWire, loss_by_duct};

/// Project property: share of the load current the wires carry in the heat
/// model, 0 to 1. Absent means 1: every load at its full current at the same
/// time, no diversity.
pub(super) const WIRE_CURRENT_FACTOR: &str = "heat.wire_current_factor";

/// A wire further than this from a duct's centre line is not in the duct, mm.
const ON_CENTRE_MM: f64 = 1.0;

/// What the wires in the ducts of one cabinet give off.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct WireHeat {
    /// Watts per duct (rail id).
    pub by_duct: BTreeMap<u64, f64>,
    /// Sentences for the Heat panel, empty when there is nothing to say.
    pub notes: Vec<String>,
}

impl WireHeat {
    #[cfg(test)]
    pub(super) fn total_w(&self) -> f64 {
        self.by_duct.values().sum()
    }
}

/// Distance from `p` to the segment `a`-`b`, mm.
fn distance(p: &Point, a: &Point, b: &Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a.x + t * dx, a.y + t * dy);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// Length of a route's path that lies in each of `listed` ducts, mm. A piece
/// of the path counts for a duct when its two ends and its middle lie on the
/// duct's centre line, so the leg from a terminal to the duct and the stretch
/// across a junction go to the right duct (or to none).
pub(super) fn length_in_ducts(
    points: &[Point],
    ducts: &[Duct],
    listed: &[u64],
) -> BTreeMap<u64, f64> {
    let mut out: BTreeMap<u64, f64> = BTreeMap::new();
    for pair in points.windows(2) {
        let (p, q) = (&pair[0], &pair[1]);
        let len = p.manhattan(q);
        if len <= 0.0 {
            continue;
        }
        let mid = Point::new((p.x + q.x) / 2.0, (p.y + q.y) / 2.0);
        let hit = listed.iter().find(|id| {
            ducts.iter().any(|d| {
                d.id == **id
                    && [p, q, &mid]
                        .iter()
                        .all(|x| distance(x, &d.a, &d.b) <= ON_CENTRE_MM)
            })
        });
        if let Some(id) = hit {
            *out.entry(*id).or_default() += len;
        }
    }
    out
}

/// The share of the load current the heat model uses, and whether the
/// project states it. A stated value outside 0..=1 is ignored.
pub(super) fn current_factor(project: &Project) -> (f64, bool) {
    let stated = match project.info().properties.get(WIRE_CURRENT_FACTOR) {
        Some(Value::Float(f)) => Some(*f),
        Some(Value::Int(i)) => Some(*i as f64),
        _ => None,
    };
    match stated {
        Some(f) if f.is_finite() && f > 0.0 && f <= 1.0 => (f, true),
        _ => (1.0, false),
    }
}

/// What one wire of a run carries.
#[derive(Debug, Clone, Copy)]
struct Carry {
    current_a: f64,
    origin: Origin,
    section_mm2: Option<f64>,
    temperature_c: f64,
    conductors: u32,
}

/// Current, cross-section and temperature of every wire the single-line
/// diagram lists on a run. A wire on two runs keeps the larger current.
fn carried(diagram: &SingleLine) -> BTreeMap<WireId, Carry> {
    let mut out: BTreeMap<WireId, Carry> = BTreeMap::new();
    for node in &diagram.nodes {
        let Some(link) = &node.link else { continue };
        let Some(current) = link.current_a.value else {
            continue;
        };
        for seg in &link.segments {
            let c = Carry {
                current_a: current,
                origin: link.current_a.origin,
                section_mm2: seg.section_mm2.value,
                temperature_c: seg.insulation.max_temperature(),
                // Out and back, or the three phases of a balanced run.
                conductors: if link.three_phase { 3 } else { 2 },
            };
            out.entry(seg.wire)
                .and_modify(|old| {
                    if c.current_a > old.current_a {
                        *old = c;
                    }
                })
                .or_insert(c);
        }
    }
    out
}

/// The heat of the wires in the ducts of `structure`.
pub(super) fn wire_heat(project: &Project, structure: StructureId) -> WireHeat {
    let ducts = novo_routing::model::ducts(project, structure);
    if ducts.is_empty() {
        return WireHeat::default();
    }
    let netlist = project.netlist();
    let diagram = single_line(project, &netlist, &TableSet::builtin(), &BTreeMap::new());
    let carry = carried(&diagram);
    let routed = novo_routing::model::route_project(project);
    let (factor, factor_stated) = current_factor(project);
    let listed: Vec<u64> = ducts.iter().map(|d| d.id).collect();

    let mut entries: Vec<DuctWire> = Vec::new();
    let (mut stated, mut calculated) = (BTreeSet::new(), BTreeSet::new());
    let (mut no_current, mut no_section) = (BTreeSet::new(), BTreeSet::new());
    for (wire_id, route) in &routed.routes {
        if !route.ducts.iter().any(|d| listed.contains(d)) {
            continue;
        }
        let wire = WireId::from(*wire_id);
        let Some(c) = carry.get(&wire) else {
            no_current.insert(*wire_id);
            continue;
        };
        let Some(section) = c.section_mm2 else {
            no_section.insert(*wire_id);
            continue;
        };
        for (duct, mm) in length_in_ducts(&route.points, &ducts, &route.ducts) {
            entries.push(DuctWire {
                duct,
                length_m: mm / 1000.0,
                section_mm2: section,
                current_a: c.current_a * factor,
                temperature_c: c.temperature_c,
                conductors: c.conductors,
            });
        }
        match c.origin {
            Origin::Stated => stated.insert(*wire_id),
            _ => calculated.insert(*wire_id),
        };
    }
    let by_duct = loss_by_duct(&entries);
    let total: f64 = by_duct.values().sum();

    let mut notes = Vec::new();
    let heating = stated.len() + calculated.len();
    if total > 0.0 {
        let basis = match (stated.len(), calculated.len()) {
            (s, 0) => format!("current stated on all {s} wires"),
            (0, c) => format!("current calculated from the loads below on all {c} wires"),
            (s, c) => {
                format!("current stated on {s} wires and calculated from the loads below on {c}")
            }
        };
        let load = if factor_stated {
            format!(
                "currents scaled to {:.0} % as the project states (Check > Electrical inputs)",
                factor * 100.0
            )
        } else {
            "no diversity: every load at its full current at the same time".to_owned()
        };
        notes.push(format!(
            "Wires in ducts add {total:.1} W ({heating} wire{}).",
            if heating == 1 { "" } else { "s" }
        ));
        notes.push(format!(
            "Wire heat basis: {basis}; {load}. Copper at the insulation's highest temperature \
             (70 °C PVC, 90 °C XLPE), so a planning upper value. The other conductors of a run are \
             assumed to lie like the one listed (3 for three-phase, 2 out and back). The heat is \
             spread evenly along each duct. Neutral and PE current is not counted (balanced load). \
             Part-load runs scale this heat linearly with the load although I²R is quadratic: that \
             overestimates."
        ));
    }
    if !no_current.is_empty() {
        notes.push(format!(
            "{} wire{} in ducts {} on no run the model knows a current for. Where they are the other \
             phases or the return of a run counted above, their heat is already in the total (the 3 or 2 \
             conductors); control and signal wires, or wires with no load below, add nothing.",
            no_current.len(),
            if no_current.len() == 1 { "" } else { "s" },
            if no_current.len() == 1 { "is" } else { "are" }
        ));
    }
    if !no_section.is_empty() {
        notes.push(format!(
            "{} wire{} in ducts have a current but no cross-section: no heat counted for them.",
            no_section.len(),
            if no_section.len() == 1 { "" } else { "s" }
        ));
    }
    WireHeat { by_duct, notes }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn duct(id: u64, a: (f64, f64), b: (f64, f64)) -> Duct {
        Duct {
            id,
            a: Point::new(a.0, a.1),
            b: Point::new(b.0, b.1),
            width: 40.0,
            height: 60.0,
        }
    }

    #[test]
    fn the_stub_to_a_terminal_is_not_in_the_duct() {
        // Terminal at (100, 50), duct along y = 200 from x 0 to 400.
        let ducts = [duct(5, (0.0, 200.0), (400.0, 200.0))];
        let path = [
            Point::new(100.0, 50.0),
            Point::new(100.0, 200.0),
            Point::new(300.0, 200.0),
            Point::new(300.0, 90.0),
        ];
        let by = length_in_ducts(&path, &ducts, &[5]);
        assert_eq!(by.len(), 1);
        assert!((by[&5] - 200.0).abs() < 1e-9, "{by:?}");
    }

    #[test]
    fn a_stretch_through_a_junction_goes_to_the_duct_it_lies_in() {
        let ducts = [
            duct(1, (0.0, 200.0), (400.0, 200.0)),
            duct(2, (200.0, 0.0), (200.0, 400.0)),
        ];
        let path = [
            Point::new(50.0, 200.0),
            Point::new(200.0, 200.0),
            Point::new(200.0, 350.0),
        ];
        let by = length_in_ducts(&path, &ducts, &[1, 2]);
        assert!((by[&1] - 150.0).abs() < 1e-9, "{by:?}");
        assert!((by[&2] - 150.0).abs() < 1e-9, "{by:?}");
    }

    #[test]
    fn a_path_outside_every_duct_has_no_length_in_one() {
        let ducts = [duct(1, (0.0, 200.0), (400.0, 200.0))];
        let path = [Point::new(0.0, 300.0), Point::new(400.0, 300.0)];
        assert!(length_in_ducts(&path, &ducts, &[1]).is_empty());
    }

    #[test]
    fn the_copper_formula_is_the_checks_one() {
        for t in [20.0, 70.0, 90.0] {
            assert!(
                (novo_thermal::wires::copper_resistivity(t)
                    - novo_checks::calc::copper_resistivity(t))
                .abs()
                    < 1e-12
            );
        }
    }
}
