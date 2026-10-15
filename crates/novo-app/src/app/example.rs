//! The example project (docs/ONBOARDING-DESIGN.md §1): "Open example" on
//! the start panel and in the command field. It is the Gate 1 motor
//! starter, built by the same fixture the tests use, so it cannot drift
//! from what is tested.

use novo_core::{Document, Result as ModelResult, Tx};

use super::{Level, NovoApp};

/// What the example project is called.
pub(crate) const EXAMPLE_NAME: &str = "Motor starter (example)";

const EXAMPLE_DESCRIPTION: &str = "Direct-on-line starter for a pump motor: main and control \
    circuit. The parts are illustrative catalogue entries, not a verified parts list.";

/// Draws the example into a project that has only its empty first page, as
/// one undo step.
pub(crate) fn build_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
    let ((), changes) = doc.transact("Open example", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        novo_tests::motor_starter_in(tx)?;
        tx.set_info(|info| {
            info.name = EXAMPLE_NAME.into();
            info.description = EXAMPLE_DESCRIPTION.into();
        })
    })?;
    Ok(changes)
}

impl NovoApp {
    /// Replaces the open project with a fresh copy of the example. It is an
    /// untitled project like any other, marked saved so that closing it
    /// untouched leaves nothing under Unsaved; the first edit makes it
    /// dirty and the usual autosave and Unsaved flow take over.
    pub(super) fn open_example(&mut self) {
        if let Err(e) = self.backend.new_project(EXAMPLE_NAME) {
            self.message(Level::Error, format!("Could not open the example: {e}"));
            return;
        }
        self.project_path = None;
        self.current_page = None;
        let built = self.backend.document_mut().map(build_example);
        match built {
            Some(Ok(changes)) => self.backend.record(&changes),
            Some(Err(e)) => {
                self.message(Level::Error, format!("Could not draw the example: {e}"));
            }
            None => {}
        }
        // Resets the views and adds the generated pages (contents, terminal
        // and cable diagrams), which belong to the example too.
        self.document_replaced();
        self.fix_current_page();
        if let Some(doc) = self.backend.document_mut() {
            doc.mark_saved();
        }
        self.message(
            Level::Info,
            "This is an example. Edit freely; Ctrl+S keeps your own copy.",
        );
    }
}
