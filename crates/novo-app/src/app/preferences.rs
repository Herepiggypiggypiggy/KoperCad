//! The Preferences window: per-user settings that are not the project's.

use eframe::egui;
use novo_editor::tool::ConnectMode;

use super::NovoApp;

impl NovoApp {
    pub(super) fn preferences_dialog(&mut self, ctx: &egui::Context) {
        if !self.wb.preferences {
            return;
        }
        let mut mode = self.settings.connect_mode();
        let mut carries = self.settings.move_carries;
        let mut calm = self.settings.reduce_motion;
        let modal = egui::Modal::new(egui::Id::new("preferences")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.heading("Preferences");
            ui.add_space(6.0);
            ui.strong("Connections");
            ui.add_space(2.0);
            ui.label("Auto connections");
            ui.weak("Wires between facing free pins after you place or move a symbol.");
            for (value, label) in [
                (ConnectMode::Draw, "Draw them (default)"),
                (ConnectMode::Ask, "Ask me each time"),
                (ConnectMode::Never, "Never"),
            ] {
                ui.radio_value(&mut mode, value, label);
            }
            ui.add_space(8.0);
            ui.label("Moving a symbol");
            ui.weak("Shift always does the other one.");
            ui.radio_value(&mut carries, false, "Leaves its wires (default)");
            ui.radio_value(&mut carries, true, "Takes its wires along");
            ui.add_space(8.0);
            ui.strong("Simulation");
            ui.add_space(2.0);
            ui.checkbox(&mut calm, "Reduce motion");
            ui.weak("The live view switches states at once and shows an arrow on running motors instead of spinning them.");
            ui.add_space(8.0);
            ui.button("Close (Esc)").clicked()
        });
        if mode != self.settings.connect_mode() {
            self.settings.set_connect_mode(mode);
        }
        self.settings.move_carries = carries;
        self.settings.reduce_motion = calm;
        if modal.inner || modal.should_close() {
            self.wb.preferences = false;
        }
    }
}
