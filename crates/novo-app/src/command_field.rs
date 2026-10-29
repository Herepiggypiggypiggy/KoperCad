//! The command field (docs/DESIGN.md §4): Ctrl+K opens it at the top, I
//! (place mode) and a tapped Space open it at the pointer. The search and
//! its grammar are novo-workflow's [`Palette`] (WS-08); this is only how
//! the rows are shown and picked.

use eframe::egui;
use novo_parts::Catalog;
use novo_symbols::Library;
use novo_workflow::homes::Home;
use novo_workflow::keymap::Focus;
use novo_workflow::palette::{Mode, Palette, RECENT_GROUP, Row, Sources, Target};

use crate::place_palette::{self as pp, Category};
use crate::theme::ui_palette;

/// Rows shown at once; the rest scroll.
const MAX_HEIGHT: f32 = 460.0;
/// The list keeps at least this much room when it has rows to fill it.
const MIN_HEIGHT: f32 = 300.0;
const WIDTH: f32 = 560.0;
/// The place palette is wider: categories, the symbol grid, the detail pane.
const PLACE_WIDTH: f32 = 880.0;
const PLACE_HEIGHT: f32 = 430.0;
const CATEGORY_WIDTH: f32 = 150.0;
const DETAIL_WIDTH: f32 = 250.0;
/// Width the card grid gets inside the place palette.
const GRID_WIDTH: f32 = PLACE_WIDTH - CATEGORY_WIDTH - DETAIL_WIDTH - 40.0;
/// Pinned tiles while the Ctrl K field is empty.
const PINNED_MAX: usize = 6;
/// Commands offered as tiles until the person has used enough of their own.
const PINNED_DEFAULT: [&str; 6] = [
    "file.save",
    "view.fit",
    "tool.place",
    "tool.connect",
    "edit.undo",
    "view.command_palette",
];

struct OpenField {
    mode: Mode,
    query: String,
    selected: usize,
    /// Screen position for the field at the pointer; `None` is the top.
    at: Option<egui::Pos2>,
    /// The folder of the place palette that is shown.
    category: Category,
    /// Scroll the selected card into view on the next frame.
    scroll: bool,
}

#[derive(Default)]
pub struct CommandField {
    /// The search engine and its memory (recent picks, favourites).
    pub palette: Palette,
    open: Option<OpenField>,
    /// Where the field was drawn this frame, for lessons to point at.
    rect: Option<egui::Rect>,
    /// Whether Claude is on, so a search with no match offers to ask it.
    pub ask_enabled: bool,
    /// What the person chose to ask Claude; the app takes it.
    pub asked: Option<String>,
    /// Labels the app words for a command from the state of the project
    /// ("Continue lesson: Placing parts fast"), by command id.
    pub relabel: Vec<(&'static str, String)>,
    /// The part catalog the place palette's detail pane lists parts from;
    /// read when the palette first shows, dropped when it closes.
    catalog: Option<Catalog>,
}

impl CommandField {
    pub fn rect(&self) -> Option<egui::Rect> {
        self.rect
    }

    pub fn open(&mut self, mode: Mode, at: Option<egui::Pos2>) {
        self.open = Some(OpenField {
            mode,
            query: String::new(),
            selected: 0,
            at,
            category: Category::Start,
            scroll: true,
        });
    }

    /// Opens the field with text already typed (a completion step).
    pub fn open_with(&mut self, mode: Mode, query: &str, at: Option<egui::Pos2>) {
        self.open(mode, at);
        if let Some(open) = &mut self.open {
            open.query = query.to_owned();
        }
    }

    pub fn close(&mut self) {
        self.open = None;
        self.catalog = None;
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// What has the keyboard while the field is open, for the keymap.
    pub fn focus(&self) -> Option<Focus> {
        let open = self.open.as_ref()?;
        Some(
            if open.mode == Mode::Place && open.query.trim().is_empty() {
                Focus::PlaceFieldEmpty
            } else {
                Focus::TextField
            },
        )
    }

    /// The rows for what is typed now.
    pub fn rows(&self, sources: &Sources<'_>) -> Vec<Row> {
        let Some(open) = &self.open else {
            return Vec::new();
        };
        let mut rows = self.palette.search_in(open.mode, &open.query, sources);
        // Commands the app cannot run yet are listed, greyed with a reason
        // (DESIGN.md §4), instead of answering with a toast.
        for row in &mut rows {
            let id = match &row.target {
                Target::Command(id) | Target::Run { command: id, .. } => *id,
                _ => continue,
            };
            if let Some((_, label)) = self.relabel.iter().find(|(c, _)| *c == id) {
                row.label.clone_from(label);
            }
            if row.disabled.is_none() && !crate::app::app_runs(id) {
                row.disabled = Some("Comes later");
            }
        }
        rows
    }

    /// Picks the row with this key (favourites 1 to 9 in an empty place
    /// field). Returns the target and closes the field.
    pub fn pick_shortcut(&mut self, shortcut: &str, sources: &Sources<'_>) -> Option<Target> {
        let row = self
            .rows(sources)
            .into_iter()
            .find(|r| r.shortcut.as_deref() == Some(shortcut) && r.disabled.is_none())?;
        self.finish(row.target.clone());
        Some(row.target)
    }

    /// Types `text` into the open field, as the keyboard would. For tests
    /// and scripts.
    pub fn type_text(&mut self, text: &str) {
        if let Some(open) = &mut self.open {
            open.query.push_str(text);
            open.selected = 0;
        }
    }

    /// Picks the highlighted row, as Enter does: a step row puts its
    /// completion in the field, any other row is returned and closes it.
    pub fn enter(&mut self, sources: &Sources<'_>) -> Option<Target> {
        let rows = self.rows(sources);
        self.rect = None;
        let open = self.open.as_mut()?;
        let row = rows.get(open.selected.min(rows.len().saturating_sub(1)))?;
        self.pick(row)
    }

    fn pick(&mut self, row: &Row) -> Option<Target> {
        if row.disabled.is_some() {
            return None;
        }
        let open = self.open.as_mut()?;
        if let Some(completion) = &row.completion {
            open.query = completion.clone();
            open.selected = 0;
            return None;
        }
        self.finish(row.target.clone());
        Some(row.target.clone())
    }

    fn finish(&mut self, target: Target) {
        self.palette.picked(target);
        self.open = None;
    }

    /// Shows the field when it is open and returns what was picked.
    /// `canvas` is where the drawing is, for placing the field.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        canvas: egui::Rect,
        sources: &Sources<'_>,
    ) -> Option<Target> {
        let place = self.open.as_ref()?.mode == Mode::Place;
        let mut rows = self.rows(sources);
        if place {
            let open = self.open.as_ref()?;
            rows = pp::rows_for(
                &open.category,
                rows,
                &open.query,
                self.palette.symbols(),
                sources,
            );
            if self.catalog.is_none() {
                self.catalog = Some(Catalog::load_default().unwrap_or_else(|_| Catalog::starter()));
            }
        }
        let query_empty = self.open.as_ref()?.query.trim().is_empty();
        let lines = match (place, sources.library) {
            (true, Some(library)) => pp::category_lines(library, sources.macros.len()),
            _ => Vec::new(),
        };
        let open = self.open.as_mut()?;
        open.selected = open.selected.min(rows.len().saturating_sub(1));

        let per_row = pp::columns(GRID_WIDTH);
        let (up, down, left, right, tab, back_tab, enter, escape) = ctx.input_mut(|i| {
            // Left and right move the caret while there is text, except with
            // Alt, which always moves the selection in the symbol grid.
            let sideways = if place && query_empty {
                egui::Modifiers::NONE
            } else {
                egui::Modifiers::ALT
            };
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                place && i.consume_key(sideways, egui::Key::ArrowLeft),
                place && i.consume_key(sideways, egui::Key::ArrowRight),
                place && i.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
                place && i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if escape {
            self.close();
            return None;
        }
        let step = if place { per_row } else { 1 };
        let mut moved = false;
        if up && open.selected >= step {
            open.selected -= step;
            moved = true;
        } else if up && !place {
            open.selected = 0;
        }
        if down && open.selected + step < rows.len() {
            open.selected += step;
            moved = true;
        }
        if left && open.selected > 0 {
            open.selected -= 1;
            moved = true;
        }
        if right && open.selected + 1 < rows.len() {
            open.selected += 1;
            moved = true;
        }
        if tab || back_tab {
            let at = lines
                .iter()
                .position(|l| l.category == open.category)
                .unwrap_or(0);
            let next = if back_tab {
                (at + lines.len() - 1) % lines.len().max(1)
            } else {
                (at + 1) % lines.len().max(1)
            };
            if let Some(line) = lines.get(next) {
                open.category = line.category.clone();
                open.selected = 0;
                open.scroll = true;
                // The rows are those of the old category: show it next frame.
                ctx.request_repaint();
            }
        }
        if moved {
            open.scroll = true;
        }

        let width = if place { PLACE_WIDTH } else { WIDTH };
        let (pivot, pos) = match open.at {
            Some(at) => {
                let x =
                    at.x.min(canvas.right() - width - 8.0)
                        .max(canvas.left() + 8.0);
                let y =
                    at.y.min(canvas.bottom() - MAX_HEIGHT - 60.0)
                        .max(canvas.top() + 8.0);
                (egui::Align2::LEFT_TOP, egui::pos2(x, y))
            }
            None => (
                egui::Align2::CENTER_TOP,
                egui::pos2(canvas.center().x, canvas.top() + 12.0),
            ),
        };
        let hint = match open.mode {
            Mode::All => "Search or run: K1, 12, no K1, export pdf…",
            Mode::Place => "Place: no, nc, coil, m3, terminal, K1 no… or 1 to 9",
        };
        let mut clicked: Option<usize> = None;
        let mut ask_clicked = false;
        let mut new_category: Option<Category> = None;
        let ask_row = rows.is_empty()
            && self.ask_enabled
            && open.mode == Mode::All
            // Two words at least, so a mistyped command is not sent anywhere.
            && open.query.split_whitespace().count() >= 2;
        let scroll = std::mem::take(&mut open.scroll);
        let catalog = self.catalog.as_ref();
        let library = sources.library;
        let area = egui::Area::new(egui::Id::new("command_field"))
            .order(egui::Order::Foreground)
            .pivot(pivot)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(width);
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut open.query)
                            .hint_text(hint)
                            .desired_width(f32::INFINITY),
                    );
                    edit.request_focus();
                    if edit.changed() {
                        open.selected = 0;
                    }
                    ui.separator();
                    if place {
                        place_body(
                            ui,
                            &rows,
                            &lines,
                            &open.category,
                            open.selected,
                            scroll,
                            library,
                            catalog,
                            &mut clicked,
                            &mut new_category,
                        );
                        return;
                    }
                    if rows.is_empty() {
                        if ask_row {
                            let text = format!(
                                "Ask Claude: {}   (Enter sends this to Claude)",
                                open.query.trim()
                            );
                            if ui
                                .selectable_label(true, text)
                                .on_hover_text(
                                    "Sends this question to Claude (your own API key). \
                                     Enter asks.",
                                )
                                .clicked()
                            {
                                ask_clicked = true;
                            }
                        } else {
                            ui.weak("No match");
                        }
                    }
                    egui::ScrollArea::vertical()
                        .max_height(MAX_HEIGHT)
                        .min_scrolled_height(MIN_HEIGHT)
                        .show(ui, |ui| {
                            let tiles = if query_empty {
                                pinned_tiles(&rows)
                            } else {
                                Vec::new()
                            };
                            if !tiles.is_empty() {
                                ui.label(
                                    egui::RichText::new("PINNED")
                                        .small()
                                        .color(ui_palette().text_secondary),
                                );
                                if let Some(i) = tile_row(ui, &rows, &tiles, open.selected) {
                                    clicked = Some(i);
                                }
                                ui.add_space(4.0);
                            }
                            let mut group = "";
                            for (i, row) in rows.iter().enumerate() {
                                // Recent commands are the tiles above.
                                if query_empty && row.group == RECENT_GROUP && tiles.contains(&i) {
                                    continue;
                                }
                                if !row.group.is_empty() && row.group != group {
                                    group = row.group;
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new(row.group.to_uppercase())
                                            .small()
                                            .color(ui_palette().text_secondary),
                                    );
                                }
                                let response = row_ui(ui, row, i == open.selected, library);
                                if i == open.selected && moved {
                                    response.scroll_to_me(None);
                                }
                                if response.clicked() {
                                    clicked = Some(i);
                                }
                            }
                        });
                });
            });
        if let Some(category) = new_category
            && let Some(open) = self.open.as_mut()
        {
            open.category = category;
            open.selected = 0;
            open.scroll = true;
        }
        if ask_row && (ask_clicked || enter) {
            let q = self.open.take().map(|o| o.query.trim().to_string());
            self.asked = q.filter(|q| !q.is_empty());
            self.rect = Some(area.response.rect);
            return None;
        }
        let picked = match (clicked, enter) {
            (Some(i), _) => rows.get(i).and_then(|row| self.pick(row)),
            (None, true) => rows
                .get(self.open.as_ref().map_or(0, |o| o.selected))
                .and_then(|row| self.pick(row)),
            _ => None,
        };
        self.rect = Some(area.response.rect);
        if self.open.is_some() && area.response.clicked_elsewhere() {
            self.close();
        }
        picked
    }
}

/// The body of the place palette: categories, symbol grid, detail pane.
#[allow(clippy::too_many_arguments)]
fn place_body(
    ui: &mut egui::Ui,
    rows: &[Row],
    lines: &[pp::CategoryLine],
    category: &Category,
    selected: usize,
    scroll: bool,
    library: Option<&Library>,
    catalog: Option<&Catalog>,
    clicked: &mut Option<usize>,
    new_category: &mut Option<Category>,
) {
    let p = ui_palette();
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(CATEGORY_WIDTH, PLACE_HEIGHT),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(CATEGORY_WIDTH);
                ui.set_height(PLACE_HEIGHT);
                if let Some(c) = pp::category_column(ui, lines, category) {
                    *new_category = Some(c);
                }
            },
        );
        ui.separator();
        ui.allocate_ui_with_layout(
            egui::vec2(GRID_WIDTH, PLACE_HEIGHT),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(GRID_WIDTH);
                ui.set_height(PLACE_HEIGHT);
                if rows.is_empty() {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("No match").color(p.text_tertiary));
                    return;
                }
                let out = pp::card_grid(ui, rows, library, selected, scroll);
                if out.clicked.is_some() {
                    *clicked = out.clicked;
                }
            },
        );
        ui.separator();
        ui.allocate_ui_with_layout(
            egui::vec2(DETAIL_WIDTH, PLACE_HEIGHT),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(DETAIL_WIDTH);
                ui.set_height(PLACE_HEIGHT);
                pp::detail_pane(ui, rows.get(selected), library, catalog);
            },
        );
    });
}

/// The rows shown as tiles while the field is empty: the commands used
/// last, then the starter set, at most [`PINNED_MAX`].
fn pinned_tiles(rows: &[Row]) -> Vec<usize> {
    let mut tiles: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.group == RECENT_GROUP && matches!(r.target, Target::Command(_)))
        .map(|(i, _)| i)
        .collect();
    for id in PINNED_DEFAULT {
        if tiles.len() >= PINNED_MAX {
            break;
        }
        let found = rows
            .iter()
            .position(|r| r.target == Target::Command(id) && r.group != RECENT_GROUP);
        if let Some(i) = found
            && !tiles.iter().any(|&t| rows[t].target == rows[i].target)
        {
            tiles.push(i);
        }
    }
    tiles.truncate(PINNED_MAX);
    tiles
}

/// A row of tiles: icon above a short label. Returns the row clicked.
fn tile_row(ui: &mut egui::Ui, rows: &[Row], tiles: &[usize], selected: usize) -> Option<usize> {
    let p = ui_palette();
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for &i in tiles {
            let row = &rows[i];
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(82.0, 62.0), egui::Sense::click());
            let enabled = row.disabled.is_none();
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    enabled,
                    i == selected,
                    row.label.as_str(),
                )
            });
            let fill = if i == selected {
                p.accent_wash
            } else if response.hovered() && enabled {
                p.hover
            } else {
                p.control
            };
            let line = if i == selected {
                p.accent
            } else {
                p.border_subtle
            };
            ui.painter().rect_filled(rect, 6.0, fill);
            ui.painter().rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(1.0, line),
                egui::StrokeKind::Inside,
            );
            let ink = if enabled {
                p.text_primary
            } else {
                p.text_tertiary
            };
            pp::kind_glyph(row).paint(
                ui.painter(),
                egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, rect.top() + 21.0),
                    egui::Vec2::splat(22.0),
                ),
                ink,
            );
            let name = match &row.target {
                Target::Command(id) => novo_theme::presentation::short_label(id)
                    .map_or_else(|| row.label.clone(), str::to_owned),
                _ => row.label.clone(),
            };
            novo_theme::widgets::paint_text_ellipsis(
                ui.painter(),
                egui::pos2(rect.left() + 5.0, rect.bottom() - 12.0),
                &name,
                egui::FontId::proportional(11.0),
                ink,
                rect.width() - 10.0,
            );
            if let Some(reason) = row.disabled {
                response.clone().on_hover_text(reason);
            } else {
                response.clone().on_hover_text(&row.label);
            }
            if response.clicked() && enabled {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// Where a command is found, as the chip after its label: the menu path, or
/// the rail, centre or drawer it moved to, or "Keys" when only the field
/// lists it.
fn path_chip(row: &Row) -> Option<String> {
    let (Target::Command(id) | Target::Run { command: id, .. }) = &row.target else {
        return None;
    };
    Some(match novo_workflow::homes::homes(id).first()? {
        Home::Menu(place) => match place.submenu {
            Some(sub) => format!("{} › {sub}", place.menu),
            None => place.menu.to_owned(),
        },
        Home::Rail => "Rail".to_owned(),
        Home::Centre => "Centre".to_owned(),
        Home::DrawerToolbar => "Drawer".to_owned(),
        Home::PaletteOnly(_) => "Keys".to_owned(),
    })
}

/// One row: picture, label, grey detail, then from the right the key cap
/// (or why it is disabled), the menu path chip and the type chip.
fn row_ui(
    ui: &mut egui::Ui,
    row: &Row,
    selected: bool,
    library: Option<&Library>,
) -> egui::Response {
    let p = ui_palette();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::click());
    let enabled = row.disabled.is_none();
    // The text is painted, so name the row for the accessibility tree; the
    // detail follows the label as it did when the row was one button.
    let name = if row.detail.is_empty() || row.detail == row.group {
        row.label.clone()
    } else {
        format!("{}   {}", row.label, row.detail)
    };
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            enabled,
            selected,
            name.as_str(),
        )
    });
    let fill = if selected {
        p.accent_wash
    } else if response.hovered() && enabled {
        p.hover
    } else {
        egui::Color32::TRANSPARENT
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, fill);
    let ink = if enabled {
        p.text_primary
    } else {
        p.text_tertiary
    };
    let picture = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 18.0, rect.center().y),
        egui::Vec2::splat(20.0),
    );
    let symbol = match &row.target {
        Target::Symbol(id) => library.and_then(|l| l.get(id)),
        _ => None,
    };
    match symbol {
        Some(symbol) => pp::paint_symbol(painter, picture, symbol, None, ink, false, false),
        None => pp::kind_glyph(row).paint(painter, picture, ink),
    }
    let cy = rect.center().y;
    let mut right = rect.right() - 8.0;
    match (&row.disabled, &row.shortcut) {
        (Some(reason), _) => {
            let g = painter.layout_no_wrap(
                (*reason).to_owned(),
                egui::FontId::proportional(11.0),
                p.text_tertiary,
            );
            painter.galley(
                egui::pos2(right - g.size().x, cy - g.size().y / 2.0),
                g.clone(),
                p.text_tertiary,
            );
            right -= g.size().x + 8.0;
        }
        (None, Some(key)) => right = pp::paint_key_cap(painter, right, cy, key),
        (None, None) => {}
    }
    right = pp::paint_chip(painter, right, cy, &pp::type_chip(row), false);
    if let Some(path) = path_chip(row) {
        right = pp::paint_chip(painter, right, cy, &path, true);
    }
    let x = rect.left() + 36.0;
    let label = novo_theme::widgets::paint_text_ellipsis(
        painter,
        egui::pos2(x, cy),
        &row.label,
        egui::FontId::proportional(13.5),
        ink,
        (right - x).max(40.0),
    );
    // Under a group heading the group name would only repeat it; a command's
    // detail is the menu path, which is the chip.
    let detail_is_path = matches!(row.target, Target::Command(_) | Target::Run { .. });
    // An add-on's name is in its chip; only its description follows.
    let detail = match pp::addon_parts(row) {
        Some((_, description)) => description,
        None => row.detail.as_str(),
    };
    if !detail.is_empty() && row.detail != row.group && !detail_is_path {
        let from = label.right() + 8.0;
        if right - from > 30.0 {
            novo_theme::widgets::paint_text_ellipsis(
                painter,
                egui::pos2(from, cy),
                detail,
                egui::FontId::proportional(12.0),
                p.text_secondary,
                right - from,
            );
        }
    }
    if let Some(reason) = row.disabled {
        response.clone().on_hover_text(reason);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_workflow::commands::Context;
    use novo_workflow::keymap::Keymap;

    fn ready() -> Context {
        Context {
            has_project: true,
            has_page: true,
            ..Context::default()
        }
    }

    /// Runs one frame of the field and returns what was picked and the
    /// painted shapes.
    fn frame(
        field: &mut CommandField,
        ctx: &egui::Context,
        sources: &Sources<'_>,
        input: egui::RawInput,
    ) -> (Option<Target>, usize) {
        let mut picked = None;
        let mut out = ctx.run_ui(input, |ui| {
            let canvas = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1366.0, 768.0));
            picked = field.show(ui.ctx(), canvas, sources);
        });
        out.textures_delta.clear();
        (picked, out.shapes.len())
    }

    fn input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1366.0, 768.0),
            )),
            ..Default::default()
        }
    }

    fn key(key: egui::Key) -> egui::RawInput {
        let mut raw = input();
        raw.events.push(egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        raw
    }

    #[test]
    fn the_place_palette_draws_and_picks_a_symbol_with_the_keyboard() {
        let library = Library::builtin().unwrap();
        let keymap = Keymap::default();
        let sources = Sources {
            project: None,
            library: Some(&library),
            keymap: &keymap,
            context: ready(),
            lessons: &[],
            macros: &[],
            addons: &[],
        };
        let mut field = CommandField::default();
        let ctx = egui::Context::default();
        field.open(Mode::Place, None);
        let (_, shapes) = frame(&mut field, &ctx, &sources, input());
        assert!(shapes > 200, "{shapes} shapes: the symbols are drawn");
        // Down moves one row of cards; Enter picks that card.
        let (picked, _) = frame(&mut field, &ctx, &sources, key(egui::Key::ArrowRight));
        assert!(picked.is_none());
        let rows = field.rows(&sources);
        let (picked, _) = frame(&mut field, &ctx, &sources, key(egui::Key::Enter));
        let Some(Target::Symbol(id)) = picked else {
            panic!("Enter picks a symbol, got {picked:?}");
        };
        assert_eq!(Target::Symbol(id), rows[1].target);
        assert!(!field.is_open());
    }

    #[test]
    fn tab_walks_the_categories_and_a_folder_lists_its_symbols() {
        let library = Library::builtin().unwrap();
        let keymap = Keymap::default();
        let sources = Sources {
            project: None,
            library: Some(&library),
            keymap: &keymap,
            context: ready(),
            lessons: &[],
            macros: &[],
            addons: &[],
        };
        let mut field = CommandField::default();
        let ctx = egui::Context::default();
        field.open(Mode::Place, None);
        frame(&mut field, &ctx, &sources, input());
        // Start, then Circuits, then the first folder.
        frame(&mut field, &ctx, &sources, key(egui::Key::Tab));
        frame(&mut field, &ctx, &sources, key(egui::Key::Tab));
        let first = library.iter().map(|s| s.category.clone()).min().unwrap();
        assert_eq!(
            field.open.as_ref().unwrap().category,
            Category::Library(first.clone())
        );
        let (picked, _) = frame(&mut field, &ctx, &sources, key(egui::Key::Enter));
        let Some(Target::Symbol(id)) = picked else {
            panic!("the first symbol of the folder is picked");
        };
        assert_eq!(library.get(&id).unwrap().category, first);
    }

    #[test]
    fn the_command_palette_shows_tiles_and_rows_with_pictures() {
        let library = Library::builtin().unwrap();
        let keymap = Keymap::default();
        let sources = Sources {
            project: None,
            library: Some(&library),
            keymap: &keymap,
            context: ready(),
            lessons: &[],
            macros: &[],
            addons: &[],
        };
        let mut field = CommandField::default();
        let ctx = egui::Context::default();
        field.open(Mode::All, None);
        let (_, empty) = frame(&mut field, &ctx, &sources, input());
        assert!(empty > 100, "{empty} shapes");
        let rows = field.rows(&sources);
        let tiles = pinned_tiles(&rows);
        assert!(!tiles.is_empty() && tiles.len() <= PINNED_MAX);
        for &i in &tiles {
            assert!(matches!(rows[i].target, Target::Command(_)));
        }
        // Enter on the first row runs it, as before the redesign.
        let (picked, _) = frame(&mut field, &ctx, &sources, key(egui::Key::Enter));
        assert_eq!(picked.as_ref(), Some(&rows[0].target));
    }

    #[test]
    fn the_default_tiles_are_real_commands_with_a_picture() {
        for id in PINNED_DEFAULT {
            assert!(
                novo_workflow::commands::get(id).is_some(),
                "{id} is not a command"
            );
            assert!(
                novo_theme::presentation::of(id).is_some(),
                "{id} has no picture"
            );
        }
    }

    #[test]
    fn a_command_chip_names_where_it_lives() {
        let row = Row {
            target: Target::Command("file.save"),
            kind: novo_workflow::palette::Kind::Command,
            label: "Save".into(),
            detail: String::new(),
            group: "",
            completion: None,
            shortcut: None,
            disabled: None,
            score: 0,
        };
        assert_eq!(path_chip(&row).as_deref(), Some("File"));
    }
}
