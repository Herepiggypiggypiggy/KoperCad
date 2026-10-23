//! Theme and density: the person's choice reaches egui and the canvases.

use eframe::egui;

use super::NovoApp;

impl NovoApp {
    /// Applies the chosen theme and density when they changed, and restyles
    /// the canvases when the resolved theme (dark or light) differs from
    /// the one they were drawn with. Cheap when nothing changed.
    pub(super) fn sync_appearance(&mut self, ctx: &egui::Context) {
        let choice = (
            self.settings.theme_choice(),
            self.settings.density_choice(),
            self.settings.high_contrast(),
        );
        let mut restyle = false;
        if self.appearance_applied != Some(choice) {
            // The contrast first: the egui styles are built from it.
            restyle = novo_theme::set_high_contrast(choice.2);
            novo_theme::set_density_for(ctx, choice.1, choice.2);
            novo_theme::set_theme(ctx, choice.0);
            self.appearance_applied = Some(choice);
        }
        // Only when the setting changes, so a zoom key press stays in effect.
        let scale = self.settings.ui_scale_factor();
        let id = egui::Id::new("ui_scale_applied");
        let applied = ctx.data(|d| d.get_temp::<f32>(id)).unwrap_or(1.0);
        if (applied - scale).abs() > f32::EPSILON {
            ctx.set_zoom_factor(scale);
            ctx.data_mut(|d| d.insert_temp(id, scale));
        }
        if crate::theme::set_dark(ctx.theme() == egui::Theme::Dark) || restyle {
            self.canvas.restyle();
            self.cabinet.restyle();
        }
    }
}
