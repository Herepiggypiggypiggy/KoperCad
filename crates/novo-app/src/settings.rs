//! Preferences remembered between sessions.

use novo_editor::tool::{ConnectMode, WiringMode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MAX_RECENT: usize = 10;
const MAX_RECENT_ACTIONS: usize = 20;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Whether the outline takes space beside the canvas. `None` pins it
    /// on wide windows only (docs/DESIGN.md §3).
    pub outline_pinned: Option<bool>,
    /// The project open when the app last ran, reopened on start.
    pub last_open: Option<PathBuf>,
    /// Most recently used project files, newest first.
    pub recent_projects: Vec<PathBuf>,
    /// Ids of recently run actions, newest first; the palette lists them first.
    pub recent_actions: Vec<String>,
    /// How the wire tool takes clicks, `"corners"` or `"lines"`: the
    /// person's preference, not the project's (docs/design/wiring-modes.md).
    pub wiring_mode: String,
    /// What a drop does about facing free pins, `"draw"`, `"ask"` or
    /// `"never"` (docs/DESIGN.md §2): a per-user preference.
    pub connect_mode: String,
    /// The initials a revision is issued under (docs/DESIGN.md §7),
    /// asked once and remembered.
    pub initials: String,
    /// Whether moving a symbol takes its wires along by default (Shift
    /// does the other one): a per-user preference.
    pub move_carries: bool,
}

impl Settings {
    pub const STORAGE_KEY: &str = "novo_settings";

    /// The wiring mode; Corners unless the person chose Lines.
    pub fn wiring_mode(&self) -> WiringMode {
        match self.wiring_mode.as_str() {
            "lines" => WiringMode::Lines,
            _ => WiringMode::Corners,
        }
    }

    pub fn set_wiring_mode(&mut self, mode: WiringMode) {
        self.wiring_mode = match mode {
            WiringMode::Corners => "corners",
            WiringMode::Lines => "lines",
        }
        .to_owned();
    }

    /// The auto-connection mode; Draw unless the person chose otherwise.
    pub fn connect_mode(&self) -> ConnectMode {
        match self.connect_mode.as_str() {
            "ask" => ConnectMode::Ask,
            "never" => ConnectMode::Never,
            _ => ConnectMode::Draw,
        }
    }

    pub fn set_connect_mode(&mut self, mode: ConnectMode) {
        self.connect_mode = match mode {
            ConnectMode::Draw => "draw",
            ConnectMode::Ask => "ask",
            ConnectMode::Never => "never",
        }
        .to_owned();
    }

    pub fn remember_recent(&mut self, path: &Path) {
        self.recent_projects.retain(|p| p != path);
        self.recent_projects.insert(0, path.to_owned());
        self.recent_projects.truncate(MAX_RECENT);
    }

    pub fn remember_action(&mut self, id: &str) {
        self.recent_actions.retain(|a| a != id);
        self.recent_actions.insert(0, id.to_owned());
        self.recent_actions.truncate(MAX_RECENT_ACTIONS);
    }

    pub fn forget_recent(&mut self, path: &Path) {
        self.recent_projects.retain(|p| p != path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_is_newest_first_without_duplicates() {
        let mut s = Settings::default();
        s.remember_recent(Path::new("a.novo"));
        s.remember_recent(Path::new("b.novo"));
        s.remember_recent(Path::new("a.novo"));
        assert_eq!(
            s.recent_projects,
            vec![PathBuf::from("a.novo"), PathBuf::from("b.novo")]
        );
    }

    #[test]
    fn settings_from_older_versions_still_load() {
        // Older settings had a theme choice and panel toggles; both are gone.
        // Panel toggles went with the Workbench (docs/DESIGN.md §3).
        let old = r#"(theme: Light, show_messages_panel: true, recent_actions: ["view.fit"])"#;
        let s: Settings = ron::from_str(old).unwrap();
        assert_eq!(s.recent_actions, vec!["view.fit"]);
        assert_eq!(s.outline_pinned, None);
    }

    #[test]
    fn recent_actions_move_to_the_front() {
        let mut s = Settings::default();
        s.remember_action("file.save");
        s.remember_action("view.fit");
        s.remember_action("file.save");
        assert_eq!(s.recent_actions, vec!["file.save", "view.fit"]);
    }

    #[test]
    fn recent_is_capped() {
        let mut s = Settings::default();
        for i in 0..20 {
            s.remember_recent(Path::new(&format!("{i}.novo")));
        }
        assert_eq!(s.recent_projects.len(), MAX_RECENT);
        assert_eq!(s.recent_projects[0], PathBuf::from("19.novo"));
    }
}
