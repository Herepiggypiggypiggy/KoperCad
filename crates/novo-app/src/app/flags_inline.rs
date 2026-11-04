//! Problem triangles in rows and drawers (docs/design/problem-triangles.md,
//! sections 4 and 7; lane T8).
//!
//! Three small pieces the Terminals, Cables, PLC and wire numbering windows
//! share, so none of them writes a coloured sentence any more:
//!
//! - [`triangle`]: a 20 pt slot with the triangle in it. Hover gives the
//!   accessible name, a click opens the detail.
//! - [`flag_row`] / [`note_row`]: the triangle followed by the title in the
//!   normal text colour. The numbers are in the detail, not on the row.
//! - A transient refusal ([`NovoApp::refuse`]): an edit that was refused (a
//!   rename onto a name that is taken) shows one triangle chip at the corner
//!   of the canvas. It goes with the next key press or click, or after
//!   [`REFUSAL_LIFE`]. It is not a toast and not in the count: nothing is
//!   wrong with the drawing, the edit just did not happen.

use std::time::{Duration, Instant};

use eframe::egui;
use novo_flags::{Flag, Mapping, Severity, map_word};

use super::NovoApp;

/// How long a refusal stays when nobody presses anything.
pub(super) const REFUSAL_LIFE: Duration = Duration::from_secs(12);

/// Maps the word an engine uses for its severity (any of the eleven
/// vocabularies, in either order) into the one vocabulary. `None` is
/// information: no triangle. A word the table does not know is yellow, the
/// safe side; the unit test lists the words the app uses.
pub(super) fn severity_of(word: &str) -> Option<Severity> {
    match map_word(word) {
        Mapping::Flag(s) => Some(s),
        Mapping::NoFlag => None,
        Mapping::Unknown => Some(Severity::Yellow),
    }
}

/// The accessible name of a triangle: "Error: title" or "Warning: title".
pub(super) fn name_of(severity: Severity, unchecked: bool, title: &str) -> String {
    let level = match (severity, unchecked) {
        (Severity::Red, _) => "Error",
        (Severity::Yellow, false) => "Warning",
        (Severity::Yellow, true) => "Not checked",
    };
    format!("{level}: {title}")
}

/// A triangle in a 20 pt slot. The response is the click target.
pub(super) fn triangle(
    ui: &mut egui::Ui,
    severity: Severity,
    unchecked: bool,
    title: &str,
) -> egui::Response {
    let (slot, response) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
    let name = name_of(severity, unchecked, title);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    if ui.is_rect_visible(slot) {
        novo_theme::widgets::flag_triangle(
            ui.painter(),
            slot.center(),
            15.0,
            severity == Severity::Red,
            unchecked,
            novo_theme::Palette::of(ui),
        );
    }
    response.on_hover_text(name)
}

/// The detail popup of a clicked triangle.
pub(super) fn detail_popup(response: &egui::Response, detail: &[String]) {
    if detail.is_empty() {
        return;
    }
    egui::Popup::menu(response).show(|ui| {
        ui.set_max_width(320.0);
        for line in detail {
            ui.label(line);
        }
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(detail.join("\n"));
            ui.close();
        }
    });
}

/// A row: the triangle, then the title as plain text. A click on either
/// opens the detail.
pub(super) fn note_row(
    ui: &mut egui::Ui,
    severity: Severity,
    unchecked: bool,
    title: &str,
    detail: &[String],
) -> egui::Response {
    ui.horizontal(|ui| {
        let tri = triangle(ui, severity, unchecked, title);
        let text = ui.add(egui::Label::new(title).sense(egui::Sense::click()));
        detail_popup(&tri, detail);
        // The popup is tied to the triangle; the text opens the same one.
        if text.clicked() {
            egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&tri));
        }
        tri
    })
    .inner
}

/// A row for a flag of the store.
pub(super) fn flag_row(ui: &mut egui::Ui, flag: &Flag) -> egui::Response {
    note_row(
        ui,
        flag.severity,
        flag.unchecked,
        &flag.title,
        &flag.detail.0,
    )
}

/// A refused edit, shown for a moment.
pub(super) struct Refusal {
    severity: Severity,
    title: String,
    detail: Vec<String>,
    at: Instant,
    /// Frames drawn, so the key press that caused it does not clear it.
    frames: u32,
    open: bool,
}

/// The refusals the app holds (at most one: the newest wins).
#[derive(Default)]
pub(super) struct Refusals(Option<Refusal>);

impl Refusals {
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// The title of the refusal shown, for tests.
    #[cfg(test)]
    pub(super) fn title(&self) -> Option<&str> {
        self.0.as_ref().map(|r| r.title.as_str())
    }

    #[cfg(test)]
    pub(super) fn detail(&self) -> &[String] {
        self.0.as_ref().map_or(&[], |r| r.detail.as_slice())
    }
}

impl NovoApp {
    /// An edit that did not happen: a yellow triangle chip names it and the
    /// reason is in its detail. Replaces a warning toast.
    pub(super) fn refuse(&mut self, title: &str, detail: impl Into<String>) {
        self.refuse_as(Severity::Yellow, title, detail);
    }

    /// An edit the engine turned down: the same chip, in red. The detail is
    /// the engine's own words in plain language.
    pub(super) fn refuse_red(&mut self, title: &str, detail: impl std::fmt::Display) {
        self.refuse_as(Severity::Red, title, detail.to_string());
    }

    fn refuse_as(&mut self, severity: Severity, title: &str, detail: impl Into<String>) {
        let detail: String = detail.into();
        log::warn!("{title}: {detail}");
        let detail = crate::app::error_words::plain(&detail);
        self.wb.flags.refusals.0 = Some(Refusal {
            severity,
            title: title.to_owned(),
            detail: if detail.trim().is_empty() {
                Vec::new()
            } else {
                vec![detail]
            },
            at: Instant::now(),
            frames: 0,
            open: false,
        });
    }

    /// Draws the refusal at the bottom right of the canvas and clears it on
    /// the next key press or click elsewhere, or when it is old.
    pub(super) fn refusal_chip(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        let Some(r) = self.wb.flags.refusals.0.as_mut() else {
            return;
        };
        if r.at.elapsed() >= REFUSAL_LIFE {
            self.wb.flags.refusals.0 = None;
            return;
        }
        ctx.request_repaint_after(REFUSAL_LIFE.saturating_sub(r.at.elapsed()));
        let mut clicked_chip = false;
        let area = egui::Area::new(egui::Id::new("refusal_chip"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(canvas.right_bottom() - egui::vec2(12.0, 12.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(320.0);
                    let tri = ui.horizontal(|ui| {
                        let tri = triangle(ui, r.severity, false, &r.title);
                        let text = ui.add(egui::Label::new(&r.title).sense(egui::Sense::click()));
                        clicked_chip = tri.clicked() || text.clicked();
                        tri
                    });
                    let _ = tri;
                    if r.open {
                        for line in &r.detail {
                            ui.label(line);
                        }
                    }
                });
            });
        if clicked_chip {
            r.open = !r.open;
            return;
        }
        r.frames += 1;
        // A key press clears it wherever the pointer is; a click only when
        // it lands outside the chip.
        let (key, click) = ctx.input(|i| {
            let any = |f: fn(&egui::Event) -> bool| i.events.iter().any(f);
            (
                any(|e| matches!(e, egui::Event::Key { pressed: true, .. })),
                any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. })),
            )
        });
        let inside = ctx
            .pointer_interact_pos()
            .is_some_and(|p| area.response.rect.contains(p));
        if r.frames > 1 && (key || (click && !inside)) {
            self.wb.flags.refusals.0 = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::gate1_tests::Drive;

    #[test]
    fn the_numbering_words_go_through_the_one_mapping() {
        // novo-numbering orders Info < Warning < Error, novo-checks the other
        // way round: the words, not the order, decide.
        assert_eq!(severity_of("error"), Some(Severity::Red));
        assert_eq!(severity_of("warning"), Some(Severity::Yellow));
        assert_eq!(severity_of("info"), None);
        assert_eq!(severity_of("something new"), Some(Severity::Yellow));
    }

    #[test]
    fn the_accessible_names_say_the_level() {
        assert_eq!(name_of(Severity::Red, false, "Taken"), "Error: Taken");
        assert_eq!(name_of(Severity::Yellow, false, "Taken"), "Warning: Taken");
        assert_eq!(
            name_of(Severity::Yellow, true, "No range"),
            "Not checked: No range"
        );
    }

    #[test]
    fn a_refusal_replaces_the_last_one_and_goes_when_old() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        d.frame();
        assert!(d.app.wb.flags.refusals.is_empty());
        d.app.refuse("First", "one");
        d.app.refuse_red("Second", "two");
        assert_eq!(d.app.wb.flags.refusals.title(), Some("Second"));
        assert_eq!(d.app.wb.flags.refusals.detail(), ["two"]);
        d.frame();
        d.frame();
        assert!(!d.app.wb.flags.refusals.is_empty(), "still there");
        if let Some(r) = d.app.wb.flags.refusals.0.as_mut() {
            r.at = Instant::now() - REFUSAL_LIFE - Duration::from_secs(1);
        }
        d.frame();
        assert!(d.app.wb.flags.refusals.is_empty(), "old ones go");
    }

    #[test]
    fn a_key_press_clears_the_refusal_but_not_the_one_that_caused_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        d.frame();
        d.app.refuse("Nothing ticked", "Tick terminals first");
        d.key(eframe::egui::Key::A, eframe::egui::Modifiers::NONE);
        assert!(!d.app.wb.flags.refusals.is_empty(), "the first frame stays");
        d.frame();
        d.key(eframe::egui::Key::A, eframe::egui::Modifiers::NONE);
        d.frame();
        assert!(d.app.wb.flags.refusals.is_empty(), "the next key clears it");
    }
}
