//! Every user-facing action. Menus, keyboard shortcuts and the command palette
//! all dispatch the same [`Action`], so each one is reachable from the keyboard.

use eframe::egui::{Key, KeyboardShortcut, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    NewProject,
    OpenProject,
    SaveProject,
    SaveProjectAs,
    CloseProject,
    Quit,
    Undo,
    Redo,
    ZoomToFit,
    ToggleProjectPanel,
    TogglePropertiesPanel,
    ToggleMessagesPanel,
    CommandPalette,
    ThemeSystem,
    ThemeLight,
    ThemeDark,
    About,
}

impl Action {
    pub const ALL: &[Action] = &[
        Action::NewProject,
        Action::OpenProject,
        Action::SaveProject,
        Action::SaveProjectAs,
        Action::CloseProject,
        Action::Quit,
        Action::Undo,
        Action::Redo,
        Action::ZoomToFit,
        Action::ToggleProjectPanel,
        Action::TogglePropertiesPanel,
        Action::ToggleMessagesPanel,
        Action::CommandPalette,
        Action::ThemeSystem,
        Action::ThemeLight,
        Action::ThemeDark,
        Action::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::NewProject => "New project",
            Action::OpenProject => "Open project…",
            Action::SaveProject => "Save",
            Action::SaveProjectAs => "Save as…",
            Action::CloseProject => "Close project",
            Action::Quit => "Quit",
            Action::Undo => "Undo",
            Action::Redo => "Redo",
            Action::ZoomToFit => "Zoom to fit",
            Action::ToggleProjectPanel => "Project panel",
            Action::TogglePropertiesPanel => "Properties panel",
            Action::ToggleMessagesPanel => "Messages panel",
            Action::CommandPalette => "Command palette…",
            Action::ThemeSystem => "Theme: follow system",
            Action::ThemeLight => "Theme: light",
            Action::ThemeDark => "Theme: dark",
            Action::About => "About NovoElectric",
        }
    }

    /// Menu the action belongs to; the palette shows it as a prefix.
    pub fn category(self) -> &'static str {
        match self {
            Action::NewProject
            | Action::OpenProject
            | Action::SaveProject
            | Action::SaveProjectAs
            | Action::CloseProject
            | Action::Quit => "File",
            Action::Undo | Action::Redo => "Edit",
            Action::ZoomToFit
            | Action::ToggleProjectPanel
            | Action::TogglePropertiesPanel
            | Action::ToggleMessagesPanel
            | Action::CommandPalette
            | Action::ThemeSystem
            | Action::ThemeLight
            | Action::ThemeDark => "View",
            Action::About => "Help",
        }
    }

    pub fn shortcut(self) -> Option<KeyboardShortcut> {
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let (modifiers, key) = match self {
            Action::NewProject => (cmd, Key::N),
            Action::OpenProject => (cmd, Key::O),
            Action::SaveProject => (cmd, Key::S),
            Action::SaveProjectAs => (cmd_shift, Key::S),
            Action::CloseProject => (cmd, Key::W),
            Action::Quit => (cmd, Key::Q),
            Action::Undo => (cmd, Key::Z),
            Action::Redo => (cmd_shift, Key::Z),
            Action::ZoomToFit => (Modifiers::NONE, Key::Home),
            Action::ToggleProjectPanel => (cmd, Key::Num1),
            Action::TogglePropertiesPanel => (cmd, Key::Num2),
            Action::ToggleMessagesPanel => (cmd, Key::Num3),
            Action::CommandPalette => (cmd, Key::K),
            Action::ThemeSystem | Action::ThemeLight | Action::ThemeDark | Action::About => {
                return None;
            }
        };
        Some(KeyboardShortcut::new(modifiers, key))
    }

    /// Actions with shortcuts, most modifiers first.
    ///
    /// egui matches shortcuts loosely (Ctrl+Shift+S also matches Ctrl+S), so
    /// the more specific shortcut has to be tried first.
    pub fn by_shortcut_specificity() -> Vec<(Action, KeyboardShortcut)> {
        let mut list: Vec<_> = Action::ALL
            .iter()
            .filter_map(|&a| a.shortcut().map(|s| (a, s)))
            .collect();
        list.sort_by_key(|(_, s)| std::cmp::Reverse(modifier_count(s.modifiers)));
        list
    }
}

fn modifier_count(m: Modifiers) -> u8 {
    u8::from(m.alt) + u8::from(m.shift) + u8::from(m.ctrl || m.command || m.mac_cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_lists_every_action_once() {
        let unique: HashSet<_> = Action::ALL.iter().collect();
        assert_eq!(unique.len(), Action::ALL.len());
    }

    #[test]
    fn shortcuts_are_unique() {
        let mut seen = HashSet::new();
        for (action, s) in Action::by_shortcut_specificity() {
            assert!(
                seen.insert((s.modifiers, s.logical_key)),
                "{action:?} reuses a shortcut"
            );
        }
    }

    #[test]
    fn shift_variants_are_tried_first() {
        let order = Action::by_shortcut_specificity();
        let pos = |a| order.iter().position(|(x, _)| *x == a).unwrap();
        assert!(pos(Action::SaveProjectAs) < pos(Action::SaveProject));
        assert!(pos(Action::Redo) < pos(Action::Undo));
    }
}
