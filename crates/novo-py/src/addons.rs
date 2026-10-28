//! The add-on manager for scripts: list, install, turn on and off, remove. The same store and
//! the same rules as the program's Manage add-ons window (`novo-addons`); the Python functions
//! of the same names are thin wrappers over these.

use std::path::Path;

use novo_addons::report::{inspection_json, set_json};
use novo_addons::{AddonError, AppInfo, InstallOptions, Store, Version, default_root};
use serde_json::Value;

use crate::ScriptError;

fn refused(e: AddonError) -> ScriptError {
    ScriptError::Addons(e.to_string())
}

fn store(root: Option<&Path>) -> Result<Store, ScriptError> {
    let dir = match root {
        Some(r) => r.to_path_buf(),
        None => novo_storage::profile::data_local_dir()
            .map(|d| default_root(&d))
            .ok_or_else(|| {
                ScriptError::Addons("this computer has no profile folder; pass root=".into())
            })?,
    };
    Ok(Store::new(
        dir,
        AppInfo {
            version: Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_default(),
            api_version: novo_addons::API_VERSION,
        },
    ))
}

/// What is installed: `{"addons": [...], "warnings": [...]}`.
pub fn addons_json(root: Option<&Path>) -> Result<Value, ScriptError> {
    Ok(set_json(&store(root)?.scan()))
}

/// Installs a `.kcaddon`. A file no trusted publisher signed needs `trust_unsigned`.
pub fn addons_install(
    path: &Path,
    trust_unsigned: bool,
    enable: bool,
    root: Option<&Path>,
) -> Result<Value, ScriptError> {
    let i = store(root)?
        .install_file(
            path,
            InstallOptions {
                accept_unsigned: trust_unsigned,
                enable,
            },
        )
        .map_err(refused)?;
    Ok(inspection_json(&i))
}

/// Turns an add-on on or off.
pub fn addons_set_enabled(id: &str, on: bool, root: Option<&Path>) -> Result<(), ScriptError> {
    store(root)?.set_enabled(id, on).map_err(refused)
}

/// Removes an add-on and all its versions.
pub fn addons_remove(id: &str, root: Option<&Path>) -> Result<(), ScriptError> {
    store(root)?.remove(id).map_err(refused)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_folder_lists_nothing_and_a_bad_file_is_refused_in_words() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("addons");
        let listed = addons_json(Some(&root)).unwrap();
        assert_eq!(listed["addons"].as_array().unwrap().len(), 0);
        let junk = dir.path().join("x.kcaddon");
        std::fs::write(&junk, b"not a zip").unwrap();
        let err = addons_install(&junk, true, true, Some(&root)).unwrap_err();
        assert!(
            err.to_string().contains("not a readable add-on file"),
            "{err}"
        );
        assert!(addons_remove("nothing-here", Some(&root)).is_err());
    }
}
