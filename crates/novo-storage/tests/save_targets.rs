//! A save to a place that cannot hold the file is refused in words that name
//! the place the person chose, before anything is written (hardware findings
//! FEC-01, FEC-04, FEC-05, FEC-10).

use std::fs;

use novo_core::Document;
use novo_storage::{ProjectFile, StorageError};

#[test]
fn save_as_to_a_missing_folder_names_the_folder_not_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("nofolder").join("sub").join("x.kopercad");
    let err = ProjectFile::create(&target, Document::default().project())
        .map(|_| ())
        .unwrap_err();
    let text = err.to_string();
    assert!(matches!(err, StorageError::CannotSave { .. }), "{text}");
    assert!(text.contains("the folder does not exist"), "{text}");
    assert!(!text.contains("unable to open database"), "{text}");
    assert!(!text.contains(".saving"), "{text}");
    assert_eq!(text.matches("x.kopercad").count(), 1, "{text}");
}

#[test]
fn save_as_to_a_device_name_is_refused_before_anything_is_written() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["NUL.kopercad", "con", "COM1.kopercad"] {
        let err = ProjectFile::create(dir.path().join(name), Document::default().project())
            .map(|_| ())
            .unwrap_err();
        assert!(
            matches!(err, StorageError::ReservedName(_)),
            "{name}: {err}"
        );
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0, "nothing made");
}

#[test]
fn opening_a_device_name_never_touches_it() {
    let dir = tempfile::tempdir().unwrap();
    let err = ProjectFile::read(dir.path().join("CON"))
        .map(|_| ())
        .unwrap_err();
    assert!(matches!(err, StorageError::ReservedName(_)), "{err}");
}

#[test]
fn an_empty_file_is_refused_and_left_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.kopercad");
    fs::write(&path, b"").unwrap();
    for open in [
        ProjectFile::open(&path).map(|_| ()),
        ProjectFile::read(&path).map(|_| ()),
    ] {
        assert!(
            matches!(open, Err(StorageError::NotAProject(_))),
            "{open:?}"
        );
    }
    assert_eq!(fs::metadata(&path).unwrap().len(), 0, "still zero bytes");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1, "no journal");
}

#[cfg(unix)]
#[test]
fn a_write_protected_folder_is_named_in_a_save_as() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let ro = dir.path().join("ro");
    fs::create_dir(&ro).unwrap();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
    // Root ignores folder permissions; the check only means something otherwise.
    let can_write = fs::write(ro.join("probe"), b"").is_ok();
    let err = ProjectFile::create(ro.join("x.kopercad"), Document::default().project())
        .map(|_| ())
        .err();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
    if can_write {
        return;
    }
    let text = err.expect("refused").to_string();
    assert!(text.contains("x.kopercad cannot be saved"), "{text}");
    assert!(text.contains("write-protected"), "{text}");
    assert!(!text.contains("Open it read-only"), "{text}");
    assert!(!text.contains(".saving"), "{text}");
}
