//! The New crane project window (File > Templates and examples > Overhead crane):
//! how many hoist units, and how the hoists and the travel motions are
//! driven. The answers pick the circuits the template's pages carry.

use eframe::egui;
use novo_theme::widgets;

use super::NovoApp;
use super::template_crane::{CraneOptions, Drive, MAX_HOIST_UNITS};

const DRIVES: [&str; 2] = ["Two-speed, contactors", "Frequency drive"];

fn drive_index(d: Drive) -> usize {
    usize::from(d == Drive::Frequency)
}

fn drive_of(i: usize) -> Drive {
    if i == 1 {
        Drive::Frequency
    } else {
        Drive::TwoSpeed
    }
}

impl NovoApp {
    /// Opens the window with the usual choices.
    pub(super) fn open_crane_options(&mut self) {
        self.wb.crane_new = Some(CraneOptions::default());
    }

    pub(super) fn crane_options_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut options) = self.wb.crane_new else {
            return;
        };
        let mut create = false;
        let modal = super::panel::modal(
            ctx,
            "crane_new",
            "New crane project",
            [novo_theme::dialog_width::MEDIUM, 0.0],
            |ui| {
                super::dialog_ui::hint(
                    ui,
                    "Starts a project with the crane structure, the cover pages and a circuit \
from the library on each page. You can change all of it afterwards.",
                );
                super::dialog_ui::section(ui, "Hoist units", true);
                let mut units = usize::from(options.hoist_units) - 1;
                let labels: Vec<String> = (1..=MAX_HOIST_UNITS).map(|n| n.to_string()).collect();
                let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
                if widgets::segmented(ui, &mut units, &labels) {
                    options.hoist_units = units as u8 + 1;
                }
                super::dialog_ui::hint(ui, "Each hoist unit gets its own cabinets and a page.");
                super::dialog_ui::section(ui, "Hoist drive", false);
                let mut hoist = drive_index(options.hoist_drive);
                if widgets::segmented(ui, &mut hoist, &DRIVES) {
                    options.hoist_drive = drive_of(hoist);
                }
                super::dialog_ui::section(ui, "Travel drive (bridge and crab)", false);
                let mut travel = drive_index(options.travel_drive);
                if widgets::segmented(ui, &mut travel, &DRIVES) {
                    options.travel_drive = drive_of(travel);
                }
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    create = widgets::primary_button(ui, "Create project").clicked();
                });
            },
        );
        if create {
            self.wb.crane_new = None;
            self.new_crane_project(options);
        } else if modal.should_close() {
            self.wb.crane_new = None;
        } else {
            self.wb.crane_new = Some(options);
        }
    }
}
