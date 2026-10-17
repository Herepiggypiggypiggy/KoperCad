//! REL-COMPAT follow-ups: the golden file through the command line and the
//! scripting layer, files that cannot be written, and files with parts of
//! the database missing.

use std::path::{Path, PathBuf};

use novo_cli::exit;
use novo_py::ScriptProject;
use novo_storage::{ProjectFile, Session, StorageError};

const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/golden/v0.1-motor-starter.novo"
);
const GOLDEN_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/golden/v0.1-motor-starter.json"
);

fn golden_copy() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Straße ü.novo");
    std::fs::copy(GOLDEN, &path).unwrap();
    (dir, path)
}

fn expected_json() -> String {
    std::fs::read_to_string(GOLDEN_JSON).unwrap()
}

fn cli(args: &[&str]) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = novo_cli::run(
        std::iter::once("kopercad-cli").chain(args.iter().copied()),
        &mut out,
        &mut err,
    );
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn set_user_version(path: &Path, version: u32) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "user_version", version).unwrap();
}

// ------------------------------------------------------------ CLI, scripts

#[test]
fn the_command_line_reads_the_golden_file_and_leaves_it_alone() {
    let (dir, path) = golden_copy();
    let before = std::fs::read(&path).unwrap();
    let (code, out, err) = cli(&["info", "--json", s(&path)]);
    assert_eq!(code, exit::OK, "{err}");
    let info: serde_json::Value = serde_json::from_str(&out).unwrap();
    let golden: serde_json::Value = serde_json::from_str(&expected_json()).unwrap();
    assert_eq!(
        info["page_list"].as_array().unwrap().len(),
        golden["pages"].as_array().unwrap().len()
    );
    assert_eq!(info["devices"], golden["devices"].as_array().unwrap().len());

    let json_out = dir.path().join("out.json");
    let (code, _, err) = cli(&["export", "json", s(&path), "-o", s(&json_out)]);
    assert_eq!(code, exit::OK, "{err}");
    assert!(std::fs::metadata(&json_out).unwrap().len() > 1000);
    assert_eq!(std::fs::read(&path).unwrap(), before, "never writes");
}

#[test]
fn the_command_line_refuses_a_newer_file_in_words() {
    let (dir, path) = golden_copy();
    set_user_version(&path, 99);
    let before = std::fs::read(&path).unwrap();
    for args in [vec!["info"], vec!["export", "json"]] {
        let mut args: Vec<&str> = args;
        args.push(s(&path));
        let (code, _, err) = cli(&args);
        assert_ne!(code, exit::OK);
        assert!(err.contains("newer version of KoperCAD"), "{err}");
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn the_scripting_layer_opens_the_golden_file_and_refuses_a_newer_one() {
    // `import kopercad` calls these same functions.
    let (_dir, path) = golden_copy();
    let script = ScriptProject::open(&path).unwrap();
    assert_eq!(script.project().to_json(), expected_json());
    script.close().unwrap();

    set_user_version(&path, 99);
    let err = ScriptProject::open(&path).map(|_| ()).unwrap_err();
    assert!(
        err.to_string().contains("newer version of KoperCAD"),
        "{err}"
    );
}

// ----------------------------------------------------- files not writable

/// Makes `path` read-only and reports whether that stops this process from
/// writing it (it does not for root, where the test then has nothing to say).
fn lock_down(path: &Path) -> bool {
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(path, perms).unwrap();
    std::fs::OpenOptions::new().append(true).open(path).is_err()
}

#[test]
fn a_read_only_file_is_refused_for_editing_in_words_but_reads_fine() {
    let (_dir, path) = golden_copy();
    if !lock_down(&path) {
        eprintln!("running as a user that ignores file permissions: skipped");
        return;
    }
    let before = std::fs::read(&path).unwrap();
    // Reading (CLI, Open read-only) works on read-only media.
    assert_eq!(ProjectFile::read(&path).unwrap().to_json(), expected_json());
    // Editing says what is wrong and what to do.
    for err in [
        Session::open(&path).map(|_| ()).unwrap_err(),
        ProjectFile::open(&path).map(|_| ()).unwrap_err(),
    ] {
        assert!(matches!(err, StorageError::ReadOnly(_)), "{err:?}");
        let text = err.to_string();
        assert!(
            text.contains("read-only") && text.contains("Save as"),
            "{text}"
        );
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn a_file_in_a_read_only_folder_is_refused_for_editing_without_creating_files() {
    // Judged from permission bits, so this holds for root too.
    let (dir, path) = golden_copy();
    let folder = dir.path();
    let mut perms = std::fs::metadata(folder).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(folder, perms.clone()).unwrap();
    let err = Session::open(&path).map(|_| ()).unwrap_err();
    assert!(matches!(err, StorageError::ReadOnly(_)), "{err:?}");
    assert_eq!(ProjectFile::read(&path).unwrap().to_json(), expected_json());
    #[allow(clippy::permissions_set_readonly_false)]
    {
        perms.set_readonly(false);
        std::fs::set_permissions(folder, perms).unwrap();
    }
    let names: Vec<_> = std::fs::read_dir(folder).unwrap().collect();
    assert_eq!(names.len(), 1, "nothing was created next to the project");
}

#[test]
fn opening_for_editing_creates_no_probe_file_next_to_the_project() {
    let (dir, path) = golden_copy();
    let opened = Session::open(&path).unwrap();
    let during: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    opened.session.close().unwrap();
    assert!(
        during.iter().all(|n| !n.contains("writetest")),
        "{during:?}"
    );
}

// ------------------------------------------------------ missing pieces

fn drop_tables(path: &Path, tables: &[&str]) {
    let conn = rusqlite::Connection::open(path).unwrap();
    for t in tables {
        conn.execute_batch(&format!("DROP TABLE {t};")).unwrap();
    }
}

#[test]
fn a_file_missing_its_snapshot_or_sidecar_table_still_opens_and_saves() {
    for missing in [
        vec!["snapshots"],
        vec!["sidecar"],
        vec!["snapshots", "sidecar"],
    ] {
        let (_dir, path) = golden_copy();
        drop_tables(&path, &missing);
        // Reading never writes, and still gives the drawing.
        assert_eq!(ProjectFile::read(&path).unwrap().to_json(), expected_json());
        let opened = Session::open(&path).unwrap_or_else(|e| panic!("{missing:?}: {e}"));
        assert_eq!(opened.project.to_json(), expected_json(), "{missing:?}");
        let mut session = opened.session;
        let kept = (!missing.contains(&"sidecar")).then_some("{\"checks\":[],\"punch\":[]}");
        assert_eq!(
            session.sidecar("commissioning").unwrap().as_deref(),
            kept,
            "{missing:?}"
        );
        session.put_sidecar("commissioning", "{}").unwrap();
        session.save(&opened.project).unwrap();
        session.close().unwrap();
        let again = Session::open(&path).unwrap();
        assert_eq!(again.project.to_json(), expected_json());
        again.session.close().unwrap();
    }
}

#[test]
fn a_file_missing_its_core_tables_is_refused_in_words_never_a_panic() {
    for missing in ["records", "meta"] {
        let (_dir, path) = golden_copy();
        drop_tables(&path, &[missing]);
        let before = std::fs::read(&path).unwrap();
        for result in [
            Session::open(&path).map(|_| ()),
            ProjectFile::read(&path).map(|_| ()),
        ] {
            let err = result.expect_err(missing);
            let text = err.to_string();
            assert!(
                !text.is_empty() && text.contains("Straße"),
                "{missing}: the message names the file: {text}"
            );
        }
        assert_eq!(std::fs::read(&path).unwrap(), before, "{missing}");
    }
}

#[test]
fn save_as_into_a_missing_folder_does_not_claim_the_file_is_read_only() {
    let (dir, _) = golden_copy();
    let project = ProjectFile::read(GOLDEN).unwrap();
    let target = dir
        .path()
        .join("no such folder")
        .join("deeper")
        .join("x.novo");
    let err = ProjectFile::create(&target, &project)
        .map(|_| ())
        .unwrap_err();
    assert!(!matches!(err, StorageError::ReadOnly(_)), "{err:?}");
    assert!(!err.to_string().contains("read-only"), "{err}");
    assert!(!target.exists());
}
