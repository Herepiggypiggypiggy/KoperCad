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
        "Drag it (Move on); R turns it, Del removes it",
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

/// Whether the card is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hints {
    pub(super) open: bool,
}

impl Default for Hints {
    fn default() -> Self {
        // Open at first so a new user sees how to move; the chip stays
        // after it is closed.
        Hints { open: true }
    }
}

impl Hints {
    pub(super) fn toggle(&mut self) {
        self.open ^= true;
    }

    /// The legend at the view's bottom right corner.
    pub(super) fn show(&mut self, ctx: &egui::Context, view: egui::Rect) {
        egui::Area::new(egui::Id::new("view3d_hints"))
            .order(egui::Order::Middle)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(view.right_bottom() + egui::vec2(-MARGIN, -MARGIN))
            .show(ctx, |ui| {
                if !self.open {
                    if ui
                        .button("? Mouse and keys")
                        .on_hover_text("Show how to move around and select in 3D")
                        .clicked()
                    {
                        self.open = true;
                    }
                    return;
                }
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Mouse and keys");
                        if ui.small_button("✕").on_hover_text("Fold away").clicked() {
                            self.open = false;
                        }
                    });
                    egui::Grid::new("view3d_hints_grid")
                        .num_columns(2)
                        .spacing([12.0, 2.0])
                        .show(ui, |ui| {
                            for (what, how) in ROWS {
                                ui.label(egui::RichText::new(*what).small().strong());
                                ui.label(egui::RichText::new(*how).small());
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
    fn the_card_starts_open_and_toggles() {
        let mut h = Hints::default();
        assert!(h.open);
        h.toggle();
        assert!(!h.open);
    }

    #[test]
    fn the_legend_names_the_hide_shortcuts_the_keymap_binds() {
        let row = ROWS
            .iter()
            .find(|(what, _)| *what == "Hide and isolate")
            .expect("the hide row is listed");
        for chord in ["Ctrl+H", "Ctrl+I", "Ctrl+Shift+H"] {
            assert!(row.1.contains(chord), "{chord} is in the row");
        }
        let keymap = novo_workflow::keymap::Keymap::default();
        for id in ["view.3d_hide", "view.3d_isolate", "view.3d_show_all"] {
            assert!(
                keymap.bindings().iter().any(|b| b.command == id),
                "{id} is still bound"
            );
        }
    }

    #[test]
    fn every_row_has_both_columns() {
        assert!(ROWS.iter().all(|(a, b)| !a.is_empty() && !b.is_empty()));
    }
}
