//! Recovery files in a per-user folder instead of beside the project.

use std::path::{Path, PathBuf};

use novo_core::*;
use novo_storage::{Session, autosave_path, recovery_path_in};

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

/// A project folder and a separate recovery folder.
fn dirs() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("plans").join("pump.novo");
    std::fs::create_dir_all(project.parent().unwrap()).unwrap();
    let recovery = dir.path().join("appdata").join("recovery");
    (dir, project, recovery)
}

fn edit(doc: &mut Document, session: &mut Session, name: &str) {
    let (_, changes) = doc
        .transact("Add device", |tx| tx.add_device(tag(name), "Test"))
        .unwrap();
    session.record(&changes);
}

#[test]
fn autosave_goes_to_the_recovery_folder_not_beside_the_project() {
    let (_dir, path, recovery) = dirs();
    let mut doc = Document::default();
    let mut session = Session::create_in(&path, doc.project(), Some(&recovery)).unwrap();
    edit(&mut doc, &mut session, "-K1");
    session.autosave(doc.project()).unwrap();

    let file = session.recovery_path();
    assert_eq!(file.parent(), Some(recovery.as_path()));
    assert!(file.exists());
    assert!(!autosave_path(&path).exists());
    assert_eq!(file, recovery_path_in(&recovery, &path));
}

#[test]
fn work_is_offered_back_after_a_crash_and_cleaned_up_after_a_save() {
    let (_dir, path, recovery) = dirs();
    let mut doc = Document::default();
    let mut session = Session::create_in(&path, doc.project(), Some(&recovery)).unwrap();
    let saved = doc.project().clone();
    edit(&mut doc, &mut session, "-K1");
    session.autosave(doc.project()).unwrap();
    drop(session); // a crash: no clean close

    let opened = Session::open_in(&path, Some(&recovery)).unwrap();
    assert_eq!(opened.project.to_json(), saved.to_json());
    let recovered = opened.recovered.expect("the unsaved work is offered");
    assert_eq!(recovered.project.to_json(), doc.project().to_json());

    // The real file was not touched by any of this.
    let mut session = opened.session;
    let recovery_file = session.recovery_path();
    assert!(recovery_file.exists());
    session.accept_recovery();
    session.save(&recovered.project).unwrap();
    assert!(!recovery_file.exists());
    session.close().unwrap();
    assert!(
        Session::open_in(&path, Some(&recovery))
            .unwrap()
            .recovered
            .is_none()
    );
}

#[test]
fn a_clean_close_and_a_discard_leave_nothing_behind() {
    let (_dir, path, recovery) = dirs();
    let mut doc = Document::default();
    let mut session = Session::create_in(&path, doc.project(), Some(&recovery)).unwrap();
    edit(&mut doc, &mut session, "-K1");
    session.autosave(doc.project()).unwrap();
    let file = session.recovery_path();
    session.close().unwrap();
    assert!(!file.exists());

    let mut session = Session::create_in(&path, doc.project(), Some(&recovery)).unwrap();
    edit(&mut doc, &mut session, "-K2");
    session.autosave(doc.project()).unwrap();
    drop(session);
    let mut opened = Session::open_in(&path, Some(&recovery)).unwrap();
    assert!(opened.recovered.is_some());
    opened.session.discard_recovery().unwrap();
    assert!(!file.exists());
}

#[test]
fn projects_with_the_same_name_in_different_folders_do_not_share_a_recovery_file() {
    let (dir, path, recovery) = dirs();
    let other = dir.path().join("archive").join("pump.novo");
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    let a = recovery_path_in(&recovery, &path);
    let b = recovery_path_in(&recovery, &other);
    assert_ne!(a, b);
    // Stable: the same project always maps to the same file.
    assert_eq!(a, recovery_path_in(&recovery, &path));
    assert!(
        a.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("pump-")
    );
}

#[test]
fn an_autosave_beside_the_project_from_an_older_version_is_still_offered() {
    let (_dir, path, recovery) = dirs();
    let mut doc = Document::default();
    // An older version: recovery file beside the project.
    let mut old = Session::create(&path, doc.project()).unwrap();
    edit(&mut doc, &mut old, "-K1");
    old.autosave(doc.project()).unwrap();
    drop(old);
    assert!(autosave_path(&path).exists());

    let opened = Session::open_in(&path, Some(&recovery)).unwrap();
    let recovered = opened.recovered.expect("the old recovery file is offered");
    assert_eq!(recovered.project.to_json(), doc.project().to_json());
    let mut session = opened.session;
    session.discard_recovery().unwrap();
    assert!(!autosave_path(&path).exists());
}

#[test]
fn an_unusable_recovery_folder_falls_back_to_beside_the_project() {
    let (dir, path, _) = dirs();
    let blocked = dir.path().join("not-a-folder");
    std::fs::write(&blocked, b"a file where a folder should be").unwrap();
    let doc = Document::default();
    let session = Session::create_in(&path, doc.project(), Some(&blocked)).unwrap();
    assert_eq!(session.recovery_path(), autosave_path(&path));
}

#[test]
fn save_as_moves_the_recovery_file_to_the_new_projects_name() {
    let (dir, path, recovery) = dirs();
    let mut doc = Document::default();
    let mut session = Session::create_in(&path, doc.project(), Some(&recovery)).unwrap();
    edit(&mut doc, &mut session, "-K1");
    session.autosave(doc.project()).unwrap();
    let before = session.recovery_path();
    assert!(before.exists());

    let renamed: &Path = &dir.path().join("plans").join("pump 2.novo");
    session.save_as(renamed, doc.project()).unwrap();
    assert!(!before.exists());
    assert_ne!(session.recovery_path(), before);
    assert_eq!(
        session.recovery_path(),
        recovery_path_in(&recovery, renamed)
    );

    edit(&mut doc, &mut session, "-K2");
    session.autosave(doc.project()).unwrap();
    assert!(session.recovery_path().exists());
}
