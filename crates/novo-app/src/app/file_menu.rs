//! The project menu's data sections. Besides the hand-placed project
//! entries (new, open, save), the menu lists commands by the prefix of
//! their id, so a feature appears in the menu by being registered in
//! `novo-workflow` and handled in `run_command`, with no edit here:
//!
//! | prefix | menu section |
//! | --- | --- |
//! | `import.` | Import |
//! | `export.` | Export |
//! | `parts.`, `macro.`, `library.` | Libraries |
//! | `revision.` | Revisions |
//! | `project.`, `app.` | Settings |
//!
//! Only commands the app runs are listed, so nothing in the menu is a
//! promise. Greyed entries say why in a tooltip (no project open, ...).

use eframe::egui;
use novo_workflow::commands::{self, CommandInfo, availability};

use super::{NovoApp, workbench::app_runs};

/// A menu section and the id prefixes it collects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Import,
    Export,
    Libraries,
    Revisions,
    Settings,
}

impl Section {
    pub(crate) const ALL: [Section; 5] = [
        Section::Libraries,
        Section::Import,
        Section::Export,
        Section::Revisions,
        Section::Settings,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Section::Import => "Import",
            Section::Export => "Export",
            Section::Libraries => "Libraries",
            Section::Revisions => "Revisions",
            Section::Settings => "Settings",
        }
    }

    fn prefixes(self) -> &'static [&'static str] {
        match self {
            Section::Import => &["import."],
            Section::Export => &["export."],
            Section::Libraries => &["parts.", "macro.", "library."],
            Section::Revisions => &["revision."],
            Section::Settings => &["project.", "app."],
        }
    }

    /// Whether a command id belongs here.
    pub(crate) fn takes(self, id: &str) -> bool {
        self.prefixes().iter().any(|p| id.starts_with(p))
    }
}

/// The commands of one section, in registry order, keeping those `runs`
/// accepts.
pub(crate) fn section_commands(
    section: Section,
    all: &'static [CommandInfo],
    runs: impl Fn(&str) -> bool,
) -> Vec<&'static CommandInfo> {
    all.iter()
        .filter(|c| section.takes(c.id) && runs(c.id))
        .collect()
}

impl NovoApp {
    /// The menu sections below the project entries. Sections with nothing
    /// the app runs are left out. Clicks are returned, not run, because
    /// the caller is inside the menu closure.
    pub(super) fn file_menu_sections(&self, ui: &mut egui::Ui) -> Vec<&'static str> {
        let mut picked = Vec::new();
        let context = self.command_context();
        for section in Section::ALL {
            let items = section_commands(section, commands::COMMANDS, app_runs);
            if items.is_empty() {
                continue;
            }
            ui.separator();
            if matches!(section, Section::Import | Section::Export) {
                ui.menu_button(section.label(), |ui| {
                    command_buttons(ui, &items, &context, &mut picked)
                });
            } else {
                command_buttons(ui, &items, &context, &mut picked);
            }
        }
        picked
    }
}

fn command_buttons(
    ui: &mut egui::Ui,
    items: &[&'static CommandInfo],
    context: &commands::Context,
    picked: &mut Vec<&'static str>,
) {
    for command in items {
        let state = availability(command, context);
        let response = ui.add_enabled(state.is_ok(), egui::Button::new(command.label));
        let response = match state {
            Err(reason) => response.on_disabled_hover_text(reason),
            Ok(()) => response,
        };
        if response.clicked() {
            picked.push(command.id);
            ui.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_land_in_the_section_of_their_prefix() {
        assert!(Section::Export.takes("export.pdf"));
        assert!(Section::Import.takes("import.sheet"));
        assert!(Section::Libraries.takes("parts.browser"));
        assert!(Section::Libraries.takes("macro.browser"));
        assert!(Section::Revisions.takes("revision.compare"));
        assert!(Section::Settings.takes("app.preferences"));
        assert!(!Section::Export.takes("file.save"));
        assert!(Section::ALL.iter().all(|s| !s.takes("edit.copy")));
    }

    #[test]
    fn every_id_is_in_at_most_one_section() {
        for c in commands::COMMANDS {
            let n = Section::ALL.iter().filter(|s| s.takes(c.id)).count();
            assert!(n <= 1, "{} is in {n} sections", c.id);
        }
    }

    #[test]
    fn only_commands_the_app_runs_are_listed() {
        let all = commands::COMMANDS;
        let none = section_commands(Section::Export, all, |_| false);
        assert!(none.is_empty());
        let some = section_commands(Section::Export, all, |id| id == "export.pdf");
        assert_eq!(some.len(), 1);
        assert_eq!(some[0].id, "export.pdf");
    }

    #[test]
    fn the_app_runs_pdf_export_and_preferences() {
        let items = |s| section_commands(s, commands::COMMANDS, app_runs);
        assert!(items(Section::Export).iter().any(|c| c.id == "export.pdf"));
        assert!(
            items(Section::Settings)
                .iter()
                .any(|c| c.id == "app.preferences")
        );
    }
}
