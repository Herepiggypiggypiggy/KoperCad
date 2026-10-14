//! Preferences remembered between sessions.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MAX_RECENT: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn preference(self) -> eframe::egui::ThemePreference {
        match self {
            Theme::System => eframe::egui::ThemePreference::System,
            Theme::Light => eframe::egui::ThemePreference::Light,
            Theme::Dark => eframe::egui::ThemePreference::Dark,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub show_project_panel: bool,
    pub show_properties_panel: bool,
    pub show_messages_panel: bool,
    /// Most recently used project files, newest first.
    pub recent_projects: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            show_project_panel: true,
            show_properties_panel: true,
            show_messages_panel: false,
            recent_projects: Vec::new(),
        }
    }
}

impl Settings {
    pub const STORAGE_KEY: &str = "novo_settings";

    pub fn remember_recent(&mut self, path: &Path) {
        self.recent_projects.retain(|p| p != path);
        self.recent_projects.insert(0, path.to_owned());
        self.recent_projects.truncate(MAX_RECENT);
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
    fn recent_is_capped() {
        let mut s = Settings::default();
        for i in 0..20 {
            s.remember_recent(Path::new(&format!("{i}.novo")));
        }
        assert_eq!(s.recent_projects.len(), MAX_RECENT);
        assert_eq!(s.recent_projects[0], PathBuf::from("19.novo"));
    }
}
