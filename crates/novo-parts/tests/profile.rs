//! The user's macro folder follows `--profile-dir`. Its own test binary: the
//! profile folder is process-wide and the first one set wins.

#[test]
fn the_macros_folder_is_under_the_profile_folder() {
    let profile = std::env::temp_dir().join("kopercad-profile-test");
    novo_storage::profile::set_root(&profile);
    let dir = novo_parts::user_macros_dir().expect("a macros folder");
    assert!(dir.starts_with(&profile), "{}", dir.display());
}

#[test]
fn the_catalog_inbox_library_and_mappings_follow_the_profile_folder() {
    let profile = std::env::temp_dir().join("kopercad-profile-test");
    novo_storage::profile::set_root(&profile);
    let base = profile.join("roaming").join("KoperCAD");
    assert_eq!(
        novo_parts::user_catalog_path(),
        Some(base.join("parts.json"))
    );
    assert_eq!(novo_parts::default_inbox(), Some(base.join("inbox")));
    assert_eq!(
        novo_parts::default_library_root(),
        Some(base.join("library"))
    );
    assert_eq!(
        novo_parts::saved_mappings_path(),
        Some(base.join("parts-table-mappings.json"))
    );
}
