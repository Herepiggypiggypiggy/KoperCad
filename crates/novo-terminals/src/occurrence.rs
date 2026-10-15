//! Where a terminal is drawn and what each of its sides is wired to.
//!
//! A terminal has two sides. The **internal** side faces the cabinet
//! (devices, other terminals), the **external** side faces the field
//! (motors, sensors, cables). On a drawn terminal the first connection point
//! of the terminal (`1`, the top or left one) is the internal side and the
//! rest are the external side. That is the whole rule; nothing about it is
//! guessed from the drawing.

use std::collections::{BTreeSet, HashSet, VecDeque};

use novo_core::{
    Endpoint, PageId, PinRef, Project, SymbolId, SymbolInstance, SymbolRole, TerminalId, WireId,
};

/// The far end of a wire leaving a terminal side.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    /// Designation of what the wire reaches: `-K1:13`, `-X2:4`, or the name
    /// of an interruption point.
    pub designation: String,
    /// The connection point reached.
    pub pin: PinRef,
    /// The wire that leaves the terminal towards this target.
    pub wire: WireId,
}

/// One side of a drawn terminal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Side {
    /// Connection points of the symbol on this side.
    pub pins: Vec<u16>,
    /// Wires that land on this side: the conductors in the clamp.
    pub wires: Vec<WireId>,
    /// What those wires reach, following wires through junctions, sorted.
    pub targets: Vec<Target>,
}

impl Side {
    pub fn is_connected(&self) -> bool {
        !self.wires.is_empty()
    }
}

/// How a drawn terminal is wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    /// Wired on both sides: the terminal passes the connection through.
    Through,
    /// Wired on the internal side only: a single-sided end.
    InternalEnd,
    /// Wired on the external side only: a single-sided end.
    ExternalEnd,
    /// Not wired at all.
    Open,
}

impl State {
    fn of(internal: bool, external: bool) -> State {
        match (internal, external) {
            (true, true) => State::Through,
            (true, false) => State::InternalEnd,
            (false, true) => State::ExternalEnd,
            (false, false) => State::Open,
        }
    }

    /// Short text for the schematic and the strip editor.
    pub fn label(self) -> &'static str {
        crate::strings::state_label(self)
    }
}

/// One drawing of a terminal on a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub symbol: SymbolId,
    pub page: PageId,
    pub internal: Side,
    pub external: Side,
}

impl Occurrence {
    pub fn state(&self) -> State {
        State::of(self.internal.is_connected(), self.external.is_connected())
    }

    /// Clamps this drawing needs when every clamp takes one wire per side
    /// (1-in/1-out): at least one, more when several wires land on a side.
    pub fn clamps(&self) -> usize {
        self.internal
            .wires
            .len()
            .max(self.external.wires.len())
            .max(1)
    }
}

/// Pins of `symbol` that belong to `terminal`, in pin order.
fn pins_of(symbol: &SymbolInstance, terminal: TerminalId) -> Vec<u16> {
    (0..symbol.pins.len())
        .filter_map(|p| u16::try_from(p).ok())
        .filter(|&p| symbol.terminal_of_pin(p) == Some(terminal))
        .collect()
}

/// Follows wires from `pins` of `symbol` through junctions and collects the
/// first connection points of other symbols they reach.
fn side(project: &Project, symbol: &SymbolInstance, pins: Vec<u16>) -> Side {
    let mut wires = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for &pin in &pins {
        let start = Endpoint::pin(symbol.id, pin);
        for first in project.wires_at(start) {
            wires.insert(first);
            let mut seen: HashSet<Endpoint> = HashSet::from([start]);
            let mut used: HashSet<WireId> = HashSet::from([first]);
            let mut queue = VecDeque::new();
            let wire = project.wire(first).expect("indexed wire exists");
            queue.push_back(if wire.from == start {
                wire.to
            } else {
                wire.from
            });
            while let Some(end) = queue.pop_front() {
                if !seen.insert(end) {
                    continue;
                }
                match end {
                    Endpoint::Pin { symbol: other, pin } => {
                        if other == symbol.id {
                            continue;
                        }
                        let pin = PinRef::new(other, pin);
                        let designation = project
                            .pin_designation(pin)
                            .unwrap_or_else(|| "?".to_owned());
                        let designation = match project.symbol(other).map(|s| &s.role) {
                            Some(SymbolRole::Interruption { name }) => format!("→ {name}"),
                            Some(SymbolRole::PotentialFeed { potential }) => project
                                .potential(*potential)
                                .map_or(designation, |p| p.name.clone()),
                            _ => designation,
                        };
                        targets.insert(Target {
                            designation,
                            pin,
                            wire: first,
                        });
                    }
                    Endpoint::Junction { .. } => {
                        for next in project.wires_at(end) {
                            if used.insert(next) {
                                let w = project.wire(next).expect("indexed wire exists");
                                queue.push_back(if w.from == end { w.to } else { w.from });
                            }
                        }
                    }
                    // An open end leads nowhere.
                    Endpoint::Free { .. } => {}
                }
            }
        }
    }
    Side {
        pins,
        wires: wires.into_iter().collect(),
        targets: targets.into_iter().collect(),
    }
}

/// The drawing of `terminal` in one symbol, or `None` when the symbol does
/// not draw it.
pub fn occurrence_in(
    project: &Project,
    symbol: &SymbolInstance,
    terminal: TerminalId,
) -> Option<Occurrence> {
    let mut pins = pins_of(symbol, terminal);
    if pins.is_empty() {
        return None;
    }
    let external = pins.split_off(1);
    Some(Occurrence {
        symbol: symbol.id,
        page: symbol.page,
        internal: side(project, symbol, pins),
        external: side(project, symbol, external),
    })
}

/// Every drawing of a terminal, in page order then symbol order.
pub fn occurrences(project: &Project, terminal: TerminalId) -> Vec<Occurrence> {
    let pages: Vec<PageId> = project.pages_in_order().iter().map(|p| p.id).collect();
    let page_rank = |page: PageId| pages.iter().position(|&p| p == page).unwrap_or(usize::MAX);
    let mut symbols = project.symbols_of_terminal(terminal);
    symbols.sort_by_key(|s| (page_rank(s.page), s.id));
    symbols
        .into_iter()
        .filter_map(|s| occurrence_in(project, s, terminal))
        .collect()
}
