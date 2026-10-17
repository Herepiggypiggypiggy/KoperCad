//! The user's macro folder follows `--profile-dir`. Its own test binary: the
//! profile folder is process-wide and the first one set wins.

#[test]
fn the_macros_folder_is_under_the_profile_folder() {
    let profile = std::env::temp_dir().join("kopercad-profile-test");
    novo_storage::profile::set_root(&profile);
    let dir = novo_parts::user_macros_dir().expect("a macros folder");
    assert!(dir.starts_with(&profile), "{}", dir.display());
}
