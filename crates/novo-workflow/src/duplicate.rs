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
//!   or when it has no main function and all its symbols are copied.
//! - A device whose main function stays behind still gets a new tag when
//!   its main contacts are copied (a symbol with two or more switched
//!   poles): copying the main circuit of a starter whose coil is on the
//!   control page gives `-K2` main poles, which the checker flags as
//!   having no coil, rather than a second set of main poles on `-K1`.
//!   "Main contacts" is a heuristic on the symbol's switched paths, not a
//!   flag from the library: a two-pole auxiliary block copied without its
//!   coil also gets a new device, and main poles drawn as three one-pole
//!   symbols stay on the original device.
//! - A lone auxiliary contact copied without its coil stays a contact of
//!   the original device (a second contact of `-K1`).
//! - Texts attached to a copied symbol, junction or wire (a device tag
//!   label, a wire number label) are copied with it and attached to the
//!   copy. Selected free texts are copied and moved by the offset.
//! - Terminals are copied onto the same strip with the next free number;
//!   non-numeric names (`PE`, `N`) are kept.
//! - A selected text attached to an object that is not copied is left out.
//! - Wires are copied when both ends are copied, also when the wire itself
//!   was not selected. Wire numbers are not copied (they are regenerated);
//!   colours and cross-sections are.
//! - Potential feeds and interruption points keep their potential and name.
//! - [`TagMode::Keep`] keeps the original tags (Ctrl+Shift+V), for moving
//!   circuits between projects; same-project duplicates show in the
//!   checker. That includes copied main poles (a second device with the
//!   same tag) and terminals (same names on the strip).

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    AnyId, DeviceId, DeviceKind, Endpoint, Junction, JunctionId, PageId, Point, Project, Result,
    SymbolId, SymbolInstance, SymbolRole, TerminalId, Text, TextId, Tx, Wire, WireId,
};

use crate::clipboard::{Clip, paste};

/// How copied devices are tagged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagMode {
    /// The next free number per letter code (Ctrl+D, Ctrl+V).
    #[default]
    NextFree,
    /// The original tags, duplicates included.
    Keep,
    /// The original tags where no device has them, else the next free
    /// ones (Ctrl+Shift+V; a paste after a cut).
    KeepIfFree,
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
    /// Attached texts that came along with their objects, and selected
    /// free texts.
    pub texts: BTreeMap<TextId, TextId>,
}

impl Duplicated {
    /// Every new object, for selecting the copy.
    pub fn new_ids(&self) -> Vec<AnyId> {
        let mut ids: Vec<AnyId> = Vec::new();
        ids.extend(self.symbols.values().map(|&id| AnyId::from(id)));
        ids.extend(self.junctions.values().map(|&id| AnyId::from(id)));
        ids.extend(self.wires.values().map(|&id| AnyId::from(id)));
        ids.extend(self.texts.values().map(|&id| AnyId::from(id)));
        ids
    }
}

/// Copies `items` (symbols, junctions, wires and free texts; other ids are
/// ignored), with the texts attached to them, to
/// `page`, moved by `offset`. A snapshot pasted at once, see
/// [`crate::clipboard`].
pub fn duplicate(
    tx: &mut Tx<'_>,
    items: &[AnyId],
    page: PageId,
    offset: Point,
    mode: TagMode,
) -> Result<Duplicated> {
    // The copy is in the same project, so contacts keep their coil.
    let Some(clip) = Clip::capture(tx.project(), items, "", false) else {
        return Ok(Duplicated::default());
    };
    Ok(paste(tx, &clip, page, offset, mode, "")?.copied)
}

/// The objects a duplicate copies.
pub(crate) struct Selection {
    symbols: BTreeSet<SymbolId>,
    junctions: BTreeSet<JunctionId>,
    wires: BTreeSet<WireId>,
    /// Selected texts that are not attached to anything.
    free_texts: BTreeSet<TextId>,
}

impl Selection {
    pub(crate) fn gather(project: &Project, items: &[AnyId]) -> Selection {
        let mut s = Selection {
            symbols: BTreeSet::new(),
            junctions: BTreeSet::new(),
            wires: BTreeSet::new(),
            free_texts: BTreeSet::new(),
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
                // An attached text comes along with its object instead.
                AnyId::Text(id) if project.text(id).is_some_and(|t| t.attached_to.is_none()) => {
                    s.free_texts.insert(id);
                }
                _ => {}
            }
        }
        // Wires between copied objects come along.
        // An open end comes along with the other end; a wire open at both
        // ends is copied only when selected.
        let inside = |end: Endpoint| match end {
            Endpoint::Pin { symbol, .. } => Some(s.symbols.contains(&symbol)),
            Endpoint::Junction { junction } => Some(s.junctions.contains(&junction)),
            Endpoint::Free { .. } => None,
        };
        let extra: Vec<WireId> = project
            .wires()
            .filter(|w| {
                let ends = [inside(w.from), inside(w.to)];
                ends.iter().flatten().count() > 0 && ends.iter().flatten().all(|&i| i)
            })
            .map(|w| w.id)
            .collect();
        s.wires.extend(extra);
        s
    }

    pub(crate) fn devices_to_copy(&self, project: &Project) -> BTreeSet<DeviceId> {
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
                Some(main) if self.symbols.contains(&main.id) => true,
                // The coil stays behind: copied main contacts still make a
                // new device, a lone auxiliary contact does not.
                Some(_) => all
                    .iter()
                    .any(|s| self.symbols.contains(&s.id) && is_main_contact(s)),
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

    pub(crate) fn free_texts(&self, project: &Project) -> Vec<Text> {
        self.free_texts
            .iter()
            .filter_map(|&id| project.text(id).cloned())
            .collect()
    }

    pub(crate) fn symbols(&self, project: &Project) -> Vec<SymbolInstance> {
        self.symbols
            .iter()
            .filter_map(|&id| project.symbol(id).cloned())
            .collect()
    }

    pub(crate) fn junctions(&self, project: &Project) -> Vec<Junction> {
        self.junctions
            .iter()
            .filter_map(|&id| project.junction(id).cloned())
            .collect()
    }

    pub(crate) fn wires(&self, project: &Project) -> Vec<Wire> {
        self.wires
            .iter()
            .filter_map(|&id| project.wire(id).cloned())
            .collect()
    }
}

/// A device's main contacts: a non-main symbol that switches two or more
/// poles (the three main poles of a contactor). Auxiliary contacts switch
/// one path (a changeover has two paths through one common pin, so it is
/// told apart by sharing that pin).
fn is_main_contact(symbol: &SymbolInstance) -> bool {
    if matches!(symbol.role, SymbolRole::Device { main: true, .. }) {
        return false;
    }
    let groups = &symbol.pass_through;
    if groups.len() < 2 {
        return false;
    }
    // Poles are separate: no pin in two groups.
    let mut seen = BTreeSet::new();
    groups.iter().flatten().all(|pin| seen.insert(*pin))
}
