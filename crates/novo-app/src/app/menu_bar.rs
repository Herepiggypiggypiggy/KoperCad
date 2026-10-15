//! The menu bar (docs/design/workspaces.md §5). It draws what
//! `novo_workflow::menus` lays out from the command registry; nothing in it
//! is a menu entry of its own except the list of recent projects. Wide
//! windows show the eight menus in the top bar; narrow ones fold them into
//! the mark's menu as submenus.

use eframe::egui;
use novo_workflow::commands::{self, CommandInfo};
use novo_workflow::menus::{self, MENUS, MenuItem};

use super::{Leaving, NovoApp};
use crate::actions::Action;

/// Windows at least this wide (effective points) show the menu names in
/// the top bar; the rest of the bar gives way first (see [`TopBarFit`]).
pub(super) const MENU_BAR_FROM: f32 = 1100.0;

/// How much the rest of the top bar gives up at a window width, so the menu
/// words still fit at 1366 (§5.3): the search button shrinks, the project
/// name is cut, and the switcher falls back to short names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TopBarFit {
    /// The search button's text and minimum width.
    pub(super) search: &'static str,
    pub(super) search_width: f32,
    /// Longest project name shown in the breadcrumb, in characters.
    pub(super) name_chars: usize,
    /// Workspace chips use their short names.
    pub(super) short_switcher: bool,
}

impl TopBarFit {
    pub(super) fn at(width: f32) -> TopBarFit {
        let (search, search_width) = if width >= 1700.0 {
            ("Search or run", 240.0)
        } else if width >= 1250.0 {
            ("Search", 0.0)
        } else {
            ("", 0.0)
        };
        TopBarFit {
            search,
            search_width,
            name_chars: if width >= 1700.0 { 32 } else { 14 },
            short_switcher: width < 1250.0,
        }
    }
}

impl NovoApp {
    /// Whether the menus are words in the top bar this frame.
    pub(super) fn menu_bar_wide(&self) -> bool {
        self.window_width >= MENU_BAR_FROM
    }

    /// F10 moves the focus to the menu bar (the first menu when the bar
    /// is words, else the mark); arrows then walk it and Enter opens one.
    /// Returns whether the key was pressed this frame.
    pub(super) fn menu_key_pressed(&self, ctx: &egui::Context) -> bool {
        ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F10))
    }

    /// The menu names, left to right, in the top bar.
    ///
    /// egui's menu bar takes all the width it is given, which would push
    /// the switcher and search off the bar, so it gets the width its menus
    /// measured last frame (a first guess on the first).
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui, focus: bool) {
        let rect = egui::Rect::from_min_size(
            ui.cursor().min,
            egui::vec2(self.menu_bar_width, ui.available_height()),
        );
        let (mut left, mut right) = (f32::MAX, f32::MIN);
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                for (n, menu) in MENUS.iter().enumerate() {
                    if let Some(button) = self.menu(ui, menu, focus && n == 0) {
                        left = left.min(button.left());
                        right = right.max(button.right());
                    }
                }
            });
        });
        if right > left {
            let measured = right - left + ui.spacing().item_spacing.x;
            if (measured - self.menu_bar_width).abs() > 0.5 {
                self.menu_bar_width = measured;
                ui.ctx().request_repaint();
            }
        }
    }

    /// The mark. Wide, a click shows About (the bar holds every menu);
    /// otherwise it opens the menus as submenus.
    pub(super) fn mark(&mut self, ui: &mut egui::Ui, focus: bool) {
        let text = egui::RichText::new(super::APP_NAME).strong();
        if self.menu_bar_wide() {
            let button = ui.add(egui::Button::new(text).frame(false));
            if focus {
                button.request_focus();
            }
            if button.on_hover_text("About").clicked() {
                self.wb.queued.push("help.about");
            }
            return;
        }
        let response = ui.menu_button(text, |ui| {
            for menu in MENUS {
                let _ = self.menu(ui, menu, false);
            }
        });
        if focus {
            response.response.request_focus();
        }
    }

    /// One menu as a button (in the bar) or a submenu (in the mark's menu).
    /// A menu with no commands is left out.
    fn menu(&mut self, ui: &mut egui::Ui, menu: &'static str, focus: bool) -> Option<egui::Rect> {
        let items = menus::layout(menu);
        if items.is_empty() {
            return None;
        }
        let response = ui.menu_button(menu, |ui| {
            for item in &items {
                match item {
                    MenuItem::Separator => {
                        ui.separator();
                    }
                    MenuItem::Command(info) => {
                        self.command_row(ui, info);
                        if info.id == "file.example" {
                            self.recent_menu(ui);
                        }
                    }
                    MenuItem::Submenu { name, items } => {
                        ui.menu_button(*name, |ui| {
                            for info in items {
                                self.command_row(ui, info);
                            }
                        });
                    }
                }
            }
        });
        if focus {
            response.response.request_focus();
        }
        Some(response.response.rect)
    }

    /// File › Recent: the projects opened lately, newest first.
    fn recent_menu(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(!self.settings.recent_projects.is_empty(), |ui| {
            ui.menu_button("Recent", |ui| {
                for path in self.settings.recent_projects.clone() {
                    if ui.button(path.display().to_string()).clicked() {
                        let ctx = ui.ctx().clone();
                        self.leave_then(&ctx, Leaving::OpenPath(path));
                        ui.close();
                    }
                }
            });
        });
    }

    /// One command: its label, its key on the right, greyed with the
    /// reason as the tooltip when it cannot run now. A click queues it, as
    /// a key press does, and closes the menu.
    fn command_row(&mut self, ui: &mut egui::Ui, info: &'static CommandInfo) {
        let reason = self.command_unavailable(info);
        let mut button = egui::Button::new(info.label);
        if let Some(keys) = self.command_shortcut_text(ui.ctx(), info.id) {
            button = button.shortcut_text(keys);
        }
        let response = ui.add_enabled(reason.is_none(), button);
        let response = match info.id {
            "export.sheets" | "export.sheets_csv" => response.on_hover_text(super::SHEETS_HINT),
            "export.assembly" => response.on_hover_text(super::ASSEMBLY_HINT),
            _ => response,
        };
        if let Some(reason) = reason {
            response.on_disabled_hover_text(reason);
        } else if response.clicked() {
            self.settings.remember_action(info.id);
            self.wb.queued.push(info.id);
            ui.close();
        }
    }

    /// Why a command cannot run now, from its needs and the shell's own
    /// rules for the action behind it.
    fn command_unavailable(&self, info: &CommandInfo) -> Option<&'static str> {
        if let Err(reason) = commands::availability(info, &self.command_context()) {
            return Some(reason);
        }
        if matches!(info.id, "file.remove_background" | "view.background") && !self.has_background()
        {
            return Some("This page has no DXF background");
        }
        match Action::from_id(info.id) {
            Some(action) if !self.is_enabled(action) => Some("Not available now"),
            _ => None,
        }
    }

    /// The key shown beside a command: the keymap's first chord, else the
    /// action's own shortcut.
    pub(super) fn command_shortcut_text(&self, ctx: &egui::Context, id: &str) -> Option<String> {
        self.wb.keymap.shortcut_text(id).or_else(|| {
            Action::from_id(id)
                .and_then(Action::shortcut)
                .map(|s| ctx.format_shortcut(&s))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::gate1_tests::Drive;

    #[test]
    fn a_menu_pick_runs_the_command() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        // What the Cabinet row does when clicked: the id is queued, then run.
        d.app.wb.queued.push("view.cabinet");
        d.frame();
        assert_eq!(d.app.workspace(), crate::app::workspace::Workspace::Cabinet);
    }

    #[test]
    fn unavailable_commands_say_why() {
        let dir = tempfile::tempdir().unwrap();
        let d = Drive::new(&dir.path().join("untitled"));
        let save = commands::get("file.save").unwrap();
        assert_eq!(d.app.command_unavailable(save), Some("No project is open"));
        let new = commands::get("file.new").unwrap();
        assert_eq!(d.app.command_unavailable(new), None);
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;
    use crate::app::gate1_tests::Drive;

    /// One frame of the app in a window `width` points wide.
    fn frame_at(d: &mut Drive, width: f32) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 768.0),
            )),
            ..Default::default()
        };
        let app = &mut d.app;
        let mut out = d.ctx.run_ui(input, |ui| app.show(ui));
        out.textures_delta.clear();
    }

    #[test]
    fn the_top_bar_fits_from_1100_and_shows_words_at_1366() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        for width in [1920.0, 1600.0, 1366.0, 1280.0, 1100.0] {
            // Two frames: the first lays out, the second has the sizes.
            frame_at(&mut d, width);
            frame_at(&mut d, width);
            assert!(d.app.menu_bar_wide(), "menus are words at {width}");
            assert!(
                d.app.top_bar_gap >= 0.0,
                "the top bar overlaps by {} at {width}",
                -d.app.top_bar_gap
            );
        }
    }

    #[test]
    fn below_1100_the_menus_fold_into_the_mark() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        frame_at(&mut d, 1000.0);
        frame_at(&mut d, 1000.0);
        assert!(!d.app.menu_bar_wide());
        assert!(d.app.top_bar_gap >= 0.0);
    }

    #[test]
    fn f10_focuses_the_first_menu() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        frame_at(&mut d, 1366.0);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1366.0, 768.0),
            )),
            events: vec![egui::Event::Key {
                key: egui::Key::F10,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        let app = &mut d.app;
        let _ = d.ctx.run_ui(input, |ui| app.show(ui));
        assert!(d.ctx.memory(|m| m.focused()).is_some(), "a menu has focus");
    }
}
