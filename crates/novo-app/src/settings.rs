//! Preferences remembered between sessions.

use novo_editor::tool::{ConnectMode, WiringMode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Times a hint is followed before it stops showing.
pub const HINT_LIMIT: u32 = 3;
const MAX_RECENT: usize = 10;
const MAX_RECENT_ACTIONS: usize = 20;

/// Whether the tool dock shows a word beside each icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockLabels {
    /// When the canvas is wide enough (the default).
    Auto,
    Always,
    Never,
}

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
    /// Whether moving a symbol leaves its wires behind as open ends by
    /// default (Shift does the other one): a per-user preference. Off, the
    /// default, a moved symbol takes its wires along. (An earlier build
    /// stored the opposite choice as `move_carries`, defaulting to leaving
    /// them; that key is ignored so nobody keeps a default they never chose.)
    pub move_leaves_wires: bool,
    /// Whether the live view skips its animations: states switch at once
    /// and a running motor shows an arrow instead of spinning.
    pub reduce_motion: bool,
    /// How tightly the contact table under a coil is drawn, `"full"` or
    /// `"compact"` (empty: full): a per-user preference.
    pub xref_density: String,
    /// The colour theme, `"dark"` (empty), `"light"` or `"system"`.
    pub appearance_theme: String,
    /// The interface density, `"standard"` (empty), `"compact"` or `"touch"`.
    pub appearance_density: String,
    /// Words beside the dock icons: `""` (auto: when the canvas has room),
    /// `"always"` or `"never"` (icons only, names stay in the tooltips).
    pub dock_labels: String,
    /// The interface size in percent: 100 (empty or 0), 125, 150 or 200,
    /// on top of the system's own scaling.
    pub ui_scale: u32,
    /// Whether the status bar is switched off (docs/design/workspaces.md §7).
    pub hide_status_bar: bool,
    /// Commissioning's large touch buttons: a tablet user sets it once.
    pub large_buttons: bool,
    /// The workspace shown when the app last ran (`Workspace::id`),
    /// reopened with the project when it is available again
    /// (docs/design/workspaces.md §4.1).
    pub last_workspace: String,
    /// Each workspace's panels as the person left them, by `Workspace::id`.
    /// A workspace with no entry has its default layout.
    ///
    /// Keys are `"<slot>/<size class>"` (`layout_presets`): a layout is kept
    /// per preset and per window size. A plain workspace id is the layout
    /// written before sizes existed and is the fallback.
    pub workspaces: BTreeMap<String, WorkspaceLayout>,
    /// The layout preset last on screen (`layout_presets::Slot::id`).
    pub last_slot: String,
    /// How far the fast-keys walk-through is (docs/ONBOARDING-DESIGN.md §2).
    pub onboarding: Onboarding,
    /// The DWG converter program the person chose (`dwg2dxf` or the ODA File
    /// Converter), tried before the ones KoperCAD finds itself.
    pub dwg_converter: Option<PathBuf>,
    /// A shared macro folder (a company library on a network drive): the
    /// macro browser lists it beside the person's own macros and offers
    /// newer revisions of macros a project was built from.
    pub company_macros: Option<PathBuf>,
    /// The start of the name a new page group gets (`G` makes `=G1`) in a
    /// project with no group to continue. Empty: the default.
    pub group_prefix: String,
    /// The same for a new cabinet (`A` makes `+A1`). Empty: the default.
    pub cabinet_prefix: String,
    /// How often each one-time hint was followed, by hint id
    /// (`app/tips.rs`, docs/design/discoverability.md layer 4). A hint
    /// stops showing once followed [`HINT_LIMIT`] times; Help > Show hints
    /// again clears the counters.
    pub hints_seen: BTreeMap<String, u32>,
    /// The id of the newest What's new entry the person has opened the
    /// list over (`assets/whats-new.md`); empty on a first run.
    pub whats_new_seen: String,
    /// Camera positions saved in the 3D view, per user, not in the project.
    pub camera_views: Vec<CameraBookmark>,
    /// A swipe in wheel lines pans the 3D view instead of zooming (Windows
    /// touchpads); the 3D toolbar's Layers > Trackpad pans.
    pub trackpad_pans: bool,
    /// What the person last chose in File > Export drawings > Export PDF with options...
    /// (`app/pdf_options.rs`): a per-user preference, kept in the settings
    /// file under the user's profile.
    pub pdf_choice: PdfChoice,
    /// Seconds between the writes that keep the open project and its
    /// recovery file current. 0: the default (`DEFAULT_AUTOSAVE_SECONDS`).
    pub autosave_seconds: u32,
}

/// The autosave interval when the person has not chosen one.
pub const DEFAULT_AUTOSAVE_SECONDS: u32 = 30;
/// The intervals Preferences offers, in seconds.
pub const AUTOSAVE_CHOICES: [u32; 5] = [10, 30, 60, 120, 300];

/// A saved 3D camera: where it looks from, in the view's own units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraBookmark {
    pub name: String,
    pub target: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
    pub orthographic: bool,
}

/// The choices of the PDF options dialog. [`PdfChoice::default`] is what the
/// one-click Export PDF commands always write, whatever was chosen here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PdfChoice {
    /// `"all"`, `"current"` or `"ranges"`.
    pub scope: String,
    /// Page numbers for `scope == "ranges"`: `1-3, 5, 7-` (1 is the first
    /// page in the project's order).
    pub ranges: String,
    /// One bookmark per page.
    pub bookmarks: bool,
    /// Clickable links between a device's parts and interruption points.
    pub links: bool,
    /// Cross reference text next to symbols.
    pub xref_notes: bool,
    /// The title block's revision row.
    pub title_revisions: bool,
    /// A dot on the bookmark of pages changed since the latest revision.
    pub changed_marks: bool,
    /// The design checks as report pages after the drawing.
    pub checks_appendix: bool,
    /// `"black"` (print style) or `"colour"`.
    pub look: String,
    /// The document title; empty uses the project name.
    pub title: String,
}

impl Default for PdfChoice {
    fn default() -> Self {
        Self {
            scope: "all".into(),
            ranges: String::new(),
            bookmarks: true,
            links: true,
            xref_notes: true,
            title_revisions: true,
            changed_marks: true,
            checks_appendix: true,
            look: "black".into(),
            title: String::new(),
        }
    }
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
    /// Whether the inspector stays docked with nothing selected; `None`
    /// leaves it as it is.
    pub inspector_pinned: Option<bool>,
    /// The floating panels the person moved in this layout, by panel id,
    /// with their top left corner in points. A panel not named sits at its
    /// own side of the canvas.
    pub panels: BTreeMap<String, [i32; 2]>,
}

impl Settings {
    /// The theme the person chose; dark by default.
    pub fn theme_choice(&self) -> novo_theme::ThemeChoice {
        novo_theme::ThemeChoice::from_id(&self.appearance_theme)
    }

    /// The interface scale as a factor: 1.0, 1.25, 1.5 or 2.0.
    pub fn ui_scale_factor(&self) -> f32 {
        match self.ui_scale {
            125 => 1.25,
            150 => 1.5,
            200 => 2.0,
            _ => 1.0,
        }
    }

    /// The density the person chose; standard by default.
    pub fn density_choice(&self) -> novo_theme::Density {
        novo_theme::Density::from_id(&self.appearance_density)
    }

    /// The density of the coil contact table.
    pub fn xref_density(&self) -> novo_canvas::model::NoteDensity {
        novo_canvas::model::NoteDensity::from_id(&self.xref_density)
    }

    /// The autosave interval: a chosen one inside the offered range, else
    /// the default.
    pub fn autosave_interval(&self) -> std::time::Duration {
        let s = match self.autosave_seconds {
            0 => DEFAULT_AUTOSAVE_SECONDS,
            s => s.clamp(
                AUTOSAVE_CHOICES[0],
                AUTOSAVE_CHOICES[AUTOSAVE_CHOICES.len() - 1],
            ),
        };
        std::time::Duration::from_secs(u64::from(s))
    }

    /// The dock-label choice; auto unless the person chose otherwise.
    pub fn dock_labels(&self) -> DockLabels {
        match self.dock_labels.as_str() {
            "always" => DockLabels::Always,
            "never" => DockLabels::Never,
            _ => DockLabels::Auto,
        }
    }

    pub fn set_dock_labels(&mut self, labels: DockLabels) {
        self.dock_labels = match labels {
            DockLabels::Auto => "",
            DockLabels::Always => "always",
            DockLabels::Never => "never",
        }
        .to_owned();
    }

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
    fn the_autosave_interval_defaults_and_stays_in_the_offered_range() {
        use std::time::Duration;
        let mut s = Settings::default();
        assert_eq!(s.autosave_interval(), Duration::from_secs(30));
        s.autosave_seconds = 60;
        assert_eq!(s.autosave_interval(), Duration::from_secs(60));
        s.autosave_seconds = 1;
        assert_eq!(s.autosave_interval(), Duration::from_secs(10));
        s.autosave_seconds = 86_400;
        assert_eq!(s.autosave_interval(), Duration::from_secs(300));
        let old: Settings = ron::from_str("(recent_actions: [\"view.fit\"])").unwrap();
        assert_eq!(old.autosave_seconds, 0);
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
    fn the_trackpad_choice_is_off_in_old_settings_and_survives_a_round_trip() {
        let old: Settings = ron::from_str("(recent_actions: [\"view.fit\"])").unwrap();
        assert!(!old.trackpad_pans);
        let on = Settings {
            trackpad_pans: true,
            ..Settings::default()
        };
        let back: Settings = ron::from_str(&ron::to_string(&on).unwrap()).unwrap();
        assert!(back.trackpad_pans);
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
                inspector_pinned: Some(true),
                panels: BTreeMap::from([("inspector".to_owned(), [900, 80])]),
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
