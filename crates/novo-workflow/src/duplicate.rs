//! Smart duplicate and paste (Ctrl+D, Ctrl+V, Ctrl+Shift+V; J4).
//!
//! Copies symbols, junctions and wires to a page, moved by an offset, as
//! part of the caller's transaction (one undo step). Each copied device
//! gets the next free number for its letter code, and its copied functions
//! stay together: a coil copied with its contacts becomes `-K2` with
//! `-K2` contacts. Copying a motor circuit needs no manual renaming.
//!
//! Rules:
//!
//! - A device gets a new tag when its main function (the coil) is copied,
//!   or when it has no main function and all its symbols are copied. A
//!   lone contact copied without its coil stays a contact of the original
//!   device (a second contact of `-K1`).
//! - Terminals are copied onto the same strip with the next free number;
//!   non-numeric names (`PE`, `N`) are kept.
//! - Wires are copied when both ends are copied, also when the wire itself
//!   was not selected. Wire numbers are not copied (they are regenerated);
//!   colours and cross-sections are.
//! - Potential feeds and interruption points keep their potential and name.
//! - [`TagMode::Keep`] keeps the original tags (Ctrl+Shift+V), for moving
//!   circuits between projects; same-project duplicates show in the
//!   checker.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    AnyId, Device, DeviceId, DeviceKind, Endpoint, Junction, JunctionId, PageId, Point, Project,
    Result, SymbolId, SymbolInstance, SymbolRole, Terminal, TerminalId, Tx, Wire, WireId,
    split_code,
};

/// How copied devices are tagged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagMode {
    /// The next free number per letter code (Ctrl+D, Ctrl+V).
    #[default]
    NextFree,
    /// The original tags (Ctrl+Shift+V).
    Keep,
}

/// What a duplicate created, keyed by the original ids.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Duplicated {
    pub symbols: BTreeMap<SymbolId, SymbolId>,
    pub junctions: BTreeMap<JunctionId, JunctionId>,
    pub wires: BTreeMap<WireId, WireId>,
    /// Devices that got a copy. Devices the copies still belong to (a
    /// lone contact's coil) are not listed.
    pub devices: BTreeMap<DeviceId, DeviceId>,
    pub terminals: BTreeMap<TerminalId, TerminalId>,
}

impl Duplicated {
    /// Every new object, for selecting the copy.
    pub fn new_ids(&self) -> Vec<AnyId> {
        let mut ids: Vec<AnyId> = Vec::new();
        ids.extend(self.symbols.values().map(|&id| AnyId::from(id)));
        ids.extend(self.junctions.values().map(|&id| AnyId::from(id)));
        ids.extend(self.wires.values().map(|&id| AnyId::from(id)));
        ids
    }
}

/// Copies `items` (symbols, junctions and wires; other ids are ignored) to
/// `page`, moved by `offset`.
pub fn duplicate(
    tx: &mut Tx<'_>,
    items: &[AnyId],
    page: PageId,
    offset: Point,
    mode: TagMode,
) -> Result<Duplicated> {
    let selection = Selection::gather(tx.project(), items);
    let mut out = Duplicated::default();

    // Devices that get a copy, in tag order so numbers follow it.
    let mut devices: Vec<&Device> = selection
        .devices_to_copy(tx.project())
        .into_iter()
        .filter_map(|id| tx.project().device(id))
        .collect();
    devices.sort_by(|a, b| a.tag.cmp(&b.tag));
    let devices: Vec<Device> = devices.into_iter().cloned().collect();
    for device in devices {
        let tag = match mode {
            TagMode::Keep => device.tag.clone(),
            TagMode::NextFree => {
                let next = tx
                    .project()
                    .next_tag_number(&device.tag.parent(), device.tag.letter_code());
                device.tag.with_number(next)
            }
        };
        let new = tx.insert(|id| Device {
            id,
            tag,
            ..device.clone()
        })?;
        out.devices.insert(device.id, new);
    }

    // Symbols, with roles pointing at the copies.
    for original in selection.symbols(tx.project()) {
        let role = match &original.role {
            SymbolRole::Device { device, main } => SymbolRole::Device {
                device: out.devices.get(device).copied().unwrap_or(*device),
                main: *main,
            },
            SymbolRole::Terminal { terminals } => {
                let mut copies = Vec::with_capacity(terminals.len());
                for &t in terminals {
                    let copy = copy_terminal(tx, t)?;
                    out.terminals.insert(t, copy);
                    copies.push(copy);
                }
                SymbolRole::Terminal { terminals: copies }
            }
            other => other.clone(),
        };
        let mut placement = original.placement;
        placement.origin = placement.origin + offset;
        let new = tx.insert(|id| SymbolInstance {
            id,
            page,
            placement,
            role,
            ..original.clone()
        })?;
        out.symbols.insert(original.id, new);
    }

    for original in selection.junctions(tx.project()) {
        let position = original.position + offset;
        let new = tx.insert(|id| Junction { id, page, position })?;
        out.junctions.insert(original.id, new);
    }

    let wires: Vec<Wire> = selection.wires(tx.project());
    for original in wires {
        let (Some(from), Some(to)) = (
            map_endpoint(&out, original.from),
            map_endpoint(&out, original.to),
        ) else {
            continue;
        };
        let route = original.route.iter().map(|&p| p + offset).collect();
        let new = tx.insert(|id| Wire {
            id,
            page,
            from,
            to,
            route,
            number: None,
            ..original.clone()
        })?;
        out.wires.insert(original.id, new);
    }
    Ok(out)
}

fn map_endpoint(out: &Duplicated, end: Endpoint) -> Option<Endpoint> {
    match end {
        Endpoint::Pin { symbol, pin } => out
            .symbols
            .get(&symbol)
            .map(|&s| Endpoint::Pin { symbol: s, pin }),
        Endpoint::Junction { junction } => out
            .junctions
            .get(&junction)
            .map(|&j| Endpoint::Junction { junction: j }),
    }
}

/// A new terminal on the same strip: the next number, or the same
/// non-numeric name.
fn copy_terminal(tx: &mut Tx<'_>, original: TerminalId) -> Result<TerminalId> {
    let project = tx.project();
    let Some(terminal) = project.terminal(original).cloned() else {
        return Err(novo_core::ModelError::NotFound(original.into()));
    };
    let siblings = project.terminals_of_strip(terminal.strip);
    let numeric = |name: &str| match split_code(name) {
        ("", Some(n), "") => Some(n),
        _ => None,
    };
    let name = if numeric(&terminal.name).is_some() {
        let next = siblings
            .iter()
            .filter_map(|t| numeric(&t.name))
            .max()
            .map_or(1, |n| n + 1);
        next.to_string()
    } else {
        terminal.name.clone()
    };
    let order = siblings.iter().map(|t| t.order).max().unwrap_or(0) + 1;
    tx.insert(|id| Terminal {
        id,
        name,
        order,
        jumper: None,
        ..terminal
    })
}

/// The objects a duplicate copies.
struct Selection {
    symbols: BTreeSet<SymbolId>,
    junctions: BTreeSet<JunctionId>,
    wires: BTreeSet<WireId>,
}

impl Selection {
    fn gather(project: &Project, items: &[AnyId]) -> Selection {
        let mut s = Selection {
            symbols: BTreeSet::new(),
            junctions: BTreeSet::new(),
            wires: BTreeSet::new(),
        };
        for &item in items {
            match item {
                AnyId::Symbol(id) if project.symbol(id).is_some() => {
                    s.symbols.insert(id);
                }
                AnyId::Junction(id) if project.junction(id).is_some() => {
                    s.junctions.insert(id);
                }
                AnyId::Wire(id) if project.wire(id).is_some() => {
                    s.wires.insert(id);
                }
                _ => {}
            }
        }
        // Wires between copied objects come along.
        let inside = |end: Endpoint| match end {
            Endpoint::Pin { symbol, .. } => s.symbols.contains(&symbol),
            Endpoint::Junction { junction } => s.junctions.contains(&junction),
        };
        let extra: Vec<WireId> = project
            .wires()
            .filter(|w| inside(w.from) && inside(w.to))
            .map(|w| w.id)
            .collect();
        s.wires.extend(extra);
        s
    }

    fn devices_to_copy(&self, project: &Project) -> BTreeSet<DeviceId> {
        let mut out = BTreeSet::new();
        let referenced: BTreeSet<DeviceId> = self
            .symbols
            .iter()
            .filter_map(|&id| project.symbol(id)?.device())
            .collect();
        for device in referenced {
            let all = project.symbols_of_device(device);
            let main = all
                .iter()
                .find(|s| matches!(s.role, SymbolRole::Device { main: true, .. }));
            let copy = match main {
                Some(main) => self.symbols.contains(&main.id),
                None => all.iter().all(|s| self.symbols.contains(&s.id)),
            };
            // Terminal strips are never copied; their terminals are.
            let is_strip = project
                .device(device)
                .is_some_and(|d| d.kind == DeviceKind::TerminalStrip);
            if copy && !is_strip {
                out.insert(device);
            }
        }
        out
    }

    fn symbols(&self, project: &Project) -> Vec<SymbolInstance> {
        self.symbols
            .iter()
            .filter_map(|&id| project.symbol(id).cloned())
            .collect()
    }

    fn junctions(&self, project: &Project) -> Vec<Junction> {
        self.junctions
            .iter()
            .filter_map(|&id| project.junction(id).cloned())
            .collect()
    }

    fn wires(&self, project: &Project) -> Vec<Wire> {
        self.wires
            .iter()
            .filter_map(|&id| project.wire(id).cloned())
            .collect()
    }
}
