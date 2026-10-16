//! Editing a cable's cores: put wires on cores, take them off, swap two
//! cores, mark spares, add a shield and say where it is connected.
//!
//! Every edit runs inside a [`Tx`], so a sequence of them is one undo step.
//!
//! Spare marks and shield connections live in cable properties for now
//! (`cable.spare`, `cable.shield.<core>`): `CableCore` has no properties of
//! its own. The request to WS-02 for `CableCore::spare` and a typed shield
//! connection is in `handoff/ws12/ROADMAP.md`.

use novo_core::{Cable, CableId, ModelError, Project, Result, TerminalId, Tx, Value, WireId};

use crate::kind::{CableType, SHIELD};
use crate::propose::{assign, conductor};

/// Cable property listing the cores marked spare, by name: `3,5`.
pub const SPARE_KEY: &str = "cable.spare";
/// Prefix of the cable property recording where a shield core is
/// connected: `cable.shield.SH = "pe"` or `"terminal:42"`.
pub const SHIELD_KEY_PREFIX: &str = "cable.shield.";

/// Where a shield is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldConnection {
    /// To protective earth at the cable gland or PE rail.
    Pe,
    /// To a terminal (a shield terminal on the strip).
    Terminal(TerminalId),
    /// Through a drawn wire on the shield core.
    Wire(WireId),
}

fn cable_of(project: &Project, cable: CableId) -> Result<&Cable> {
    project
        .cable(cable)
        .ok_or(ModelError::NotFound(cable.into()))
}

fn check_core(project: &Project, cable: CableId, core: usize) -> Result<()> {
    if core < cable_of(project, cable)?.cores.len() {
        Ok(())
    } else {
        Err(ModelError::InvalidCore { cable, core })
    }
}

/// Puts a wire on a core. A wire already on another core of the same cable
/// moves; a wire on another cable is refused (take it off there first).
/// The core's previous wire, if any, comes off.
pub fn assign_core(tx: &mut Tx<'_>, cable: CableId, core: usize, wire: WireId) -> Result<()> {
    check_core(tx.project(), cable, core)?;
    if tx.project().wire(wire).is_none() {
        return Err(ModelError::NotFound(wire.into()));
    }
    tx.update::<Cable>(cable, |c| {
        for slot in &mut c.cores {
            if slot.wire == Some(wire) {
                slot.wire = None;
            }
        }
        c.cores[core].wire = Some(wire);
    })
}

/// Takes the wire off a core; the core becomes free.
pub fn unassign_core(tx: &mut Tx<'_>, cable: CableId, core: usize) -> Result<()> {
    check_core(tx.project(), cable, core)?;
    tx.assign_core(cable, core, None)
}

/// Takes a wire off whatever core carries it. Does nothing for a wire on
/// no cable.
pub fn unassign_wire(tx: &mut Tx<'_>, wire: WireId) -> Result<()> {
    match tx.project().core_of_wire(wire) {
        Some((cable, core)) => tx.assign_core(cable, core, None),
        None => Ok(()),
    }
}

/// Swaps the wires of two cores of one cable (BN and BK after a phase
/// rotation check).
pub fn swap_cores(tx: &mut Tx<'_>, cable: CableId, a: usize, b: usize) -> Result<()> {
    check_core(tx.project(), cable, a)?;
    check_core(tx.project(), cable, b)?;
    if a == b {
        return Ok(());
    }
    tx.update::<Cable>(cable, |c| {
        let wa = c.cores[a].wire;
        c.cores[a].wire = c.cores[b].wire;
        c.cores[b].wire = wa;
    })
}

/// Names of the cores marked spare.
pub fn spare_cores(cable: &Cable) -> Vec<String> {
    match cable.properties.get(SPARE_KEY) {
        Some(Value::Text(text)) => text
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Whether a core is marked spare.
pub fn is_spare(cable: &Cable, core: usize) -> bool {
    cable
        .cores
        .get(core)
        .is_some_and(|c| spare_cores(cable).contains(&c.name))
}

/// Marks a core spare (kept free on purpose, shown as spare on the cable
/// screen) or clears the mark. A spare core that still carries a wire is
/// flagged by [`check`](crate::check::check).
pub fn set_spare(tx: &mut Tx<'_>, cable: CableId, core: usize, spare: bool) -> Result<()> {
    check_core(tx.project(), cable, core)?;
    let record = cable_of(tx.project(), cable)?;
    let name = record.cores[core].name.clone();
    let mut list = spare_cores(record);
    list.retain(|s| *s != name);
    if spare {
        list.push(name);
        let order: Vec<&str> = record.cores.iter().map(|c| c.name.as_str()).collect();
        list.sort_by_key(|s| order.iter().position(|o| o == s));
    }
    let value = (!list.is_empty()).then(|| Value::Text(list.join(",")));
    tx.set_property(cable, SPARE_KEY, value)
}

/// Adds an overall shield core (`SH`, or `SH2`... when the cable has one)
/// and returns its index.
pub fn add_shield(tx: &mut Tx<'_>, cable: CableId) -> Result<usize> {
    let record = cable_of(tx.project(), cable)?;
    let taken = |name: &str| record.cores.iter().any(|c| c.name == name);
    let name = std::iter::once(SHIELD.to_owned())
        .chain((2..).map(|n| format!("{SHIELD}{n}")))
        .find(|n| !taken(n))
        .expect("an unused shield name exists");
    let index = record.cores.len();
    tx.update::<Cable>(cable, |c| {
        let mut core = novo_core::CableCore::new(name);
        core.shield = true;
        c.cores.push(core);
    })?;
    Ok(index)
}

/// Where a shield core is connected: a wire on it, else the recorded
/// connection.
pub fn shield_connection(cable: &Cable, core: usize) -> Option<ShieldConnection> {
    let c = cable.cores.get(core)?;
    if !c.shield {
        return None;
    }
    if let Some(wire) = c.wire {
        return Some(ShieldConnection::Wire(wire));
    }
    match cable.properties.get(&shield_key(&c.name)) {
        Some(Value::Text(text)) if text == "pe" => Some(ShieldConnection::Pe),
        Some(Value::Text(text)) => text
            .strip_prefix("terminal:")
            .and_then(|raw| raw.parse::<u64>().ok())
            .map(|raw| ShieldConnection::Terminal(TerminalId::from(raw))),
        _ => None,
    }
}

fn shield_key(core_name: &str) -> String {
    format!("{SHIELD_KEY_PREFIX}{core_name}")
}

/// Connects a shield core to PE, a terminal or a wire, or disconnects it
/// with `None`. Refused for a core that is not a shield.
pub fn connect_shield(
    tx: &mut Tx<'_>,
    cable: CableId,
    core: usize,
    connection: Option<ShieldConnection>,
) -> Result<()> {
    check_core(tx.project(), cable, core)?;
    let record = cable_of(tx.project(), cable)?;
    let c = &record.cores[core];
    if !c.shield {
        return Err(ModelError::InvalidValue {
            id: cable.into(),
            field: "shield",
        });
    }
    let key = shield_key(&c.name);
    let had_wire = c.wire.is_some();
    let value = match connection {
        Some(ShieldConnection::Pe) => Some(Value::Text("pe".into())),
        Some(ShieldConnection::Terminal(t)) => {
            if tx.project().terminal(t).is_none() {
                return Err(ModelError::NotFound(t.into()));
            }
            Some(Value::Text(format!("terminal:{}", t.raw())))
        }
        Some(ShieldConnection::Wire(_)) | None => None,
    };
    if let Some(ShieldConnection::Wire(wire)) = connection {
        assign_core(tx, cable, core, wire)?;
    } else if had_wire {
        tx.assign_core(cable, core, None)?;
    }
    let present = cable_of(tx.project(), cable)?.properties.contains_key(&key);
    if value.is_some() || present {
        tx.set_property(cable, key, value)?;
    }
    Ok(())
}

/// Renames a cable (`-W2`). Refused when another cable already has the
/// tag.
pub fn rename_cable(tx: &mut Tx<'_>, cable: CableId, tag: novo_core::DeviceTag) -> Result<()> {
    cable_of(tx.project(), cable)?;
    if tx.project().cables().any(|c| c.id != cable && c.tag == tag) {
        return Err(ModelError::InvalidValue {
            id: cable.into(),
            field: "tag",
        });
    }
    tx.update::<Cable>(cable, |c| c.tag = tag)
}

/// Deletes a cable. Its wires stay, free of any core; they are proposed as
/// a cable again when they cross a location boundary.
pub fn delete_cable(tx: &mut Tx<'_>, cable: CableId) -> Result<()> {
    cable_of(tx.project(), cable)?;
    tx.remove(cable)
}

/// Gives a cable another type: new cores, the wires put on them the way a
/// proposal would (PE on green-yellow, lines on BN BK GY). The part is
/// cleared, since it was for the old type, and spare marks stay on cores
/// that keep their name.
///
/// Like a proposal, this rebuilds the cores: hand-made core assignments
/// (swaps) and per-core overrides are reset. Shields that carry a wire or a
/// recorded connection are kept in order (the type's shield first, further
/// ones as extra shield cores with their names). Refused, changing nothing,
/// when the wires do not fit the new type, or when a connected shield has
/// no place on it (a shielded cable to an unshielded type).
pub fn retype_cable(tx: &mut Tx<'_>, cable: CableId, cable_type: &CableType) -> Result<()> {
    let project = tx.project();
    let record = cable_of(project, cable)?;
    let netlist = project.netlist();
    let mut wires = Vec::new();
    let mut conductors = Vec::new();
    // Shields in use: name, wire, recorded connection value.
    let mut shields: Vec<(String, Option<WireId>, Option<Value>)> = Vec::new();
    for (index, core) in record.cores.iter().enumerate() {
        if core.shield {
            let recorded = record.properties.get(&shield_key(&core.name)).cloned();
            let connected = shield_connection(record, index).is_some();
            if core.wire.is_some() || connected || recorded.is_some() {
                shields.push((core.name.clone(), core.wire, recorded));
            }
        } else if let Some(wire) = core.wire
            && let Some(w) = project.wire(wire)
        {
            wires.push(wire);
            conductors.push(conductor(project, &netlist, w));
        }
    }
    if !shields.is_empty() && !cable_type.shield {
        return Err(ModelError::InvalidValue {
            id: cable.into(),
            field: "cable type",
        });
    }
    let Some(cores) = assign(&conductors, cable_type) else {
        return Err(ModelError::InvalidValue {
            id: cable.into(),
            field: "cable type",
        });
    };
    let mut built = cable_type.build_cores();
    for (wire, core) in wires.into_iter().zip(cores) {
        built[core].wire = Some(wire);
    }
    // Shield i goes on the type's shield (the first), further ones on extra
    // shield cores that keep their names.
    let mut shield_props: Vec<(String, Option<Value>)> = Vec::new();
    for (i, (old_name, wire, recorded)) in shields.into_iter().enumerate() {
        let at = if i == 0 {
            built.iter().position(|c| c.shield)
        } else {
            None
        };
        let at = at.unwrap_or_else(|| {
            let mut extra = novo_core::CableCore::new(old_name);
            extra.shield = true;
            built.push(extra);
            built.len() - 1
        });
        built[at].wire = wire;
        shield_props.push((built[at].name.clone(), recorded));
    }
    let stale: Vec<String> = record
        .properties
        .keys()
        .filter(|k| k.starts_with(SHIELD_KEY_PREFIX))
        .cloned()
        .collect();
    let spare: Vec<String> = spare_cores(record)
        .into_iter()
        .filter(|name| built.iter().any(|c| c.name == *name))
        .collect();
    let name = cable_type.name();
    for key in stale {
        tx.set_property(cable, key, None)?;
    }
    for (core_name, recorded) in shield_props {
        if recorded.is_some() {
            tx.set_property(cable, shield_key(&core_name), recorded)?;
        }
    }
    tx.update::<Cable>(cable, |c| {
        c.cores = built;
        c.part = None;
        c.description = name;
    })?;
    let value = (!spare.is_empty()).then(|| Value::Text(spare.join(",")));
    tx.set_property(cable, SPARE_KEY, value)
}
