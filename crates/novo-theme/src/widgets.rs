//! Small building blocks that carry the house style, so screens don't each
//! reinvent a panel header or a primary button.

use crate::{fonts, tokens};
use egui::{Button, Response, RichText, Stroke, Ui, Vec2};

/// A quiet uppercase label that opens a panel section, in place of a heading.
pub fn section_label(ui: &mut Ui, text: &str) -> Response {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(tokens::FONT_SMALL - 0.5)
            .family(fonts::strong_family())
            .extra_letter_spacing(1.2)
            .color(tokens::TEXT_MUTED),
    )
}

/// The main action of a project-level screen or a command preview, such as
/// "Place 12 devices": copper text on the normal control fill with a copper
/// hairline, rather than a loud solid fill. At most one per screen.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        Button::new(
            RichText::new(text)
                .family(fonts::strong_family())
                .color(tokens::ACCENT_HOVER),
        )
        .stroke(Stroke::new(1.0, tokens::ACCENT)),
    )
}

/// A square, frameless dock button showing only an icon. `active` marks the
/// current tool with a copper icon and a short copper bar beneath it.
pub fn tool_button(ui: &mut Ui, icon: &str, active: bool, tooltip: &str) -> Response {
    let color = if active {
        tokens::ACCENT_HOVER
    } else {
        tokens::TEXT_MUTED
    };
    let side = tokens::CONTROL_HEIGHT + 4.0;
    let response = ui
        .add(
            Button::new(RichText::new(icon).size(16.0).color(color))
                .frame(false)
                .min_size(Vec2::splat(side)),
        )
        .on_hover_text(tooltip);
    if active {
        let r = response.rect;
        let bar = egui::Rect::from_center_size(
            egui::pos2(r.center().x, r.bottom() - 1.0),
            Vec2::new(12.0, 2.0),
        );
        ui.painter().rect_filled(bar, 1, tokens::ACCENT);
    }
    response
}

/// One option of a segmented switch: quiet text, the current one lifted
/// onto a control fill.
pub fn segment(ui: &mut Ui, text: &str, current: bool) -> Response {
    let rich = RichText::new(text).color(if current {
        tokens::TEXT
    } else {
        tokens::TEXT_MUTED
    });
    let mut button = Button::new(rich).frame(current);
    if current {
        button = button
            .fill(tokens::BG_3)
            .stroke(Stroke::new(1.0, tokens::BORDER_STRONG));
    }
    ui.add(button)
}

/// The live design check summary: a quiet pill that reads "No issues", or
/// counts errors and warnings, each in its own colour, both when both exist
/// ("2 errors · 5 warnings"). Infos are left out on purpose: they are not
/// issues (DESIGN.md §3, `novo_checks::Summary::is_clean`), and the Issues
/// list shows them.
pub fn issue_pill(ui: &mut Ui, errors: usize, warnings: usize) -> Response {
    let small = egui::TextStyle::Small.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    for (text, color) in pill_parts(errors, warnings) {
        job.append(&text, 0.0, egui::TextFormat::simple(small.clone(), color));
    }
    ui.add(
        Button::new(job)
            .fill(tokens::BG_2)
            .stroke(Stroke::new(1.0, tokens::BORDER))
            .corner_radius(11),
    )
}

/// The pill's text runs and their colours.
fn pill_parts(errors: usize, warnings: usize) -> Vec<(String, egui::Color32)> {
    use crate::icons::status;
    let count =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut parts = Vec::new();
    if errors == 0 && warnings == 0 {
        parts.push((format!("{}  No issues", status::OK), tokens::SUCCESS));
    }
    if errors > 0 {
        let text = format!("{}  {}", status::ERROR, count(errors, "error", "errors"));
        parts.push((text, tokens::ERROR));
    }
    if errors > 0 && warnings > 0 {
        parts.push(("  ·  ".to_owned(), tokens::TEXT_FAINT));
    }
    if warnings > 0 {
        let text = format!(
            "{}  {}",
            status::WARNING,
            count(warnings, "warning", "warnings")
        );
        parts.push((text, tokens::WARNING));
    }
    parts
}

/// A read-only label and value row for the inspector.
pub fn field(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(84.0, tokens::CONTROL_HEIGHT - 4.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(84.0);
                ui.label(RichText::new(label).color(tokens::TEXT_MUTED));
            },
        );
        ui.label(RichText::new(value).monospace().color(tokens::TEXT));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(errors: usize, warnings: usize) -> String {
        pill_parts(errors, warnings)
            .into_iter()
            .map(|(t, _)| t.chars().filter(char::is_ascii).collect::<String>())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn issue_pill_counts_errors_and_warnings_together() {
        assert_eq!(text(0, 0), "No issues");
        assert_eq!(text(1, 0), "1 error");
        assert_eq!(text(0, 5), "5 warnings");
        assert_eq!(text(2, 1), "2 errors 1 warning");
        let colours: Vec<_> = pill_parts(2, 1).into_iter().map(|(_, c)| c).collect();
        assert_eq!(
            colours,
            [tokens::ERROR, tokens::TEXT_FAINT, tokens::WARNING]
        );
    }
}
