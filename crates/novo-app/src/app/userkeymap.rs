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
        novo_storage::profile::data_dir()?
            .join(crate::DATA_DIR_NAME)
            .join(KEYMAP_FILE),
    )
}

/// The default keymap with the file at `path` applied, and one line per
/// skipped entry. A missing or unreadable file means the defaults.
pub fn load(path: &Path) -> (Keymap, Vec<String>) {
    let mut keymap = Keymap::workflow();
    let Ok(bytes) = std::fs::read(path) else {
        return (keymap, Vec::new());
    };
    let text = decode(&bytes);
    let notes = keymap
        .apply_user_file(&text)
        .into_iter()
        .map(|p| format!("line {}: {}", p.line, p.message))
        .collect();
    (keymap, notes)
}

/// The file's text: UTF-8 (a byte order mark is dropped), or UTF-16 with its
/// mark, which Notepad's "Unicode" save writes.
fn decode(bytes: &[u8]) -> String {
    let utf16 = |rest: &[u8], le: bool| {
        let (pairs, _) = rest.as_chunks::<2>();
        let units: Vec<u16> = pairs
            .iter()
            .map(|&pair| {
                if le {
                    u16::from_le_bytes(pair)
                } else {
                    u16::from_be_bytes(pair)
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// What a new keymap file starts with: the format, as comments, so there is
/// nothing to guess. Every line is a comment, so it changes no key.
pub const TEMPLATE: &str = "\
# KoperCAD keymap. Lines starting with # are ignored.
# One binding per line:  command = [scope:] chord
# Find a command id in the shortcut sheet (?) or Ctrl+K.
# The first line for a command replaces its default keys; more lines add keys.
#
# tool.cable = Ctrl+Shift+C
# edit.duplicate = anywhere: Ctrl+D
# edit.duplicate = Alt+D
# tool.repeat = none
#
# Scopes: canvas (only while the drawing has focus), anywhere, panel.
# A line with a typo is skipped and reported; the other lines still apply.
# Save the file and restart KoperCAD to use the new keys.
";

/// Makes sure the keymap file exists at `path`: written with the
/// [`TEMPLATE`] when missing, left alone otherwise.
pub fn ensure_file(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, TEMPLATE)
}

/// Hands `path` to the user's own editor (Notepad on Windows). Does nothing
/// under test.
pub(super) fn open_in_editor(path: &Path) -> std::io::Result<()> {
    if cfg!(test) {
        return Ok(());
    }
    let mut command = if cfg!(windows) {
        std::process::Command::new("notepad.exe")
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-t");
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    command.arg(path).spawn().map(|_| ())
}

impl NovoApp {
    /// `help.keymap_file`: creates `%APPDATA%\KoperCAD\keymap.txt` with a
    /// commented template when it is missing, and opens it.
    pub(super) fn open_keymap_file(&mut self) {
        let Some(path) = keymap_path() else {
            self.message(
                Level::Warning,
                "No place for a keymap file on this computer",
            );
            return;
        };
        if let Err(e) = ensure_file(&path) {
            self.message(
                Level::Error,
                format!("Could not create {}: {e}", path.display()),
            );
            return;
        }
        match open_in_editor(&path) {
            Ok(()) => self.message(
                Level::Info,
                format!(
                    "Keymap file: {} · save it and restart KoperCAD to use the new keys",
                    path.display()
                ),
            ),
            Err(e) => self.message(
                Level::Warning,
                format!("Open {} in a text editor ({e})", path.display()),
            ),
        }
    }

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
    fn the_template_is_created_once_and_changes_no_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("KoperCAD").join(KEYMAP_FILE);
        ensure_file(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), TEMPLATE);
        let (map, notes) = load(&path);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(map.bindings(), Keymap::workflow().bindings());
        // The user's own file is never overwritten.
        std::fs::write(&path, "tool.cable = F6\n").unwrap();
        ensure_file(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "tool.cable = F6\n");
    }

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

    #[test]
    fn a_file_with_a_byte_order_mark_or_in_utf16_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEYMAP_FILE);
        let mut utf8 = vec![0xEF, 0xBB, 0xBF];
        utf8.extend_from_slice(b"tool.cable = F6\r\n");
        std::fs::write(&path, utf8).unwrap();
        let (map, notes) = load(&path);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(map.shortcut_text("tool.cable").as_deref(), Some("F6"));

        let mut utf16 = vec![0xFF, 0xFE];
        for unit in "tool.cable = F7\r\n".encode_utf16() {
            utf16.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(&path, utf16).unwrap();
        let (map, notes) = load(&path);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(map.shortcut_text("tool.cable").as_deref(), Some("F7"));
    }
}
