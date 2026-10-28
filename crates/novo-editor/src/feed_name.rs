//! The potential a new feed carries when nobody said which (DESIGN.md §2:
//! what has one right answer happens on its own, and what does not never
//! merges anything by accident).
//!
//! A feed placed without a potential gets a *provisional* one, a fresh `L`
//! name that no potential in the project has, so it can never join an
//! unrelated conductor by chance. It stays provisional until it is attached
//! to something that tells what it is: a net that already carries a
//! potential (the feed takes it), a `PE` or `N` pin or terminal, the 24 V
//! and 0 V pins of a supply, or the return side of a coil. Then it takes
//! that potential, in the same step as the wire that attached it, and the
//! provisional name goes away. A feed attached to nothing that says
//! anything keeps its fresh name.
//!
//! The mark lives on the feed (`feed.provisional`, the name it was given),
//! so it survives saving; a feed whose potential a person changed since is
//! never touched again.

use novo_core::{
    Endpoint, Net, Netlist, PageId, PotentialId, PotentialKind, Project, Result, SymbolId,
    SymbolKey, SymbolRole, TerminalKind, Tx, Value,
};

use crate::edit::find_or_add_potential;
use crate::potential::{PotentialSpec, draws_as_earth};

/// The feed property that marks a provisional potential; its value is the
/// name the feed was given.
pub const PROVISIONAL: &str = "feed.provisional";

/// What a feed should carry.
enum Pick {
    Existing(PotentialId),
    Spec(PotentialSpec),
}

/// Marks `feed` as carrying `name` only provisionally.
pub(crate) fn mark(tx: &mut Tx<'_>, feed: SymbolId, name: &str) -> Result<()> {
    tx.set_property(feed, PROVISIONAL, Some(Value::from(name)))
}

/// Gives the provisional feeds on `page` the potential of what they are
/// attached to, where that says something. Cheap when there are none.
pub fn retarget(tx: &mut Tx<'_>, page: PageId) -> Result<()> {
    let project = tx.project();
    let provisional: Vec<(SymbolId, String)> = project
        .page_contents(page)
        .symbols
        .into_iter()
        .filter_map(|s| {
            let sym = project.symbol(s)?;
            match sym.properties.get(PROVISIONAL)? {
                Value::Text(name) => Some((s, name.clone())),
                _ => None,
            }
        })
        .collect();
    for (feed, name) in provisional {
        retarget_one(tx, feed, &name)?;
    }
    Ok(())
}

fn retarget_one(tx: &mut Tx<'_>, feed: SymbolId, name: &str) -> Result<()> {
    let project = tx.project();
    let Some(sym) = project.symbol(feed) else {
        return Ok(());
    };
    let SymbolRole::PotentialFeed { potential } = sym.role else {
        return tx.set_property(feed, PROVISIONAL, None);
    };
    // A person changed it since: theirs now.
    if project.potential(potential).is_none_or(|p| p.name != name) {
        return tx.set_property(feed, PROVISIONAL, None);
    }
    // Alone on its own net there is nothing to go by: leave it untouched
    // (no role writes, no second netlist).
    let alone = {
        let netlist = project.netlist();
        netlist
            .net_of(Endpoint::pin(feed, 0))
            .map(|n| netlist.net(n))
            .is_none_or(|net| net.endpoints.len() <= 1 && net.wires.is_empty())
    };
    if alone {
        return Ok(());
    }
    // The net as the rest of the drawing makes it, without this feed's own
    // provisional definition.
    tx.set_role(feed, SymbolRole::Unassigned)?;
    let pick = {
        let project = tx.project();
        infer(project, &project.netlist(), feed)
    };
    let Some(pick) = pick else {
        return tx.set_role(feed, SymbolRole::PotentialFeed { potential });
    };
    let target = match pick {
        Pick::Existing(id) => id,
        Pick::Spec(spec) => find_or_add_potential(tx, &spec.name, spec.kind, spec.voltage)?,
    };
    tx.set_role(feed, SymbolRole::PotentialFeed { potential: target })?;
    tx.set_property(feed, PROVISIONAL, None)?;
    if target != potential {
        retire(tx, feed, potential, target)?;
    }
    Ok(())
}

/// After a feed moved from the provisional potential to `target`: draws it
/// as earth or line as the potential needs, and drops the provisional
/// potential when nothing uses it any more.
fn retire(tx: &mut Tx<'_>, feed: SymbolId, old: PotentialId, target: PotentialId) -> Result<()> {
    let project = tx.project();
    let earth = project
        .potential(target)
        .is_some_and(|p| draws_as_earth(p.kind));
    if let Some(sym) = project.symbol(feed) {
        let key = sym.symbol.0.as_str();
        let base = key.strip_suffix("_pe").unwrap_or(key);
        let wanted = if earth {
            format!("{base}_pe")
        } else {
            base.to_owned()
        };
        if wanted != key {
            tx.set_symbol_key(feed, SymbolKey(wanted))?;
        }
    }
    if tx.project().referrers(old).next().is_none() {
        tx.remove(old)?;
    }
    Ok(())
}

fn infer(project: &Project, netlist: &Netlist, feed: SymbolId) -> Option<Pick> {
    let net = netlist.net(netlist.net_of(Endpoint::pin(feed, 0))?);
    // Alone on its own net: nothing to go by.
    if net.endpoints.len() <= 1 && net.wires.is_empty() {
        return None;
    }
    // What the net already carries: the feed takes it. Several (a short, a
    // switched net) say nothing.
    match net.potentials.as_slice() {
        [single] => return Some(Pick::Existing(*single)),
        [] => {}
        _ => return None,
    }
    hint(project, netlist, net)
}

fn spec(name: &str, kind: PotentialKind, voltage: Option<f64>) -> PotentialSpec {
    PotentialSpec::new(name, kind, voltage)
}

/// The one potential of `kind` the project has, if there is exactly one.
fn only_of(project: &Project, kind: PotentialKind) -> Option<PotentialId> {
    let mut of_kind = project.potentials().filter(|p| p.kind == kind);
    let first = of_kind.next()?;
    of_kind.next().is_none().then_some(first.id)
}

fn dc_positive(project: &Project) -> Pick {
    only_of(project, PotentialKind::DcPositive).map_or_else(
        || Pick::Spec(spec("24V", PotentialKind::DcPositive, Some(24.0))),
        Pick::Existing,
    )
}

fn dc_negative(project: &Project) -> Pick {
    only_of(project, PotentialKind::DcNegative).map_or_else(
        || Pick::Spec(spec("0V", PotentialKind::DcNegative, Some(0.0))),
        Pick::Existing,
    )
}

fn neutral(project: &Project) -> Pick {
    only_of(project, PotentialKind::Neutral).map_or_else(
        || Pick::Spec(spec("N", PotentialKind::Neutral, None)),
        Pick::Existing,
    )
}

fn earth(project: &Project) -> Pick {
    only_of(project, PotentialKind::ProtectiveEarth).map_or_else(
        || Pick::Spec(spec("PE", PotentialKind::ProtectiveEarth, None)),
        Pick::Existing,
    )
}

/// What the pins and terminals on the net say, strongest first: earth,
/// neutral, a supply's DC pins, the return of a coil, a device's own
/// phase pins.
fn hint(project: &Project, netlist: &Netlist, net: &Net) -> Option<Pick> {
    let mut best: Option<(u8, Pick)> = None;
    let mut offer = |rank: u8, pick: Pick| {
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, pick));
        }
    };
    for end in &net.endpoints {
        let Endpoint::Pin { symbol, pin } = *end else {
            continue;
        };
        let Some(sym) = project.symbol(symbol) else {
            continue;
        };
        if let Some(t) = sym.terminal_of_pin(pin).and_then(|t| project.terminal(t)) {
            match t.kind {
                TerminalKind::ProtectiveEarth => offer(0, earth(project)),
                TerminalKind::Neutral => offer(1, neutral(project)),
                _ => {}
            }
            continue;
        }
        if !matches!(sym.role, SymbolRole::Device { .. } | SymbolRole::Unassigned) {
            continue;
        }
        let Some(name) = sym.pins.get(usize::from(pin)).map(|p| p.name.as_str()) else {
            continue;
        };
        match name.to_uppercase().as_str() {
            "PE" => offer(0, earth(project)),
            "N" => offer(1, neutral(project)),
            "L+" | "+24V" | "24V" => offer(2, dc_positive(project)),
            "M" | "0V" | "L-" => offer(2, dc_negative(project)),
            "A2" if sym.symbol.0.contains("/coils/") => {
                offer(3, coil_return(project, netlist, symbol));
            }
            "L1" | "L2" | "L3" => offer(
                4,
                Pick::Spec(spec(&name.to_uppercase(), PotentialKind::Line, Some(400.0))),
            ),
            _ => {}
        }
    }
    best.map(|(_, pick)| pick)
}

/// The return of a coil: 0 V when the coil's other side is on a DC supply,
/// neutral when it is on a line; without that, 0 V in a project that has
/// one and no neutral, else neutral.
fn coil_return(project: &Project, netlist: &Netlist, coil: SymbolId) -> Pick {
    let supplied = project
        .symbol(coil)
        .and_then(|s| s.pin_index("A1"))
        .and_then(|a1| netlist.net_of(Endpoint::pin(coil, a1)))
        .and_then(|n| netlist.net(n).potential())
        .and_then(|p| project.potential(p))
        .map(|p| p.kind);
    match supplied {
        Some(PotentialKind::DcPositive) => dc_negative(project),
        Some(PotentialKind::Line) => neutral(project),
        _ => {
            let has = |k| project.potentials().any(|p| p.kind == k);
            if has(PotentialKind::DcNegative) && !has(PotentialKind::Neutral) {
                dc_negative(project)
            } else {
                neutral(project)
            }
        }
    }
}
