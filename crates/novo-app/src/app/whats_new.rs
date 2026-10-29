//! What's new (docs/design/discoverability.md, layer 5): a quiet pill in
//! the top bar while there are entries the person has not seen, and the
//! list behind it (Help > What's new). It never opens by itself.
//!
//! The entries are `assets/whats-new.md`, newest first; opening the list
//! stores the newest id in `Settings::whats_new_seen`. A first run has
//! nothing to compare with, so it starts with nothing unseen.

use eframe::egui;

use super::NovoApp;
use crate::settings::Settings;

/// One entry of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Entry {
    pub id: String,
    pub title: String,
    /// Where the user finds it, from the `Where:` line.
    pub whence: String,
    pub text: String,
}

/// The entries of a `whats-new.md`, in file order (newest first).
pub(super) fn parse(src: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    for line in src.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            let (id, title) = head.split_once(" · ").unwrap_or((head, head));
            entries.push(Entry {
                id: id.trim().to_owned(),
                title: title.trim().to_owned(),
                whence: String::new(),
                text: String::new(),
            });
        } else if let Some(entry) = entries.last_mut() {
            let line = line.trim();
            if let Some(whence) = line.strip_prefix("Where:") {
                entry.whence = whence.trim().to_owned();
            } else if !line.is_empty() {
                if !entry.text.is_empty() {
                    entry.text.push(' ');
                }
                entry.text.push_str(line);
            }
        }
    }
    entries
}

/// The shipped entries.
pub(super) fn entries() -> Vec<Entry> {
    parse(include_str!("../../../../assets/whats-new.md"))
}

/// The entries newer than the one the person last saw. An id that is not
/// in the list (empty, or from a newer build) counts every entry as new.
pub(super) fn unseen<'a>(entries: &'a [Entry], seen: &str) -> &'a [Entry] {
    match entries.iter().position(|e| e.id == seen) {
        Some(n) => &entries[..n],
        None => entries,
    }
}

/// A first run: nothing seen and no walk-through progress, so nothing in
/// the list is news to this person.
pub(super) fn first_run(settings: &Settings) -> bool {
    settings.whats_new_seen.is_empty() && settings.onboarding == Default::default()
}

impl NovoApp {
    /// How many entries the pill counts.
    pub(super) fn whats_new_count(&mut self) -> usize {
        let all = entries();
        if first_run(&self.settings) {
            if let Some(newest) = all.first() {
                self.settings.whats_new_seen = newest.id.clone();
            }
            return 0;
        }
        unseen(&all, &self.settings.whats_new_seen).len()
    }

    /// The pill: `What's new 3`, small, in the top bar while there is
    /// something unseen. A click opens the list.
    pub(super) fn whats_new_pill(&mut self, ui: &mut egui::Ui) {
        let count = self.whats_new_count();
        if count == 0 {
            return;
        }
        // The sparkle and the count: the word is in the tooltip and the menus.
        let response =
            ui.add(egui::Button::new(format!("{} {count}", novo_theme::icons::SPARKLE)).small());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                format!("What's new {count}"),
            )
        });
        let response = response.on_hover_text(format!(
            "What's new: {count} {} since you last looked",
            if count == 1 { "change" } else { "changes" }
        ));
        self.anchor_response("whats_new.pill", &response, "What's new");
        if response.clicked() {
            self.show_whats_new = true;
        }
    }

    pub(super) fn whats_new_window(&mut self, ctx: &egui::Context) {
        if !self.show_whats_new {
            return;
        }
        let all = entries();
        let fresh = unseen(&all, &self.settings.whats_new_seen).len();
        let modal = super::panel::modal_fill(
            ctx,
            "whats_new",
            "What's new",
            [novo_theme::dialog_width::MEDIUM, 480.0],
            |ui| {
                super::panel::fill_above(ui, 44.0, |ui| {
                    for (n, entry) in all.iter().enumerate() {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.strong(&entry.title);
                            if n < fresh {
                                ui.weak("new");
                            }
                        });
                        ui.label(&entry.text);
                        if !entry.whence.is_empty() {
                            ui.weak(format!("Where: {}", entry.whence));
                        }
                    }
                });
                ui.add_space(8.0);
                super::dialog_ui::done(ui)
            },
        );
        if modal.inner || modal.should_close() {
            self.show_whats_new = false;
            if let Some(newest) = all.first() {
                self.settings.whats_new_seen = newest.id.clone();
            }
        }
    }
}
