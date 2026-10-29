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
        self.settle_dialogs_after_resize(ctx);
        if crate::theme::set_dark(ctx.theme() == egui::Theme::Dark) || restyle {
            self.canvas.restyle();
            self.cabinet.restyle();
        }
    }

    /// A dialog is centred from the size it had last frame, and a modal's
    /// size includes its backdrop, which is the whole screen. When the screen
    /// shrinks (a smaller Interface size, a smaller window) the dialog keeps
    /// the old, larger size and sits partly off screen. Starting the areas
    /// afresh while a dialog is open, on the frame the size changed, centres
    /// it again.
    fn settle_dialogs_after_resize(&self, ctx: &egui::Context) {
        let size = ctx.content_rect().size();
        let id = egui::Id::new("screen_size_seen");
        let seen = ctx.data(|d| d.get_temp::<egui::Vec2>(id));
        ctx.data_mut(|d| d.insert_temp(id, size));
        if seen.is_some_and(|s| s != size) && ctx.memory(|m| m.top_modal_layer().is_some()) {
            ctx.memory_mut(|m| m.reset_areas());
        }
    }
}
