//! Ctrl+D in the app: smart duplicate (novo-workflow, WS-08; DESIGN §3).

use novo_core::Point;
use novo_workflow::duplicate::{TagMode, duplicate};

use super::{Level, NovoApp};

/// Gap between the selection and its copy, in grid steps.
const GAP_STEPS: f64 = 2.0;

impl NovoApp {
    /// Copies the selection to the right of itself with the next free tags,
    /// as one undo step, and selects the copy so Ctrl+D can be repeated.
    pub(super) fn duplicate_selection(&mut self) {
        let selection = self.selection();
        let Some(page) = self.current_page else {
            return;
        };
        let Some(view) = self.canvas.schematic_ref() else {
            return;
        };
        let (Some((width, _)), grid) = (view.selection_size(), view.editor.grid) else {
            self.message(Level::Info, "Select something to duplicate");
            return;
        };
        let step = if grid > 0.0 { grid } else { 2.5 };
        let dx = ((width / step).ceil() + GAP_STEPS) * step;
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        match doc.transact("Duplicate", |tx| {
            duplicate(tx, &selection, page, Point::new(dx, 0.0), TagMode::NextFree)
        }) {
            Ok((copy, changes)) => {
                self.changed_outside_canvas(&changes);
                if let Some(view) = self.canvas.schematic() {
                    view.select_ids(&copy.new_ids());
                }
            }
            Err(e) => self.message(Level::Error, format!("Could not duplicate: {e}")),
        }
    }
}
