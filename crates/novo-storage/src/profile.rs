//! Where KoperCAD keeps per-user files, and how to move all of them.
//!
//! Normally the folders come from the platform (`dirs`: on Windows the known
//! folders Roaming, Local and Documents, which setting `APPDATA` does not
//! change). With a profile folder (`--profile-dir <folder>` on the command
//! line, or the `KOPERCAD_PROFILE_DIR` environment variable) every one of
//! them is a subfolder of it instead, so a test launch starts clean and never
//! touches the real settings. The folder is created on first use; it needs no
//! admin rights.

use std::path::PathBuf;
use std::sync::OnceLock;

/// The environment variable that names the profile folder.
pub const ENV: &str = "KOPERCAD_PROFILE_DIR";

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Sets the profile folder for this run (the `--profile-dir` flag). Only the
/// first call counts; call it before anything reads a folder.
pub fn set_root(dir: impl Into<PathBuf>) {
    let _ = ROOT.set(dir.into());
}

/// The profile folder, if one was asked for.
pub fn root() -> Option<PathBuf> {
    ROOT.get()
        .cloned()
        .or_else(|| from_env(std::env::var_os(ENV)))
}

fn from_env(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value.filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn under(sub: &str, platform: fn() -> Option<PathBuf>) -> Option<PathBuf> {
    match root() {
        Some(root) => Some(root.join(sub)),
        None => platform(),
    }
}

/// `%APPDATA%` (Roaming) on Windows: settings, parts catalog, templates.
pub fn data_dir() -> Option<PathBuf> {
    under("roaming", dirs::data_dir)
}

/// The config folder (`%APPDATA%` on Windows).
pub fn config_dir() -> Option<PathBuf> {
    under("config", dirs::config_dir)
}

/// `%LOCALAPPDATA%` on Windows: recovery files, unsaved projects, caches.
pub fn data_local_dir() -> Option<PathBuf> {
    under("local", dirs::data_local_dir)
}

/// The cache folder.
pub fn cache_dir() -> Option<PathBuf> {
    under("cache", dirs::cache_dir)
}

/// Documents: where new projects are saved.
pub fn document_dir() -> Option<PathBuf> {
    under("documents", dirs::document_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_or_missing_variable_means_no_profile() {
        assert_eq!(from_env(None), None);
        assert_eq!(from_env(Some("".into())), None);
        assert_eq!(
            from_env(Some("C:\\test-profile".into())),
            Some(PathBuf::from("C:\\test-profile"))
        );
    }

    #[test]
    fn each_kind_of_folder_gets_its_own_subfolder() {
        // The run's profile is process-wide; this process sets none, so the
        // platform answers. The subfolder names are fixed for testers.
        if root().is_none() {
            assert_eq!(data_dir(), dirs::data_dir());
        }
        set_root("profile");
        assert_eq!(data_dir(), Some(PathBuf::from("profile/roaming")));
        assert_eq!(data_local_dir(), Some(PathBuf::from("profile/local")));
        assert_eq!(document_dir(), Some(PathBuf::from("profile/documents")));
    }
}
