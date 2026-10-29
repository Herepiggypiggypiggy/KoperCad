//! The Project rules window: the explicit rules a project carries in its
//! file (`ProjectInfo.rules`): what terminals with the same name mean, how
//! joined clamps are linked, whether a terminal named PE or N defines that
//! potential, and which generated pages the project wants.
//!
//! The window also holds the company pin templates (the usual pin names per
//! device class, `novo_symbols::parse_company_templates`) as TOML text in the
//! project property `pin.templates`, which the Pin names check reads.
//!
//! The company file `%APPDATA%\KoperCAD\pin-templates.toml` (the platform's
//! data folder elsewhere) is the same text shared by all of a person's
//! projects: "Load company file" fills the box from it, "Save as company file"
//! writes the box to it. The project keeps its own copy, so it travels with
//! the project file and the check stays a pure function of the project.
//!
//! Where the user finds it: Project > Rules, Ctrl+K "rules".

use eframe::egui;
use novo_core::{JoinLink, ProjectRules, SameName, Value};

use super::{Level, NovoApp};

/// File name of the shared company pin templates in the KoperCAD data folder.
pub const PIN_TEMPLATES_FILE: &str = "pin-templates.toml";

/// `%APPDATA%\KoperCAD\pin-templates.toml`.
pub fn shared_pin_templates_path() -> Option<std::path::PathBuf> {
    Some(
        novo_storage::profile::data_dir()?
            .join(crate::DATA_DIR_NAME)
            .join(PIN_TEMPLATES_FILE),
    )
}

/// The text of the company file, or what is wrong with it in plain words.
fn read_shared_templates(path: &std::path::Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("No company file to load ({}): {e}", path.display()))?;
    novo_symbols::parse_company_templates(&text)
        .map_err(|e| format!("The company file is not readable: {e}"))?;
    Ok(text)
}

/// Writes `text` (already checked to read) as the company file.
fn write_shared_templates(path: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// The report kinds a project can switch off, as (generator id, title).
fn generated_kinds() -> Vec<(String, String)> {
    novo_reports::standard_reports()
        .into_iter()
        .filter(|r| r.proposed_by_default())
        .map(|r| (r.id().to_owned(), r.title()))
        .collect()
}

impl NovoApp {
    /// The rules the project carries now.
    pub(super) fn project_rules(&self) -> ProjectRules {
        self.backend
            .document()
            .map(|d| d.project().info().rules.clone())
            .unwrap_or_default()
    }

    /// Stores changed rules as one undo step. A rule the model refuses (same
    /// name "Refuse" while a strip has two terminals of one name) is
    /// reported and leaves the project as it was. When a generated kind was
    /// switched back on, its page is added again if it has something to list.
    pub(super) fn set_project_rules(&mut self, rules: ProjectRules) {
        let before = self.project_rules();
        if before == rules {
            return;
        }
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let result = doc.transact("Project rules", |tx| {
            novo_terminals::set_rules(tx, rules.terminals)?;
            tx.set_info(|i| {
                i.rules.potentials = rules.potentials;
                i.rules.generated_pages_off = rules.generated_pages_off.clone();
            })
        });
        match result {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                let back_on = before
                    .generated_pages_off
                    .iter()
                    .any(|k| !rules.generated_pages_off.contains(k));
                if back_on {
                    self.add_generated_pages();
                }
            }
            Err(e) => self.message(Level::Warning, format!("Project rules: {e}")),
        }
    }

    /// The stored company pin templates text, empty when there is none.
    fn stored_pin_templates(&self) -> String {
        self.backend
            .document()
            .and_then(|d| {
                match d
                    .project()
                    .info()
                    .properties
                    .get(novo_checks::PIN_TEMPLATES_PROPERTY)
                {
                    Some(Value::Text(t)) => Some(t.clone()),
                    _ => None,
                }
            })
            .unwrap_or_default()
    }

    /// Stores the company pin templates as one undo step; empty text removes
    /// them. The text was read by `parse_company_templates` before.
    fn set_pin_templates(&mut self, text: &str) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let text = text.trim().to_owned();
        let result = doc.transact("Company pin templates", |tx| {
            tx.set_info(|i| {
                if text.is_empty() {
                    i.properties.remove(novo_checks::PIN_TEMPLATES_PROPERTY);
                } else {
                    i.properties.insert(
                        novo_checks::PIN_TEMPLATES_PROPERTY.to_owned(),
                        Value::Text(text.clone()),
                    );
                }
            })
        });
        match result {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Warning, format!("Pin templates: {e}")),
        }
    }

    pub(super) fn project_rules_dialog(&mut self, ctx: &egui::Context) {
        if !self.wb.project_rules {
            return;
        }
        if self.backend.document().is_none() {
            self.wb.project_rules = false;
            return;
        }
        let stored = self.project_rules();
        let mut rules = stored.clone();
        let stored_templates = self.stored_pin_templates();
        let mut templates = self
            .wb
            .pin_templates_text
            .take()
            .unwrap_or_else(|| stored_templates.clone());
        let mut save_templates = false;
        let (mut load_shared, mut save_shared) = (false, false);
        let kinds = generated_kinds();
        let width = novo_theme::dialog_width::MEDIUM;
        // The body scrolls and the Done button stays in view at every window
        // size (the window is taller than the screen with the pin names box).
        let modal = super::panel::modal_fill(
            ctx,
            "project_rules",
            "Project rules",
            [width, 640.0],
            |ui| {
                super::panel::fill_above(ui, 44.0, |ui| {
                    super::dialog_ui::hint(
                        ui,
                        "Saved in the project file. Every rule is explicit; undo takes one back.",
                    );
                    super::dialog_ui::section(ui, "Terminals with the same name on a strip", true);
                    let mut same_name = rules.terminals.same_name;
                    super::dialog_ui::choice(
                        ui,
                        "Same name",
                        "Join: two -X2:1 are one electrical point, built from several clamps. Refuse: join duplicates first, a strip with two of one name blocks this. Off: keep them separate and list them as a finding.",
                        &mut same_name,
                        &[
                            (SameName::Join, "Join them"),
                            (SameName::Refuse, "Refuse a second"),
                            (SameName::Off, "No rule"),
                        ],
                    );
                    rules.terminals.same_name = same_name;
                    ui.add_enabled_ui(rules.terminals.same_name == SameName::Join, |ui| {
                        super::dialog_ui::choice(
                            ui,
                            "Clamps of a joined terminal are linked by",
                            "",
                            &mut rules.terminals.join_link,
                            &[
                                (JoinLink::Connector, "Connector"),
                                (JoinLink::Jumper, "Jumper"),
                            ],
                        );
                    });
                    super::dialog_ui::section(ui, "Supply potentials", false);
                    ui.checkbox(
                        &mut rules.potentials.from_terminal_names,
                        "A terminal named PE, N or L1 defines that potential",
                    )
                    .on_hover_text("-X4:PE puts PE on its net without any other action");
                    super::dialog_ui::section(ui, "Generated pages", false);
                    super::dialog_ui::hint(
                        ui,
                        "Pages the project adds and keeps current. Deleting one switches it off here.",
                    );
                    ui.add_space(4.0);
                    for (id, title) in &kinds {
                        let mut wanted = rules.wants_generated(id);
                        if ui.checkbox(&mut wanted, title.as_str()).changed() {
                            if wanted {
                                rules.generated_pages_off.remove(id);
                            } else {
                                rules.generated_pages_off.insert(id.clone());
                            }
                        }
                    }
                    super::dialog_ui::section(ui, "Usual pin names", false);
                    super::dialog_ui::hint(
                        ui,
                        "Your own pin names per device class, as TOML. A class replaces the built-in \
                     names for the symbol functions it lists; Check > Issues then flags \
                     devices that differ (kind \"Pin names\"). Leave empty for the built-in names.",
                    );
                    ui.add(
                    egui::TextEdit::multiline(&mut templates)
                        .code_editor()
                        .desired_rows(5)
                        .desired_width(f32::INFINITY)
                        .hint_text("[[class]]\nid = \"motor.brake\"\nfunctions = [\"motor.ac3\"]\n[[class.set]]\nname = \"with brake\"\nrequired = [\"U1\", \"V1\", \"W1\", \"BD1\", \"BD2\"]"),
                );
                    let p = crate::theme::ui_palette();
                    let parsed = novo_symbols::parse_company_templates(&templates);
                    match &parsed {
                        Ok(_) if templates.trim().is_empty() => {
                            ui.label(
                                egui::RichText::new("Built-in names only").color(p.text_secondary),
                            );
                        }
                        Ok(classes) => {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} read",
                                    if classes.len() == 1 {
                                        "1 class".to_owned()
                                    } else {
                                        format!("{} classes", classes.len())
                                    }
                                ))
                                .color(p.success),
                            );
                        }
                        Err(e) => {
                            ui.label(egui::RichText::new(e.as_str()).color(p.error));
                        }
                    }
                    let changed = templates.trim() != stored_templates.trim();
                    if ui
                        .add_enabled(
                            parsed.is_ok() && changed,
                            egui::Button::new("Save pin names"),
                        )
                        .clicked()
                    {
                        save_templates = true;
                    }
                    ui.horizontal(|ui| {
                        load_shared = ui
                            .button("Load company file")
                            .on_hover_text(
                                "Fills the box from the pin names file shared by all your projects \
                                 (in the KoperCAD data folder). Save pin names keeps it in this project.",
                            )
                            .clicked();
                        save_shared = ui
                            .add_enabled(parsed.is_ok(), egui::Button::new("Save as company file"))
                            .on_hover_text("Makes the text above the pin names of all your projects.")
                            .clicked();
                    });
                });
                super::dialog_ui::done(ui)
            },
        );
        if save_templates {
            self.set_pin_templates(&templates);
        }
        if load_shared {
            match shared_pin_templates_path()
                .ok_or_else(|| "No data folder for the company file".to_owned())
                .and_then(|p| read_shared_templates(&p))
            {
                Ok(text) => {
                    templates = text;
                    self.message(
                        Level::Info,
                        "Loaded the company pin names; Save pin names keeps them in this project",
                    );
                }
                Err(e) => self.message(Level::Warning, e),
            }
        }
        if save_shared {
            match shared_pin_templates_path()
                .ok_or_else(|| "No data folder for the company file".to_owned())
                .and_then(|p| write_shared_templates(&p, &templates))
            {
                Ok(()) => self.message(Level::Info, "Saved as the company pin names file"),
                Err(e) => self.message(Level::Error, format!("Could not save the file: {e}")),
            }
        }
        if !(modal.inner || modal.should_close()) {
            self.wb.pin_templates_text = Some(templates);
        }
        self.set_project_rules(rules);
        if modal.inner || modal.should_close() {
            self.wb.project_rules = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "[[class]]\nid = 'motor.brake'\nfunctions = ['motor.ac3']\n[[class.set]]\nname = 'with brake'\nrequired = ['U1', 'V1', 'W1', 'BD1', 'BD2']\n";

    #[test]
    fn the_company_file_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("KoperCAD").join(PIN_TEMPLATES_FILE);
        write_shared_templates(&path, GOOD).unwrap();
        assert_eq!(read_shared_templates(&path).unwrap(), GOOD);
    }

    #[test]
    fn a_missing_or_unreadable_company_file_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(PIN_TEMPLATES_FILE);
        assert!(
            read_shared_templates(&path)
                .unwrap_err()
                .contains("No company file")
        );
        std::fs::write(&path, "[[class]]\nid = ").unwrap();
        assert!(
            read_shared_templates(&path)
                .unwrap_err()
                .contains("not readable")
        );
    }
}
