//! Command palette: type a few letters, press Enter to run the pick.
//!
//! The palette searches a list of [`Entry`]s rather than the [`Action`] enum
//! itself, so pages, devices and symbols can join the same search box later
//! as new [`Target`] variants.

use crate::actions::Action;
use eframe::egui;

/// What picking an entry does.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Action(Action),
}

/// One searchable row.
#[derive(Clone, Debug)]
pub struct Entry {
    pub target: Target,
    /// Shown before the label and searched with it, such as "File".
    pub group: String,
    pub label: String,
    pub shortcut: Option<egui::KeyboardShortcut>,
}

impl Entry {
    pub fn action(action: Action) -> Self {
        Entry {
            target: Target::Action(action),
            group: action.category().to_owned(),
            label: action.label().to_owned(),
            shortcut: action.shortcut(),
        }
    }
}

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

    /// Shows the palette if it is open and returns what the user picked.
    /// `recent` lists recently picked targets, newest first.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        entries: &[Entry],
        recent: &[Target],
    ) -> Option<Target> {
        if !self.open {
            return None;
        }

        let ranked = rank(&self.query, entries, recent);
        self.selected = self.selected.min(ranked.len().saturating_sub(1));

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
        if down && self.selected + 1 < ranked.len() {
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
                if ranked.is_empty() {
                    ui.weak("No match");
                }
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for (row, &index) in ranked.iter().enumerate() {
                            let entry = &entries[index];
                            let shortcut = entry
                                .shortcut
                                .map(|s| ui.ctx().format_shortcut(&s))
                                .unwrap_or_default();
                            let text = format!("{}: {}", entry.group, entry.label);
                            let response = ui.add(
                                egui::Button::selectable(row == self.selected, text)
                                    .shortcut_text(shortcut)
                                    .min_size(egui::vec2(ui.available_width(), 0.0)),
                            );
                            if row == self.selected {
                                response.scroll_to_me(None);
                            }
                            if response.clicked() {
                                picked = Some(entry.target.clone());
                            }
                        }
                    });
            });

        if enter {
            picked = picked.or_else(|| {
                ranked
                    .get(self.selected)
                    .map(|&i| entries[i].target.clone())
            });
        }
        if picked.is_some() || escape || modal.backdrop_response.clicked() {
            self.open = false;
        }
        picked
    }
}

/// Indices of the entries that match `query`, best first. Better fuzzy
/// matches win; ties go to the most recently used, then to list order. With
/// an empty query that puts the last thing used at the top.
pub fn rank(query: &str, entries: &[Entry], recent: &[Target]) -> Vec<usize> {
    let mut scored: Vec<(i32, usize, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let haystack = format!("{} {}", entry.group, entry.label);
            let score = fuzzy_score(query, &haystack)?;
            let recency = recent
                .iter()
                .position(|t| *t == entry.target)
                .unwrap_or(usize::MAX);
            Some((score, recency, index))
        })
        .collect();
    scored.sort_by_key(|&(score, recency, index)| (std::cmp::Reverse(score), recency, index));
    scored.into_iter().map(|(_, _, index)| index).collect()
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

    fn all_actions() -> Vec<Entry> {
        Action::ALL.iter().map(|&a| Entry::action(a)).collect()
    }

    fn top(query: &str, entries: &[Entry], recent: &[Target]) -> Option<Target> {
        rank(query, entries, recent)
            .first()
            .map(|&i| entries[i].target.clone())
    }

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
        assert_eq!(
            top("save", &all_actions(), &[]),
            Some(Target::Action(Action::SaveProject))
        );
    }

    #[test]
    fn last_used_is_the_top_row() {
        let recent = [
            Target::Action(Action::ZoomToFit),
            Target::Action(Action::About),
        ];
        let entries = all_actions();
        let order = rank("", &entries, &recent);
        assert_eq!(entries[order[0]].target, recent[0]);
        assert_eq!(entries[order[1]].target, recent[1]);
    }

    #[test]
    fn recency_breaks_ties_but_not_better_matches() {
        // "save" matches Save and Save as equally; the recent one wins.
        let recent = [Target::Action(Action::SaveProjectAs)];
        assert_eq!(
            top("save", &all_actions(), &recent),
            Some(Target::Action(Action::SaveProjectAs))
        );
        // A recent action that matches worse stays below the better match.
        let recent = [Target::Action(Action::About)];
        assert_eq!(
            top("save", &all_actions(), &recent),
            Some(Target::Action(Action::SaveProject))
        );
    }
}
