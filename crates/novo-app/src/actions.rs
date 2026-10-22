//! Every user-facing action. The project menu, keyboard shortcuts and the
//! command palette all dispatch the same [`Action`], so each one is reachable
//! from the keyboard.

use eframe::egui::{Key, KeyboardShortcut, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    NewProject,
    OpenProject,
    OpenReadOnly,
    RecoverProject,
    OpenExample,
    TryFastKeys,
    ShowWalkthrough,
    Learn,
    SaveProject,
    SaveProjectAs,
    CloseProject,
    Quit,
    Undo,
    Redo,
    ZoomToFit,
    ShowSchematic,
    ShowCabinet,
    SizeEnclosure,
    EnclosureList,
    StandardsCheck,
    EmcCheck,
    Protection,
    MotorStart,
    PinOutline,
    FocusMode,
    FullScreen,
    CommandPalette,
    About,
}

impl Action {
    pub const ALL: &[Action] = &[
        Action::NewProject,
        Action::OpenProject,
        Action::OpenReadOnly,
        Action::RecoverProject,
        Action::OpenExample,
        Action::TryFastKeys,
        Action::ShowWalkthrough,
        Action::Learn,
        Action::SaveProject,
        Action::SaveProjectAs,
        Action::CloseProject,
        Action::Quit,
        Action::Undo,
        Action::Redo,
        Action::ZoomToFit,
        Action::ShowSchematic,
        Action::ShowCabinet,
        Action::SizeEnclosure,
        Action::EnclosureList,
        Action::StandardsCheck,
        Action::EmcCheck,
        Action::Protection,
        Action::MotorStart,
        Action::PinOutline,
        Action::FocusMode,
        Action::FullScreen,
        Action::CommandPalette,
        Action::About,
    ];

    /// Stable identifier, used to remember recent actions between sessions
    /// and, later, by scripts and keymaps.
    pub fn id(self) -> &'static str {
        match self {
            Action::NewProject => "file.new",
            Action::OpenProject => "file.open",
            Action::OpenReadOnly => "file.open_read_only",
            Action::RecoverProject => "file.recover",
            Action::OpenExample => "file.example",
            Action::TryFastKeys => "file.practice",
            Action::ShowWalkthrough => "help.walkthrough",
            Action::Learn => "help.learn",
            Action::SaveProject => "file.save",
            Action::SaveProjectAs => "file.save_as",
            Action::CloseProject => "file.close",
            Action::Quit => "file.quit",
            Action::Undo => "edit.undo",
            Action::Redo => "edit.redo",
            Action::ZoomToFit => "view.fit",
            Action::ShowSchematic => "view.schematic",
            Action::ShowCabinet => "view.cabinet",
            Action::SizeEnclosure => "cabinet.size-enclosure",
            Action::EnclosureList => "cabinet.enclosures",
            Action::StandardsCheck => "check.standards",
            Action::EmcCheck => "check.emc",
            Action::Protection => "view.protection",
            Action::MotorStart => "simulate.motor-start",
            Action::PinOutline => "view.pin_outline",
            Action::FocusMode => "view.focus",
            Action::FullScreen => "view.full_screen",
            Action::CommandPalette => "view.command_palette",
            Action::About => "help.about",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.iter().copied().find(|a| a.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::NewProject => "New project",
            Action::OpenProject => "Open project…",
            Action::OpenReadOnly => "Open read-only…",
            Action::RecoverProject => "Recover a damaged project…",
            Action::OpenExample => "Open example",
            Action::TryFastKeys => "Try the fast keys",
            Action::ShowWalkthrough => "Fast keys card on this project",
            Action::Learn => "Learn…",
            Action::SaveProject => "Save",
            Action::SaveProjectAs => "Save as…",
            Action::CloseProject => "Close project",
            Action::Quit => "Quit",
            Action::Undo => "Undo",
            Action::Redo => "Redo",
            Action::ZoomToFit => "Zoom to fit",
            Action::ShowSchematic => "Schematic",
            Action::ShowCabinet => "Cabinet",
            Action::SizeEnclosure => "Size the enclosure…",
            Action::EnclosureList => "Enclosures…",
            Action::StandardsCheck => "Standards check…",
            Action::EmcCheck => "EMC check…",
            Action::Protection => "Protection: curves, selectivity, rating…",
            Action::MotorStart => "Simulate motor start…",
            Action::PinOutline => "Pin outline",
            Action::FocusMode => "Focus mode",
            Action::FullScreen => "Full screen",
            Action::CommandPalette => "Command palette…",
            Action::About => "About KoperCAD",
        }
    }

    /// Group the action belongs to (docs/DESIGN.md §4); the palette shows it
    /// as a prefix.
    pub fn category(self) -> &'static str {
        match self {
            Action::NewProject
            | Action::OpenProject
            | Action::OpenReadOnly
            | Action::RecoverProject
            | Action::OpenExample
            | Action::TryFastKeys
            | Action::SaveProject
            | Action::SaveProjectAs
            | Action::CloseProject
            | Action::Quit
            | Action::About => "Project",
            Action::Undo | Action::Redo => "Edit",
            Action::ZoomToFit
            | Action::ShowSchematic
            | Action::ShowCabinet
            | Action::SizeEnclosure
            | Action::EnclosureList
            | Action::StandardsCheck
            | Action::EmcCheck
            | Action::Protection
            | Action::MotorStart
            | Action::PinOutline
            | Action::FocusMode
            | Action::FullScreen
            | Action::ShowWalkthrough
            | Action::Learn
            | Action::CommandPalette => "View",
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
            Action::ShowSchematic => (cmd, Key::Num1),
            Action::ShowCabinet => (cmd, Key::Num2),
            Action::FocusMode => (cmd_shift, Key::F),
            Action::FullScreen => (Modifiers::NONE, Key::F11),
            Action::CommandPalette => (cmd, Key::K),
            Action::PinOutline
            | Action::About
            | Action::OpenReadOnly
            | Action::RecoverProject
            | Action::OpenExample
            | Action::TryFastKeys
            | Action::ShowWalkthrough
            | Action::Learn
            | Action::SizeEnclosure
            | Action::StandardsCheck
            | Action::EmcCheck
            | Action::EnclosureList
            | Action::Protection
            | Action::MotorStart => return None,
        };
        Some(KeyboardShortcut::new(modifiers, key))
    }

    /// Every shortcut for the action; the first is the one shown.
    pub fn shortcuts(self) -> Vec<KeyboardShortcut> {
        let mut all: Vec<_> = self.shortcut().into_iter().collect();
        if self == Action::Redo {
            // Windows convention, next to Ctrl+Shift+Z.
            all.push(KeyboardShortcut::new(Modifiers::COMMAND, Key::Y));
        }
        all
    }

    /// True for shortcuts a focused text field needs for itself: plain keys
    /// such as Home, and Ctrl+Z / Ctrl+Y for the field's own undo.
    pub fn yields_to_text_input(self, shortcut: &KeyboardShortcut) -> bool {
        shortcut.modifiers.is_none() || matches!(self, Action::Undo | Action::Redo)
    }

    /// Actions with shortcuts, most modifiers first.
    ///
    /// egui matches shortcuts loosely (Ctrl+Shift+S also matches Ctrl+S), so
    /// the more specific shortcut has to be tried first.
    pub fn by_shortcut_specificity() -> Vec<(Action, KeyboardShortcut)> {
        let mut list: Vec<_> = Action::ALL
            .iter()
            .flat_map(|&a| a.shortcuts().into_iter().map(move |s| (a, s)))
            .collect();
        list.sort_by_key(|(_, s)| std::cmp::Reverse(modifier_count(s.modifiers)));
        list
    }
}

/// Shortcuts held for upcoming features (see docs/WORKFLOW.md). No action
/// may take one of these until its feature claims it.
pub const RESERVED_SHORTCUTS: &[(Modifiers, Key, &str)] = &[
    (Modifiers::NONE, Key::I, "canvas tool: insert symbol"),
    (
        Modifiers::NONE,
        Key::Q,
        "canvas tool: place (hold for the wheel)",
    ),
    (Modifiers::NONE, Key::W, "canvas tool: wire"),
    (Modifiers::NONE, Key::T, "canvas tool: terminal"),
    (Modifiers::NONE, Key::C, "canvas tool: cable"),
    (Modifiers::NONE, Key::P, "canvas tool: potential"),
    (Modifiers::NONE, Key::M, "canvas tool: move"),
    (Modifiers::NONE, Key::R, "canvas tool: rotate"),
    (Modifiers::NONE, Key::X, "canvas tool: mirror"),
    (Modifiers::NONE, Key::G, "canvas tool: go to"),
    (Modifiers::NONE, Key::F2, "edit tag in place"),
    (Modifiers::NONE, Key::PageUp, "previous page"),
    (Modifiers::NONE, Key::PageDown, "next page"),
    (Modifiers::ALT, Key::ArrowLeft, "jump back"),
    (Modifiers::ALT, Key::ArrowRight, "jump forward"),
    (Modifiers::COMMAND, Key::D, "smart duplicate"),
    (Modifiers::COMMAND, Key::Num3, "drawer: terminals"),
    (Modifiers::COMMAND, Key::Num4, "drawer: cables"),
    (Modifiers::COMMAND, Key::Num5, "drawer: devices"),
];

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

    #[test]
    fn reserved_shortcuts_stay_free() {
        for (action, s) in Action::by_shortcut_specificity() {
            let taken = RESERVED_SHORTCUTS
                .iter()
                .find(|(m, k, _)| *m == s.modifiers && *k == s.logical_key);
            assert!(
                taken.is_none(),
                "{action:?} takes a reserved shortcut: {taken:?}"
            );
        }
    }

    #[test]
    fn plain_key_shortcuts_yield_to_text_fields() {
        for (action, s) in Action::by_shortcut_specificity() {
            if s.modifiers.is_none() {
                assert!(action.yields_to_text_input(&s), "{action:?}");
            }
        }
    }

    #[test]
    fn ids_round_trip() {
        for &a in Action::ALL {
            assert_eq!(Action::from_id(a.id()), Some(a));
        }
    }

    #[test]
    fn every_action_is_a_command_in_the_registry() {
        // One list of commands (docs/design/workspaces.md §5.1): the menus
        // and the command field read novo-workflow's registry, so an
        // action missing there would have no menu row and no palette row.
        for &a in Action::ALL {
            assert!(
                novo_workflow::commands::is_known(a.id()),
                "{a:?} ({}) is not in the command registry",
                a.id()
            );
        }
    }

    #[test]
    fn redo_also_answers_to_ctrl_y() {
        let keys: Vec<_> = Action::Redo
            .shortcuts()
            .iter()
            .map(|s| s.logical_key)
            .collect();
        assert_eq!(keys, vec![Key::Z, Key::Y]);
    }
}
