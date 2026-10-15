//! The Project settings window: what belongs to the project file, not to
//! the user. Today that is the drawing standard, which picks the symbol pack.

use eframe::egui;
use novo_symbols::Pack;

use super::{Level, NovoApp};

impl NovoApp {
    /// The project's drawing standard; IEC when the project does not say.
    pub(super) fn standard(&self) -> Pack {
        self.backend
            .document()
            .and_then(|d| Pack::from_id(&d.project().info().rules.standard))
            .unwrap_or(Pack::Iec)
    }

    /// " · NFPA" for a project that is not IEC, else nothing: IEC is the
    /// default and needs no label.
    pub(super) fn standard_suffix(&self) -> String {
        match self.standard() {
            Pack::Iec => String::new(),
            pack => format!(" \u{b7} {}", pack.id().to_uppercase()),
        }
    }

    /// Saves the standard in the project (one undo step).
    pub(super) fn set_standard(&mut self, pack: Pack) {
        if pack == self.standard() {
            return;
        }
        let id = if pack == Pack::Iec { "" } else { pack.id() };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        match doc.transact("Drawing standard", |tx| {
            tx.set_info(|i| i.rules.standard = id.to_owned())
        }) {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Error, format!("Drawing standard: {e}")),
        }
    }

    pub(super) fn project_settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.wb.project_settings {
            return;
        }
        if self.backend.document().is_none() {
            self.wb.project_settings = false;
            return;
        }
        let mut pack = self.standard();
        let modal = egui::Modal::new(egui::Id::new("project_settings")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.heading("Project settings");
            ui.add_space(6.0);
            ui.strong("Drawing standard");
            ui.weak(
                "Picks the symbol pack for new symbols; placed symbols stay as drawn. \
NFPA symbols appear once the editor loads that pack.",
            );
            for (value, label) in [
                (Pack::Iec, "IEC 60617 (default)"),
                (Pack::Nfpa, "NFPA 79 / ANSI Y32.2"),
            ] {
                ui.radio_value(&mut pack, value, label);
            }
            ui.add_space(8.0);
            ui.button("Close (Esc)").clicked()
        });
        self.set_standard(pack);
        if modal.inner || modal.should_close() {
            self.wb.project_settings = false;
        }
    }
}
