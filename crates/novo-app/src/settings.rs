//! Preferences remembered between sessions.

use novo_editor::tool::{ConnectMode, WiringMode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// Whether the live view skips its animations: states switch at once
    /// and a running motor shows an arrow instead of spinning.
    pub reduce_motion: bool,
    /// Commissioning's large touch buttons: a tablet user sets it once.
    pub large_buttons: bool,
    /// The workspace shown when the app last ran (`Workspace::id`),
    /// reopened with the project when it is available again
    /// (docs/design/workspaces.md §4.1).
    pub last_workspace: String,
    /// Each workspace's panels as the person left them, by `Workspace::id`.
    /// A workspace with no entry has its default layout.
    pub workspaces: BTreeMap<String, WorkspaceLayout>,
    /// How far the fast-keys walk-through is (docs/ONBOARDING-DESIGN.md §2).
    pub onboarding: Onboarding,
    /// The start of the name a new page group gets (`G` makes `=G1`) in a
    /// project with no group to continue. Empty: the default.
    pub group_prefix: String,
    /// The same for a new cabinet (`A` makes `+A1`). Empty: the default.
    pub cabinet_prefix: String,
}

/// The walk-through's progress, per user: kept so closing the app mid-way
/// resumes, and so a skipped walk-through never comes back by itself.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Onboarding {
    /// Step ids the person has done, in the order they did them.
    pub ticked: Vec<String>,
    /// Skipped, or all steps done and closed: never shown by itself again.
    pub dismissed: bool,
    /// The walk-through has been opened at least once, so the start panel
    /// stops pointing at it.
    pub started: bool,
}

impl Onboarding {
    pub fn is_ticked(&self, step: &str) -> bool {
        self.ticked.iter().any(|t| t == step)
    }

    /// Ticks a step; true when it was not ticked before.
    pub fn tick(&mut self, step: &str) -> bool {
        if self.is_ticked(step) {
            return false;
        }
        self.ticked.push(step.to_owned());
        true
    }

    /// Starts over, shown again (the command's reset).
    pub fn restart(&mut self) {
        self.ticked.clear();
        self.dismissed = false;
        self.started = true;
    }
}

/// What one workspace remembers about its panels: a snapshot taken when the
/// person leaves it and put back when they return. Per user, not per
/// project, because it describes their screen. Strings keep an old settings
/// file readable when a panel is renamed or removed; an unknown one means
/// the default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceLayout {
    /// Whether the outline takes space beside the canvas; `None` follows
    /// the window width.
    pub outline_pinned: Option<bool>,
    /// `"pages"`, `"issues"` or `"changes"`.
    pub outline_mode: String,
    /// `"terminals"`, `"cables"` or `"devices"`; `None` is closed.
    pub drawer: Option<String>,
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
    fn workspace_layouts_round_trip() {
        let mut s = Settings {
            last_workspace: "cabinet".into(),
            ..Settings::default()
        };
        s.workspaces.insert(
            "schematic".into(),
            WorkspaceLayout {
                outline_pinned: Some(true),
                outline_mode: "issues".into(),
                drawer: Some("cables".into()),
            },
        );
        let text = ron::to_string(&s).unwrap();
        let back: Settings = ron::from_str(&text).unwrap();
        assert_eq!(back, s);
        // Settings written before workspaces existed still load.
        let old: Settings = ron::from_str("(recent_actions: [\"view.fit\"])").unwrap();
        assert!(old.workspaces.is_empty());
        assert_eq!(old.last_workspace, "");
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
