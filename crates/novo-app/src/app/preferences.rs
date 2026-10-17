//! The Preferences window: per-user settings that are not the project's.

use eframe::egui;
use novo_canvas::model::NoteDensity;
use novo_editor::tool::ConnectMode;

use super::NovoApp;

impl NovoApp {
    pub(super) fn preferences_dialog(&mut self, ctx: &egui::Context) {
        if !self.wb.preferences {
            return;
        }
        let mut mode = self.settings.connect_mode();
        let mut carries = !self.settings.move_leaves_wires;
        let mut calm = self.settings.reduce_motion;
        let mut company = self.settings.company_macros.clone();
        let mut density = self.settings.xref_density();
        let mut theme = self.settings.theme_choice();
        let mut ui_density = self.settings.density_choice();
        let mut dock_labels = self.settings.dock_labels();
        let mut ui_scale = self.settings.ui_scale_factor().to_bits();
        let modal = egui::Modal::new(egui::Id::new("preferences")).show(ctx, |ui| {
            ui.set_width(
                novo_theme::dialog_width::MEDIUM
                    .min(ctx.content_rect().width() - 48.0)
                    .max(240.0),
            );
            ui.label(
                egui::RichText::new("Preferences")
                    .size(15.0)
                    .family(novo_theme::fonts::strong_family()),
            );
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 180.0).max(200.0))
                .show(ui, |ui| {
                    section(ui, "Appearance", true);
                    choice(
                        ui,
                        "Theme",
                        "System follows the light or dark setting of your computer.",
                        &mut theme,
                        &[
                            (novo_theme::ThemeChoice::Dark, "Dark"),
                            (novo_theme::ThemeChoice::Light, "Light"),
                            (novo_theme::ThemeChoice::System, "System"),
                        ],
                    );
                    let densities = novo_theme::Density::ALL.map(|d| (d, d.label()));
                    choice(
                        ui,
                        "Density",
                        "Compact fits more on a small screen; Touch has large controls for a tablet.",
                        &mut ui_density,
                        &densities,
                    );
                    choice(
                        ui,
                        "Words on the tool dock",
                        "Auto shows a word beside each icon when the drawing has room. Hover always shows the name and key.",
                        &mut dock_labels,
                        &[
                            (crate::settings::DockLabels::Auto, "Auto"),
                            (crate::settings::DockLabels::Always, "Always"),
                            (crate::settings::DockLabels::Never, "Icons only"),
                        ],
                    );
                    choice(
                        ui,
                        "Interface size",
                        "On top of Windows scaling. Raise it if text is small on a high resolution screen; lower it on a small laptop screen.",
                        &mut ui_scale,
                        &[
                            (1.0_f32.to_bits(), "100%"),
                            (1.25_f32.to_bits(), "125%"),
                            (1.5_f32.to_bits(), "150%"),
                            (2.0_f32.to_bits(), "200%"),
                        ],
                    );
                    section(ui, "Connections", false);
                    choice(
                        ui,
                        "Auto connections",
                        "Wires between facing free pins after you place or move a symbol.",
                        &mut mode,
                        &[
                            (ConnectMode::Draw, "Draw them"),
                            (ConnectMode::Ask, "Ask me"),
                            (ConnectMode::Never, "Never"),
                        ],
                    );
                    choice(
                        ui,
                        "Moving a symbol",
                        "Shift always does the other one.",
                        &mut carries,
                        &[(false, "Leaves its wires"), (true, "Takes its wires along")],
                    );
                    section(ui, "Cross references", false);
                    choice(
                        ui,
                        "Contact table under a coil",
                        "One row per contact with its symbol, pins and page.column. Compact fits more on a busy sheet.",
                        &mut density,
                        &[(NoteDensity::Full, "Full"), (NoteDensity::Compact, "Compact")],
                    );
                    section(ui, "Macros", false);
                    row_title(
                        ui,
                        "Company macro folder",
                        "A shared folder of macros, e.g. on a network drive. The Macros window lists it and offers newer revisions of macros a project was built from.",
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Choose...").clicked()
                            && let Some(dir) = crate::dialogs::FileDialog::new()
                                .set_title("Company macro folder")
                                .pick_folder()
                        {
                            company = Some(dir);
                        }
                        if company.is_some() && ui.button("Clear").clicked() {
                            company = None;
                        }
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(
                                    company
                                        .as_ref()
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_else(|| "None".into()),
                                )
                                .weak(),
                            )
                            .truncate(),
                        );
                    });
                    section(ui, "Simulation", false);
                    ui.checkbox(&mut calm, "Reduce motion");
                    hint(
                        ui,
                        "The live view switches states at once and shows an arrow on running motors instead of spinning them.",
                    );
                });
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                novo_theme::widgets::primary_button(ui, "Done").clicked()
            })
            .inner
        });
        if mode != self.settings.connect_mode() {
            self.settings.set_connect_mode(mode);
        }
        self.settings.appearance_theme = theme.id().into();
        self.settings.appearance_density = ui_density.id().into();
        self.settings.set_dock_labels(dock_labels);
        self.settings.ui_scale = (f32::from_bits(ui_scale) * 100.0).round() as u32;
        if density != self.settings.xref_density() {
            self.settings.xref_density = density.id().into();
        }
        self.settings.company_macros = company;
        self.settings.move_leaves_wires = !carries;
        self.settings.reduce_motion = calm;
        if modal.inner || modal.should_close() {
            self.wb.preferences = false;
        }
    }
}

/// A section: a caption-caps label, with extra space above (except the first).
fn section(ui: &mut egui::Ui, title: &str, first: bool) {
    ui.add_space(if first { 4.0 } else { 14.0 });
    novo_theme::widgets::section_label(ui, title);
    ui.add_space(4.0);
}

fn hint(ui: &mut egui::Ui, text: &str) {
    ui.add(
        egui::Label::new(
            egui::RichText::new(text)
                .size(egui::TextStyle::Small.resolve(ui.style()).size.max(12.0))
                .weak(),
        )
        .wrap(),
    );
}

fn row_title(ui: &mut egui::Ui, label: &str, text: &str) {
    ui.label(label);
    hint(ui, text);
    ui.add_space(4.0);
}

/// One setting: its name, a line of help and the choices as a segmented
/// control.
fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    help: &str,
    value: &mut T,
    options: &[(T, &str)],
) {
    row_title(ui, label, help);
    let mut index = options.iter().position(|(v, _)| v == value).unwrap_or(0);
    let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
    if novo_theme::widgets::segmented(ui, &mut index, &labels) {
        *value = options[index].0;
    }
    ui.add_space(8.0);
}

#[cfg(test)]
mod stack_tests {
    use super::super::gate1_tests::Drive;
    use crate::actions::Action;
    use eframe::egui;

    #[test]
    fn opening_preferences_closes_the_floating_enclosure_panel() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::OpenExample);
        d.app.sizing.panel_open = true;
        d.app.wb.queued.push("app.preferences");
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(683.0, 384.0),
                )),
                ..Default::default()
            };
            let app = &mut d.app;
            let mut out = d.ctx.run_ui(input, |ui| app.show(ui));
            out.textures_delta.clear();
        }
        assert!(d.app.wb.preferences);
        assert!(!d.app.sizing.panel_open);
    }
}
