//! `parts.describe`: writes device and part descriptions from part data by
//! an editable recipe per kind of device (Edit > Part tools > Describe devices
//! from parts). Recipes are saved in the user's profile.

use eframe::egui;
use novo_parts::{CatalogPart, Facts, RecipeSet, describe_devices, describe_parts, recipes_path};

use super::part_picker::selected_devices;
use super::{Level, NovoApp};

/// The recipe editor with its working copy of the recipes.
pub(crate) struct DescribeDialog {
    set: RecipeSet,
    /// Recipe id being edited.
    kind: String,
    /// Replace descriptions that are already written.
    overwrite: bool,
}

/// A made-up part for the live preview, so the recipe can be judged
/// without a project.
fn sample() -> Facts {
    let mut part = CatalogPart::new("Sample", "S-1");
    part.type_number = "TYPE-09".into();
    for (key, value) in [
        ("poles", novo_core::Value::Int(3)),
        ("ac3_current_a", novo_core::Value::Float(9.0)),
        ("rated_power_kw", novo_core::Value::Float(4.0)),
        ("rated_voltage_v", novo_core::Value::Int(400)),
        ("aux_contacts_no", novo_core::Value::Int(1)),
        ("aux_contacts_nc", novo_core::Value::Int(1)),
        ("coil_voltage_v", novo_core::Value::Int(48)),
        ("coil_frequency_hz", novo_core::Value::Int(50)),
        ("coil_current_type", novo_core::Value::Text("AC".into())),
        ("setting_min_a", novo_core::Value::Float(1.6)),
        ("setting_max_a", novo_core::Value::Float(2.5)),
    ] {
        part.data.insert(key.into(), value);
    }
    Facts::of_catalog(&part)
}

impl NovoApp {
    /// The command `parts.describe`.
    pub(super) fn open_describe_recipes(&mut self) {
        let (set, warning) = recipes_path()
            .map(|p| RecipeSet::load_checked(&p))
            .unwrap_or_default();
        if let Some(why) = warning {
            self.message(
                Level::Warning,
                format!("Using the built-in recipes: {why}. Saving replaces that file."),
            );
        }
        let kind = set.recipes.keys().next().cloned().unwrap_or_default();
        self.wb.describe = Some(DescribeDialog {
            set,
            kind,
            overwrite: false,
        });
    }

    fn apply_descriptions(&mut self, to_parts: bool) {
        let Some(dialog) = &self.wb.describe else {
            return;
        };
        let (set, overwrite) = (dialog.set.clone(), dialog.overwrite);
        let selection = self.selection();
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let command = if to_parts {
            describe_parts(project, &set, overwrite)
        } else {
            // The selection when there is one, otherwise every device.
            let mut devices = selected_devices(project, &selection);
            if devices.is_empty() && overwrite {
                self.message(
                    Level::Info,
                    "Select the devices to rewrite first; Replace is not applied to every device at once",
                );
                return;
            }
            if devices.is_empty() {
                devices = project.devices().map(|d| d.id).collect();
            }
            describe_devices(project, &devices, &set, overwrite)
        };
        let what = if to_parts { "part" } else { "device" };
        let Some(command) = command else {
            let hint = if overwrite {
                ""
            } else {
                " that have none (tick Replace to rewrite existing ones)"
            };
            self.message(
                Level::Info,
                format!("No {what} description to write{hint}: no part of a known kind with data"),
            );
            return;
        };
        let n = match &command {
            novo_core::Command::Batch { commands, .. } => commands.len(),
            _ => 1,
        };
        match self.execute(command) {
            Some(Ok(_)) => self.message(
                Level::Info,
                format!(
                    "Wrote {n} {what} description{} from the recipes",
                    if n == 1 { "" } else { "s" }
                ),
            ),
            Some(Err(e)) => self.message(Level::Error, format!("Could not describe: {e}")),
            None => {}
        }
    }

    pub(super) fn describe_recipes_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.wb.describe else {
            return;
        };
        let palette = crate::theme::ui_palette();
        let (mut apply_devices, mut apply_parts, mut save, mut close) =
            (false, false, false, false);
        let modal = super::panel::modal_fill(
            ctx,
            "describe_recipes",
            "Describe devices from parts",
            [760.0, 560.0],
            |ui| {
                ui.weak(
                    "A recipe writes the description of one kind of device from its part data. \
                     [ ] marks a piece that is left out when its data is unknown; nothing is guessed.",
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Presets:");
                    if ui.small_button("English").clicked() {
                        dialog.set = RecipeSet::english();
                    }
                    if ui.small_button("Dutch").clicked() {
                        dialog.set = RecipeSet::dutch();
                    }
                });
                ui.separator();
                let ids: Vec<String> = dialog.set.recipes.keys().cloned().collect();
                super::panel::fill_above(ui, 76.0, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Kind of device");
                        let shown = dialog
                            .set
                            .recipes
                            .get(&dialog.kind)
                            .map_or(String::new(), |r| r.kind.clone());
                        egui::ComboBox::from_id_salt("describe_kind")
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for id in &ids {
                                    let word = &dialog.set.recipes[id].kind;
                                    if ui.selectable_label(&dialog.kind == id, word).clicked() {
                                        dialog.kind = id.clone();
                                    }
                                }
                            });
                    });
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.vertical(|ui| {
                            let facts = sample();
                            let Some(recipe) = dialog.set.recipes.get_mut(&dialog.kind) else {
                                return;
                            };
                            ui.horizontal(|ui| {
                                ui.label("Kind word");
                                ui.text_edit_singleline(&mut recipe.kind);
                                ui.checkbox(&mut recipe.unit_space, "Space before units");
                            });
                            ui.horizontal(|ui| {
                                ui.label("Contact letters (main, make, break)");
                                for field in [
                                    &mut recipe.marks.main,
                                    &mut recipe.marks.make,
                                    &mut recipe.marks.brk,
                                ] {
                                    ui.add(egui::TextEdit::singleline(field).desired_width(30.0));
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Between segments");
                                ui.add(
                                    egui::TextEdit::singleline(&mut recipe.separator)
                                        .desired_width(50.0),
                                );
                            });
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("Preview on a sample part").strong());
                            ui.monospace(recipe.describe(&facts).text);
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("Segments").strong());
                            let problems = recipe.problems();
                            let mut remove = None;
                            for (at, segment) in recipe.segments.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(segment).desired_width(300.0),
                                    );
                                    if ui.small_button("Remove").clicked() {
                                        remove = Some(at);
                                    }
                                    if let Some((_, e)) = problems.iter().find(|(i, _)| *i == at) {
                                        ui.colored_label(palette.warning, e.to_string());
                                    }
                                });
                            }
                            if let Some(at) = remove {
                                recipe.segments.remove(at);
                            }
                            if ui.small_button("Add segment").clicked() {
                                recipe.segments.push("{kind}".into());
                            }
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Fields").strong());
                            ui.weak(
                                novo_parts::describe::FIELDS
                                    .iter()
                                    .map(|(f, _)| format!("{{{f}}}"))
                                    .collect::<Vec<_>>()
                                    .join("  "),
                            );
                        });
                    });
                });
                ui.add_space(8.0);
                ui.checkbox(
                    &mut dialog.overwrite,
                    "Replace descriptions that are already written",
                );
                ui.horizontal(|ui| {
                    apply_devices =
                        novo_theme::widgets::primary_button(ui, "Describe selected devices")
                            .on_hover_text("With nothing selected, every device")
                            .clicked();
                    apply_parts = ui
                        .button("Describe parts (bill of materials)")
                        .on_hover_text("Fills the description column of the project's parts")
                        .clicked();
                    save = ui.button("Save recipes").clicked();
                    close = novo_theme::widgets::ghost_button(ui, "Close").clicked();
                });
            },
        );
        if save {
            self.save_recipes();
        }
        if apply_devices {
            self.apply_descriptions(false);
        }
        if apply_parts {
            self.apply_descriptions(true);
        }
        if close || modal.should_close() {
            self.wb.describe = None;
        }
    }

    fn save_recipes(&mut self) {
        let Some(dialog) = &self.wb.describe else {
            return;
        };
        let Some(path) = recipes_path() else {
            self.message(Level::Error, "No profile folder to save the recipes in");
            return;
        };
        match dialog.set.save(&path) {
            Ok(()) => self.message(Level::Info, "Recipes saved"),
            Err(e) => self.message(Level::Error, format!("Could not save the recipes: {e}")),
        }
    }
}
