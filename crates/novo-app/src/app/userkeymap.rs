//! The user's keymap file (novo-workflow `userkeys`, WS-08): read once at
//! start, its mistakes shown as one warning.

use std::path::{Path, PathBuf};

use novo_workflow::keymap::Keymap;

use super::{Level, NovoApp};

/// File name inside the KoperCAD folder under `%APPDATA%`.
pub const KEYMAP_FILE: &str = "keymap.txt";

/// `%APPDATA%\KoperCAD\keymap.txt` (the platform's data folder elsewhere).
pub fn keymap_path() -> Option<PathBuf> {
    Some(
        dirs::data_dir()?
            .join(crate::DATA_DIR_NAME)
            .join(KEYMAP_FILE),
    )
}

/// The default keymap with the file at `path` applied, and one line per
/// skipped entry. A missing or unreadable file means the defaults.
pub fn load(path: &Path) -> (Keymap, Vec<String>) {
    let mut keymap = Keymap::workflow();
    let Ok(text) = std::fs::read_to_string(path) else {
        return (keymap, Vec::new());
    };
    let notes = keymap
        .apply_user_file(&text)
        .into_iter()
        .map(|p| format!("line {}: {}", p.line, p.message))
        .collect();
    (keymap, notes)
}

impl NovoApp {
    /// Applies the user's keymap file, if there is one.
    pub(super) fn load_user_keymap(&mut self) {
        let Some(path) = keymap_path() else {
            return;
        };
        let (keymap, notes) = load(&path);
        self.wb.keymap = keymap;
        if let Some(first) = notes.first() {
            let more = notes.len() - 1;
            let tail = if more > 0 {
                format!(" (and {more} more)")
            } else {
                String::new()
            };
            self.message(
                Level::Warning,
                format!("{KEYMAP_FILE} {first}{tail}; those lines were skipped"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_leaves_the_defaults_and_a_bad_line_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEYMAP_FILE);
        let (map, notes) = load(&path);
        assert!(notes.is_empty());
        assert_eq!(map.bindings(), Keymap::workflow().bindings());

        std::fs::write(&path, "tool.cable = F6\nbogus = F7\n").unwrap();
        let (map, notes) = load(&path);
        assert_eq!(map.shortcut_text("tool.cable").as_deref(), Some("F6"));
        assert_eq!(notes, vec!["line 2: unknown command `bogus`".to_string()]);
    }
}
