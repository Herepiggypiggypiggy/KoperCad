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

/// Rows the menu puts first, in this order; the rest follow in registry
/// order.
const PREFERRED: &[&str] = &["project.rules", "project.standard", "app.preferences"];

/// The row's label inside a submenu, where the section name already says
/// what it does: "Export PDF" is "PDF" under Export.
fn short_label(section: Section, label: &str) -> String {
    let verb = section.label();
    let rest = label
        .strip_prefix(verb)
        .map(str::trim_start)
        .filter(|r| !r.is_empty());
    match rest {
        Some(r) => {
            let mut chars = r.chars();
            let first = chars.next().map(|c| c.to_uppercase().to_string());
            // Keep acronyms (PDF, BOM) as they are.
            let keep = r.chars().nth(1).is_some_and(char::is_uppercase);
            match first {
                Some(f) if !keep => f + chars.as_str(),
                _ => r.to_owned(),
            }
        }
        None => label.to_owned(),
    }
}

/// Whether the section's rows go into a submenu: only when it holds two or
/// more, so a single export stays one click.
fn is_submenu(section: Section, count: usize) -> bool {
    matches!(section, Section::Import | Section::Export) && count >= 2
}

fn sort_preferred(items: &mut [&'static CommandInfo]) {
    items.sort_by_key(|c| {
        PREFERRED
            .iter()
            .position(|p| *p == c.id)
            .unwrap_or(usize::MAX)
    });
}

impl NovoApp {
    /// The menu sections below the project entries. Sections with nothing
    /// the app runs are left out. Clicks are returned, not run, because
    /// the caller is inside the menu closure.
    pub(super) fn file_menu_sections(&self, ui: &mut egui::Ui) -> Vec<&'static str> {
        let mut picked = Vec::new();
        let context = self.command_context();
        for section in Section::ALL {
            let mut items = section_commands(section, commands::COMMANDS, app_runs);
            if items.is_empty() {
                continue;
            }
            sort_preferred(&mut items);
            ui.separator();
            if is_submenu(section, items.len()) {
                ui.menu_button(section.label(), |ui| {
                    self.command_buttons(ui, section, &items, &context, true, &mut picked)
                });
            } else {
                if items.len() > 1 || !matches!(section, Section::Import | Section::Export) {
                    novo_theme::widgets::section_label(ui, section.label());
                }
                self.command_buttons(ui, section, &items, &context, false, &mut picked);
            }
        }
        picked
    }

    fn command_buttons(
        &self,
        ui: &mut egui::Ui,
        section: Section,
        items: &[&'static CommandInfo],
        context: &commands::Context,
        short: bool,
        picked: &mut Vec<&'static str>,
    ) {
        for command in items {
            let state = availability(command, context);
            let label = if short {
                short_label(section, command.label)
            } else {
                command.label.to_owned()
            };
            let mut button = egui::Button::new(label);
            if let Some(keys) = self.wb.keymap.shortcut_text(command.id) {
                button = button.shortcut_text(keys);
            }
            let response = ui.add_enabled(state.is_ok(), button);
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

    #[test]
    fn labels_in_a_submenu_drop_the_repeated_verb() {
        assert_eq!(short_label(Section::Export, "Export PDF"), "PDF");
        assert_eq!(
            short_label(Section::Export, "Export bill of materials"),
            "Bill of materials"
        );
        assert_eq!(short_label(Section::Import, "Spreadsheet"), "Spreadsheet");
    }

    #[test]
    fn one_export_is_a_plain_row_and_two_make_a_submenu() {
        assert!(!is_submenu(Section::Export, 1));
        assert!(is_submenu(Section::Export, 2));
        assert!(!is_submenu(Section::Settings, 5));
    }

    #[test]
    fn settings_rows_follow_the_preferred_order() {
        let mut items = section_commands(Section::Settings, commands::COMMANDS, app_runs);
        sort_preferred(&mut items);
        let ids: Vec<_> = items.iter().map(|c| c.id).collect();
        let at = |id| ids.iter().position(|i| *i == id);
        if let (Some(std), Some(prefs)) = (at("project.standard"), at("app.preferences")) {
            assert!(std < prefs);
        }
    }
}
