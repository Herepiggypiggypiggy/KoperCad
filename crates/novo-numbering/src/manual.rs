//! Hand-typed numbers: set on a whole net, locked against the engine.

use novo_core::{ModelError, Result, Tx, Value, Wire, WireId};

use crate::group::{LOCKED, groups};
use crate::scheme::Scheme;

/// Gives the net of `wire` the number `number` and locks it, so no fill or
/// renumber changes it. An empty number clears the net's number and the
/// lock. Refused when another net already carries the number (a bare PE or N
/// name excepted). Returns the wires changed.
pub fn set_number(tx: &mut Tx<'_>, wire: WireId, number: &str) -> Result<Vec<WireId>> {
    let number = number.trim();
    let scheme = Scheme::load(tx.project());
    let netlist = tx.project().netlist();
    let all = groups(tx.project(), &netlist);
    let Some(own) = all.iter().position(|g| g.wires.contains(&wire)) else {
        return Err(ModelError::NotFound(wire.into()));
    };
    if !number.is_empty() {
        let clash = all.iter().enumerate().any(|(i, g)| {
            i != own
                && g.numbers.iter().any(|(n, _)| n == number)
                && !(g.shares(number, &scheme) && all[own].shares(number, &scheme))
        });
        if clash {
            return Err(ModelError::Refused {
                what: "Wire number".into(),
                reason: format!("{number} is already used on another net"),
            });
        }
    }
    let wires = all[own].wires.clone();
    for &id in &wires {
        tx.update::<Wire>(id, |w| {
            if number.is_empty() {
                w.number = None;
                w.properties.remove(LOCKED);
            } else {
                w.number = Some(number.to_owned());
                w.properties.insert(LOCKED.into(), Value::Bool(true));
            }
        })?;
    }
    Ok(wires)
}

/// Releases the lock on the net of `wire`; the number stays until the next
/// renumber.
pub fn unlock(tx: &mut Tx<'_>, wire: WireId) -> Result<()> {
    let netlist = tx.project().netlist();
    let all = groups(tx.project(), &netlist);
    let Some(g) = all.iter().find(|g| g.wires.contains(&wire)) else {
        return Err(ModelError::NotFound(wire.into()));
    };
    for &id in &g.wires.clone() {
        tx.update::<Wire>(id, |w| {
            w.properties.remove(LOCKED);
        })?;
    }
    Ok(())
}

/// Whether the wire's net holds a hand-typed number.
pub fn is_locked(project: &novo_core::Project, wire: WireId) -> bool {
    project
        .wire(wire)
        .is_some_and(|w| matches!(w.properties.get(LOCKED), Some(Value::Bool(true))))
}
