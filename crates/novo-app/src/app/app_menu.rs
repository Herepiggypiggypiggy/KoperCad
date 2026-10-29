//! The app menu (docs/design/visual-redesign.md section 5.1). The mark opens one
//! two-pane menu: the eight areas on the left, the commands of the area under the
//! pointer on the right, a search field on top that filters across all areas, and
//! About and Preferences in the footer. It reads the same registry layout as the
//! menu bar (`novo_workflow::menus`), so a command is in the bar and here or in
//! neither, and it takes its icon and short label from the presentation table of
//! `novo-theme`. The full sentence of a command is the tooltip.
//!
//! The menu words in the top bar are a setting (`Settings::show_menu_names`).

use std::collections::HashSet;

use eframe::egui;
use novo_theme::icons;
use novo_theme::presentation;
use novo_workflow::commands::CommandInfo;
use novo_workflow::menus::{self, MENUS, MenuItem};

use super::NovoApp;

/// Width of the whole menu, of the area list, in points.
const WIDTH: f32 = 640.0;
const AREAS_WIDTH: f32 = 176.0;
/// One row, in points.
const ROW: f32 = 28.0;

/// What the app menu remembers between frames.
#[derive(Default)]
pub(super) struct AppMenu {
    /// The area shown on the right (an index into [`MENUS`]).
    pub(super) area: usize,
    /// The search text; empty shows the area.
    pub(super) filter: String,
    /// The highlighted row of the right pane (arrow keys, Enter).
    pub(super) row: usize,
    /// Whether the arrow keys move in the right pane (else in the area list).
    pub(super) in_rows: bool,
    /// Groups ("3D", "Open") unfolded in the right pane.
    pub(super) expanded: HashSet<&'static str>,
    /// The popup was open last frame.
    pub(super) was_open: bool,
    /// The search field takes the focus once, in the first frame.
    fresh: bool,
    /// A button elsewhere in the bar asks for the menu at this area (an
    /// index into [`MENUS`]); the next frame opens it.
    pub(super) open_at: Option<usize>,
}

/// One line of the right pane.
#[derive(Clone, Copy)]
pub(super) enum Entry {
    Command {
        info: &'static CommandInfo,
        /// Inside an unfolded group.
        indent: bool,
    },
    /// A submenu of the area, folded or unfolded.
    Group {
        name: &'static str,
        count: usize,
        open: bool,
    },
    Separator,
}

/// The icon of an area, a generic one (the electrical icons belong to commands).
fn area_icon(menu: &str) -> &'static str {
    match menu {
        "File" => icons::FILE,
        "Edit" => icons::PENCIL_SIMPLE,
        "Draw" => icons::PENCIL_LINE,
        "View" => icons::EYE,
        "Check" => icons::CHECK_CIRCLE,
        "Reports" => icons::FILE_TEXT,
        "Simulate" => icons::PLAY,
        _ => icons::QUESTION,
    }
}

/// The key that, with Alt, picks an area: the first letter of its name (the
/// eight are all different).
fn mnemonic(menu: &str) -> Option<egui::Key> {
    match menu.chars().next()? {
        'F' => Some(egui::Key::F),
        'E' => Some(egui::Key::E),
        'D' => Some(egui::Key::D),
        'V' => Some(egui::Key::V),
        'C' => Some(egui::Key::C),
        'R' => Some(egui::Key::R),
        'S' => Some(egui::Key::S),
        'H' => Some(egui::Key::H),
        _ => None,
    }
}

/// The lines of one area: its commands, its submenus as groups (their rows
/// follow while unfolded), its separators.
pub(super) fn area_entries(menu: &str, expanded: &HashSet<&'static str>) -> Vec<Entry> {
    let mut out = Vec::new();
    for item in menus::drawn(menu) {
        match item {
            MenuItem::Separator => {
                if !matches!(out.last(), None | Some(Entry::Separator)) {
                    out.push(Entry::Separator);
                }
            }
            MenuItem::Command(info) => out.push(Entry::Command {
                info,
                indent: false,
            }),
            MenuItem::Submenu { name, items } => {
                let open = expanded.contains(name);
                out.push(Entry::Group {
                    name,
                    count: items.len(),
                    open,
                });
                if open {
                    out.extend(
                        items
                            .into_iter()
                            .map(|info| Entry::Command { info, indent: true }),
                    );
                }
            }
        }
    }
    while matches!(out.last(), Some(Entry::Separator)) {
        out.pop();
    }
    out
}

/// Every command of every area whose label, short label, id or keywords hold
/// all the words of `filter`: those named by the words first, then those found
/// by a keyword or id, each group in menu order.
pub(super) fn search_entries(filter: &str) -> Vec<Entry> {
    let words: Vec<String> = filter
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();
    // Commands whose own name holds every word come before those found only by
    // a keyword or the id, so Enter on "pdf" runs "Export PDF".
    let mut named = Vec::new();
    let mut by_keyword = Vec::new();
    let mut seen = HashSet::new();
    for menu in MENUS {
        for (info, _) in menus::entries(menu) {
            if !seen.insert(info.id) {
                continue;
            }
            let name = format!(
                "{} {}",
                info.label,
                presentation::short_label(info.id).unwrap_or("")
            )
            .to_lowercase();
            let hay = format!("{name} {} {}", info.id, info.keywords.join(" ")).to_lowercase();
            let entry = Entry::Command {
                info,
                indent: false,
            };
            if words.iter().all(|w| name.contains(w.as_str())) {
                named.push(entry);
            } else if words.iter().all(|w| hay.contains(w.as_str())) {
                by_keyword.push(entry);
            }
        }
    }
    named.extend(by_keyword);
    named
}

/// "View > 3D": where a command sits in the menus.
fn path_of(info: &CommandInfo) -> String {
    match menus::place(info) {
        Some(menus::MenuPlace {
            menu,
            submenu: Some(sub),
        }) => format!("{menu} > {sub}"),
        Some(place) => place.menu.to_owned(),
        None => String::new(),
    }
}

impl NovoApp {
    /// Whether the menu words are in the top bar this frame: a wide window
    /// and the setting.
    pub(super) fn menu_names_shown(&self) -> bool {
        self.menu_bar_wide() && self.settings.show_menu_names()
    }

    /// The mark, with a caret: the one button that opens the app menu. `open`
    /// opens it from the keyboard (F10).
    pub(super) fn mark(&mut self, ui: &mut egui::Ui, open: bool) {
        let pal = crate::theme::ui_palette();
        let text = egui::RichText::new(format!("{} {}", super::APP_NAME, icons::CARET_DOWN))
            .strong()
            .color(pal.text_strong);
        let button = ui.add(egui::Button::new(text).frame(false));
        self.anchor_response("menu.project", &button, super::APP_NAME);
        let popup_id = egui::Popup::default_response_id(&button);
        if open {
            egui::Popup::open_id(ui.ctx(), popup_id);
        }
        let is_open = egui::Popup::is_id_open(ui.ctx(), popup_id);
        if is_open && !self.wb.app_menu.was_open {
            let menu = &mut self.wb.app_menu;
            menu.filter.clear();
            menu.row = 0;
            menu.in_rows = false;
            menu.fresh = true;
            self.learn_event(novo_learn::Event::Opened("menu.project".to_owned()));
        }
        self.wb.app_menu.was_open = is_open;
        egui::Popup::menu(&button)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| self.app_menu_ui(ui));
        button.on_hover_text(format!("Menu ({})", "F10"));
    }

    /// The Help icon button of the top bar: the app menu at its Help area. With
    /// the menu words in the bar the Help menu is already there.
    pub(super) fn help_button(&mut self, ui: &mut egui::Ui) {
        if self.menu_names_shown() || !self.is_open() {
            return;
        }
        let response = ui.add(egui::Button::new(icons::QUESTION).frame(false));
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Help"));
        self.anchor_response("help.button", &response, "Help");
        if response.on_hover_text("Help and lessons").clicked() {
            self.wb.app_menu.open_at = MENUS.iter().position(|m| *m == "Help");
        }
    }

    /// The lines of the right pane now.
    fn app_menu_entries(&self) -> Vec<Entry> {
        let menu = &self.wb.app_menu;
        if menu.filter.trim().is_empty() {
            area_entries(MENUS[menu.area.min(MENUS.len() - 1)], &menu.expanded)
        } else {
            search_entries(&menu.filter)
        }
    }

    fn app_menu_ui(&mut self, ui: &mut egui::Ui) {
        let pal = crate::theme::ui_palette();
        ui.set_width(WIDTH);
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 2.0);

        // The search field takes what is typed from the first frame.
        let search = ui.add(
            egui::TextEdit::singleline(&mut self.wb.app_menu.filter)
                .hint_text(format!("{} Search all menus", icons::MAGNIFYING_GLASS))
                .desired_width(f32::INFINITY),
        );
        if std::mem::take(&mut self.wb.app_menu.fresh) {
            search.request_focus();
        }
        if search.changed() {
            self.wb.app_menu.row = 0;
            self.wb.app_menu.in_rows = !self.wb.app_menu.filter.trim().is_empty();
        }
        ui.add_space(4.0);

        let searching = !self.wb.app_menu.filter.trim().is_empty();
        let entries = self.app_menu_entries();
        let runnable: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !matches!(e, Entry::Separator))
            .map(|(i, _)| i)
            .collect();
        self.app_menu_keys(ui.ctx(), &entries, &runnable, searching);

        let height = (ui.ctx().content_rect().height() - 190.0).clamp(220.0, 460.0);
        ui.horizontal_top(|ui| {
            // The areas.
            ui.allocate_ui_with_layout(
                egui::vec2(AREAS_WIDTH, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    for (n, menu) in MENUS.iter().enumerate() {
                        let current = !searching && n == self.wb.app_menu.area;
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), ROW),
                            egui::Sense::click(),
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Button,
                                true,
                                current,
                                *menu,
                            )
                        });
                        if response.hovered() || response.clicked() {
                            self.wb.app_menu.area = n;
                            if !self.wb.app_menu.filter.is_empty() && response.clicked() {
                                self.wb.app_menu.filter.clear();
                            }
                        }
                        paint_row(ui, rect, current, response.hovered());
                        let fg = if current {
                            pal.text_strong
                        } else {
                            pal.text_primary
                        };
                        ui.painter().text(
                            egui::pos2(rect.left() + 18.0, rect.center().y),
                            egui::Align2::CENTER_CENTER,
                            area_icon(menu),
                            egui::FontId::proportional(16.0),
                            if current { pal.accent } else { fg },
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 36.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            *menu,
                            egui::TextStyle::Body.resolve(ui.style()),
                            fg,
                        );
                        if !self.menu_names_shown() {
                            self.anchor(&format!("menu.{menu}"), rect, menu);
                        }
                    }
                },
            );
            // A hairline between the panes, as tall as they are (a separator
            // would take the whole height the popup could have).
            let (line, _) = ui.allocate_exact_size(egui::vec2(1.0, height), egui::Sense::hover());
            ui.painter().rect_filled(line, 0, pal.border_subtle);
            // The commands.
            ui.allocate_ui_with_layout(
                egui::vec2(WIDTH - AREAS_WIDTH - 14.0, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if entries.is_empty() {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(
                                        "Nothing in the menus matches. Ctrl+K searches everything.",
                                    )
                                    .color(pal.text_secondary),
                                );
                            }
                            for (n, entry) in entries.iter().enumerate() {
                                let highlighted = self.wb.app_menu.in_rows
                                    && runnable.get(self.wb.app_menu.row) == Some(&n);
                                self.app_menu_entry(ui, *entry, n, highlighted, searching);
                            }
                        });
                },
            );
        });

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(format!("{} About", icons::INFO)).clicked() {
                self.wb.queued.push("help.about");
                ui.close();
            }
            if ui.button(format!("{} Preferences", icons::GEAR)).clicked() {
                self.wb.queued.push("app.preferences");
                ui.close();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("Arrow keys and Enter; Alt+letter picks an area")
                        .small()
                        .color(pal.text_tertiary),
                );
            });
        });
    }

    /// Arrow keys, Enter and Alt+letter while the menu is open.
    fn app_menu_keys(
        &mut self,
        ctx: &egui::Context,
        entries: &[Entry],
        runnable: &[usize],
        searching: bool,
    ) {
        let (down, up, left, right, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                // Left and right belong to the text cursor while there is text.
                !searching && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
                !searching && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            )
        });
        for (n, menu) in MENUS.iter().enumerate() {
            if let Some(key) = mnemonic(menu)
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, key))
            {
                let state = &mut self.wb.app_menu;
                state.area = n;
                state.filter.clear();
                state.row = 0;
                state.in_rows = true;
            }
        }
        let state = &mut self.wb.app_menu;
        let count = runnable.len();
        if state.in_rows {
            if down && count > 0 {
                state.row = (state.row + 1).min(count - 1);
            }
            if up {
                if state.row == 0 {
                    // Back to the field (search) or the areas.
                    state.in_rows = searching;
                } else {
                    state.row -= 1;
                }
            }
            if left {
                state.in_rows = false;
            }
        } else {
            if down && !searching {
                state.area = (state.area + 1).min(MENUS.len() - 1);
            }
            if up && !searching {
                state.area = state.area.saturating_sub(1);
            }
            if (right || (down && searching)) && count > 0 {
                state.in_rows = true;
                state.row = 0;
            }
        }
        if state.row >= count.max(1) {
            state.row = count.saturating_sub(1);
        }
        if enter {
            // Enter in the field runs the first result; Enter on the right pane runs the
            // highlighted row; on the areas it moves to the right pane.
            let at = if state.in_rows || searching {
                runnable
                    .get(if state.in_rows { state.row } else { 0 })
                    .copied()
            } else {
                state.in_rows = count > 0;
                state.row = 0;
                None
            };
            if let Some(entry) = at.and_then(|i| entries.get(i).copied()) {
                match entry {
                    Entry::Command { info, .. } => {
                        if self.command_unavailable(info).is_none() {
                            self.settings.remember_action(info.id);
                            self.wb.queued.push(info.id);
                            egui::Popup::close_all(ctx);
                        }
                    }
                    Entry::Group { name, .. } => {
                        let state = &mut self.wb.app_menu;
                        if !state.expanded.remove(name) {
                            state.expanded.insert(name);
                        }
                    }
                    Entry::Separator => {}
                }
            }
        }
    }

    /// One line of the right pane.
    fn app_menu_entry(
        &mut self,
        ui: &mut egui::Ui,
        entry: Entry,
        _n: usize,
        highlighted: bool,
        searching: bool,
    ) {
        let pal = crate::theme::ui_palette();
        match entry {
            Entry::Separator => {
                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);
            }
            Entry::Group { name, count, open } => {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ROW),
                    egui::Sense::click(),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        format!(
                            "{name}, {count} commands, {}",
                            if open { "open" } else { "folded" }
                        ),
                    )
                });
                paint_row(ui, rect, highlighted, response.hovered());
                let chevron = if open {
                    icons::CARET_DOWN
                } else {
                    icons::CARET_RIGHT
                };
                ui.painter().text(
                    egui::pos2(rect.left() + 18.0, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    chevron,
                    egui::FontId::proportional(14.0),
                    pal.text_secondary,
                );
                ui.painter().text(
                    egui::pos2(rect.left() + 36.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    name,
                    egui::TextStyle::Body.resolve(ui.style()),
                    pal.text_primary,
                );
                ui.painter().text(
                    egui::pos2(rect.right() - 8.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    count.to_string(),
                    egui::TextStyle::Small.resolve(ui.style()),
                    pal.text_tertiary,
                );
                if response.clicked() {
                    let expanded = &mut self.wb.app_menu.expanded;
                    if !expanded.remove(name) {
                        expanded.insert(name);
                    }
                }
            }
            Entry::Command { info, indent } => {
                let reason = self.command_unavailable(info);
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ROW),
                    if reason.is_some() {
                        egui::Sense::hover()
                    } else {
                        egui::Sense::click()
                    },
                );
                let enabled = reason.is_none();
                let accessible = presentation::short_label(info.id).unwrap_or(info.label);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, accessible)
                });
                paint_row(ui, rect, highlighted, enabled && response.hovered());
                let left = rect.left() + if indent { 18.0 } else { 0.0 };
                let color = if enabled {
                    pal.text_primary
                } else {
                    pal.text_tertiary
                };
                presentation::glyph(info.id).paint(
                    ui.painter(),
                    egui::Rect::from_center_size(
                        egui::pos2(left + 18.0, rect.center().y),
                        egui::Vec2::splat(16.0),
                    ),
                    if enabled { pal.text_secondary } else { color },
                );
                let short = presentation::short_label(info.id).unwrap_or(info.label);
                let label = if info.id == "help.next_lesson" {
                    self.command_label(info)
                } else {
                    short.to_owned()
                };
                let galley = ui.painter().text(
                    egui::pos2(left + 36.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &label,
                    egui::TextStyle::Body.resolve(ui.style()),
                    color,
                );
                let mut right = rect.right() - 8.0;
                if let Some(keys) = self.command_shortcut_text(ui.ctx(), info.id) {
                    let galley = ui.painter().text(
                        egui::pos2(right, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        keys,
                        egui::FontId::monospace(egui::TextStyle::Small.resolve(ui.style()).size),
                        pal.text_tertiary,
                    );
                    right = galley.left() - 10.0;
                }
                // The reason a grey row is grey, in words, when it fits.
                if let Some(reason) = reason {
                    let width = ui
                        .painter()
                        .layout_no_wrap(
                            reason.to_owned(),
                            egui::TextStyle::Small.resolve(ui.style()),
                            pal.text_tertiary,
                        )
                        .size()
                        .x;
                    if galley.right() + 16.0 + width < right {
                        ui.painter().text(
                            egui::pos2(right, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            reason,
                            egui::TextStyle::Small.resolve(ui.style()),
                            pal.text_tertiary,
                        );
                        right -= width + 10.0;
                    }
                }
                if searching {
                    let path = path_of(info);
                    if galley.right() + 16.0 < right {
                        ui.painter().text(
                            egui::pos2(right, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            path,
                            egui::TextStyle::Small.resolve(ui.style()),
                            pal.text_secondary,
                        );
                    }
                }
                if let Some(option) = super::view3d_options::ViewOption::from_command(info.id)
                    && self.view3d_option(option)
                {
                    ui.painter().text(
                        egui::pos2(rect.right() - 8.0, rect.top() + 4.0),
                        egui::Align2::RIGHT_TOP,
                        icons::CHECK,
                        egui::FontId::proportional(10.0),
                        pal.accent,
                    );
                }
                self.anchor(&format!("cmd.{}", info.id), rect, info.label);
                // The tooltip carries the full sentence, then the reason it is grey.
                let mut tip = if short == info.label {
                    String::new()
                } else {
                    info.label.to_owned()
                };
                if let Some(hint) = super::menu_bar::hint_for(info.id) {
                    if !tip.is_empty() {
                        tip.push_str("\n\n");
                    }
                    tip.push_str(hint);
                }
                if let Some(reason) = reason {
                    if !tip.is_empty() {
                        tip.push_str("\n\n");
                    }
                    tip.push_str(reason);
                }
                let response = if tip.is_empty() {
                    response
                } else {
                    response.on_hover_text(tip)
                };
                if enabled && response.clicked() {
                    self.settings.remember_action(info.id);
                    self.wb.queued.push(info.id);
                    ui.close();
                }
            }
        }
    }
}

/// The background of a row: the accent wash while highlighted, the hover
/// colour under the pointer.
fn paint_row(ui: &egui::Ui, rect: egui::Rect, highlighted: bool, hovered: bool) {
    let pal = crate::theme::ui_palette();
    let rounding = egui::CornerRadius::same(4);
    if highlighted {
        ui.painter().rect_filled(rect, rounding, pal.accent_wash);
        let bar = egui::Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height()));
        ui.painter().rect_filled(bar, 1, pal.accent);
    } else if hovered {
        ui.painter().rect_filled(rect, rounding, pal.hover);
    }
}
