//! The building blocks that carry the house style, so screens don't each
//! reinvent a panel header, a button or a list row
//! (docs/design/design-system.md §4).
//!
//! Every widget reads the colours of the theme it is drawn in
//! ([`Palette::of`]) and the control height of the current density, so the
//! same call looks right in dark and light and at every density.

use crate::icons::status;
use crate::{Palette, fonts, icon_size, radius, space};
use egui::{
    Align, Align2, Button, CornerRadius, FontId, Frame, Layout, Margin, Rect, Response, RichText,
    Sense, Stroke, Ui, UiBuilder, Vec2,
};

/// The height of a control at the current density.
fn control_height(ui: &Ui) -> f32 {
    ui.spacing().interact_size.y
}

/// How serious a banner, toast or pill is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warning,
    Error,
}

impl Level {
    /// The icon that always travels with the colour.
    pub const fn icon(self) -> &'static str {
        match self {
            Level::Info => status::INFO,
            Level::Success => status::OK,
            Level::Warning => status::WARNING,
            Level::Error => status::ERROR,
        }
    }

    pub fn color(self, p: &Palette) -> egui::Color32 {
        match self {
            Level::Info => p.info,
            Level::Success => p.success,
            Level::Warning => p.warning,
            Level::Error => p.error,
        }
    }

    pub fn wash(self, p: &Palette) -> egui::Color32 {
        match self {
            Level::Info => p.info_wash,
            Level::Success => p.success_wash,
            Level::Warning => p.warning_wash,
            Level::Error => p.error_wash,
        }
    }
}

// ---------------------------------------------------------------------------
// Labels and values

/// A quiet uppercase label that opens a panel section, in place of a heading.
pub fn section_label(ui: &mut Ui, text: &str) -> Response {
    let p = Palette::of(ui);
    ui.label(
        RichText::new(text.to_uppercase())
            .size(ui.style().text_styles[&egui::TextStyle::Small].size - 0.5)
            .family(fonts::strong_family())
            .extra_letter_spacing(1.2)
            .color(p.text_secondary),
    )
}

/// A panel section that folds: a caret, the title in caption caps and, on the
/// right, a short `summary` ("3 set", "12") that says what is inside while it
/// is closed. A hairline above separates it from the section before. The
/// open state is remembered by `id_salt`; `default_open` is the first look.
pub fn collapsing_section<R>(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    title: &str,
    summary: &str,
    default_open: bool,
    body: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let p = Palette::of(ui);
    let id = ui.make_persistent_id(id_salt);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        default_open,
    );
    ui.add_space(space::S);
    let height = control_height(ui) + 2.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    ui.painter().hline(
        rect.x_range(),
        rect.top() - space::XS,
        Stroke::new(1.0, p.border_subtle),
    );
    if response.clicked() {
        state.toggle(ui);
    }
    let color = if response.hovered() {
        p.text_primary
    } else {
        p.text_secondary
    };
    let caret = if state.is_open() {
        crate::icons::CARET_DOWN
    } else {
        crate::icons::CARET_RIGHT
    };
    let small = ui.style().text_styles[&egui::TextStyle::Small]
        .size
        .max(12.0);
    let painter = ui.painter();
    painter.text(
        egui::pos2(rect.left() + 2.0, rect.center().y),
        Align2::LEFT_CENTER,
        caret,
        FontId::proportional(small + 1.0),
        color,
    );
    painter.text(
        egui::pos2(rect.left() + 18.0, rect.center().y),
        Align2::LEFT_CENTER,
        title.to_uppercase(),
        FontId::new(small, fonts::strong_family()),
        color,
    );
    if !summary.is_empty() {
        painter.text(
            egui::pos2(rect.right() - 2.0, rect.center().y),
            Align2::RIGHT_CENTER,
            summary,
            FontId::proportional(small),
            p.text_secondary,
        );
    }
    let out = state.show_body_unindented(ui, |ui| {
        ui.add_space(space::XS);
        body(ui)
    });
    state.store(ui.ctx());
    out.map(|o| o.inner)
}

/// Paints one line of text left-aligned at `pos` (vertically centred) and
/// cuts it with an ellipsis at `max_width`, never in the middle of a glyph.
/// Returns the rectangle it took.
pub fn paint_text_ellipsis(
    painter: &egui::Painter,
    pos: egui::Pos2,
    text: &str,
    font: FontId,
    color: egui::Color32,
    max_width: f32,
) -> Rect {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping {
        max_width: max_width.max(1.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('\u{2026}'),
    };
    let galley = painter.layout_job(job);
    let top_left = egui::pos2(pos.x, pos.y - galley.size().y / 2.0);
    let rect = Rect::from_min_size(top_left, galley.size());
    painter.galley(top_left, galley, color);
    rect
}

/// A read-only label and value row for the inspector.
pub fn field(ui: &mut Ui, label: &str, value: &str) {
    let p = Palette::of(ui);
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(84.0, control_height(ui) - 4.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(84.0);
                ui.label(RichText::new(label).color(p.text_secondary));
            },
        );
        ui.label(RichText::new(value).monospace().color(p.text_primary));
    });
}

/// A keycap: the shortcut hint beside a command, in the same look
/// everywhere.
pub fn kbd(ui: &mut Ui, text: &str) -> Response {
    let p = Palette::of(ui);
    let small = ui.style().text_styles[&egui::TextStyle::Small].size;
    Frame::new()
        .fill(p.control)
        .stroke(Stroke::new(1.0, p.border_subtle))
        .corner_radius(radius::XS)
        .inner_margin(Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .monospace()
                    .size(small)
                    .color(p.text_secondary),
            )
        })
        .response
}

// ---------------------------------------------------------------------------
// Buttons

/// The one main action of a dialog or a command preview ("Place 12
/// devices"): a copper fill. At most one per screen.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let p = Palette::of(ui);
    ui.add(
        Button::new(
            RichText::new(text)
                .family(fonts::strong_family())
                .color(p.on_accent),
        )
        .fill(p.accent)
        .stroke(Stroke::NONE),
    )
}

/// A button with no frame until the pointer is over it: toolbars and row
/// actions.
pub fn ghost_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(Button::new(text).frame_when_inactive(false))
}

/// A destructive action: a normal button with error-coloured text. The
/// confirmation, not the colour, names what is lost.
pub fn danger_button(ui: &mut Ui, text: &str) -> Response {
    let p = Palette::of(ui);
    ui.add(Button::new(RichText::new(text).color(p.error)))
}

/// A square button showing only an icon; the tooltip names it.
pub fn icon_button(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    let p = Palette::of(ui);
    let side = control_height(ui) + 2.0;
    ui.add(
        Button::new(
            RichText::new(icon)
                .size(icon_size::BUTTON)
                .color(p.text_secondary),
        )
        .frame_when_inactive(false)
        .min_size(Vec2::splat(side)),
    )
    .on_hover_text(tooltip)
}

/// An icon button that can be on: the icon turns copper on a copper wash.
pub fn icon_toggle(ui: &mut Ui, icon: &str, on: bool, tooltip: &str) -> Response {
    let p = Palette::of(ui);
    let side = control_height(ui) + 2.0;
    let color = if on { p.accent_hover } else { p.text_secondary };
    let mut button = Button::new(RichText::new(icon).size(icon_size::BUTTON).color(color))
        .min_size(Vec2::splat(side));
    button = if on {
        button
            .fill(p.accent_wash)
            .stroke(Stroke::new(1.0, p.accent))
    } else {
        button.frame_when_inactive(false)
    };
    ui.add(button).on_hover_text(tooltip)
}

/// A square, frameless dock button showing only an icon. `active` marks the
/// current tool with a copper icon and a short copper bar beneath it.
pub fn tool_button(ui: &mut Ui, icon: &str, active: bool, tooltip: &str) -> Response {
    let p = Palette::of(ui);
    let color = if active {
        p.accent_hover
    } else {
        p.text_secondary
    };
    let side = control_height(ui) + 4.0;
    let response = ui
        .add(
            Button::new(RichText::new(icon).size(icon_size::BUTTON).color(color))
                .frame(false)
                .min_size(Vec2::splat(side)),
        )
        .on_hover_text(tooltip);
    if active {
        let r = response.rect;
        let bar = Rect::from_center_size(
            egui::pos2(r.center().x, r.bottom() - 1.0),
            Vec2::new(12.0, 2.0),
        );
        ui.painter().rect_filled(bar, 1, p.accent);
    }
    response
}

/// A dock button with a short text label beside its icon, for people who
/// read words faster than pictures. Same look and active bar as
/// [`tool_button`].
pub fn tool_button_labeled(
    ui: &mut Ui,
    icon: &str,
    label: &str,
    active: bool,
    tooltip: &str,
) -> Response {
    let p = Palette::of(ui);
    let color = if active {
        p.accent_hover
    } else {
        p.text_secondary
    };
    let mut job = egui::text::LayoutJob::default();
    job.append(
        icon,
        0.0,
        egui::TextFormat::simple(
            egui::FontId::new(icon_size::BUTTON, egui::FontFamily::Proportional),
            color,
        ),
    );
    job.append(
        label,
        space::S,
        egui::TextFormat::simple(
            egui::TextStyle::Button.resolve(ui.style()),
            if active { p.text_primary } else { color },
        ),
    );
    let side = control_height(ui) + 4.0;
    let response = ui
        .add(
            Button::new(job)
                .frame(false)
                .min_size(Vec2::new(side, side)),
        )
        .on_hover_text(tooltip);
    if active {
        let r = response.rect;
        let bar = Rect::from_center_size(
            egui::pos2(r.center().x, r.bottom() - 1.0),
            Vec2::new(12.0, 2.0),
        );
        ui.painter().rect_filled(bar, 1, p.accent);
    }
    response
}

/// One option of a segmented switch: quiet text, the current one lifted
/// onto a control fill.
pub fn segment(ui: &mut Ui, text: &str, current: bool) -> Response {
    let p = Palette::of(ui);
    let rich = RichText::new(text).color(if current {
        p.text_primary
    } else {
        p.text_secondary
    });
    // Without a frame until hovered, but with the button's padding.
    let mut button = Button::new(rich).frame_when_inactive(current);
    if current {
        button = button
            .fill(p.control)
            .stroke(Stroke::new(1.0, p.border_strong));
    }
    ui.add(button)
}

/// Two to four exclusive options in a pill track. Returns whether the
/// choice changed.
pub fn segmented(ui: &mut Ui, current: &mut usize, labels: &[&str]) -> bool {
    let p = Palette::of(ui);
    let mut changed = false;
    Frame::new()
        .fill(p.sunken)
        .stroke(Stroke::new(1.0, p.border_subtle))
        .corner_radius(radius::CONTROL + 1)
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(|ui| {
                for (i, label) in labels.iter().enumerate() {
                    if segment(ui, label, *current == i).clicked() && *current != i {
                        *current = i;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Switches a panel's content: text with a copper underline under the
/// current one and a hairline under the row. Returns whether the choice
/// changed.
pub fn tabs(ui: &mut Ui, current: &mut usize, labels: &[&str]) -> bool {
    let p = Palette::of(ui);
    let height = control_height(ui) + 4.0;
    let body = ui.style().text_styles[&egui::TextStyle::Body].clone();
    let strong = FontId::new(body.size, fonts::strong_family());
    let mut changed = false;
    let start = ui.cursor().min;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::XS;
        for (i, label) in labels.iter().enumerate() {
            let is_current = *current == i;
            let font = if is_current { &strong } else { &body };
            let galley = ui.painter().layout_no_wrap(
                (*label).to_owned(),
                font.clone(),
                egui::Color32::PLACEHOLDER,
            );
            let width = galley.size().x + 2.0 * space::M + 2.0;
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
            // The tab is painted, so name it for screen readers and the
            // lesson checks that look for a control by its label.
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    is_current,
                    *label,
                )
            });
            let color = if is_current {
                p.text_primary
            } else if response.hovered() {
                p.text_strong
            } else {
                p.text_secondary
            };
            let pos = egui::pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            );
            ui.painter().galley(pos, galley, color);
            if is_current {
                let bar = Rect::from_min_max(
                    egui::pos2(rect.left() + space::S, rect.bottom() - 2.0),
                    egui::pos2(rect.right() - space::S, rect.bottom()),
                );
                ui.painter().rect_filled(bar, 1, p.accent);
            }
            if response.clicked() && !is_current {
                *current = i;
                changed = true;
            }
        }
    });
    let y = start.y + height - 0.5;
    ui.painter().hline(
        ui.max_rect().x_range(),
        y,
        Stroke::new(1.0, p.border_subtle),
    );
    changed
}

/// A visibility or filter toggle (a layer, a map, a badge): neutral, so it
/// never competes with the drawing. On is a `hover` fill, primary text and
/// an accent check; off is quiet text. The copper wash is for tools and
/// modes only (design-system.md section 3).
pub fn filter_toggle(ui: &mut Ui, text: &str, on: bool) -> Response {
    let p = Palette::of(ui);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    if on {
        job.append(
            "\u{2714} ",
            0.0,
            egui::TextFormat::simple(font.clone(), p.accent_hover),
        );
    }
    let color = if on { p.text_primary } else { p.text_secondary };
    job.append(text, 0.0, egui::TextFormat::simple(font, color));
    let mut button = Button::new(job);
    button = if on {
        button
            .fill(p.hover)
            .stroke(Stroke::new(1.0, p.border_strong))
    } else {
        button
            .fill(p.control)
            .stroke(Stroke::new(1.0, p.border_subtle))
    };
    ui.add(button)
}

/// A compact value or filter: `selected` fills it with the copper wash.
pub fn chip(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let p = Palette::of(ui);
    let small = ui.style().text_styles[&egui::TextStyle::Small].size;
    let color = if selected {
        p.accent_hover
    } else {
        p.text_secondary
    };
    let mut button = Button::new(RichText::new(text).size(small).color(color))
        .corner_radius(radius::PILL)
        .min_size(Vec2::new(0.0, control_height(ui) - 4.0));
    button = if selected {
        button
            .fill(p.accent_wash)
            .stroke(Stroke::new(1.0, p.accent))
    } else {
        button
            .fill(p.control)
            .stroke(Stroke::new(1.0, p.border_subtle))
    };
    ui.add(button)
}

/// The live design check summary: a quiet pill that reads "No issues", or
/// counts errors and warnings, each in its own colour, both when both exist
/// ("2 errors · 5 warnings"). Infos are left out on purpose: they are not
/// issues (DESIGN.md §3, `novo_checks::Summary::is_clean`), and the Issues
/// list shows them.
pub fn issue_pill(ui: &mut Ui, errors: usize, warnings: usize) -> Response {
    let p = Palette::of(ui);
    let small = egui::TextStyle::Small.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    for (text, color) in pill_parts(p, errors, warnings) {
        job.append(&text, 0.0, egui::TextFormat::simple(small.clone(), color));
    }
    ui.add(
        Button::new(job)
            .fill(p.raised)
            .stroke(Stroke::new(1.0, p.border_subtle))
            .corner_radius(radius::PILL),
    )
}

/// The pill's text runs and their colours.
fn pill_parts(p: &Palette, errors: usize, warnings: usize) -> Vec<(String, egui::Color32)> {
    let count =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut parts = Vec::new();
    if errors == 0 && warnings == 0 {
        parts.push((format!("{}  No issues", status::OK), p.success));
    }
    if errors > 0 {
        let text = format!("{}  {}", status::ERROR, count(errors, "error", "errors"));
        parts.push((text, p.error));
    }
    if errors > 0 && warnings > 0 {
        parts.push(("  ·  ".to_owned(), p.text_tertiary));
    }
    if warnings > 0 {
        let text = format!(
            "{}  {}",
            status::WARNING,
            count(warnings, "warning", "warnings")
        );
        parts.push((text, p.warning));
    }
    parts
}

// ---------------------------------------------------------------------------
// Rows, menus and messages

/// A progress bar with a caption line above it: `caption` on the left,
/// `detail` (counts, in the number font) on the right. `done` and `failed`
/// are fractions of the whole, 0 to 1; the done part is green and the
/// failed part follows it in the error colour. The bar is 6 pt high at every
/// density.
pub fn progress_bar(ui: &mut Ui, caption: &str, detail: &str, done: f32, failed: f32) -> Response {
    let p = Palette::of(ui);
    let mut small = egui::TextStyle::Small.resolve(ui.style());
    // Never below 12 pt, so the caption and counts read at every density.
    small.size = small.size.max(12.0);
    let width = ui.available_width();
    ui.horizontal(|ui| {
        ui.set_width(width);
        ui.label(
            RichText::new(caption)
                .font(small.clone())
                .color(p.text_secondary),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(detail)
                    .font(FontId::new(small.size, egui::FontFamily::Monospace))
                    .color(p.text_primary),
            );
        });
    });
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 6.0), Sense::hover());
    let round = CornerRadius::same(3);
    ui.painter().rect_filled(rect, round, p.sunken);
    let done = done.clamp(0.0, 1.0);
    let failed = failed.clamp(0.0, 1.0 - done);
    let part = |from: f32, to: f32| {
        Rect::from_min_max(
            egui::pos2(rect.left() + rect.width() * from, rect.top()),
            egui::pos2(rect.left() + rect.width() * to, rect.bottom()),
        )
    };
    if done > 0.0 {
        ui.painter().rect_filled(part(0.0, done), round, p.success);
    }
    if failed > 0.0 {
        ui.painter()
            .rect_filled(part(done, done + failed), round, p.error);
    }
    ui.painter().rect_stroke(
        rect,
        round,
        Stroke::new(1.0, p.border_subtle),
        egui::StrokeKind::Inside,
    );
    response
}

/// A list row: full width, one control high plus a point, with hover and
/// selected looks (the selected row has the copper wash and a copper bar on
/// its left). `add_contents` lays out the row's content from left to right.
pub fn row(ui: &mut Ui, selected: bool, add_contents: impl FnOnce(&mut Ui)) -> Response {
    let height = control_height(ui) + 2.0;
    row_of_height(ui, selected, height, add_contents)
}

/// [`row`] with a height of its own, for a row that carries a second line
/// (a finger-sized list at the cabinet).
pub fn row_of_height(
    ui: &mut Ui,
    selected: bool,
    height: f32,
    add_contents: impl FnOnce(&mut Ui),
) -> Response {
    let p = Palette::of(ui);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    paint_row_background(ui, p, rect, selected, response.hovered());
    let inner = rect.shrink2(Vec2::new(space::M, 0.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    // A selectable label would take the press for its own text selection and the
    // row would never see the click.
    child.style_mut().interaction.selectable_labels = false;
    add_contents(&mut child);
    response
}

fn paint_row_background(ui: &Ui, p: &Palette, rect: Rect, selected: bool, hovered: bool) {
    let rounding = CornerRadius::same(radius::CONTROL);
    if selected {
        ui.painter().rect_filled(rect, rounding, p.accent_wash);
        let bar = Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height()));
        ui.painter().rect_filled(bar, 1, p.accent);
    } else if hovered {
        ui.painter().rect_filled(rect, rounding, p.control);
    }
}

/// A menu or command row: optional icon, label, and a right-aligned
/// keycap. A disabled row shows `why` in place of the shortcut.
pub fn menu_item(
    ui: &mut Ui,
    icon: Option<&str>,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
) -> Response {
    let p = Palette::of(ui);
    let height = control_height(ui) + 2.0;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), sense);
    if enabled && response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(radius::CONTROL), p.hover);
    }
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = egui::TextStyle::Small.resolve(ui.style());
    let text_color = if enabled {
        p.text_primary
    } else {
        p.text_tertiary
    };
    if let Some(icon) = icon {
        ui.painter().text(
            egui::pos2(
                rect.left() + space::M + icon_size::ROW / 2.0,
                rect.center().y,
            ),
            Align2::CENTER_CENTER,
            icon,
            FontId::new(icon_size::ROW, egui::FontFamily::Proportional),
            if enabled {
                p.text_secondary
            } else {
                p.text_tertiary
            },
        );
    }
    ui.painter().text(
        egui::pos2(
            rect.left() + space::M + icon_size::ROW + space::M,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        label,
        body,
        text_color,
    );
    if let Some(shortcut) = shortcut {
        ui.painter().text(
            egui::pos2(rect.right() - space::M, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::new(small.size, egui::FontFamily::Monospace),
            p.text_tertiary,
        );
    }
    response
}

/// Inline status in a panel: the level's icon, one sentence, and optional
/// actions on the right, on the level's wash.
pub fn banner(
    ui: &mut Ui,
    level: Level,
    text: &str,
    add_actions: impl FnOnce(&mut Ui),
) -> Response {
    let p = Palette::of(ui);
    Frame::new()
        .fill(level.wash(p))
        .corner_radius(radius::CONTROL)
        .inner_margin(Margin::symmetric(space::M as i8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(level.icon()).color(level.color(p)));
                ui.label(RichText::new(text).color(p.text_primary));
                ui.with_layout(Layout::right_to_left(Align::Center), add_actions);
            });
        })
        .response
}

/// [`banner`] with no actions, whose text wraps to the width it is given (a long
/// sentence in `banner` stays on one line and runs off the window).
pub fn banner_wrapped(ui: &mut Ui, level: Level, text: &str) -> Response {
    let p = Palette::of(ui);
    Frame::new()
        .fill(level.wash(p))
        .corner_radius(radius::CONTROL)
        .inner_margin(Margin::symmetric(space::M as i8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(level.icon()).color(level.color(p)));
                ui.add(egui::Label::new(RichText::new(text).color(p.text_primary)).wrap());
            });
        })
        .response
}

/// How large an [`empty_state`] is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptySize {
    /// Inside a panel or drawer.
    Panel,
    /// A whole screen: no project, first run.
    Screen,
}

/// What an empty surface is for and the next step
/// (docs/design/design-system.md §6): an icon, a title, one sentence and the
/// actions, centred and at most 360 pt wide.
pub fn empty_state(
    ui: &mut Ui,
    size: EmptySize,
    icon: &str,
    title: &str,
    body: &str,
    add_actions: impl FnOnce(&mut Ui),
) {
    let p = Palette::of(ui);
    let title_size = match size {
        EmptySize::Panel => ui.style().text_styles[&egui::TextStyle::Heading].size,
        EmptySize::Screen => ui.style().text_styles[&egui::TextStyle::Heading].size + 5.0,
    };
    ui.vertical_centered(|ui| {
        ui.set_max_width(dialog_width_small());
        ui.add_space(space::XXL);
        ui.label(
            RichText::new(icon)
                .size(icon_size::EMPTY)
                .color(p.text_tertiary),
        );
        ui.add_space(space::M);
        ui.label(
            RichText::new(title)
                .size(title_size)
                .family(fonts::strong_family())
                .color(p.text_primary),
        );
        ui.add_space(space::S);
        ui.label(RichText::new(body).color(p.text_secondary));
        ui.add_space(space::L);
        add_actions(ui);
        ui.add_space(space::XXL);
    });
}

fn dialog_width_small() -> f32 {
    crate::dialog_width::SMALL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(errors: usize, warnings: usize) -> String {
        pill_parts(&Palette::DARK, errors, warnings)
            .into_iter()
            .map(|(t, _)| t.chars().filter(char::is_ascii).collect::<String>())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn a_filter_toggle_is_never_the_copper_wash() {
        let ctx = egui::Context::default();
        crate::apply(&ctx);
        let mut seen = Vec::new();
        for on in [false, true] {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let r = filter_toggle(ui, "Contours", on);
                assert!(r.rect.width() > 10.0);
            });
            out.textures_delta.clear();
            seen.push(on);
        }
        assert_eq!(seen, [false, true]);
        let p = Palette::DARK;
        assert_ne!(p.hover, p.accent_wash);
    }

    #[test]
    fn issue_pill_counts_errors_and_warnings_together() {
        let p = Palette::DARK;
        assert_eq!(text(0, 0), "No issues");
        assert_eq!(text(1, 0), "1 error");
        assert_eq!(text(0, 5), "5 warnings");
        assert_eq!(text(2, 1), "2 errors 1 warning");
        let colours: Vec<_> = pill_parts(&p, 2, 1).into_iter().map(|(_, c)| c).collect();
        assert_eq!(colours, [p.error, p.text_tertiary, p.warning]);
    }

    #[test]
    fn every_level_has_an_icon_a_colour_and_a_wash() {
        for p in [&Palette::DARK, &Palette::LIGHT] {
            for level in [Level::Info, Level::Success, Level::Warning, Level::Error] {
                assert!(!level.icon().is_empty());
                assert_ne!(level.color(p), level.wash(p));
            }
        }
    }

    /// Lays every widget out under one theme and density; a panic here is a
    /// layout bug (negative sizes, missing fonts).
    fn lay_out_all(theme: crate::ThemeChoice, density: crate::Density) {
        let ctx = egui::Context::default();
        crate::apply_with(&ctx, theme, density);
        let mut current = 0usize;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            section_label(ui, "Section");
            field(ui, "Label", "Value");
            kbd(ui, "Ctrl+K");
            primary_button(ui, "Place 12 devices");
            ghost_button(ui, "Cancel");
            danger_button(ui, "Delete page");
            icon_button(ui, crate::icons::action::SAVE, "Save");
            icon_toggle(ui, crate::icons::action::PIN, true, "Pin");
            tool_button(ui, crate::icons::tool::SELECT, true, "Select");
            segmented(ui, &mut current, &["Pages", "Issues", "Changes"]);
            tabs(ui, &mut current, &["Terminals", "Cables", "Devices"]);
            chip(ui, "=A1", true);
            issue_pill(ui, 1, 2);
            row(ui, true, |ui| {
                ui.label("Motor starter");
            });
            menu_item(
                ui,
                Some(crate::icons::action::SAVE),
                "Save",
                Some("Ctrl+S"),
                true,
            );
            menu_item(ui, None, "Rules", Some("Comes later"), false);
            banner(ui, Level::Warning, "2 pages have no title", |ui| {
                ghost_button(ui, "Show");
            });
            empty_state(
                ui,
                EmptySize::Screen,
                crate::icons::panel::PAGES,
                "Start a project",
                "Draw a schematic, or open the example.",
                |ui| {
                    primary_button(ui, "New project");
                },
            );
        });
        output.textures_delta.clear();
    }

    #[test]
    fn every_widget_lays_out_in_every_theme_and_density() {
        for theme in [crate::ThemeChoice::Dark, crate::ThemeChoice::Light] {
            for density in crate::Density::ALL {
                lay_out_all(theme, density);
            }
        }
    }

    #[test]
    fn tabs_and_segmented_report_a_change_once() {
        let ctx = egui::Context::default();
        crate::apply(&ctx);
        let mut current = 1usize;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(!segmented(ui, &mut current, &["A", "B"]));
            assert!(!tabs(ui, &mut current, &["A", "B"]));
        });
        output.textures_delta.clear();
        assert_eq!(current, 1);
    }
}
