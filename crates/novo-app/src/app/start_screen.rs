//! The start screen (docs/design/visual-redesign.md section 10, board
//! "Start screen"): tiles for the ways in on the left, recent projects as
//! cards with a picture of their first page on the right, and cards for the
//! parts of the program a new person would not find alone.
//!
//! The pictures are read in the background, read-only, and drawn with the
//! macro browser's symbol painter: symbols, wires and junctions of the first
//! schematic page, scaled to fit. They show no sheet frame or text.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Vec2};
use novo_core::{BlockDefinition, Record};
use novo_parts::Thumbnail;
use novo_theme::Palette;
use novo_theme::icons;

use super::{Leaving, NovoApp, macro_thumb};
use crate::actions::Action;
use crate::theme::ui_palette;

/// Recent projects shown as cards (the settings keep ten).
const CARDS_SHOWN: usize = 6;
/// Below this width the two columns stack.
const TWO_COLUMNS_FROM: f32 = 820.0;
const LEFT_WIDTH: f32 = 380.0;

/// What the card of one project needs, read from the file.
struct Card {
    pages: usize,
    thumb: Thumbnail,
}

enum State {
    Loading,
    Ready(Arc<Card>),
    Unreadable,
}

struct Entry {
    stamp: Option<SystemTime>,
    state: State,
}

/// The cards read so far, by path; shared with the reading threads.
#[derive(Clone, Default)]
struct Cards(Arc<Mutex<HashMap<PathBuf, Entry>>>);

impl Cards {
    /// The state of `path`, starting a read when the file is new or changed.
    fn get(&self, ctx: &egui::Context, path: &Path) -> (Option<SystemTime>, StateView) {
        let stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = map.get(path).is_some_and(|e| e.stamp == stamp);
        if !fresh {
            map.insert(
                path.to_owned(),
                Entry {
                    stamp,
                    state: State::Loading,
                },
            );
            if stamp.is_some() {
                let shared = self.clone();
                let ctx = ctx.clone();
                let path = path.to_owned();
                std::thread::spawn(move || {
                    let state = match read_card(&path) {
                        Some(card) => State::Ready(Arc::new(card)),
                        None => State::Unreadable,
                    };
                    let mut map = shared.0.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(e) = map.get_mut(&path)
                        && e.stamp == stamp
                    {
                        e.state = state;
                    }
                    ctx.request_repaint();
                });
            }
        }
        let view = match map.get(path).map(|e| &e.state) {
            Some(State::Ready(card)) => StateView::Ready(Arc::clone(card)),
            Some(State::Unreadable) => StateView::Unreadable,
            _ if stamp.is_none() => StateView::Missing,
            _ => StateView::Loading,
        };
        (stamp, view)
    }
}

enum StateView {
    Loading,
    Ready(Arc<Card>),
    Unreadable,
    Missing,
}

/// Reads the project without writing and draws its first schematic page
/// into picture data. `None` when the file cannot be read.
fn read_card(path: &Path) -> Option<Card> {
    let project = novo_storage::ProjectFile::read(path).ok()?;
    let pages = project.pages_in_order();
    let schematic = |p: &&&novo_core::Page| matches!(p.kind, novo_core::PageKind::Schematic);
    let has_symbols = |id: novo_core::PageId| project.symbols().any(|s| s.page == id);
    let first = pages
        .iter()
        .filter(schematic)
        .find(|p| has_symbols(p.id))
        .or_else(|| pages.first())
        .map(|p| p.id);
    let members: Vec<Record> = project
        .records()
        .filter(|r| {
            matches!(r, Record::Symbol(_) | Record::Junction(_) | Record::Wire(_))
                && first.is_some()
                && r.page() == first
        })
        .collect();
    let def = BlockDefinition {
        id: 0.into(),
        name: String::new(),
        description: String::new(),
        parameters: Vec::new(),
        members,
        source: None,
        properties: Default::default(),
    };
    let mut thumb = novo_parts::thumbnail(&def);
    // Symbol artwork reaches past its pins (a motor circle, a coil body):
    // leave room so the picture is not cut at the edge.
    if let Some((min, max)) = thumb.bounds {
        let room = 10.0 + 0.04 * (max.x - min.x).max(max.y - min.y);
        thumb.bounds = Some((
            novo_core::Point::new(min.x - room, min.y - room),
            novo_core::Point::new(max.x + room, max.y + room),
        ));
    }
    Some(Card {
        pages: pages.len(),
        thumb,
    })
}

/// "Today", "Yesterday" or "N days ago".
fn when(stamp: SystemTime) -> String {
    let secs = SystemTime::now()
        .duration_since(stamp)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match secs {
        0..86_400 => "Today".to_owned(),
        86_400..172_800 => "Yesterday".to_owned(),
        _ => format!("{} days ago", secs / 86_400),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// One tile: an icon in a square, a title, a caption under it and, on the
/// right, the key. Painted by hand so the whole tile is one target.
struct TileSpec<'a> {
    icon: &'a str,
    title: &'a str,
    caption: &'a str,
    key: Option<String>,
    /// The tile that is the main way in: copper outline and wash.
    primary: bool,
    enabled: bool,
}

fn row_tile(ui: &mut egui::Ui, spec: &TileSpec) -> egui::Response {
    let p = ui_palette();
    let width = ui.available_width();
    let icon_box = if spec.primary { 40.0 } else { 32.0 };
    let pad = if spec.primary { 14.0 } else { 10.0 };
    let key_width = spec
        .key
        .as_ref()
        .map_or(0.0, |k| 14.0 + 7.0 * k.chars().count() as f32);
    let text_x = pad + icon_box + 12.0;
    let wrap = (width - text_x - pad - key_width).max(60.0);
    let title_font = FontId::proportional(if spec.primary { 15.0 } else { 14.0 });
    let title_color = if spec.enabled {
        p.text_primary
    } else {
        p.text_tertiary
    };
    let title = ui
        .painter()
        .layout_no_wrap(spec.title.to_owned(), title_font, title_color);
    let caption = (!spec.caption.is_empty()).then(|| {
        ui.painter().layout(
            spec.caption.to_owned(),
            FontId::proportional(12.0),
            p.text_secondary,
            wrap,
        )
    });
    let text_height = title.size().y + caption.as_ref().map_or(0.0, |c| c.size().y + 1.0);
    let height = (text_height.max(icon_box) + 2.0 * pad).max(if spec.primary { 66.0 } else { 0.0 });
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(width, height),
        if spec.enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    paint_surface(ui, rect, &response, spec.primary, p);
    let icon_rect = Rect::from_min_size(
        rect.left_top() + Vec2::new(pad, (height - icon_box) / 2.0),
        Vec2::splat(icon_box),
    );
    paint_icon_box(ui, icon_rect, spec.icon, spec.primary, spec.enabled, p);
    let mut y = rect.top() + (height - text_height) / 2.0;
    let x = rect.left() + text_x;
    let title_h = title.size().y;
    ui.painter().galley(egui::pos2(x, y), title, title_color);
    y += title_h + 1.0;
    if let Some(c) = caption {
        ui.painter().galley(egui::pos2(x, y), c, p.text_secondary);
    }
    if let Some(key) = &spec.key {
        paint_key(ui, rect, key, pad, p);
    }
    let enabled = spec.enabled;
    let label = spec.title.to_owned();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
    if spec.enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// A vertical tile: icon on top, title, a caption that wraps.
fn card_tile(
    ui: &mut egui::Ui,
    width: f32,
    icon: &str,
    title: &str,
    caption: &str,
) -> egui::Response {
    let p = ui_palette();
    let pad = 12.0;
    let title_galley =
        ui.painter()
            .layout_no_wrap(title.to_owned(), FontId::proportional(14.0), p.text_primary);
    let caption_galley = ui.painter().layout(
        caption.to_owned(),
        FontId::proportional(12.0),
        p.text_secondary,
        width - 2.0 * pad,
    );
    // One height per row: the longest caption of a narrow card needs three lines.
    let natural = pad + 32.0 + 10.0 + title_galley.size().y + 2.0 + caption_galley.size().y + pad;
    let height = natural.max(if width < 240.0 { 126.0 } else { 100.0 });
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    paint_surface(ui, rect, &response, false, p);
    let icon_rect = Rect::from_min_size(rect.left_top() + Vec2::splat(pad), Vec2::splat(32.0));
    paint_icon_box(ui, icon_rect, icon, false, true, p);
    let mut y = icon_rect.bottom() + 10.0;
    let x = rect.left() + pad;
    let title_h = title_galley.size().y;
    ui.painter()
        .galley(egui::pos2(x, y), title_galley, p.text_primary);
    y += title_h + 2.0;
    ui.painter()
        .galley(egui::pos2(x, y), caption_galley, p.text_secondary);
    let label = title.to_owned();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_surface(ui: &egui::Ui, rect: Rect, response: &egui::Response, primary: bool, p: &Palette) {
    let radius = egui::CornerRadius::same(novo_theme::radius::SURFACE);
    let fill = match (primary, response.hovered() && response.sense.senses_click()) {
        (true, _) => p.accent_wash,
        (false, true) => p.hover,
        (false, false) => p.control,
    };
    ui.painter().rect_filled(rect, radius, fill);
    let stroke = if response.has_focus() {
        Stroke::new(2.0, p.accent)
    } else if primary {
        Stroke::new(1.0, p.accent)
    } else if response.hovered() {
        Stroke::new(1.0, p.border_strong)
    } else {
        Stroke::new(1.0, p.border_subtle)
    };
    ui.painter()
        .rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
}

fn paint_icon_box(
    ui: &egui::Ui,
    rect: Rect,
    icon: &str,
    primary: bool,
    enabled: bool,
    p: &Palette,
) {
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(8), p.raised);
    let color = if !enabled {
        p.text_tertiary
    } else if primary {
        p.accent
    } else {
        p.text_secondary
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(rect.width() * 0.55),
        color,
    );
}

fn paint_key(ui: &egui::Ui, tile: Rect, key: &str, pad: f32, p: &Palette) {
    let galley =
        ui.painter()
            .layout_no_wrap(key.to_owned(), FontId::monospace(11.0), p.text_secondary);
    let size = galley.size() + Vec2::new(10.0, 4.0);
    let rect = Rect::from_min_size(
        egui::pos2(tile.right() - pad - size.x, tile.center().y - size.y / 2.0),
        size,
    );
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(novo_theme::radius::XS),
        p.sunken,
    );
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(novo_theme::radius::XS),
        Stroke::new(1.0, p.border_subtle),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.min + Vec2::new(5.0, 2.0), galley, p.text_secondary);
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    let p = ui_palette();
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(11.0)
            .extra_letter_spacing(1.0)
            .color(p.text_secondary),
    );
}

/// The mark, "KoperCAD" and "BY KOPERVONK", left aligned.
fn lockup_left(ui: &mut egui::Ui) {
    let p = ui_palette();
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
    novo_theme::brand::paint_mark(ui.painter(), rect, true);
    ui.add_space(10.0);
    let font = FontId::new(26.0, novo_theme::fonts::strong_family());
    let mut job = egui::text::LayoutJob::default();
    job.append(
        "Koper",
        0.0,
        egui::TextFormat::simple(font.clone(), p.text_primary),
    );
    job.append("CAD", 0.0, egui::TextFormat::simple(font, p.accent));
    ui.label(job);
    ui.label(
        egui::RichText::new(format!("BY {}", novo_theme::brand::MAKER.to_uppercase()))
            .size(12.0)
            .family(novo_theme::fonts::strong_family())
            .extra_letter_spacing(1.2)
            .color(p.text_secondary),
    );
}

/// Fits `n` columns of at least `min` points in `width`, at most `max_cols`.
fn columns(width: f32, gap: f32, min: f32, max_cols: usize) -> usize {
    (((width + gap) / (min + gap)).floor() as usize).clamp(1, max_cols)
}

impl NovoApp {
    /// With no project open: the ways in, the recent projects and what else
    /// is in the program.
    pub(super) fn start_screen(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        let p = ui_palette();
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, p.sunken);
        if rect.width() >= TWO_COLUMNS_FROM {
            let split = rect.left() + LEFT_WIDTH;
            let left = Rect::from_min_max(rect.min, egui::pos2(split, rect.bottom()));
            let right = Rect::from_min_max(egui::pos2(split, rect.top()), rect.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("start_left")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let inner = Rect::from_min_max(
                            ui.max_rect().min + Vec2::new(56.0, 48.0),
                            egui::pos2(ui.max_rect().right() - 32.0, f32::INFINITY),
                        );
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            self.start_ways_in(ui, picked);
                        });
                    });
            });
            ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("start_right")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let inner = Rect::from_min_max(
                            ui.max_rect().min + Vec2::new(16.0, 48.0),
                            egui::pos2(ui.max_rect().right() - 48.0, f32::INFINITY),
                        );
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            self.start_recent(ui, picked);
                            ui.add_space(24.0);
                            self.start_more(ui);
                            ui.add_space(32.0);
                        });
                    });
            });
        } else {
            egui::ScrollArea::vertical()
                .id_salt("start_stacked")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let side = if rect.width() > 480.0 { 32.0 } else { 16.0 };
                    let inner = Rect::from_min_max(
                        ui.max_rect().min + Vec2::new(side, 24.0),
                        egui::pos2(ui.max_rect().right() - side, f32::INFINITY),
                    );
                    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                        self.start_ways_in(ui, picked);
                        ui.add_space(24.0);
                        self.start_recent(ui, picked);
                        ui.add_space(24.0);
                        self.start_more(ui);
                        ui.add_space(32.0);
                    });
                });
        }
    }

    /// The mark and the four tiles, then the quiet extras.
    fn start_ways_in(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        let p = ui_palette();
        lockup_left(ui);
        ui.add_space(24.0);
        let first_run = self.first_run();
        let key_of = |ui: &egui::Ui, a: Action| a.shortcut().map(|s| ui.ctx().format_shortcut(&s));
        let tiles = [
            (
                Action::NewProject,
                icons::FILE_PLUS,
                "A name and Enter",
                !first_run,
            ),
            (Action::OpenProject, icons::FOLDER_OPEN, "", false),
            (
                Action::OpenExample,
                icons::GRADUATION_CAP,
                "The Gate 1 motor starter",
                false,
            ),
            (
                Action::TryFastKeys,
                icons::KEYBOARD,
                "Two minutes: place, wire, cable, live view",
                first_run,
            ),
        ];
        for (action, icon, caption, primary) in tiles {
            let spec = TileSpec {
                icon,
                title: action.label(),
                caption,
                key: key_of(ui, action),
                primary,
                enabled: self.is_enabled(action),
            };
            if row_tile(ui, &spec).clicked() {
                picked.push(action);
            }
            ui.add_space(8.0);
        }
        // Lessons: the entry the learning modules anchor to.
        let label = self.learn.say("learn_menu");
        let learn = row_tile(
            ui,
            &TileSpec {
                icon: icons::BOOK_OPEN,
                title: &label,
                caption: "Lessons for every part of the program",
                key: None,
                primary: false,
                enabled: true,
            },
        );
        self.anchor_response("start.learn", &learn, "Learn");
        if learn.clicked() {
            self.learn.window = true;
        }
        ui.add_space(14.0);
        // Quiet, not a headline: most people never need it, and the ones who
        // do look for the word "recover".
        let recover = ui
            .add(
                egui::Button::new(
                    egui::RichText::new("File damaged? Recover it…")
                        .underline()
                        .color(ui.visuals().hyperlink_color),
                )
                .frame(false),
            )
            .on_hover_text(
                "Reads what can be read from a damaged project and opens it as a recovered \
                 copy beside it. The damaged file is not changed.",
            );
        if recover.clicked() {
            let ctx = ui.ctx().clone();
            self.leave_then(&ctx, Leaving::Recover);
        }
        let unsaved = self.backend.unsaved_projects();
        if !unsaved.is_empty() {
            ui.add_space(16.0);
            section_label(ui, "Unsaved");
            let (newest, older) = unsaved.split_at(unsaved.len().min(super::UNSAVED_SHOWN));
            let mut open = None;
            let mut rows = |ui: &mut egui::Ui, list: &[(PathBuf, SystemTime)]| {
                for (path, changed) in list {
                    let row = ui.add(
                        egui::Button::new(format!(
                            "{}, {}",
                            crate::backend::kept_label(path),
                            super::ago(*changed)
                        ))
                        .frame(false),
                    );
                    if row.clicked() {
                        open = Some(path.clone());
                    }
                }
            };
            rows(ui, newest);
            if !older.is_empty() {
                egui::CollapsingHeader::new(format!("{} older", older.len()))
                    .id_salt("unsaved_older")
                    .show(ui, |ui| rows(ui, older));
            }
            if let Some(path) = open {
                let ctx = ui.ctx().clone();
                self.leave_then(&ctx, Leaving::OpenPath(path));
            }
        }
        ui.add_space(20.0);
        ui.label(
            egui::RichText::new("Ctrl K finds every command · F10 opens the menu")
                .size(12.0)
                .color(p.text_secondary),
        );
    }

    /// Recent projects as cards with a picture of the first page.
    fn start_recent(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        let p = ui_palette();
        ui.horizontal(|ui| {
            section_label(ui, "Recent projects");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let browse = ui.add(egui::Button::new(
                    egui::RichText::new(format!("{}  Browse", icons::FOLDER_OPEN))
                        .color(p.text_secondary),
                ));
                if browse.clicked() && self.is_enabled(Action::OpenProject) {
                    picked.push(Action::OpenProject);
                }
            });
        });
        ui.add_space(8.0);
        let recent: Vec<PathBuf> = self
            .settings
            .recent_projects
            .iter()
            .take(CARDS_SHOWN)
            .cloned()
            .collect();
        if recent.is_empty() {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 96.0), Sense::hover());
            ui.painter().rect_stroke(
                rect,
                egui::CornerRadius::same(novo_theme::radius::SURFACE),
                Stroke::new(1.0, p.border_subtle),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Projects you open or save appear here.",
                FontId::proportional(13.0),
                p.text_secondary,
            );
            return;
        }
        let cache: Cards = ui
            .ctx()
            .data_mut(|d| d.get_temp::<Cards>(egui::Id::new("start_cards")))
            .unwrap_or_else(|| {
                let c = Cards::default();
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::new("start_cards"), c.clone()));
                c
            });
        let gap = 14.0;
        let width = ui.available_width();
        let cols = columns(width, gap, 220.0, 3);
        let card_w = (width - gap * (cols as f32 - 1.0)) / cols as f32;
        let thumb_h = (card_w * 0.66).clamp(110.0, 200.0);
        let mut open = None;
        for row in recent.chunks(cols) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for path in row {
                    let size = Vec2::new(card_w, thumb_h + 48.0);
                    let layout = egui::Layout::top_down(egui::Align::Min);
                    ui.allocate_ui_with_layout(size, layout, |ui| {
                        ui.set_width(card_w);
                        let (stamp, state) = cache.get(ui.ctx(), path);
                        if self.project_card(ui, path, stamp, &state, card_w, thumb_h) {
                            open = Some(path.clone());
                        }
                    });
                }
            });
            ui.add_space(14.0);
        }
        if let Some(path) = open {
            let ctx = ui.ctx().clone();
            self.leave_then(&ctx, Leaving::OpenPath(path));
        }
    }

    /// One recent project; true when it was clicked.
    fn project_card(
        &self,
        ui: &mut egui::Ui,
        path: &Path,
        stamp: Option<SystemTime>,
        state: &StateView,
        width: f32,
        thumb_h: f32,
    ) -> bool {
        let p = ui_palette();
        let name = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, thumb_h), Sense::click());
        let radius = egui::CornerRadius::same(6);
        ui.painter().rect_filled(rect, radius, p.sunken);
        match state {
            StateView::Ready(card) if card.thumb.bounds.is_some() => {
                macro_thumb::paint(ui, rect, &card.thumb);
            }
            other => {
                let text = match other {
                    StateView::Loading => "Reading…",
                    StateView::Missing => "File not found",
                    StateView::Unreadable => "No preview",
                    StateView::Ready(card) if card.pages == 0 => "No pages yet",
                    StateView::Ready(_) => "Empty page",
                };
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(12.0),
                    p.text_tertiary,
                );
            }
        }
        let stroke = if response.hovered() || response.has_focus() {
            Stroke::new(1.5, p.accent)
        } else {
            Stroke::new(1.0, p.border_control)
        };
        ui.painter()
            .rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(egui::RichText::new(&name).strong().color(p.text_primary)).truncate(),
        );
        let mut parts: Vec<String> = Vec::new();
        if let Some(s) = stamp {
            parts.push(when(s));
        }
        if let StateView::Ready(card) = state {
            parts.push(plural(card.pages, "page", "pages"));
        }
        let caption = if matches!(state, StateView::Missing) {
            "File not found".to_owned()
        } else {
            parts.join(" · ")
        };
        ui.add(
            egui::Label::new(
                egui::RichText::new(caption)
                    .size(12.0)
                    .color(p.text_secondary),
            )
            .truncate(),
        );
        let label = format!("Open {name}");
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
        let clicked = response.clicked();
        response
            .on_hover_text(path.display().to_string())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        clicked
    }

    /// Cards for parts of the program a new person would not find alone;
    /// each opens its lesson.
    fn start_more(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "What else is in here");
        ui.add_space(8.0);
        let items = [
            (
                icons::canvas::CABINET,
                "Cabinet",
                "Size the enclosure, place devices, cut lists",
                "cabinet-layout",
            ),
            (
                icons::canvas::VIEW_3D,
                "3D",
                "Look inside the cabinet. Needs a mounting plate",
                "view-3d",
            ),
            (
                icons::THERMOMETER_HOT,
                "Heat",
                "Temperatures and airflow, under Simulate",
                "heat-and-sizing",
            ),
            (
                icons::canvas::COMMISSIONING,
                "Commissioning",
                "Checks and a punch list",
                "commissioning",
            ),
        ];
        let gap = 12.0;
        let width = ui.available_width();
        let cols = columns(width, gap, 170.0, 4);
        let w = (width - gap * (cols as f32 - 1.0)) / cols as f32;
        let mut lesson = None;
        for row in items.chunks(cols) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (icon, title, caption, id) in row {
                    if card_tile(ui, w, icon, title, caption).clicked() {
                        lesson = Some(*id);
                    }
                }
            });
            ui.add_space(gap);
        }
        if let Some(id) = lesson {
            self.start_lesson(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_follow_the_width() {
        assert_eq!(columns(1000.0, 14.0, 220.0, 3), 3);
        assert_eq!(columns(500.0, 14.0, 220.0, 3), 2);
        assert_eq!(columns(200.0, 14.0, 220.0, 3), 1);
        assert_eq!(columns(2000.0, 12.0, 170.0, 4), 4);
    }

    #[test]
    fn age_reads_as_a_day_word() {
        let now = SystemTime::now();
        assert_eq!(when(now), "Today");
        assert_eq!(
            when(now - std::time::Duration::from_secs(30 * 3600)),
            "Yesterday"
        );
        assert_eq!(
            when(now - std::time::Duration::from_secs(5 * 86_400 + 60)),
            "5 days ago"
        );
        assert_eq!(plural(1, "page", "pages"), "1 page");
        assert_eq!(plural(31, "page", "pages"), "31 pages");
    }

    #[test]
    fn an_unreadable_file_has_no_card() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("junk.novo");
        std::fs::write(&path, b"not a project").unwrap();
        assert!(read_card(&path).is_none());
        assert!(read_card(&dir.path().join("missing.novo")).is_none());
    }

    #[test]
    fn a_saved_project_gives_a_card_with_its_page_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.novo");
        let project = novo_core::Project::new("Card test");
        novo_storage::ProjectFile::create(&path, &project).unwrap();
        let card = read_card(&path);
        // A new project has no page yet: the card is still there, without a picture.
        let card = card.expect("a project with no pages still gives a card");
        assert_eq!(card.pages, project.pages().count());
        assert!(card.thumb.bounds.is_none());
    }
}
