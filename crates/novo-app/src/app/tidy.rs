//! Tidy routes: the `draw.tidy` command and the "Re-route" row of the wire
//! right-click menu. Re-routes the selected wires, removes the dead stubs
//! and dots they leave at junctions and joins open ends that lie on a pin.
//!
//! Where the user finds it: Draw > Tidy routes, Ctrl+K "tidy", right-click a
//! wire > Re-route. Select the whole page first (Ctrl+A) to tidy it all.

use std::collections::BTreeSet;

use novo_core::{AnyId, Endpoint, JunctionId, PageId, Project, WireId};
use novo_editor::edit;

use super::{Level, NovoApp};

/// What a selection offers to tidy.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct TidyTargets {
    pub wires: Vec<WireId>,
    pub junctions: Vec<JunctionId>,
    pub pages: BTreeSet<PageId>,
}

/// The wires and junctions among `selection`, plus the junctions the
/// wires end at (their dots may be left over after a re-route).
pub(crate) fn targets(project: &Project, selection: &[AnyId]) -> TidyTargets {
    let mut out = TidyTargets::default();
    let mut junctions: BTreeSet<JunctionId> = BTreeSet::new();
    for id in selection {
        match *id {
            AnyId::Wire(w) => {
                let Some(wire) = project.wire(w) else {
                    continue;
                };
                out.wires.push(w);
                out.pages.insert(wire.page);
                for end in [wire.from, wire.to] {
                    if let Endpoint::Junction { junction } = end {
                        junctions.insert(junction);
                    }
                }
            }
            AnyId::Junction(j) => {
                if let Some(junction) = project.junction(j) {
                    junctions.insert(j);
                    out.pages.insert(junction.page);
                }
            }
            _ => {}
        }
    }
    out.junctions = junctions.into_iter().collect();
    out
}

impl NovoApp {
    /// `draw.tidy`: one undo step over the selection.
    pub(super) fn tidy_selection(&mut self) {
        let selection = self.selection();
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let found = targets(doc.project(), &selection);
        if found.wires.is_empty() && found.junctions.is_empty() {
            self.message(
                Level::Info,
                "Select wires to tidy (Ctrl+A selects the page), then Tidy routes",
            );
            return;
        }
        let result = doc.transact("Tidy routes", |tx| {
            for &w in &found.wires {
                edit::reroute(tx, w)?;
            }
            edit::tidy_junctions(tx, &found.junctions)?;
            let mut joined = 0;
            for &page in &found.pages {
                joined += edit::join_coincident(tx, page)?;
            }
            Ok(joined)
        });
        match result {
            Ok((joined, changes)) => {
                self.changed_outside_canvas(&changes);
                let text = match (found.wires.len(), joined) {
                    (n, 0) => format!("Tidied {n} wires"),
                    (n, j) => format!("Tidied {n} wires, joined {j} open ends"),
                };
                self.message(Level::Info, text);
            }
            Err(e) => self.message(Level::Error, format!("Tidy routes: {e}")),
        }
    }
}
