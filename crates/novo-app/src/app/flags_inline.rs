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
//!
//! A refused edit is not drawn here: it is a transient flag of the edit
//! flags (`flags_edit`, lane T7), one path for every window.

use eframe::egui;
use novo_flags::{Flag, Mapping, Severity, map_word};

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
    let row = ui.horizontal(|ui| {
        let tri = triangle(ui, severity, unchecked, title);
        let text = ui.add(egui::Label::new(title).sense(egui::Sense::click()));
        detail_popup(&tri, detail);
        // The popup is tied to the triangle; the text opens the same one.
        if text.clicked() {
            egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&tri));
        }
        tri
    });
    // The whole row, so a click on the words counts as on the chip.
    row.response
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
