//! Older project files open and upgrade safely.
//!
//! `fixtures/v2-pump.novo` was written by this crate at schema version 2 and
//! is committed, so whatever the next migration does, a real old file must
//! still open. `fixtures/v2-pump.json` is the project it holds. The v1 file
//! is made from the v2 one the way version 1 differed from it (no snapshot
//! table). To rewrite the fixtures after an intended format change, run
//! `KOPERCAD_REGEN_FIXTURES=1 cargo test -p novo-storage --test upgrade`
//! and review the diff.

use std::path::{Path, PathBuf};

use novo_core::*;
use novo_storage::{ProjectFile, Session, StorageError, autosave_path, schema};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v2-pump.novo");
const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v2-pump.json");

fn pump() -> Project {
    let mut doc = Document::default();
    doc.transact("Pump", |tx| {
        tx.add_page("1")?;
        tx.add_page("2")?;
        tx.add_device(DeviceTag::parse("-K1")?, "Pump contactor")?;
        tx.add_device(DeviceTag::parse("-M1")?, "Pump motor")?;
        Ok(())
    })
    .unwrap();
    doc.project().clone()
}

#[test]
fn regenerate_fixtures() {
    if std::env::var_os("KOPERCAD_REGEN_FIXTURES").is_none() {
        return;
    }
    let project = pump();
    let _ = std::fs::remove_file(FIXTURE);
    ProjectFile::create(FIXTURE, &project)
        .unwrap()
        .close()
        .unwrap();
    std::fs::write(EXPECTED, project.to_json()).unwrap();
}

/// A copy of the committed fixture in a temp folder.
fn copy_of_fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pump.novo");
    std::fs::copy(FIXTURE, &path).unwrap();
    (dir, path)
}

fn expected_json() -> String {
    std::fs::read_to_string(EXPECTED).unwrap()
}

/// Turns a current-format file into a version 1 file.
fn downgrade_to_v1(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(
        "DROP TABLE IF EXISTS snapshots; DROP TABLE IF EXISTS sidecar; PRAGMA user_version = 1;",
    )
    .unwrap();
}

fn version_of(path: &Path) -> u32 {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

#[test]
fn the_committed_version_2_file_opens_and_upgrades_with_a_backup() {
    let (_dir, path) = copy_of_fixture();
    let before = std::fs::read(&path).unwrap();
    assert_eq!(ProjectFile::read(&path).unwrap().to_json(), expected_json());
    assert_eq!(std::fs::read(&path).unwrap(), before, "read never writes");

    let opened = Session::open(&path).unwrap();
    assert_eq!(opened.project.to_json(), expected_json());
    assert!(opened.recovered.is_none() && opened.set_aside.is_none());
    opened.session.close().unwrap();
    assert_eq!(version_of(&path), schema::SCHEMA_VERSION);
    let backup = path.with_file_name("pump.novo.v2.bak");
    assert_eq!(version_of(&backup), 2, "the backup keeps the old version");
    assert_eq!(
        ProjectFile::read(&backup).unwrap().to_json(),
        expected_json()
    );
    let file = ProjectFile::open(&path).unwrap();
    assert_eq!(file.sidecar("commissioning").unwrap(), None);
}

#[test]
fn a_version_1_file_is_upgraded_with_a_backup_and_loses_nothing() {
    let (dir, path) = copy_of_fixture();
    downgrade_to_v1(&path);
    assert_eq!(version_of(&path), 1);

    let opened = Session::open(&path).unwrap();
    assert_eq!(opened.project.to_json(), expected_json());
    opened.session.close().unwrap();
    assert_eq!(version_of(&path), schema::SCHEMA_VERSION);

    // The backup is the file as it was, still version 1 and still readable.
    let backup = dir.path().join("pump.novo.v1.bak");
    assert!(backup.exists());
    assert_eq!(version_of(&backup), 1);
    assert_eq!(
        ProjectFile::read(&backup).unwrap().to_json(),
        expected_json()
    );
}

#[test]
fn reading_a_version_1_file_does_not_upgrade_it_on_disk() {
    let (dir, path) = copy_of_fixture();
    downgrade_to_v1(&path);
    let before = std::fs::read(&path).unwrap();
    assert_eq!(ProjectFile::read(&path).unwrap().to_json(), expected_json());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!dir.path().join("pump.novo.v1.bak").exists());
}

#[test]
fn a_damaged_version_1_file_is_refused_but_its_backup_is_kept() {
    let (dir, path) = copy_of_fixture();
    downgrade_to_v1(&path);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE records SET data = 'not json' WHERE id = (SELECT MIN(id) FROM records)",
        [],
    )
    .unwrap();
    drop(conn);
    assert!(Session::open(&path).is_err());
    // The pre-upgrade copy exists, so the damaged original can be inspected.
    assert!(dir.path().join("pump.novo.v1.bak").exists());
}

#[test]
fn a_recovery_file_written_by_the_same_old_file_is_still_offered_after_the_upgrade() {
    let (_dir, path) = copy_of_fixture();
    downgrade_to_v1(&path);

    // The old version crashed with unsaved work: a recovery file exists.
    let mut doc = Document::new(ProjectFile::read(&path).unwrap());
    let mut session = {
        // Session::create would replace the file, so open and edit instead.
        Session::open(&path).unwrap().session
    };
    let (_, changes) = doc
        .transact("Add device", |tx| {
            tx.add_device(DeviceTag::parse("-K9")?, "Spare")
        })
        .unwrap();
    session.record(&changes);
    session.autosave(doc.project()).unwrap();
    drop(session);
    assert!(autosave_path(&path).exists());

    let opened = Session::open(&path).unwrap();
    let recovered = opened.recovered.expect("unsaved work survives the upgrade");
    assert_eq!(recovered.project.to_json(), doc.project().to_json());
    assert_eq!(
        opened.project.to_json(),
        expected_json(),
        "the file itself is as saved"
    );
}

#[test]
fn files_from_a_newer_version_are_refused_and_left_alone() {
    let (_dir, path) = copy_of_fixture();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", schema::SCHEMA_VERSION + 1)
        .unwrap();
    drop(conn);
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        Session::open(&path).map(|_| ()),
        Err(StorageError::NewerFormat { .. })
    ));
    assert!(matches!(
        ProjectFile::read(&path).map(|_| ()),
        Err(StorageError::NewerFormat { .. })
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
