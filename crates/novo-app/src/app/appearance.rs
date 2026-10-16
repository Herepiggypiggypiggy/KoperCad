//! Theme and density: the person's choice reaches egui and the canvases.

use eframe::egui;

use super::NovoApp;

impl NovoApp {
    /// Applies the chosen theme and density when they changed, and restyles
    /// the canvases when the resolved theme (dark or light) differs from
    /// the one they were drawn with. Cheap when nothing changed.
    pub(super) fn sync_appearance(&mut self, ctx: &egui::Context) {
        let choice = (self.settings.theme_choice(), self.settings.density_choice());
        if self.appearance_applied != Some(choice) {
            novo_theme::set_density(ctx, choice.1);
            novo_theme::set_theme(ctx, choice.0);
            self.appearance_applied = Some(choice);
        }
        if crate::theme::set_dark(ctx.theme() == egui::Theme::Dark) {
            self.canvas.restyle();
            self.cabinet.restyle();
        }
    }
}
