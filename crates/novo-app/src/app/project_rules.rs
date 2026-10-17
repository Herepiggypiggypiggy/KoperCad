//! The Project rules window: the explicit rules a project carries in its
//! file (`ProjectInfo.rules`): what terminals with the same name mean, how
//! joined clamps are linked, whether a terminal named PE or N defines that
//! potential, and which generated pages the project wants.
//!
//! Where the user finds it: Project > Rules, Ctrl+K "rules".

use eframe::egui;
use novo_core::{JoinLink, ProjectRules, SameName};

use super::{Level, NovoApp};

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
        let kinds = generated_kinds();
        let modal = egui::Modal::new(egui::Id::new("project_rules")).show(ctx, |ui| {
            ui.set_width(novo_theme::dialog_width::MEDIUM);
            super::dialog_ui::title(
                ui,
                "Project rules",
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
                    &[(JoinLink::Connector, "Connector"), (JoinLink::Jumper, "Jumper")],
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
            super::dialog_ui::done(ui)
        });
        self.set_project_rules(rules);
        if modal.inner || modal.should_close() {
            self.wb.project_rules = false;
        }
    }
}
