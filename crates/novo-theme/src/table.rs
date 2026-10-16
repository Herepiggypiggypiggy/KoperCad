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
}

/// Splits `available` width between columns in proportion to `weights`,
/// leaving room for the gaps, never narrower than a minimum.
pub fn column_widths(available: f32, weights: &[f64]) -> Vec<f32> {
    let total: f64 = weights.iter().map(|w| w.max(0.1)).sum();
    let room = (available - GAP * weights.len() as f32).max(240.0);
    weights
        .iter()
        .map(|w| ((w.max(0.1) / total) as f32 * room).max(MIN_COLUMN))
        .collect()
}

fn layout_of(align: TableAlign) -> Layout {
    match align {
        TableAlign::Left => Layout::left_to_right(Align::Center),
        TableAlign::Center => Layout::centered_and_justified(egui::Direction::LeftToRight),
        TableAlign::Right => Layout::right_to_left(Align::Center),
    }
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
}
