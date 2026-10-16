//! Wire groups: the wires that must carry one number.
//!
//! Numbers belong to the electrical node, not to a drawn segment: every wire
//! of a net carries the same number, across junctions and across pages
//! (through interruption points). A contact or fuse ends the net, so the
//! wire after it gets its own number, as on a printed wire list. So does a
//! device the net conducts through (an overload relay's heaters): the wire
//! into it and the wire out of it are two wires and carry two numbers.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use novo_core::{Netlist, PageId, PotentialKind, Project, Value, WireId, natural_cmp};

use crate::scheme::{Scheme, SchemeKind};

/// Wire property that marks a hand-typed number the engine never changes.
pub const LOCKED: &str = "wirenum.locked";

/// A set of wires that share one number.
#[derive(Debug, Clone)]
pub struct Group {
    /// The wires, in id order.
    pub wires: Vec<WireId>,
    /// Page of the first wire in reading order.
    pub page: PageId,
    pub page_name: String,
    pub column: u16,
    /// Position of the group's first point, for reading order.
    pub x: f64,
    pub y: f64,
    /// The single potential the net carries, with its kind.
    pub potential: Option<(String, PotentialKind)>,
    /// Cable tag and core of a wire in the group.
    pub cable: Option<(String, String)>,
    /// Function aspect of the page structure, joined (`A2`).
    pub function: String,
    /// A wire of the group holds a hand-typed number.
    pub locked: bool,
    /// The number most wires of the group carry (ties go to the lower).
    pub current: Option<String>,
    /// Every distinct number on the group's wires, with its wire count.
    pub numbers: Vec<(String, usize)>,
    /// Wires without a number.
    pub unnumbered: usize,
    pub(crate) page_rank: usize,
}

/// What a group's number is built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// Exactly this text.
    Fixed { text: String, shared: bool },
    /// `stem` followed by a count.
    Counter { stem: String },
}

impl Group {
    /// The target number of this group under a scheme.
    pub(crate) fn target(&self, scheme: &Scheme) -> Target {
        let sep = &scheme.separator;
        let pre = &scheme.prefix;
        let by_page = || Target::Counter {
            stem: format!("{pre}{}{sep}", self.page_name),
        };
        match scheme.kind {
            SchemeKind::Sequential => Target::Counter { stem: pre.clone() },
            SchemeKind::ByPage => by_page(),
            SchemeKind::ByColumn => Target::Counter {
                stem: format!("{pre}{}{sep}{}{sep}", self.page_name, self.column),
            },
            SchemeKind::ByPotential => match &self.potential {
                Some((name, kind)) if scheme.bare_potentials && is_bare(*kind) => Target::Fixed {
                    text: name.clone(),
                    shared: true,
                },
                Some((name, _)) => Target::Counter {
                    stem: format!("{pre}{name}{sep}"),
                },
                None => by_page(),
            },
            SchemeKind::ByCable => match &self.cable {
                Some((tag, core)) => Target::Fixed {
                    text: format!("{pre}{tag}{sep}{core}"),
                    shared: false,
                },
                None => by_page(),
            },
            SchemeKind::ByFunction if !self.function.is_empty() => Target::Counter {
                stem: format!("{pre}{}{sep}", self.function),
            },
            SchemeKind::ByFunction => by_page(),
        }
    }

    /// Whether repeating `number` on other nets is intended.
    pub(crate) fn shares(&self, number: &str, scheme: &Scheme) -> bool {
        scheme.bare_potentials
            && self
                .potential
                .as_ref()
                .is_some_and(|(name, kind)| is_bare(*kind) && name == number)
    }
}

fn is_bare(kind: PotentialKind) -> bool {
    matches!(
        kind,
        PotentialKind::ProtectiveEarth | PotentialKind::Neutral | PotentialKind::Pen
    )
}

/// The count a number carries after `stem`, when it is `stem` plus digits.
pub(crate) fn count_after(number: &str, stem: &str) -> Option<u32> {
    let rest = number.strip_prefix(stem)?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

/// Whether a number is one this target would produce.
pub(crate) fn fits(target: &Target, number: &str) -> bool {
    match target {
        Target::Fixed { text, .. } => text == number,
        Target::Counter { stem } => count_after(number, stem).is_some(),
    }
}

/// Every group of the project, in reading order: page order, then column,
/// then strand by strand (left to right), each strand top to bottom. A
/// three-phase feed numbers L1 1, 2, 3, then L2 4, 5, 6, not 1, 4, 5 across
/// the sheet.
pub fn groups(project: &Project, netlist: &Netlist) -> Vec<Group> {
    let mut pages: Vec<_> = project.pages().collect();
    pages.sort_by(|a, b| {
        a.structure
            .to_string()
            .cmp(&b.structure.to_string())
            .then_with(|| natural_cmp(&a.name, &b.name))
    });
    let rank: BTreeMap<PageId, usize> = pages.iter().enumerate().map(|(i, p)| (p.id, i)).collect();

    // Wires by net, plus every wire outside a net as its own group.
    let mut sets: Vec<Vec<WireId>> = netlist
        .nets()
        .iter()
        .filter(|n| !n.wires.is_empty())
        .flat_map(|n| split_at_device_bridges(project, &n.wires))
        .collect();
    let in_net: std::collections::HashSet<WireId> = sets.iter().flatten().copied().collect();
    sets.extend(
        project
            .wires()
            .filter(|w| !in_net.contains(&w.id))
            .map(|w| vec![w.id]),
    );

    let mut out: Vec<Group> = sets
        .into_iter()
        .filter_map(|mut ids| {
            ids.sort();
            build(project, netlist, ids, &rank)
        })
        .collect();
    out.sort_by(compare);
    out
}

/// Splits a net's wires where the net only continues through a device's
/// internal bridge (an overload relay's heaters): the wire into the device
/// and the wire out of it are two wires on the bench and need two numbers,
/// although the netlist conducts through the device. Terminals, interruption
/// points and potential feeds stay transparent, as do junctions.
fn split_at_device_bridges(project: &Project, wires: &[WireId]) -> Vec<Vec<WireId>> {
    use novo_core::{Endpoint, SymbolRole};
    use std::collections::HashMap;

    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }

    // The thing a wire end hangs on: wires that share a key are one wire.
    #[derive(Hash, PartialEq, Eq)]
    enum Key {
        Endpoint(Endpoint),
        Symbol(novo_core::SymbolId),
        Terminal(novo_core::TerminalId),
        Interruption(String),
    }
    let keys = |end: Endpoint| -> Vec<Key> {
        let Endpoint::Pin { symbol, .. } = end else {
            return vec![Key::Endpoint(end)];
        };
        let Some(sym) = project.symbol(symbol) else {
            return vec![Key::Endpoint(end)];
        };
        match &sym.role {
            SymbolRole::Device { .. } => vec![Key::Endpoint(end)],
            SymbolRole::Terminal { .. } => {
                let mut keys = vec![Key::Symbol(symbol)];
                if let Some(t) = end.as_pin().and_then(|p| sym.terminal_of_pin(p.pin)) {
                    keys.push(Key::Terminal(t));
                }
                keys
            }
            SymbolRole::Interruption { name } => vec![Key::Interruption(name.clone())],
            SymbolRole::PotentialFeed { .. } | SymbolRole::Unassigned => vec![Key::Symbol(symbol)],
        }
    };

    let mut parent: Vec<usize> = (0..wires.len()).collect();
    let mut first: HashMap<Key, usize> = HashMap::new();
    for (i, id) in wires.iter().enumerate() {
        let Some(wire) = project.wire(*id) else {
            continue;
        };
        for end in wire.ends() {
            for key in keys(end) {
                let other = *first.entry(key).or_insert(i);
                let (a, b) = (find(&mut parent, i), find(&mut parent, other));
                parent[a] = b;
            }
        }
    }
    let mut parts: BTreeMap<usize, Vec<WireId>> = BTreeMap::new();
    for (i, id) in wires.iter().enumerate() {
        parts.entry(find(&mut parent, i)).or_default().push(*id);
    }
    parts.into_values().collect()
}

fn compare(a: &Group, b: &Group) -> Ordering {
    a.page_rank
        .cmp(&b.page_rank)
        .then(a.column.cmp(&b.column))
        .then(a.x.total_cmp(&b.x))
        .then(a.y.total_cmp(&b.y))
        .then_with(|| a.wires.cmp(&b.wires))
}

fn build(
    project: &Project,
    netlist: &Netlist,
    wires: Vec<WireId>,
    rank: &BTreeMap<PageId, usize>,
) -> Option<Group> {
    // The first wire in reading order places the group.
    let mut best: Option<(usize, u16, f64, f64, PageId)> = None;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let (mut unnumbered, mut locked) = (0, false);
    let mut locked_number: Option<String> = None;
    for &id in &wires {
        let w = project.wire(id)?;
        let page = project.page(w.page)?;
        let at = w
            .ends()
            .into_iter()
            .filter_map(|e| project.endpoint_position(e))
            .min_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)))
            .unwrap_or_default();
        let key = (
            rank.get(&w.page).copied().unwrap_or(usize::MAX),
            page.format.column_at(at.x),
            at.y,
            at.x,
            w.page,
        );
        if best.is_none_or(|b| (b.0, b.1, b.2, b.3) > (key.0, key.1, key.2, key.3)) {
            best = Some(key);
        }
        match w.number.as_deref().filter(|n| !n.is_empty()) {
            Some(n) => *counts.entry(n.to_owned()).or_default() += 1,
            None => unnumbered += 1,
        }
        if matches!(w.properties.get(LOCKED), Some(Value::Bool(true))) {
            locked = true;
            if locked_number.is_none() {
                locked_number = w.number.clone().filter(|n| !n.is_empty());
            }
        }
    }
    let (page_rank, column, y, x, page) = best?;
    let page_ref = project.page(page)?;
    let potential = wires
        .iter()
        .find_map(|&id| netlist.net_of_wire(project.wire(id)?))
        .and_then(|n| netlist.net(n).potential())
        .and_then(|p| project.potential(p))
        .map(|p| (p.name.clone(), p.kind));
    let cable = wires.iter().find_map(|&id| {
        let cable = project.cable(project.cable_of_wire(id)?)?;
        let core = cable.cores.iter().find(|c| c.wire == Some(id))?;
        Some((cable.tag.product.join("."), core.name.clone()))
    });
    let mut numbers: Vec<(String, usize)> = counts.into_iter().collect();
    numbers.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| natural_cmp(&a.0, &b.0)));
    Some(Group {
        wires,
        page,
        page_name: page_ref.name.clone(),
        column,
        x,
        y,
        potential,
        cable,
        function: page_ref.structure.function.join("."),
        locked,
        current: locked_number.or_else(|| numbers.first().map(|n| n.0.clone())),
        numbers,
        unnumbered,
        page_rank,
    })
}
