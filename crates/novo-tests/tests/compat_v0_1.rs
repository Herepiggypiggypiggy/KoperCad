//! REL-COMPAT: project files stay readable across builds, and nothing a user
//! saved is ever lost to a format change or a crash (release blocker B3).
//!
//! - `golden/v0.1-motor-starter.novo` is the project file KoperCAD 0.1 writes
//!   (schema version 3): the Gate 1 motor starter with an issued revision,
//!   its snapshot and a sidecar document. It is committed, so whatever the
//!   next migration or model change does, this exact file must still open and
//!   give `golden/v0.1-motor-starter.json`. Rewrite both only for an intended
//!   format change: `KOPERCAD_REGEN_GOLDEN=1 cargo test -p novo-tests --test
//!   compat_v0_1 regenerate_golden`, then review the diff and say why in the
//!   pull request (policy: `docs/FILE-COMPATIBILITY.md`).
//! - Older schemas (1 and 2) are cut from the golden file the way those
//!   versions differed, and every earlier model JSON on file (the per-version
//!   fixtures and the lesson projects) is saved to a `.novo` and reopened.
//! - A file from a newer build is refused with a readable message and is
//!   byte-for-byte untouched; a file with an unknown kind of object says so.
//! - A real process killed in the middle of saving leaves the last good save
//!   or the new one, never a mix; a killed process with autosaved work is
//!   offered that work again by the app's backend on the next start.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use novo_app::{ProjectBackend, StorageBackend};
use novo_core::*;
use novo_storage::{ProjectFile, Session, StorageError, schema};
use novo_tests::motor_starter;

const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/golden/v0.1-motor-starter.novo"
);
const GOLDEN_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/golden/v0.1-motor-starter.json"
);
const SIDECAR: (&str, &str) = ("commissioning", "{\"checks\":[],\"punch\":[]}");

/// The schema version KoperCAD 0.1 writes. If this fails, a migration was
/// added: that is allowed, but the golden file must stay as it is and a
/// golden file for the new version must be added beside it.
const V0_1_SCHEMA: u32 = 3;

/// The project in the golden file and the id of its issued revision.
fn golden_project() -> (Project, RevisionId) {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let outcome = doc
        .execute(Command::IssueRevision {
            index: "A".into(),
            date: "2026-10-17".into(),
            author: "TV".into(),
            description: "First issue".into(),
            note: String::new(),
        })
        .unwrap();
    let [AnyId::Revision(id)] = outcome.created[..] else {
        panic!("{:?}", outcome.created)
    };
    (doc.project().clone(), id)
}

#[test]
fn regenerate_golden() {
    if std::env::var_os("KOPERCAD_REGEN_GOLDEN").is_none() {
        return;
    }
    let (project, revision) = golden_project();
    std::fs::create_dir_all(Path::new(GOLDEN).parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(GOLDEN);
    let mut file = ProjectFile::create(GOLDEN, &project).unwrap();
    file.put_snapshot(revision, &project).unwrap();
    file.put_sidecar(SIDECAR.0, SIDECAR.1).unwrap();
    file.close().unwrap();
    std::fs::write(GOLDEN_JSON, project.to_json()).unwrap();
}

fn temp() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Motor starter ü.novo");
    (dir, path)
}

fn golden_copy() -> (tempfile::TempDir, PathBuf) {
    let (dir, path) = temp();
    std::fs::copy(GOLDEN, &path).unwrap();
    (dir, path)
}

fn expected_json() -> String {
    std::fs::read_to_string(GOLDEN_JSON).unwrap()
}

fn user_version(path: &Path) -> u32 {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

/// How many times the file has been written (the `save_seq` row).
fn save_seq(path: &Path) -> u64 {
    let conn = rusqlite::Connection::open(path).unwrap();
    let value: String = conn
        .query_row("SELECT value FROM meta WHERE key = 'save_seq'", [], |r| {
            r.get(0)
        })
        .unwrap();
    value.parse().unwrap()
}

fn set_user_version(path: &Path, version: u32) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "user_version", version).unwrap();
}

// ---------------------------------------------------------------- golden

#[test]
fn the_golden_file_is_schema_v0_1_and_this_build_still_writes_it() {
    assert_eq!(user_version(Path::new(GOLDEN)), V0_1_SCHEMA);
    assert_eq!(
        schema::SCHEMA_VERSION,
        V0_1_SCHEMA,
        "the schema moved past 0.1: keep this golden file, add one for the new version"
    );
}

#[test]
fn the_golden_file_opens_and_matches_its_pinned_json() {
    let (_dir, path) = golden_copy();
    let before = std::fs::read(&path).unwrap();
    assert_eq!(ProjectFile::read(&path).unwrap().to_json(), expected_json());
    assert_eq!(std::fs::read(&path).unwrap(), before, "read never writes");

    let opened = Session::open(&path).unwrap();
    assert_eq!(opened.project.to_json(), expected_json());
    assert!(opened.recovered.is_none());
    assert!(opened.set_aside.is_none());
    let revisions: Vec<_> = opened.project.revisions().collect();
    assert_eq!(revisions.len(), 1);
    assert!(opened.session.snapshot(revisions[0].id).unwrap().is_some());
    assert_eq!(
        opened.session.sidecar(SIDECAR.0).unwrap().as_deref(),
        Some(SIDECAR.1)
    );
    opened.session.close().unwrap();
    // A current-schema file is opened without a migration backup.
    assert!(!backups_beside(&path), "no upgrade, no backup");
}

fn backups_beside(path: &Path) -> bool {
    std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().ends_with(".bak"))
}

#[test]
fn the_golden_file_survives_save_as_close_reopen_json_equal() {
    let (dir, path) = golden_copy();
    let opened = Session::open(&path).unwrap();
    let mut session = opened.session;
    let copy = dir.path().join("Copy ä ö ü.novo");
    session.save_as(&copy, &opened.project).unwrap();
    session.close().unwrap();
    for file in [&path, &copy] {
        let again = Session::open(file).unwrap();
        assert_eq!(again.project.to_json(), expected_json(), "{file:?}");
        assert_eq!(
            again.session.sidecar(SIDECAR.0).unwrap().as_deref(),
            Some(SIDECAR.1)
        );
        again.session.close().unwrap();
    }
}

#[test]
fn a_current_build_saves_the_same_project_json_as_the_golden_file() {
    // The golden project is rebuilt in code: a model change that silently
    // alters what the motor starter looks like on disk shows up here.
    let (project, _) = golden_project();
    assert_eq!(project.to_json(), expected_json());
}

// ------------------------------------------------------------ old schemas

/// Turns the golden file into what schema `version` (1 or 2) looked like.
fn downgrade(path: &Path, version: u32) {
    let conn = rusqlite::Connection::open(path).unwrap();
    if version < 3 {
        conn.execute_batch("DROP TABLE sidecar;").unwrap();
    }
    if version < 2 {
        conn.execute_batch("DROP TABLE snapshots;").unwrap();
    }
    conn.pragma_update(None, "user_version", version).unwrap();
}

#[test]
fn schema_1_and_2_files_upgrade_keep_a_backup_and_lose_no_drawing_data() {
    for version in [1, 2] {
        let (dir, path) = golden_copy();
        downgrade(&path, version);
        assert_eq!(user_version(&path), version);
        let old_bytes = std::fs::read(&path).unwrap();

        // Reading does not touch the file.
        assert_eq!(ProjectFile::read(&path).unwrap().to_json(), expected_json());
        assert_eq!(std::fs::read(&path).unwrap(), old_bytes);

        let opened = Session::open(&path).unwrap();
        assert_eq!(opened.project.to_json(), expected_json(), "v{version}");
        opened.session.close().unwrap();
        assert_eq!(user_version(&path), V0_1_SCHEMA);

        let backup = dir.path().join(format!(
            "{}.v{version}.bak",
            path.file_name().unwrap().to_string_lossy()
        ));
        assert!(
            std::fs::metadata(&backup).is_ok_and(|m| m.len() > 0),
            "backup of the v{version} file"
        );
        // The backup is itself a file that opens as it was.
        assert_eq!(user_version(&backup), version);
        assert_eq!(
            ProjectFile::read(&backup).unwrap().to_json(),
            expected_json()
        );
    }
}

/// Model JSON written by earlier builds, kept in the repository.
fn earlier_json_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![root.join("crates/novo-core/tests/fixtures/format_v1.json")];
    let lessons = root.join("assets/lessons");
    for dir in std::fs::read_dir(lessons).unwrap().filter_map(Result::ok) {
        for name in ["project.json", "finished.json"] {
            let file = dir.path().join(name);
            if file.exists() {
                files.push(file);
            }
        }
    }
    files
}

#[test]
fn every_earlier_model_json_in_the_repository_loads_saves_and_reopens() {
    let files = earlier_json_files();
    assert!(files.len() > 10, "found {} files", files.len());
    for json_file in files {
        let text = std::fs::read_to_string(&json_file).unwrap();
        let project = Project::from_json(&text)
            .unwrap_or_else(|e| panic!("{} no longer loads: {e}", json_file.display()));
        let (_dir, path) = temp();
        Session::create(&path, &project).unwrap().close().unwrap();
        let again = Session::open(&path).unwrap();
        assert_eq!(
            again.project.to_json(),
            project.to_json(),
            "{}",
            json_file.display()
        );
        again.session.close().unwrap();
    }
}

#[test]
fn the_record_rows_of_the_pinned_record_fixture_load_from_a_database() {
    // `records_v1.jsonl` pins the per-record form the rows hold. Put the
    // records in a v1-shaped database by hand and open it.
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../novo-core/tests/fixtures/records_v1.jsonl");
    let text = std::fs::read_to_string(fixture).unwrap();
    let mut records = Vec::new();
    for line in text.lines() {
        // The file also pins history operations; only records have "record".
        if let Ok(record) = serde_json::from_str::<Record>(line) {
            records.push(record);
        }
    }
    assert!(records.len() > 5);
    let info = Project::default().info().clone();
    let next_id = records.iter().map(|r| r.id().raw()).max().unwrap() + 1;
    let project = Project::from_records(info, next_id, records).unwrap();
    let (_dir, path) = temp();
    Session::create(&path, &project).unwrap().close().unwrap();
    let again = Session::open(&path).unwrap();
    assert_eq!(again.project.to_json(), project.to_json());
    again.session.close().unwrap();
}

// ---------------------------------------------------------- newer schemas

#[test]
fn a_file_from_a_newer_schema_is_refused_with_a_readable_message_and_untouched() {
    let (dir, path) = golden_copy();
    set_user_version(&path, V0_1_SCHEMA + 1);
    let before = std::fs::read(&path).unwrap();

    let message = |err: StorageError| {
        assert!(matches!(err, StorageError::NewerFormat { .. }), "{err:?}");
        err.to_string()
    };
    let m = message(Session::open(&path).map(|_| ()).unwrap_err());
    assert!(m.contains("newer version of KoperCAD"), "{m}");
    assert!(m.contains(&format!("format {}", V0_1_SCHEMA + 1)), "{m}");
    assert!(m.contains("Motor starter"), "names the file: {m}");
    message(ProjectFile::read(&path).map(|_| ()).unwrap_err());
    message(ProjectFile::open(&path).map(|_| ()).unwrap_err());

    // Through the app: the message reaches the status line and nothing is
    // opened, written, backed up or left behind.
    let mut backend = StorageBackend::default().with_recovery_dir(dir.path().join("recovery"));
    let err = backend.open(&path).unwrap_err();
    assert!(
        err.to_string().contains("newer version of KoperCAD"),
        "{err}"
    );
    assert!(backend.document().is_none());
    drop(backend);

    assert_eq!(std::fs::read(&path).unwrap(), before, "never overwritten");
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names.len(),
        1,
        "no backup or recovery file appeared: {names:?}"
    );
}

#[test]
fn a_newer_file_cannot_be_replaced_by_save_or_save_as_over_it() {
    let (dir, path) = golden_copy();
    set_user_version(&path, V0_1_SCHEMA + 1);
    let before = std::fs::read(&path).unwrap();
    // Creating a project at the newer file's path is the "save over" route
    // of an older build: it replaces the file only when it is not a project
    // of any version.
    let (project, _) = golden_project();
    let result = ProjectFile::create(&path, &project);
    assert!(result.is_err(), "an older build must not overwrite it");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(dir);
}

#[test]
fn an_unknown_kind_of_object_is_reported_as_coming_from_a_newer_version() {
    let (_dir, path) = golden_copy();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "INSERT INTO records (id, kind, page, data) VALUES (999999, 'hologram', NULL, ?1)",
        [r#"{"record":"hologram","id":999999}"#],
    )
    .unwrap();
    drop(conn);
    let err = ProjectFile::read(&path).unwrap_err();
    let text = err.to_string();
    assert!(matches!(err, StorageError::UnknownRecord { .. }), "{err:?}");
    assert!(
        text.contains("hologram") && text.contains("newer version"),
        "{text}"
    );
}

// ------------------------------------------------------------- hard crash

/// The two projects the saving child alternates between.
fn project_with_pages(n: usize) -> Project {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    doc.transact("Pages", |tx| {
        for i in 0..n {
            tx.add_page(format!("X{i}"))?;
        }
        Ok(())
    })
    .unwrap();
    doc.project().clone()
}

/// Runs as a child process: saves two different projects over a file for as
/// long as it lives. The parent kills it at a random moment.
#[test]
fn child_save_loop() {
    let Some(path) = std::env::var_os("KOPERCAD_CRASH_CHILD_FILE") else {
        return;
    };
    let a = project_with_pages(5);
    let b = project_with_pages(60);
    // A full save replaces every record in one transaction, the longest
    // write the app does (an incremental save is a shorter transaction).
    let mut file = ProjectFile::open(&path).unwrap();
    for i in 0.. {
        file.save(if i % 2 == 0 { &b } else { &a }).unwrap();
    }
}

fn run_child(test: &str, path: &Path, lifetime: Duration) {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--nocapture", "--test-threads=1"])
        .env("KOPERCAD_CRASH_CHILD_FILE", path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(lifetime);
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn a_process_killed_while_saving_leaves_a_whole_file_every_time() {
    let a = project_with_pages(5).to_json();
    let b = project_with_pages(60).to_json();
    let mut saves_seen = 0;
    for round in 0..10u64 {
        let (_dir, path) = temp();
        Session::create(&path, &project_with_pages(5))
            .unwrap()
            .close()
            .unwrap();
        // Staggered kill moments: some land in startup, most in the loop.
        run_child(
            "child_save_loop",
            &path,
            Duration::from_millis(250 + round * 37),
        );
        let opened = Session::open(&path)
            .unwrap_or_else(|e| panic!("round {round}: the file after the kill: {e}"));
        let json = opened.project.to_json();
        assert!(
            json == a || json == b,
            "round {round}: neither the old nor the new save"
        );
        opened.session.close().unwrap();
        saves_seen += usize::from(save_seq(&path) > 1);
    }
    assert!(
        saves_seen >= 5,
        "the child must have been saving when it was killed ({saves_seen} of 10 rounds)"
    );
}

/// Child: opens a project through the app backend, edits it, writes the
/// autosave and dies without closing.
#[test]
fn child_autosave_then_die() {
    let Some(path) = std::env::var_os("KOPERCAD_CRASH_CHILD_FILE") else {
        return;
    };
    let recovery = std::env::var_os("KOPERCAD_CRASH_CHILD_RECOVERY").unwrap();
    let opened = Session::open_in(&path, Some(Path::new(&recovery))).unwrap();
    let mut session = opened.session;
    let mut doc = Document::new(opened.project);
    let (_, changes) = doc
        .transact("Added after the last save", |tx| {
            tx.add_page("CRASH").map(|_| ())
        })
        .unwrap();
    session.record(&changes);
    session.autosave(doc.project()).unwrap();
    // No close, no drop: the process ends as a crash does.
    std::process::abort();
}

#[test]
fn after_a_killed_process_the_app_offers_the_autosaved_work_again() {
    let (dir, path) = temp();
    let (project, _) = golden_project();
    Session::create(&path, &project).unwrap().close().unwrap();
    let saved_json = project.to_json();

    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["child_autosave_then_die", "--exact", "--nocapture"])
        .env("KOPERCAD_CRASH_CHILD_FILE", &path)
        .env("KOPERCAD_CRASH_CHILD_RECOVERY", dir.path().join("recovery"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.wait().unwrap();

    // The next start: the saved file is whole and unchanged, and the app
    // backend offers the work that was not saved.
    assert_eq!(ProjectFile::read(&path).unwrap().to_json(), saved_json);
    let mut backend = StorageBackend::default().with_recovery_dir(dir.path().join("recovery"));
    backend.open(&path).unwrap();
    let offer = backend.recovery_offer();
    assert!(offer.is_some(), "the relaunch offers recovery");
    backend.resolve_recovery(true).unwrap();
    let pages: Vec<String> = backend
        .document()
        .unwrap()
        .project()
        .pages()
        .map(|p| p.name.clone())
        .collect();
    assert!(pages.contains(&"CRASH".to_owned()), "{pages:?}");
    backend.close().unwrap();
}
