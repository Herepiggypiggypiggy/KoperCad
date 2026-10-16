//! Cables where they belong: wires that run from a device in one location
//! (`+CAB`) to a device in another (`+FLD`) leave the cabinet, and wires
//! that leave together between the same two devices (a terminal strip and
//! a motor, two strips) go in one cable.
//!
//! A proposal picks the cable type from the wires: colour-coded (`4G1.5`:
//! L1 BN, L2 BK, L3 GY, N BU, PE GNYE) for up to five supply conductors,
//! numbered (`7G1.5`, `12x1`) otherwise, sized by the largest wire
//! cross-section. Accepting creates the cable with every wire on its core
//! in one undo step. A declined proposal is remembered on its wires
//! (`cable.propose = false`, see [`dismiss_proposal`]) so it does not come
//! back after every edit.
//!
//! Like the terminal suggestions (WS-11), the first cut looks at wires that
//! run straight from one pin to another; wires through junctions are not
//! proposed yet.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use novo_core::{
    CableId, DeviceId, DeviceTag, Netlist, PotentialKind, Project, Result, Tx, Value, Wire, WireId,
    natural_cmp,
};
use novo_parts::{Catalog, find_project_part};

use crate::ends;
use crate::kind::{CableType, GNYE, NUMBERED_SIZES, is_protective, standard_cross_section};

/// Wire property that records a declined cable proposal.
pub const DISMISS_KEY: &str = "cable.propose";

/// What a wire carries, for picking its core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conductor {
    /// A line conductor, by potential name (`L1`, `L`).
    Line(String),
    Neutral,
    DcPositive,
    DcNegative,
    /// Anything else, by a sort name (wire number or designation).
    Signal(String),
    /// PE or PEN: always the green-yellow core.
    Protective,
}

impl Conductor {
    fn rank(&self) -> u8 {
        match self {
            Conductor::Line(_) => 0,
            Conductor::Neutral => 1,
            Conductor::DcPositive => 2,
            Conductor::DcNegative => 3,
            Conductor::Signal(_) => 4,
            Conductor::Protective => 5,
        }
    }

    fn sort_name(&self) -> &str {
        match self {
            Conductor::Line(name) | Conductor::Signal(name) => name,
            _ => "",
        }
    }

    /// Stable core order: L1, L2, L3, N, L+, L-, signals, PE.
    fn order(&self, other: &Self) -> Ordering {
        self.rank()
            .cmp(&other.rank())
            .then_with(|| natural_cmp(self.sort_name(), other.sort_name()))
    }
}

/// One end of a cable: the device (or terminal strip) the wires land on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CableSide {
    pub device: DeviceId,
    pub location: Vec<String>,
}

/// One proposed cable.
#[derive(Debug, Clone, PartialEq)]
pub struct CableProposal {
    /// The tag it gets: `-W` plus the next free number in the function of
    /// the cabinet-side device (`=A1-W1`).
    pub tag: DeviceTag,
    /// Cabinet side first, then the field side.
    pub ends: [CableSide; 2],
    /// The wires in core order.
    pub wires: Vec<WireId>,
    /// What each wire carries, parallel to `wires`.
    pub conductors: Vec<Conductor>,
    pub cable_type: CableType,
    /// The core index each wire goes on, parallel to `wires`.
    pub cores: Vec<usize>,
}

impl CableProposal {
    /// Uses another cable type (a shielded variant, a bigger cable). Returns
    /// `false` and changes nothing when the wires do not fit it.
    pub fn retype(&mut self, cable_type: CableType) -> bool {
        match assign(&self.conductors, &cable_type) {
            Some(cores) => {
                self.cable_type = cable_type;
                self.cores = cores;
                true
            }
            None => false,
        }
    }
}

/// What a wire carries: from the potential of its net, else its colour or
/// a pin named `PE`, else a signal.
pub fn conductor(project: &Project, netlist: &Netlist, wire: &Wire) -> Conductor {
    let potential = wire
        .potential
        .or_else(|| netlist.potential_of(wire.from))
        .or_else(|| netlist.potential_of(wire.to))
        .and_then(|p| project.potential(p));
    if let Some(p) = potential {
        match p.kind {
            PotentialKind::ProtectiveEarth | PotentialKind::Pen => return Conductor::Protective,
            PotentialKind::Line => return Conductor::Line(p.name.clone()),
            PotentialKind::Neutral => return Conductor::Neutral,
            PotentialKind::DcPositive => return Conductor::DcPositive,
            PotentialKind::DcNegative => return Conductor::DcNegative,
            PotentialKind::Signal => {}
        }
    }
    let protective_pin = wire.ends().iter().any(|&end| {
        end.as_pin()
            .and_then(|pin| project.symbol(pin.symbol)?.pins.get(usize::from(pin.pin)))
            .is_some_and(|pin| is_protective(&pin.name))
            || ends::designation(project, end)
                .and_then(|d| d.rsplit(':').next().map(is_protective))
                .unwrap_or(false)
    });
    if protective_pin || wire.color.as_deref().is_some_and(is_protective) {
        return Conductor::Protective;
    }
    let name = match (&wire.number, potential) {
        (Some(number), _) => number.clone(),
        (None, Some(p)) => p.name.clone(),
        (None, None) => String::new(),
    };
    Conductor::Signal(name)
}

/// Puts conductors on the cores of a type. PE only ever goes on the
/// green-yellow core and nothing else does; on colour-coded types lines
/// take BN, BK, GY and neutral BU, on numbered types every conductor takes
/// the next free number. `None` when the conductors do not fit.
pub fn assign(conductors: &[Conductor], cable_type: &CableType) -> Option<Vec<usize>> {
    let colour = !cable_type.is_numbered();
    let mut used = vec![false; cable_type.cores.len()];
    let mut out: Vec<Option<usize>> = vec![None; conductors.len()];
    let find = |used: &[bool], name: &str| {
        cable_type
            .cores
            .iter()
            .enumerate()
            .position(|(i, c)| !used[i] && c.eq_ignore_ascii_case(name))
    };
    // Pass 1: cores whose colour or GNYE says what they carry.
    let mut lines = 0;
    for (i, conductor) in conductors.iter().enumerate() {
        let wanted = match conductor {
            Conductor::Protective => Some(GNYE),
            Conductor::Line(_) if colour => {
                lines += 1;
                Some(*["BN", "BK", "GY"].get(lines - 1)?)
            }
            Conductor::Neutral if colour => Some("BU"),
            _ => None,
        };
        if let Some(name) = wanted {
            let core = find(&used, name)?;
            used[core] = true;
            out[i] = Some(core);
        }
    }
    // Pass 2: everything else on the next free ordinary core.
    for (i, slot) in out.iter_mut().enumerate() {
        if slot.is_some() {
            continue;
        }
        debug_assert!(!matches!(conductors[i], Conductor::Protective));
        let core = (0..cable_type.cores.len())
            .find(|&c| !used[c] && !is_protective(&cable_type.cores[c]))?;
        used[core] = true;
        *slot = Some(core);
    }
    out.into_iter().collect()
}

/// The smallest usual type the conductors fit, and where each goes.
pub fn choose_type(conductors: &[Conductor], cross_section_mm2: f64) -> (CableType, Vec<usize>) {
    let pe = conductors.contains(&Conductor::Protective);
    let needed = conductors.len();
    let supply = conductors.iter().all(|c| {
        matches!(
            c,
            Conductor::Line(_) | Conductor::Neutral | Conductor::Protective
        )
    });
    for count in (needed.max(2)..=5).filter(|_| supply) {
        let t = CableType::standard(count, pe, cross_section_mm2);
        // No colour-coded type of this size (there is no 2G): try the next.
        if t.is_numbered() {
            continue;
        }
        if let Some(cores) = assign(conductors, &t) {
            return (t, cores);
        }
    }
    // Numbered: one GNYE at most; further PE conductors cannot share it,
    // so they are counted as ordinary cores and flagged by the checks.
    let count = NUMBERED_SIZES
        .iter()
        .copied()
        .find(|&n| n >= needed)
        .unwrap_or(needed);
    let t = CableType::numbered(count, pe, cross_section_mm2);
    if let Some(cores) = assign(conductors, &t) {
        return (t, cores);
    }
    let fallback: Vec<Conductor> = conductors
        .iter()
        .enumerate()
        .map(|(i, c)| match c {
            Conductor::Protective if conductors[..i].contains(&Conductor::Protective) => {
                Conductor::Signal(String::new())
            }
            other => other.clone(),
        })
        .collect();
    let cores = assign(&fallback, &t).expect("a numbered type with room fits any conductors");
    (t, cores)
}

/// The cross-section a cable for these wires needs: the largest wire
/// cross-section, rounded up to a standard size; 1.5 mm² for supply
/// conductors and 1 mm² for signals when no wire says.
fn cross_section(wires: &[&Wire], conductors: &[Conductor]) -> f64 {
    let largest = wires
        .iter()
        .filter_map(|w| w.cross_section_mm2)
        .fold(None, |acc: Option<f64>, s| {
            Some(acc.map_or(s, |a| a.max(s)))
        });
    match largest {
        Some(s) => standard_cross_section(s),
        None if conductors.iter().all(|c| matches!(c, Conductor::Signal(_))) => 1.0,
        None => 1.5,
    }
}

/// Cables to propose: wires that cross a location boundary, run straight
/// from pin to pin, are on no cable and were not declined, grouped by the
/// two devices they connect. Ordered by cabinet-side device tag, then
/// field-side tag.
pub fn propose_cables(project: &Project) -> Vec<CableProposal> {
    let netlist = project.netlist();
    // (cabinet device, field device) -> wires
    let mut groups: BTreeMap<(DeviceId, DeviceId), Vec<&Wire>> = BTreeMap::new();
    let mut locations: BTreeMap<DeviceId, Vec<String>> = BTreeMap::new();
    for wire in project.wires() {
        if project.cable_of_wire(wire.id).is_some()
            || matches!(wire.properties.get(DISMISS_KEY), Some(Value::Bool(false)))
        {
            continue;
        }
        let [from, to] = wire.ends();
        let (Some(da), Some(db)) = (ends::owner(project, from), ends::owner(project, to)) else {
            continue;
        };
        let (Some(la), Some(lb)) = (ends::location(project, from), ends::location(project, to))
        else {
            continue;
        };
        if la == lb {
            continue;
        }
        let (ra, rb) = (ends::rank(project, &la), ends::rank(project, &lb));
        let key = if ra > rb || (ra == rb && da < db) {
            (da, db)
        } else {
            (db, da)
        };
        locations.insert(da, la);
        locations.insert(db, lb);
        groups.entry(key).or_default().push(wire);
    }

    let tag_of = |d: DeviceId| project.device(d).map(|d| d.tag.clone()).unwrap_or_default();
    let mut keys: Vec<(DeviceId, DeviceId)> = groups.keys().copied().collect();
    keys.sort_by(|a, b| {
        natural_cmp(&tag_of(a.0).to_string(), &tag_of(b.0).to_string())
            .then_with(|| natural_cmp(&tag_of(a.1).to_string(), &tag_of(b.1).to_string()))
            .then(a.cmp(b))
    });

    let mut next_numbers: BTreeMap<DeviceTag, u32> = BTreeMap::new();
    let mut out = Vec::new();
    for key in keys {
        let mut wires = groups.remove(&key).unwrap_or_default();
        let cabinet_end = |w: &Wire| {
            let end = if ends::owner(project, w.from) == Some(key.0) {
                w.from
            } else {
                w.to
            };
            ends::designation(project, end).unwrap_or_default()
        };
        let mut items: Vec<(Conductor, &Wire)> = wires
            .drain(..)
            .map(|w| (conductor(project, &netlist, w), w))
            .collect();
        items.sort_by(|(ca, wa), (cb, wb)| {
            ca.order(cb)
                .then_with(|| natural_cmp(&cabinet_end(wa), &cabinet_end(wb)))
                .then(wa.id.cmp(&wb.id))
        });
        let conductors: Vec<Conductor> = items.iter().map(|(c, _)| c.clone()).collect();
        let wires: Vec<&Wire> = items.iter().map(|(_, w)| *w).collect();
        let (cable_type, cores) = choose_type(&conductors, cross_section(&wires, &conductors));

        let structure = DeviceTag {
            function: tag_of(key.0).function,
            ..DeviceTag::default()
        };
        let number = next_numbers
            .entry(structure.clone())
            .or_insert_with(|| project.next_tag_number(&structure, "W"));
        let mut tag = structure.clone();
        tag.product = vec![format!("W{number}")];
        *number += 1;

        let side = |d: DeviceId| CableSide {
            device: d,
            location: locations.get(&d).cloned().unwrap_or_default(),
        };
        out.push(CableProposal {
            tag,
            ends: [side(key.0), side(key.1)],
            wires: wires.iter().map(|w| w.id).collect(),
            conductors,
            cable_type,
            cores,
        });
    }
    out
}

/// A cable for wires the user picked (a line drawn over them), whatever
/// their locations: the smallest usual type they fit, the next free `-W`
/// tag in the function of the first device a wire lands on, and each wire
/// on its core. `None` when no picked wire exists or none lands on a
/// device.
pub fn proposal_for_wires(project: &Project, wires: &[WireId]) -> Option<CableProposal> {
    let netlist = project.netlist();
    let mut items: Vec<(Conductor, &Wire)> = wires
        .iter()
        .filter_map(|&id| project.wire(id))
        .map(|w| (conductor(project, &netlist, w), w))
        .collect();
    items.sort_by(|(ca, wa), (cb, wb)| ca.order(cb).then(wa.id.cmp(&wb.id)));
    let conductors: Vec<Conductor> = items.iter().map(|(c, _)| c.clone()).collect();
    let picked: Vec<&Wire> = items.iter().map(|(_, w)| *w).collect();
    let owners: Vec<DeviceId> = picked
        .iter()
        .flat_map(|w| w.ends())
        .filter_map(|end| ends::owner(project, end))
        .collect();
    let first = *owners.first()?;
    let second = owners
        .iter()
        .copied()
        .find(|&d| d != first)
        .unwrap_or(first);
    let (cable_type, cores) = choose_type(&conductors, cross_section(&picked, &conductors));
    let structure = DeviceTag {
        function: project
            .device(first)
            .map(|d| d.tag.function.clone())
            .unwrap_or_default(),
        ..DeviceTag::default()
    };
    let mut tag = structure.clone();
    tag.product = vec![format!("W{}", project.next_tag_number(&structure, "W"))];
    let side = |d: DeviceId| CableSide {
        device: d,
        location: owners
            .iter()
            .position(|&o| o == d)
            .and_then(|_| {
                picked
                    .iter()
                    .flat_map(|w| w.ends())
                    .find(|&end| ends::owner(project, end) == Some(d))
                    .and_then(|end| ends::location(project, end))
            })
            .unwrap_or_default(),
    };
    Some(CableProposal {
        tag,
        ends: [side(first), side(second)],
        wires: picked.iter().map(|w| w.id).collect(),
        conductors,
        cable_type,
        cores,
    })
}

/// Accepts proposals: creates each cable with its wires on their cores and,
/// when the catalog has a part of that type, the part. Run inside one
/// transaction so accepting all is one undo step.
pub fn accept_proposals(
    tx: &mut Tx<'_>,
    proposals: &[CableProposal],
    catalog: Option<&Catalog>,
) -> Result<Vec<CableId>> {
    let mut created = Vec::with_capacity(proposals.len());
    for proposal in proposals {
        let mut cores = proposal.cable_type.build_cores();
        for (&wire, &core) in proposal.wires.iter().zip(&proposal.cores) {
            if let Some(slot) = cores.get_mut(core) {
                slot.wire = Some(wire);
            }
        }
        let cable = tx.add_cable(proposal.tag.clone(), cores)?;
        let name = proposal.cable_type.name();
        let part = match catalog.and_then(|c| proposal.cable_type.find_part(c)) {
            Some(part) => Some(match find_project_part(tx.project(), &part.key()) {
                Some(id) => id,
                None => tx.insert(|id| part.to_model(id))?,
            }),
            None => None,
        };
        tx.update::<novo_core::Cable>(cable, |c| {
            c.part = part;
            c.description = name;
        })?;
        created.push(cable);
    }
    Ok(created)
}

/// Declines a proposal for good (undoable): its wires are not proposed
/// again until [`restore_proposal`].
pub fn dismiss_proposal(tx: &mut Tx<'_>, proposal: &CableProposal) -> Result<()> {
    for &wire in &proposal.wires {
        tx.set_property(wire, DISMISS_KEY, Some(Value::Bool(false)))?;
    }
    Ok(())
}

/// The wires whose proposal was declined and not restored.
pub fn declined_wires(project: &Project) -> Vec<WireId> {
    project
        .wires()
        .filter(|w| matches!(w.properties.get(DISMISS_KEY), Some(Value::Bool(false))))
        .map(|w| w.id)
        .collect()
}

/// Forgets a declined proposal on these wires.
pub fn restore_proposal(tx: &mut Tx<'_>, wires: &[WireId]) -> Result<()> {
    for &wire in wires {
        let declined = tx
            .project()
            .wire(wire)
            .is_some_and(|w| w.properties.contains_key(DISMISS_KEY));
        if declined {
            tx.set_property(wire, DISMISS_KEY, None)?;
        }
    }
    Ok(())
}

/// One line for the "accept all" summary: `2 cables: =A1-W1 4G1.5,
/// =A1-W2 7x1`.
pub fn summarize_proposals(proposals: &[CableProposal]) -> String {
    let list: Vec<String> = proposals
        .iter()
        .map(|p| format!("{} {}", p.tag, p.cable_type.name()))
        .collect();
    crate::strings::cables_proposed(proposals.len(), &list.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_and_pe_get_a_colour_coded_3g() {
        let (t, cores) = choose_type(&[Conductor::Line("L".into()), Conductor::Protective], 1.5);
        assert_eq!(t.cores, ["BN", "BU", "GNYE"]);
        assert!(!t.is_numbered());
        assert_eq!(cores, [0, 2]);
    }
}
