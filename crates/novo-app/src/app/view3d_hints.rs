//! The 3D view's legend of mouse and keys: a small card in the corner that
//! folds down to a chip. View state only.

use eframe::egui;

/// Margin from the view's corner, in points.
const MARGIN: f32 = 12.0;

/// What the legend lists: the action, then how to do it.
pub(super) const ROWS: &[(&str, &str)] = &[
    ("Orbit", "Right-drag, or Alt + left-drag"),
    ("Pan", "Middle-drag, or Space + left-drag"),
    ("Zoom", "Mouse wheel, towards the pointer"),
    ("Select", "Click; Shift + click adds or removes"),
    ("Box select", "Drag on empty space (Move off: anywhere)"),
    (
        "Move a part",
        "Turn Move on, then drag it; R turns it, Del removes it",
    ),
    (
        "Zoom to selection",
        "F or Home (everything if nothing is selected)",
    ),
    (
        "Hide and isolate",
        "Ctrl+H hides the selection, Ctrl+I isolates it, Ctrl+Shift+H shows all",
    ),
    ("Standard views", "Click a face, edge or corner of the cube"),
    ("Door", "Shift+D opens or closes it"),
    ("Menu", "Right-click without dragging"),
];

/// Whether the card is open. It starts closed (the open card covers a quarter
/// of a small view); View > 3D > Mouse and keys opens it as a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Hints {
    pub(super) open: bool,
}

impl Hints {
    pub(super) fn toggle(&mut self) {
        self.open ^= true;
    }

    /// The legend, as a pane of the right strip while it is open.
    pub(super) fn show(&mut self, ctx: &egui::Context, view: egui::Rect) {
        if !self.open {
            return;
        }
        super::panel::Float::new(
            "view3d_hints",
            view,
            egui::Align2::RIGHT_BOTTOM,
            view.right_bottom() + egui::vec2(-MARGIN, -MARGIN),
        )
        .dock("Mouse and keys")
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Mouse and keys");
                    if ui.small_button("✕").on_hover_text("Close").clicked() {
                        self.open = false;
                    }
                });
                egui::Grid::new("view3d_hints_grid")
                    .num_columns(2)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        for (what, how) in ROWS {
                            ui.label(egui::RichText::new(*what).small().strong());
                            ui.add(egui::Label::new(egui::RichText::new(*how).small()).wrap());
                            ui.end_row();
                        }
                    });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_starts_folded_and_toggles() {
        let mut h = Hints::default();
        assert!(!h.open);
        h.toggle();
        assert!(h.open);
    }

    #[test]
    fn the_legend_names_the_hide_shortcuts_the_keymap_binds() {
        let row = ROWS
            .iter()
            .find(|(what, _)| *what == "Hide and isolate")
            .expect("the hide row is listed");
        // The text shows the chord the keymap really binds, so rebinding a
        // command fails here until the legend is edited.
        let keymap = novo_workflow::keymap::Keymap::default();
        for id in ["view.3d_hide", "view.3d_isolate", "view.3d_show_all"] {
            let chord = keymap
                .shortcut_text(id)
                .unwrap_or_else(|| panic!("{id} is bound"));
            assert!(row.1.contains(&chord), "{chord} for {id} is in the row");
        }
    }

    #[test]
    fn every_row_has_both_columns() {
        assert!(ROWS.iter().all(|(a, b)| !a.is_empty() && !b.is_empty()));
    }
}
