//! The New pump station / New motor control centre window (File > New from
//! template): how many pumps or feeders, and the one variant that picks the
//! circuits the template's pages carry.

use eframe::egui;
use novo_theme::widgets;

use super::NovoApp;
use super::template_plants::{Plant, PlantOptions};

impl NovoApp {
    /// Opens the window with the usual choices.
    pub(super) fn open_plant_options(&mut self, plant: Plant) {
        self.wb.plant_new = Some(PlantOptions::new(plant));
    }

    pub(super) fn plant_options_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut options) = self.wb.plant_new else {
            return;
        };
        let plant = options.plant;
        let mut create = false;
        let modal = super::panel::modal(
            ctx,
            "plant_new",
            plant.window_title(),
            [novo_theme::dialog_width::MEDIUM, 0.0],
            |ui| {
                super::dialog_ui::hint(
                    ui,
                    "Starts a project with the structure, the cover pages and a circuit from the \
library on each page. You can change all of it afterwards.",
                );
                super::dialog_ui::section(ui, plant.count_label(), true);
                let mut count = usize::from(options.count) - 1;
                let labels: Vec<String> = (1..=plant.max_count()).map(|n| n.to_string()).collect();
                let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
                if widgets::segmented(ui, &mut count, &labels) {
                    options.count = count as u8 + 1;
                }
                super::dialog_ui::hint(
                    ui,
                    "Each one gets its own page and, in a motor control centre, its own section.",
                );
                super::dialog_ui::section(ui, "Options", false);
                if plant == Plant::PumpStation {
                    ui.checkbox(&mut options.frequency, "Frequency drive on each pump");
                }
                ui.checkbox(&mut options.extra, plant.extra_label());
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    create = widgets::primary_button(ui, "Create project").clicked();
                });
            },
        );
        if create {
            self.wb.plant_new = None;
            self.new_plant_project(options);
        } else if modal.should_close() {
            self.wb.plant_new = None;
        } else {
            self.wb.plant_new = Some(options);
        }
    }
}
