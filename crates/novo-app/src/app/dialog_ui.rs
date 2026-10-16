//! Small helpers so the dialogs share one look: a title with a line of
//! help, captioned sections, a setting with its help and a segmented
//! choice, and one primary "Done" button (docs/design/direction.md §6).

use eframe::egui;
use novo_theme::widgets;

/// The dialog title in the title size, with an optional line of help.
pub(super) fn title(ui: &mut egui::Ui, text: &str, help: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(15.0)
            .family(novo_theme::fonts::strong_family()),
    );
    if !help.is_empty() {
        hint(ui, help);
    }
    ui.add_space(4.0);
}

/// A caption-caps section label with space above it (less for the first).
pub(super) fn section(ui: &mut egui::Ui, text: &str, first: bool) {
    ui.add_space(if first { 4.0 } else { 14.0 });
    widgets::section_label(ui, text);
    ui.add_space(4.0);
}

/// A small, quiet, wrapping line of help.
pub(super) fn hint(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).small().weak()).wrap());
}

/// A setting's name and its help line.
pub(super) fn setting(ui: &mut egui::Ui, label: &str, help: &str) {
    ui.label(label);
    if !help.is_empty() {
        hint(ui, help);
    }
    ui.add_space(4.0);
}

/// One setting with its choices as a segmented control.
pub(super) fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    help: &str,
    value: &mut T,
    options: &[(T, &str)],
) {
    setting(ui, label, help);
    let mut index = options.iter().position(|(v, _)| v == value).unwrap_or(0);
    let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
    if widgets::segmented(ui, &mut index, &labels) {
        *value = options[index].0;
    }
    ui.add_space(8.0);
}

/// The primary "Done" button at the right under the dialog; true when
/// clicked.
pub(super) fn done(ui: &mut egui::Ui) -> bool {
    ui.add_space(8.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        widgets::primary_button(ui, "Done").clicked()
    })
    .inner
}
