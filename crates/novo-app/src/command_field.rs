//! The command field (docs/DESIGN.md §4): Ctrl+K opens it at the top, I
//! (place mode) and a tapped Space open it at the pointer. The search and
//! its grammar are novo-workflow's [`Palette`] (WS-08); this is only how
//! the rows are shown and picked.

use eframe::egui;
use novo_workflow::keymap::Focus;
use novo_workflow::palette::{Kind, Mode, Palette, Row, Sources, Target};

/// Rows shown at once; the rest scroll.
const MAX_HEIGHT: f32 = 460.0;
/// The list keeps at least this much room when it has rows to fill it.
const MIN_HEIGHT: f32 = 300.0;
const WIDTH: f32 = 480.0;

struct OpenField {
    mode: Mode,
    query: String,
    selected: usize,
    /// Screen position for the field at the pointer; `None` is the top.
    at: Option<egui::Pos2>,
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
        let rows = self.rows(sources);
        let open = self.open.as_mut()?;
        open.selected = open.selected.min(rows.len().saturating_sub(1));

        let (up, down, enter, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if escape {
            self.open = None;
            return None;
        }
        if up {
            open.selected = open.selected.saturating_sub(1);
        }
        if down && open.selected + 1 < rows.len() {
            open.selected += 1;
        }

        let (pivot, pos) = match open.at {
            Some(at) => {
                let x =
                    at.x.min(canvas.right() - WIDTH - 8.0)
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
        let ask_row = rows.is_empty()
            && self.ask_enabled
            && open.mode == Mode::All
            // Two words at least, so a mistyped command is not sent anywhere.
            && open.query.split_whitespace().count() >= 2;
        let area = egui::Area::new(egui::Id::new("command_field"))
            .order(egui::Order::Foreground)
            .pivot(pivot)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(WIDTH);
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
                            let mut group = "";
                            for (i, row) in rows.iter().enumerate() {
                                if !row.group.is_empty() && row.group != group {
                                    group = row.group;
                                    ui.add_space(2.0);
                                    ui.small(row.group.to_uppercase());
                                }
                                let response = row_ui(ui, row, i == open.selected);
                                if i == open.selected && (up || down) {
                                    response.scroll_to_me(None);
                                }
                                if response.clicked() {
                                    clicked = Some(i);
                                }
                            }
                        });
                });
            });
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
            self.open = None;
        }
        picked
    }
}

/// One row: kind, label, grey detail, and the key or why it is disabled.
fn row_ui(ui: &mut egui::Ui, row: &Row, selected: bool) -> egui::Response {
    // Under a group heading the group name would only repeat it.
    let detail = if row.detail == row.group {
        ""
    } else {
        row.detail.as_str()
    };
    let mut text = row.label.clone();
    if !detail.is_empty() {
        text.push_str("   ");
    }
    let mut job = egui::text::LayoutJob::default();
    let body = egui::TextFormat {
        color: ui.visuals().text_color(),
        ..Default::default()
    };
    let weak = egui::TextFormat {
        color: ui.visuals().weak_text_color(),
        ..Default::default()
    };
    job.append(&text, 0.0, body);
    job.append(detail, 0.0, weak);
    let right = match (row.disabled, &row.shortcut) {
        (Some(reason), _) => reason.to_owned(),
        (None, Some(key)) => key.clone(),
        (None, None) if row.kind == Kind::Command => String::new(),
        (None, None) => row.kind.label().to_owned(),
    };
    let button = egui::Button::selectable(selected, job)
        .shortcut_text(right)
        .min_size(egui::vec2(ui.available_width(), 0.0));
    ui.add_enabled(row.disabled.is_none(), button)
}
