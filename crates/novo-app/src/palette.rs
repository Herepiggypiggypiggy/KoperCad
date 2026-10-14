//! Command palette: type a few letters, press Enter to run the action.

use crate::actions::Action;
use eframe::egui;

#[derive(Default)]
pub struct Palette {
    open: bool,
    query: String,
    selected: usize,
}

impl Palette {
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Shows the palette if it is open and returns the action the user picked.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        is_enabled: impl Fn(Action) -> bool,
    ) -> Option<Action> {
        if !self.open {
            return None;
        }

        let matches = matching_actions(&self.query, &is_enabled);
        self.selected = self.selected.min(matches.len().saturating_sub(1));

        let (up, down, enter, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if down && self.selected + 1 < matches.len() {
            self.selected += 1;
        }

        let mut picked = None;
        let modal = egui::Modal::new(egui::Id::new("command_palette"))
            .area(
                egui::Modal::default_area(egui::Id::new("command_palette"))
                    .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0]),
            )
            .show(ctx, |ui| {
                ui.set_width(460.0);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Type a command")
                        .desired_width(f32::INFINITY),
                );
                edit.request_focus();
                if edit.changed() {
                    self.selected = 0;
                }
                ui.separator();
                if matches.is_empty() {
                    ui.weak("No matching command");
                }
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for (row, &action) in matches.iter().enumerate() {
                            let shortcut = action
                                .shortcut()
                                .map(|s| ui.ctx().format_shortcut(&s))
                                .unwrap_or_default();
                            let text = format!("{}: {}", action.category(), action.label());
                            let response = ui.add(
                                egui::Button::selectable(row == self.selected, text)
                                    .shortcut_text(shortcut)
                                    .min_size(egui::vec2(ui.available_width(), 0.0)),
                            );
                            if row == self.selected {
                                response.scroll_to_me(None);
                            }
                            if response.clicked() {
                                picked = Some(action);
                            }
                        }
                    });
            });

        if enter {
            picked = picked.or_else(|| matches.get(self.selected).copied());
        }
        if picked.is_some() || escape || modal.backdrop_response.clicked() {
            self.open = false;
        }
        picked
    }
}

/// Enabled actions that fuzzy-match `query`, best match first.
pub fn matching_actions(query: &str, is_enabled: impl Fn(Action) -> bool) -> Vec<Action> {
    let mut scored: Vec<(i32, usize, Action)> = Action::ALL
        .iter()
        .enumerate()
        .filter(|&(_, &a)| a != Action::CommandPalette && is_enabled(a))
        .filter_map(|(order, &a)| {
            let haystack = format!("{} {}", a.category(), a.label());
            fuzzy_score(query, &haystack).map(|score| (score, order, a))
        })
        .collect();
    scored.sort_by_key(|&(score, order, _)| (std::cmp::Reverse(score), order));
    scored.into_iter().map(|(_, _, a)| a).collect()
}

/// Scores `haystack` against `query` when every query character appears in
/// order (case-insensitive). Consecutive runs and word starts score higher.
pub fn fuzzy_score(query: &str, haystack: &str) -> Option<i32> {
    let hay: Vec<char> = haystack.to_lowercase().chars().collect();
    let mut score = 0;
    let mut pos = 0;
    let mut prev_match: Option<usize> = None;
    for qc in query.to_lowercase().chars().filter(|c| !c.is_whitespace()) {
        let found = (pos..hay.len()).find(|&i| hay[i] == qc)?;
        score += 1;
        if prev_match == Some(found.wrapping_sub(1)) {
            score += 3;
        }
        if found == 0 || !hay[found - 1].is_alphanumeric() {
            score += 2;
        }
        prev_match = Some(found);
        pos = found + 1;
    }
    Some(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(fuzzy_score("", "File Save"), Some(0));
    }

    #[test]
    fn requires_characters_in_order() {
        assert!(fuzzy_score("sav", "File Save").is_some());
        assert!(fuzzy_score("vas", "File Save").is_none());
    }

    #[test]
    fn prefers_word_starts_and_runs() {
        let tight = fuzzy_score("save", "File Save").unwrap();
        let loose = fuzzy_score("save", "View Theme: system and very easy").unwrap_or(0);
        assert!(tight > loose);
    }

    #[test]
    fn save_query_ranks_save_first() {
        let found = matching_actions("save", |_| true);
        assert_eq!(found.first(), Some(&Action::SaveProject));
    }

    #[test]
    fn hides_disabled_actions_and_itself() {
        let found = matching_actions("", |a| a != Action::Undo);
        assert!(!found.contains(&Action::Undo));
        assert!(!found.contains(&Action::CommandPalette));
    }
}
