//! The one table: a sticky header on the panel colour in caption caps, 24 pt
//! rows with hairlines, numbers right-aligned in the mono face, text that
//! ends in an ellipsis instead of being cut mid-word, and a selected row in
//! the copper wash (docs/design/direction.md §8).
//!
//! The caller owns the data, the sorting and the scrolling: call
//! [`table_header`] above the scroll area (so it stays in view) and
//! [`table_row`] for each visible row inside it. Both take the same
//! [`column_widths`] so header and cells line up.

use crate::{Palette, fonts};
use egui::{Align, Color32, Layout, Rect, Response, RichText, Sense, Shape, Stroke, Ui, Vec2};

/// Height of a row and of the header.
pub const ROW_HEIGHT: f32 = 24.0;
/// Space between two columns.
const GAP: f32 = 8.0;
/// Narrowest a column is drawn.
const MIN_COLUMN: f32 = 56.0;

/// Where a column's text sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableAlign {
    Left,
    Center,
    Right,
}

/// One column: its header text and alignment. Widths come from
/// [`column_widths`].
#[derive(Clone, Debug)]
pub struct TableColumn {
    pub header: String,
    pub align: TableAlign,
}

/// One cell of a row. Numbers use the mono face so digits line up.
#[derive(Clone, Copy, Debug)]
pub struct TableCell<'a> {
    pub text: &'a str,
    pub numeric: bool,
}

/// How a row is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowState {
    #[default]
    Normal,
    /// The copper wash: the row the keys act on, or one the drawing has
    /// selected.
    Selected,
    /// The warning wash: a row whose value needs a second look (an estimate).
    Flagged,
}

/// Splits `available` width between columns in proportion to `weights`,
/// leaving room for the gaps, never narrower than a minimum.
pub fn column_widths(available: f32, weights: &[f64]) -> Vec<f32> {
    column_widths_min(available, weights, &vec![MIN_COLUMN; weights.len()])
}

/// [`column_widths`] with a minimum per column (a header that must stay
/// readable). A column held at its minimum takes its width from the others,
/// so the row still fits `available` whenever the minimums fit at all.
pub fn column_widths_min(available: f32, weights: &[f64], mins: &[f32]) -> Vec<f32> {
    let n = weights.len();
    let min_of = |i: usize| mins.get(i).copied().unwrap_or(MIN_COLUMN).max(MIN_COLUMN);
    let mut room = (available - GAP * n as f32).max(240.0);
    let mut widths = vec![0.0_f32; n];
    let mut held = vec![false; n];
    // Hold every column whose share is below its minimum, then share what is
    // left between the rest; holding can push another below its minimum, so
    // repeat until nothing changes.
    loop {
        let total: f64 = (0..n)
            .filter(|&i| !held[i])
            .map(|i| weights[i].max(0.1))
            .sum();
        let mut changed = false;
        for i in 0..n {
            if held[i] {
                widths[i] = min_of(i);
                continue;
            }
            let w = (weights[i].max(0.1) / total) as f32 * room;
            if w < min_of(i) {
                held[i] = true;
                widths[i] = min_of(i);
                room -= min_of(i);
                changed = true;
                break;
            }
            widths[i] = w;
        }
        if !changed {
            return widths;
        }
    }
}

fn layout_of(align: TableAlign) -> Layout {
    match align {
        TableAlign::Left => Layout::left_to_right(Align::Center),
        TableAlign::Center => Layout::centered_and_justified(egui::Direction::LeftToRight),
        TableAlign::Right => Layout::right_to_left(Align::Center),
    }
}

/// The narrowest each column can be and still show its whole header (and a
/// sort arrow), for [`column_widths_min`].
pub fn header_min_widths(ui: &Ui, columns: &[TableColumn]) -> Vec<f32> {
    let small = ui.style().text_styles[&egui::TextStyle::Small]
        .size
        .max(12.0);
    let font = egui::FontId::new(small, fonts::strong_family());
    columns
        .iter()
        .map(|c| {
            let text = c.header.to_uppercase();
            let letters = text.chars().count() as f32;
            let galley = ui
                .painter()
                .layout_no_wrap(text, font.clone(), Color32::PLACEHOLDER);
            // Letter spacing 0.8 per letter, the sort arrow and a hair of room.
            (galley.size().x + 0.8 * letters + 14.0 + 4.0).ceil()
        })
        .collect()
}

/// The header row. `sort` is the sorted column and whether it ascends.
/// Returns the column whose header was clicked.
pub fn table_header(
    ui: &mut Ui,
    columns: &[TableColumn],
    widths: &[f32],
    sort: Option<(usize, bool)>,
) -> Option<usize> {
    let p = Palette::of(ui);
    let small = ui.style().text_styles[&egui::TextStyle::Small]
        .size
        .max(12.0);
    let backdrop = ui.painter().add(Shape::Noop);
    let mut clicked = None;
    let row = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        for (i, column) in columns.iter().enumerate() {
            let arrow = match sort {
                Some((col, true)) if col == i => " ▲",
                Some((col, false)) if col == i => " ▼",
                _ => "",
            };
            let color = if arrow.is_empty() {
                p.text_secondary
            } else {
                p.text_primary
            };
            let text = RichText::new(format!("{}{arrow}", column.header.to_uppercase()))
                .size(small)
                .family(fonts::strong_family())
                .extra_letter_spacing(0.8)
                .color(color);
            let response = ui
                .allocate_ui_with_layout(
                    Vec2::new(widths[i], ROW_HEIGHT),
                    layout_of(column.align),
                    |ui| {
                        ui.set_min_size(Vec2::new(widths[i], ROW_HEIGHT));
                        ui.add(egui::Label::new(text).truncate().sense(Sense::click()))
                    },
                )
                .inner
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.clicked() {
                clicked = Some(i);
            }
        }
    });
    let rect = row.response.rect.expand2(Vec2::new(4.0, 0.0));
    ui.painter()
        .set(backdrop, Shape::rect_filled(rect, 0.0, p.panel));
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, p.border_strong),
    );
    ui.add_space(1.0);
    clicked
}

/// One row of cells, with a hairline below it. The response senses clicks;
/// its rect is the whole row.
pub fn table_row(
    ui: &mut Ui,
    columns: &[TableColumn],
    widths: &[f32],
    cells: &[TableCell<'_>],
    state: RowState,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> Response {
    let p = Palette::of(ui);
    let backdrop = ui.painter().add(Shape::Noop);
    let line = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        for (i, cell) in cells.iter().enumerate() {
            let align = columns.get(i).map_or(TableAlign::Left, |c| c.align);
            ui.allocate_ui_with_layout(Vec2::new(widths[i], ROW_HEIGHT), layout_of(align), |ui| {
                ui.set_min_size(Vec2::new(widths[i], ROW_HEIGHT));
                let text = RichText::new(cell.text);
                let text = if cell.numeric { text.monospace() } else { text };
                ui.add(egui::Label::new(text).truncate());
            });
        }
    });
    let rect: Rect = line.response.rect;
    let response = ui.interact(rect, ui.id().with(id), Sense::click());
    let fill: Option<Color32> = match state {
        RowState::Selected => Some(p.accent_wash),
        RowState::Flagged => Some(p.warning_wash),
        RowState::Normal if response.hovered() => Some(p.control),
        RowState::Normal => None,
    };
    if let Some(fill) = fill {
        ui.painter()
            .set(backdrop, Shape::rect_filled(rect, 0.0, fill));
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, p.border_subtle),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_weights_and_keep_a_minimum() {
        let w = column_widths(800.0, &[1.0, 3.0]);
        assert!(w[1] > w[0] * 2.5);
        let narrow = column_widths(100.0, &[1.0, 1.0, 1.0]);
        assert!(narrow.iter().all(|&x| x >= MIN_COLUMN));
    }

    #[test]
    fn held_minimums_do_not_push_the_row_past_the_available_width() {
        // A heavy column and two small ones that would fall under the minimum.
        let w = column_widths_min(600.0, &[1.0, 1.0, 20.0], &[56.0, 90.0, 56.0]);
        assert!(w[0] >= 56.0 && w[1] >= 90.0, "{w:?}");
        let used: f32 = w.iter().sum::<f32>() + GAP * 3.0;
        assert!((used - 600.0).abs() < 0.5, "{used} {w:?}");
    }
}
