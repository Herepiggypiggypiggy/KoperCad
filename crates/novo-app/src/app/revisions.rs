//! Revisions in the app (docs/DESIGN.md §7): the Issue revision window.
//! The engine (index rule, snapshot, freezes) is `novo-revisions`; the
//! window proposes everything and asks only for the description.

use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;
use novo_core::Project;
use novo_revisions::IndexRule;

use super::{Level, NovoApp};
use crate::backend::BackendError;

/// What the Issue revision window holds while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IssueDraft {
    pub index: String,
    pub date: String,
    pub author: String,
    pub description: String,
    /// A longer note on why; kept with the revision.
    pub note: String,
    /// Why the last attempt failed, shown in the window.
    pub error: Option<String>,
    /// The description field takes focus on the first frame.
    focused: bool,
}

/// The rule the project's revisions follow, read from the latest index:
/// `01` is padded, `3` is numbers, anything else is letters. A project
/// without revisions starts with letters.
pub(crate) fn index_rule(project: &Project) -> IndexRule {
    let Some(latest) = project.latest_revision() else {
        return IndexRule::Letters;
    };
    let digits = !latest.index.is_empty() && latest.index.bytes().all(|c| c.is_ascii_digit());
    match (
        digits,
        latest.index.starts_with('0') && latest.index.len() > 1,
    ) {
        (true, true) => IndexRule::Padded,
        (true, false) => IndexRule::Numbers,
        _ => IndexRule::Letters,
    }
}

/// Today's date as `YYYY-MM-DD` (UTC; the model keeps plain ISO dates).
pub(crate) fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    iso_date((secs / 86_400) as i64)
}

/// The civil date of a day count since 1970-01-01.
pub(super) fn iso_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

impl NovoApp {
    /// Opens the window with the next index, today and the remembered
    /// initials; text after the verb (`issue Motor M3 added`) is the
    /// description.
    pub(super) fn open_issue_revision(&mut self, description: Option<&str>) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let index = index_rule(project).next(project).unwrap_or_default();
        self.wb.issue = Some(IssueDraft {
            index,
            date: today(),
            author: self.settings.initials.clone(),
            description: description.unwrap_or_default().trim().to_owned(),
            note: String::new(),
            error: None,
            focused: false,
        });
    }

    pub(super) fn issue_revision_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.wb.issue.take() else {
            return;
        };
        let mut issue = false;
        let modal = egui::Modal::new(egui::Id::new("issue_revision")).show(ctx, |ui| {
            ui.set_width(380.0);
            let shown = if draft.index.is_empty() {
                "Issue revision".to_owned()
            } else {
                format!("Issue revision {}", draft.index)
            };
            ui.heading(shown);
            ui.weak("Freezes the pages as they are now and keeps a copy to compare with.");
            ui.add_space(8.0);
            ui.label("What changed");
            let text = ui.add(
                egui::TextEdit::singleline(&mut draft.description)
                    .hint_text("Motor M3 added")
                    .desired_width(f32::INFINITY),
            );
            if !draft.focused {
                text.request_focus();
                draft.focused = true;
            }
            if text.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                issue = true;
            }
            ui.add_space(6.0);
            ui.label("Note (optional)");
            ui.add(
                egui::TextEdit::multiline(&mut draft.note)
                    .hint_text("Why: the longer story, kept with the revision")
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Index");
                ui.add(egui::TextEdit::singleline(&mut draft.index).desired_width(48.0));
                ui.label("Date");
                ui.add(egui::TextEdit::singleline(&mut draft.date).desired_width(86.0));
                ui.label("By");
                ui.add(egui::TextEdit::singleline(&mut draft.author).desired_width(48.0));
            });
            if let Some(error) = &draft.error {
                ui.add_space(4.0);
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = format!("Issue {}", draft.index);
                if ui
                    .add_enabled(!draft.index.trim().is_empty(), egui::Button::new(label))
                    .clicked()
                {
                    issue = true;
                }
                ui.weak("Enter issues · Esc cancels");
            });
        });
        if modal.should_close() {
            return;
        }
        if issue {
            self.submit_issue(draft);
        } else {
            self.wb.issue = Some(draft);
        }
    }

    /// Issues the revision the window describes. On success the window
    /// closes; on a refusal it stays, saying why.
    pub(crate) fn submit_issue(&mut self, mut draft: IssueDraft) {
        let index = draft.index.trim().to_owned();
        let author = draft.author.trim().to_owned();
        let result = self.backend.issue_revision_noted(
            Some(index),
            draft.date.trim(),
            &author,
            draft.description.trim(),
            draft.note.trim(),
        );
        match result {
            Ok(issued) => {
                self.settings.initials = author;
                self.changed_outside_canvas(&issued.changes);
                let note = match &issued.unchanged_since {
                    Some(since) => format!(" · no changes since {since}"),
                    None => String::new(),
                };
                self.message(
                    Level::Info,
                    format!("Issued revision {}{note}", issued.index),
                );
            }
            Err(BackendError::Model(e)) => {
                draft.error = Some(e.to_string());
                self.wb.issue = Some(draft);
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not issue the revision: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_civil_date_of_a_day_count() {
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(59), "1970-03-01");
        assert_eq!(iso_date(11_016), "2000-02-29");
        assert_eq!(iso_date(20_741), "2026-10-15");
    }
}
