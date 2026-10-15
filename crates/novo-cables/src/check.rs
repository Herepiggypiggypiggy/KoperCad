//! Cable checks: cores used twice over, cables too small for their wires,
//! green-yellow cores on the wrong conductor and shields left floating.

use std::collections::BTreeSet;

use novo_core::{CableId, DeviceId, Netlist, Project, Value, WireId};

use crate::edit::{ShieldConnection, is_spare, shield_connection};
use crate::ends;
use crate::kind::is_protective;
use crate::propose::{Conductor, DISMISS_KEY, conductor};
use crate::strings;

/// One problem found on a cable.
#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    /// A core marked spare carries a wire.
    SpareInUse { cable: CableId, core: usize },
    /// The green-yellow core carries something other than PE (IEC 60445).
    ProtectiveCoreMisused {
        cable: CableId,
        core: usize,
        wire: WireId,
    },
    /// A PE conductor is on a core that is not green-yellow.
    ProtectiveOnOtherCore {
        cable: CableId,
        core: usize,
        wire: WireId,
    },
    /// More wires run between the cable's two ends than it has free cores.
    CoreCountExceeded {
        cable: CableId,
        /// Wires between the same two devices that are on no cable.
        unassigned: Vec<WireId>,
        free: usize,
    },
    /// The cable has more conductors than its part.
    MoreCoresThanPart {
        cable: CableId,
        cores: usize,
        part_cores: usize,
    },
    /// A shield core with no connection.
    ShieldUnconnected { cable: CableId, core: usize },
    /// A core thinner than the wire it carries.
    UndersizedCore {
        cable: CableId,
        core: usize,
        wire: WireId,
    },
}

impl Finding {
    pub fn cable(&self) -> CableId {
        match self {
            Finding::SpareInUse { cable, .. }
            | Finding::ProtectiveCoreMisused { cable, .. }
            | Finding::ProtectiveOnOtherCore { cable, .. }
            | Finding::CoreCountExceeded { cable, .. }
            | Finding::MoreCoresThanPart { cable, .. }
            | Finding::ShieldUnconnected { cable, .. }
            | Finding::UndersizedCore { cable, .. } => *cable,
        }
    }

    /// A sentence for the design check list.
    pub fn message(&self, project: &Project) -> String {
        let cable = project.cable(self.cable());
        let tag = cable.map(|c| c.tag.to_string()).unwrap_or_default();
        let core_name = |core: usize| {
            cable
                .and_then(|c| c.cores.get(core))
                .map(|c| c.name.clone())
                .unwrap_or_default()
        };
        match self {
            Finding::SpareInUse { core, .. } => strings::spare_in_use(&tag, &core_name(*core)),
            Finding::ProtectiveCoreMisused { core, .. } => {
                strings::protective_core_misused(&tag, &core_name(*core))
            }
            Finding::ProtectiveOnOtherCore { core, .. } => {
                strings::protective_on_other_core(&tag, &core_name(*core))
            }
            Finding::CoreCountExceeded {
                unassigned, free, ..
            } => strings::core_count_exceeded(&tag, unassigned.len(), *free),
            Finding::MoreCoresThanPart {
                cores, part_cores, ..
            } => strings::more_cores_than_part(&tag, *cores, *part_cores),
            Finding::ShieldUnconnected { core, .. } => {
                strings::shield_unconnected(&tag, &core_name(*core))
            }
            Finding::UndersizedCore { core, .. } => {
                strings::undersized_core(&tag, &core_name(*core))
            }
        }
    }
}

/// The two devices a cable's wires connect, when they all agree.
fn cable_devices(project: &Project, cable: CableId) -> Option<BTreeSet<DeviceId>> {
    let c = project.cable(cable)?;
    let mut pair: Option<BTreeSet<DeviceId>> = None;
    for wire in c.cores.iter().filter(|c| !c.shield).filter_map(|c| c.wire) {
        let w = project.wire(wire)?;
        let set: BTreeSet<DeviceId> = w
            .ends()
            .iter()
            .filter_map(|&e| ends::owner(project, e))
            .collect();
        if set.len() != 2 || pair.as_ref().is_some_and(|p| *p != set) {
            return None;
        }
        pair = Some(set);
    }
    pair
}

/// Checks every cable, in cable order.
pub fn check(project: &Project) -> Vec<Finding> {
    check_with(project, &project.netlist())
}

/// [`check`] with a netlist the caller already built from `project`, so a
/// caller running several checks builds it once.
pub fn check_with(project: &Project, netlist: &Netlist) -> Vec<Finding> {
    let mut findings = Vec::new();
    for c in project.cables() {
        let cable = c.id;
        for (core, slot) in c.cores.iter().enumerate() {
            if slot.shield {
                // A shield on a terminal that was deleted floats again.
                let connected = match shield_connection(c, core) {
                    Some(ShieldConnection::Terminal(t)) => project.terminal(t).is_some(),
                    other => other.is_some(),
                };
                if !connected {
                    findings.push(Finding::ShieldUnconnected { cable, core });
                }
                continue;
            }
            let Some(wire) = slot.wire.and_then(|w| project.wire(w)) else {
                continue;
            };
            if is_spare(c, core) {
                findings.push(Finding::SpareInUse { cable, core });
            }
            let protective = conductor(project, netlist, wire) == Conductor::Protective;
            match (is_protective(&slot.name), protective) {
                (true, false) => findings.push(Finding::ProtectiveCoreMisused {
                    cable,
                    core,
                    wire: wire.id,
                }),
                (false, true) => findings.push(Finding::ProtectiveOnOtherCore {
                    cable,
                    core,
                    wire: wire.id,
                }),
                _ => {}
            }
            if let (Some(core_mm2), Some(wire_mm2)) =
                (slot.cross_section_mm2, wire.cross_section_mm2)
                && core_mm2 + 1e-9 < wire_mm2
            {
                findings.push(Finding::UndersizedCore {
                    cable,
                    core,
                    wire: wire.id,
                });
            }
        }

        let conductors = c.cores.iter().filter(|c| !c.shield).count();
        if let Some(part) = c.part.and_then(|p| project.part(p))
            && let Some(Value::Text(list)) = part.properties.get(novo_parts::keys::CORES)
        {
            let part_cores = list.split(',').filter(|s| !s.trim().is_empty()).count();
            if part_cores > 0 && conductors > part_cores {
                findings.push(Finding::MoreCoresThanPart {
                    cable,
                    cores: conductors,
                    part_cores,
                });
            }
        }

        if let Some(devices) = cable_devices(project, cable) {
            let unassigned: Vec<WireId> = project
                .wires()
                .filter(|w| project.cable_of_wire(w.id).is_none())
                .filter(|w| !matches!(w.properties.get(DISMISS_KEY), Some(Value::Bool(false))))
                .filter(|w| {
                    let set: BTreeSet<DeviceId> = w
                        .ends()
                        .iter()
                        .filter_map(|&e| ends::owner(project, e))
                        .collect();
                    set == devices
                })
                .map(|w| w.id)
                .collect();
            let free = c
                .cores
                .iter()
                .enumerate()
                .filter(|(i, slot)| !slot.shield && slot.wire.is_none() && !is_spare(c, *i))
                .count();
            if unassigned.len() > free {
                findings.push(Finding::CoreCountExceeded {
                    cable,
                    unassigned,
                    free,
                });
            }
        }
    }
    findings
}
