//! Moves per-user data from the folder names used before the rename
//! (`NovoElectric`) to the KoperCAD names. Everything stays inside the user's
//! profile, so no admin rights are needed.

use std::io;
use std::path::{Path, PathBuf};

use crate::{DATA_DIR_NAME, LEGACY_DATA_DIR_NAME};

/// Moves `old` to `new` when `old` exists and `new` does not. Returns whether
/// anything moved. A rename is tried first; across drives or on a locked
/// folder it falls back to a copy, and the old folder is left in place then,
/// so nothing is ever lost.
pub(crate) fn migrate_dir(old: &Path, new: &Path) -> io::Result<bool> {
    if !old.is_dir() || new.exists() {
        return Ok(false);
    }
    if let Some(parent) = new.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(old, new).is_ok() {
        return Ok(true);
    }
    copy_dir(old, new)?;
    Ok(true)
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// The per-user roaming folder (`%APPDATA%` on Windows) that holds the
/// window settings and the user's parts catalog, moved to the new name before
/// eframe reads its settings. Other platforms keep their folders. Failures are
/// ignored: the app then starts with default settings, as on a first run.
pub(crate) fn migrate_roaming_data() {
    if !cfg!(windows) {
        return;
    }
    if let Some(base) = dirs::data_dir() {
        let _ = migrate_dir(&base.join(LEGACY_DATA_DIR_NAME), &base.join(DATA_DIR_NAME));
    }
}

/// Where new projects are saved. `<Documents>\KoperCAD`, except that someone
/// whose projects already sit in `<Documents>\NovoElectric` keeps using that
/// folder (their recent-file list points into it).
pub(crate) fn projects_dir_in(documents: &Path) -> PathBuf {
    let new = documents.join(DATA_DIR_NAME);
    let legacy = documents.join(LEGACY_DATA_DIR_NAME);
    if !new.exists() && legacy.is_dir() {
        legacy
    } else {
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_the_old_folder_once() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("NovoElectric");
        let new = tmp.path().join("KoperCAD");
        std::fs::create_dir_all(old.join("data")).unwrap();
        std::fs::write(old.join("data/app.ron"), "x").unwrap();
        assert!(migrate_dir(&old, &new).unwrap());
        assert_eq!(
            std::fs::read_to_string(new.join("data/app.ron")).unwrap(),
            "x"
        );
        assert!(!old.exists());
        assert!(!migrate_dir(&old, &new).unwrap());
    }

    #[test]
    fn never_touches_an_existing_new_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("NovoElectric");
        let new = tmp.path().join("KoperCAD");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("a"), "old").unwrap();
        assert!(!migrate_dir(&old, &new).unwrap());
        assert!(old.join("a").exists());
        assert!(!new.join("a").exists());
    }

    #[test]
    fn copy_fallback_copies_nested_files() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("from");
        std::fs::create_dir_all(from.join("a/b")).unwrap();
        std::fs::write(from.join("a/b/c.txt"), "c").unwrap();
        let to = tmp.path().join("to");
        copy_dir(&from, &to).unwrap();
        assert_eq!(std::fs::read_to_string(to.join("a/b/c.txt")).unwrap(), "c");
    }

    #[test]
    fn projects_stay_in_the_old_documents_folder_until_the_new_one_exists() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(projects_dir_in(tmp.path()).ends_with("KoperCAD"));
        std::fs::create_dir(tmp.path().join("NovoElectric")).unwrap();
        assert!(projects_dir_in(tmp.path()).ends_with("NovoElectric"));
        std::fs::create_dir(tmp.path().join("KoperCAD")).unwrap();
        assert!(projects_dir_in(tmp.path()).ends_with("KoperCAD"));
    }
}
