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
/// the top bar.
pub(super) const MENU_BAR_FROM: f32 = 1500.0;

impl NovoApp {
    /// Whether the menus are words in the top bar this frame.
    pub(super) fn menu_bar_wide(&self) -> bool {
        self.window_width >= MENU_BAR_FROM
    }

    /// The menu names, left to right, in the top bar.
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            for menu in MENUS {
                self.menu(ui, menu);
            }
        });
    }

    /// The mark's menu: About only on a wide window, where the bar holds
    /// the rest; otherwise every menu as a submenu.
    pub(super) fn mark_menu(&mut self, ui: &mut egui::Ui) {
        if self.menu_bar_wide() {
            if let Some(about) = commands::get("help.about") {
                self.command_row(ui, about);
            }
            return;
        }
        for menu in MENUS {
            self.menu(ui, menu);
        }
    }

    /// One menu as a button (in the bar) or a submenu (in the mark's menu).
    /// A menu with no commands is left out.
    fn menu(&mut self, ui: &mut egui::Ui, menu: &'static str) {
        let items = menus::layout(menu);
        if items.is_empty() {
            return;
        }
        ui.menu_button(menu, |ui| {
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
