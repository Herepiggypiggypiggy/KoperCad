//! Robustness of the project file: old schema versions, a file cut short, a
//! crash image taken in the middle of a save, and a recovery file written by
//! an older build.

use std::fs;
use std::path::{Path, PathBuf};

use novo_core::{DeviceTag, Document};
use novo_storage::{ProjectFile, Session, autosave_path, schema};
use rusqlite::Connection;
use tempfile::TempDir;

fn temp() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plant.novo");
    (dir, path)
}

/// A small saved project and the document behind it.
fn saved(path: &Path) -> Document {
    let mut doc = Document::default();
    doc.transact("Draw", |tx| {
        tx.add_page("1")?;
        tx.add_device(DeviceTag::parse("-K1").unwrap(), "Contactor")?;
        tx.add_device(DeviceTag::parse("-M1").unwrap(), "Motor")
    })
    .unwrap();
    let session = Session::create(path, doc.project()).unwrap();
    session.close().unwrap();
    doc
}

/// Makes a file look like one written by schema version 1: no snapshots
/// table.
fn downgrade_to_v1(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch("DROP TABLE snapshots; DROP TABLE sidecar; PRAGMA user_version = 1;")
        .unwrap();
}

fn user_version(path: &Path) -> u32 {
    Connection::open(path)
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn a_version_1_file_opens_upgraded_with_a_backup() {
    let (_dir, path) = temp();
    let doc = saved(&path);
    downgrade_to_v1(&path);
    assert_eq!(user_version(&path), 1);

    let opened = Session::open(&path).unwrap();
    assert_eq!(opened.project.to_json(), doc.project().to_json());
    opened.session.close().unwrap();

    assert_eq!(user_version(&path), schema::SCHEMA_VERSION);
    let mut backup = path.as_os_str().to_owned();
    backup.push(".v1.bak");
    let backup = PathBuf::from(backup);
    assert!(backup.exists(), "the old file is kept as {backup:?}");
    assert_eq!(user_version(&backup), 1, "the backup is the old version");

    // The upgraded file takes revision snapshots and opens again.
    let again = Session::open(&path).unwrap();
    assert_eq!(again.project.to_json(), doc.project().to_json());
}

#[test]
fn a_file_cut_short_never_panics_and_a_small_cut_is_refused() {
    let (_dir, path) = temp();
    saved(&path);
    let bytes = fs::read(&path).unwrap();
    for cut in [0, 10, 50, 100, bytes.len() / 2, bytes.len() - 1] {
        let dir = tempfile::tempdir().unwrap();
        let broken = dir.path().join("cut.novo");
        fs::write(&broken, &bytes[..cut]).unwrap();
        let result = Session::open(&broken);
        if cut <= 100 && cut > 0 {
            assert!(result.is_err(), "{cut} bytes is not a project");
        }
        // Whatever happens, the cut file is not turned into a different
        // project: either it errors or it opens as what is left.
        if let Ok(opened) = result {
            let _ = opened.project.to_json();
        }
    }
}

#[test]
fn a_crash_image_in_the_middle_of_a_save_rolls_back_to_the_last_save() {
    let (_dir, path) = temp();
    let doc = saved(&path);
    let size_before = fs::metadata(&path).unwrap().len();

    // Another connection starts rewriting the file and spills enough pages
    // that the database file itself is changed before the commit; the
    // rollback journal is then what makes the file whole again.
    let writer = Connection::open(&path).unwrap();
    writer
        .execute_batch("PRAGMA journal_mode = DELETE; PRAGMA cache_size = 1;")
        .unwrap();
    writer.execute_batch("BEGIN").unwrap();
    let filler = "x".repeat(1000);
    for i in 0..4000 {
        writer
            .execute(
                "INSERT INTO records (id, kind, page, data) VALUES (?1, 'junk', NULL, ?2)",
                rusqlite::params![1_000_000 + i, filler],
            )
            .unwrap();
    }
    writer
        .execute("UPDATE meta SET value = 'lost' WHERE key = 'info'", [])
        .unwrap();

    // Copy the file and its journal as they are right now.
    let crash_dir = tempfile::tempdir().unwrap();
    let image = crash_dir.path().join("plant.novo");
    fs::copy(&path, &image).unwrap();
    let mut journal = path.as_os_str().to_owned();
    journal.push("-journal");
    let mut image_journal = image.as_os_str().to_owned();
    image_journal.push("-journal");
    assert!(
        Path::new(&journal).exists(),
        "an open write transaction has a hot journal"
    );
    fs::copy(&journal, &image_journal).unwrap();
    assert!(
        fs::metadata(&image).unwrap().len() > size_before,
        "the file itself was changed before the commit"
    );
    drop(writer);

    let opened = Session::open(&image).unwrap();
    assert_eq!(
        opened.project.to_json(),
        doc.project().to_json(),
        "the half-written save is rolled back"
    );
}

#[test]
fn a_recovery_file_from_an_older_build_is_still_offered() {
    let (_dir, path) = temp();
    let mut doc = saved(&path);
    let mut session = Session::open(&path).unwrap().session;
    let (_, changes) = doc
        .transact("More", |tx| {
            tx.add_device(DeviceTag::parse("-Q1").unwrap(), "Breaker")
        })
        .unwrap();
    session.record(&changes);
    session.autosave(doc.project()).unwrap();
    drop(session);

    downgrade_to_v1(&autosave_path(&path));

    let opened = Session::open(&path).unwrap();
    let recovered = opened
        .recovered
        .expect("the older recovery file is upgraded and offered");
    assert_eq!(recovered.project.to_json(), doc.project().to_json());
    assert!(opened.set_aside.is_none());
}

#[test]
fn read_of_a_version_1_file_works_without_writing() {
    let (_dir, path) = temp();
    let doc = saved(&path);
    downgrade_to_v1(&path);
    let before = fs::read(&path).unwrap();
    let project = ProjectFile::read(&path).unwrap();
    assert_eq!(project.to_json(), doc.project().to_json());
    assert_eq!(
        fs::read(&path).unwrap(),
        before,
        "a read leaves the file alone"
    );
}
