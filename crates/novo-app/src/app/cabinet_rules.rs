//! Place rules on the Cabinet canvas (Pro Panel gap analysis, R7): the rule
//! set Place all follows (`novo_layout::rules`), as a text the user edits in
//! the Plate card and as a file in the user's profile.
//!
//! The file is `place-rules.txt` next to `keymap.txt` under `%APPDATA%`; a
//! missing file means the standard rules, and Reset deletes it. A text with
//! a mistake is never applied: the first bad line is named under the box.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_layout::rules::RuleSet;

use super::cabinet::Ask;
use super::pdf::write_replacing;
use super::{Level, NovoApp};

/// File name inside the KoperCAD folder under `%APPDATA%`.
pub const RULES_FILE: &str = "place-rules.txt";

/// `%APPDATA%\KoperCAD\place-rules.txt` (the platform's data folder elsewhere).
pub fn rules_path() -> Option<PathBuf> {
    Some(
        dirs::data_dir()?
            .join(crate::DATA_DIR_NAME)
            .join(RULES_FILE),
    )
}

/// The rule set Place all uses, with the editor's text and where it is kept.
#[derive(Debug, Clone)]
pub(super) struct RulesState {
    pub(super) rules: RuleSet,
    /// The file the rules are kept in; `None` keeps them for this session only.
    pub(super) path: Option<PathBuf>,
    /// The editor's text, edited freely until Apply.
    text: String,
    /// The rule set `text` was filled from, so an outside change refills it.
    source: RuleSet,
    /// The first mistake of the last Apply.
    error: Option<String>,
}

impl Default for RulesState {
    fn default() -> Self {
        let rules = RuleSet::standard();
        RulesState {
            text: rules.to_text(),
            source: rules.clone(),
            rules,
            path: None,
            error: None,
        }
    }
}

impl RulesState {
    /// The state for the file at `path`, and a warning when the file could
    /// not be used. A missing or unreadable file means the standard rules.
    pub(super) fn load(path: PathBuf) -> (RulesState, Option<String>) {
        let mut state = RulesState {
            path: Some(path.clone()),
            ..RulesState::default()
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return (state, None);
        };
        match RuleSet::parse(&text) {
            Ok(rules) => {
                state.text = rules.to_text();
                state.source = rules.clone();
                state.rules = rules;
                (state, None)
            }
            Err(e) => (
                state,
                Some(format!(
                    "{RULES_FILE} {e}; Place all uses the standard rules until it is fixed"
                )),
            ),
        }
    }

    /// The editor, inside the Plate card. Returns what the user asked for.
    pub(super) fn show(&mut self, ui: &mut egui::Ui) -> Option<Ask> {
        if self.source != self.rules {
            self.text = self.rules.to_text();
            self.source = self.rules.clone();
        }
        let mut ask = None;
        ui.weak(
            "Where Place all puts each kind of device: a zone of the plate (top, middle, \
             bottom), own or shared rails, gaps, and a heat rule. One rule per line, \
             for example `rule Q F G zone=top rail=shared`.",
        );
        ui.add_space(4.0);
        ui.add(
            egui::TextEdit::multiline(&mut self.text)
                .code_editor()
                .desired_rows(10)
                .desired_width(f32::INFINITY),
        );
        let changed = match RuleSet::parse(&self.text) {
            Ok(parsed) => {
                self.error = None;
                (parsed != self.rules).then_some(parsed)
            }
            Err(e) => {
                self.error = Some(e.to_string());
                None
            }
        };
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.horizontal(|ui| {
            let apply = egui::Button::new("Apply");
            let enabled = changed.is_some();
            if ui
                .add_enabled(enabled, apply)
                .on_disabled_hover_text(if self.error.is_some() {
                    "Fix the line above first"
                } else {
                    "Nothing changed"
                })
                .clicked()
                && let Some(rules) = changed
            {
                ask = Some(Ask::SetPlaceRules(rules));
            }
            if ui
                .add_enabled(
                    self.rules != RuleSet::standard() || self.text != self.rules.to_text(),
                    egui::Button::new("Reset to standard"),
                )
                .clicked()
            {
                ask = Some(Ask::ResetPlaceRules);
            }
        });
        if let Some(path) = &self.path {
            ui.weak(format!("Kept in {}", path.display()));
        }
        ask
    }
}

impl NovoApp {
    /// Reads the user's place rules file, if there is one, and says once
    /// when it could not be used.
    pub(super) fn load_place_rules(&mut self) {
        let Some(path) = rules_path() else {
            return;
        };
        let (state, warning) = RulesState::load(path);
        self.cabinet.rules = state;
        if let Some(warning) = warning {
            self.message(Level::Warning, warning);
        }
    }

    /// Uses and keeps a new rule set.
    pub(super) fn set_place_rules(&mut self, rules: RuleSet) {
        self.cabinet.rules.rules = rules;
        let kept = self.cabinet.rules.path.clone();
        match kept {
            Some(path) => match write_rules(&path, &self.cabinet.rules.rules) {
                Ok(()) => self.message(
                    Level::Info,
                    format!("Place rules saved to {}", path.display()),
                ),
                Err(e) => self.message(
                    Level::Warning,
                    format!(
                        "Place rules apply to this session only: could not write {}: {e}",
                        path.display()
                    ),
                ),
            },
            None => self.message(Level::Info, "Place rules changed"),
        }
    }

    /// Goes back to the standard rules and removes the user's file.
    pub(super) fn reset_place_rules(&mut self) {
        self.cabinet.rules.rules = RuleSet::standard();
        self.cabinet.rules.text = self.cabinet.rules.rules.to_text();
        self.cabinet.rules.source = self.cabinet.rules.rules.clone();
        self.cabinet.rules.error = None;
        if let Some(path) = self.cabinet.rules.path.clone()
            && path.exists()
            && let Err(e) = std::fs::remove_file(&path)
        {
            self.message(
                Level::Warning,
                format!("Could not remove {}: {e}", path.display()),
            );
            return;
        }
        self.message(Level::Info, "Place rules are the standard rules again");
    }
}

fn write_rules(path: &Path, rules: &RuleSet) -> std::io::Result<()> {
    write_replacing(path, rules.to_text().as_bytes())
}
