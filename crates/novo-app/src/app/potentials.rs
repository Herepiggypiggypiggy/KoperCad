//! Interruption points and PE terminals in the app: the commands behind the
//! context-menu rows, the override diamond on PE terminals set by hand, and
//! automatic PE marking after wiring (novo-terminals, novo-xref; WS-10, WS-11).

use novo_canvas::model::item_id;
use novo_canvas::{Mark, MarkKind};
use novo_core::{AnyId, ChangeSet, EntityKind, PageId, SymbolId, SymbolRole, TerminalId};

use super::{Level, NovoApp};

impl NovoApp {
    /// Interruption points among the selected symbols.
    fn selected_interruptions(&self) -> Vec<SymbolId> {
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        self.selection()
            .into_iter()
            .filter_map(|id| match id {
                AnyId::Symbol(s) => Some(s),
                _ => None,
            })
            .filter(|&s| {
                doc.project()
                    .symbol(s)
                    .is_some_and(|s| matches!(s.role, SymbolRole::Interruption { .. }))
            })
            .collect()
    }

    /// "Number automatically": renumbers the selected interruption points,
    /// or every point in the project when none is selected. A pair keeps
    /// one name. One undo step.
    pub(super) fn renumber_interruptions(&mut self) {
        let picked = self.selected_interruptions();
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let result = doc.transact("Number interruption points", |tx| {
            novo_xref::renumber_interruptions(tx, &picked)
        });
        match result {
            Ok((0, _)) => self.message(Level::Info, "Interruption points are already in order"),
            Ok((n, changes)) => {
                self.changed_outside_canvas(&changes);
                let what = if n == 1 { "point" } else { "points" };
                self.message(Level::Info, format!("Numbered {n} interruption {what}"));
            }
            Err(e) => self.message(Level::Error, format!("Number interruption points: {e}")),
        }
    }

    /// The `pe` command and "Toggle PE terminal": a choice by hand, kept
    /// until reset. "Back to automatic" hands the terminals back to the
    /// rule and runs it.
    pub(super) fn pe_command(&mut self, id: &str) {
        let terminals: Vec<TerminalId> = self.selected_terminals();
        if terminals.is_empty() {
            self.message(Level::Info, "Select one or more terminals first");
            return;
        }
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let (label, result) = if id == "terminal.toggle_pe" {
            (
                "Toggle PE terminal",
                doc.transact("Toggle PE terminal", |tx| {
                    novo_terminals::toggle_pe(tx, &terminals).map(|_| ())
                }),
            )
        } else {
            (
                "PE terminal: back to automatic",
                doc.transact("PE terminal: back to automatic", |tx| {
                    novo_terminals::reset_pe_override(tx, &terminals)?;
                    novo_terminals::mark_earthed(tx).map(|_| ())
                }),
            )
        };
        match result {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Error, format!("{label}: {e}")),
        }
    }

    /// After a wiring edit: terminals on an earthed net become PE terminals
    /// (DESIGN.md §4), inside the edit's own undo step so one undo takes
    /// both back. A terminal set by hand is left alone.
    pub(super) fn auto_pe(&mut self, changes: &ChangeSet) {
        let wired = changes.touches(EntityKind::Wire)
            || changes.touches(EntityKind::Symbol)
            || changes.touches(EntityKind::Potential);
        if !wired {
            return;
        }
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        if doc.project().terminals().next().is_none() {
            return;
        }
        let netlist = novo_core::Netlist::build(doc.project());
        if novo_terminals::earthed_terminals(doc.project(), &netlist).is_empty() {
            return;
        }
        match doc.amend_last("Mark PE terminals", novo_terminals::mark_earthed) {
            Ok((_, more)) => self.changed_outside_canvas(&more),
            Err(e) => self.message(Level::Error, format!("Mark PE terminals: {e}")),
        }
    }

    /// The override diamond on each terminal of `page` whose PE state was
    /// chosen by hand; shown while Shift+O is held (DESIGN.md §5).
    pub(super) fn pe_override_marks(&self, page: PageId) -> Vec<Mark> {
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let project = doc.project();
        project
            .terminals()
            .filter(|t| !t.pe_override.is_auto())
            .flat_map(|t| project.symbols_of_terminal(t.id))
            .filter(|s| s.page == page)
            .map(|s| {
                Mark::on(MarkKind::Override, item_id(AnyId::Symbol(s.id)))
                    .with_text("PE set by hand")
            })
            .collect()
    }
}
