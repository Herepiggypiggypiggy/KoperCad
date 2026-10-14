//! The `novo` tool, driven through `novo_cli::run` like the binary.

use std::path::{Path, PathBuf};

use novo_cli::exit;
use novo_py::{Role, ScriptProject};

struct Run {
    code: i32,
    out: String,
    err: String,
}

fn novo(args: &[&str]) -> Run {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = novo_cli::run(
        std::iter::once("novo").chain(args.iter().copied()),
        &mut out,
        &mut err,
    );
    Run {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

/// A saved two-page project; with `duplicate`, two devices share `-K1`.
fn project(dir: &Path, duplicate: bool) -> PathBuf {
    let path = dir.join("demo.novo");
    let mut p = ScriptProject::create(&path, "Demo").unwrap();
    let page = p.add_page("1", "Main circuit").unwrap();
    p.add_page("2", "Control").unwrap();
    let k1 = p.add_device("-K1", "Contactor", "general").unwrap();
    let coil = p
        .place(
            page,
            "coil",
            0.0,
            0.0,
            0,
            false,
            Role::Device {
                device: k1,
                main: true,
            },
        )
        .unwrap();
    let no = p
        .place(page, "no", 0.0, 40.0, 0, false, Role::Unassigned)
        .unwrap();
    p.connect((coil, "A2"), (no, "13")).unwrap();
    if duplicate {
        p.add_device("-K1", "Second", "general").unwrap();
    }
    p.save().unwrap();
    p.close().unwrap();
    path
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn info_reads_without_touching_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let bytes = std::fs::read(&path).unwrap();

    let run = novo(&["info", s(&path)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(run.out.contains("Project:     Demo"));
    assert!(run.out.contains("  1  Main circuit"));
    assert!(run.out.contains("Wires:       1"));

    let run = novo(&["info", "--json", s(&path)]);
    assert_eq!(run.code, exit::OK);
    let v: serde_json::Value = serde_json::from_str(&run.out).unwrap();
    assert_eq!(v["devices"], 1);
    assert_eq!(v["page_list"][1]["title"], "Control");

    assert_eq!(
        std::fs::read(&path).unwrap(),
        bytes,
        "project file unchanged"
    );
    let mut entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    entries.sort();
    assert_eq!(entries, ["demo.novo"], "nothing else written");
}

#[test]
fn export_json_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let out = dir.path().join("demo.json");
    let run = novo(&["export", "json", s(&path), "-o", s(&out)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    let text = std::fs::read_to_string(&out).unwrap();
    let loaded = novo_core::Project::from_json(&text).unwrap();
    assert_eq!(loaded.devices().count(), 1);

    let run = novo(&["export", "json", s(&path)]);
    assert_eq!(run.code, exit::OK);
    assert_eq!(run.out.trim_end(), text);
}

#[test]
fn export_never_overwrites_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let bytes = std::fs::read(&path).unwrap();
    let run = novo(&["export", "json", s(&path), "-o", s(&path)]);
    assert_eq!(run.code, exit::WRITE);
    assert!(run.err.contains("refusing to overwrite"));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn export_pdf_says_it_is_not_here_yet() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let out = dir.path().join("demo.pdf");
    let run = novo(&["export", "pdf", s(&path), "-o", s(&out)]);
    assert_eq!(run.code, exit::UNAVAILABLE);
    assert!(run.err.contains("novo-pdf"));
    assert!(!out.exists());
}

#[test]
fn check_reports_duplicate_tags() {
    let dir = tempfile::tempdir().unwrap();
    let clean = project(dir.path(), false);
    let run = novo(&["check", s(&clean)]);
    assert_eq!(run.code, exit::OK, "{}", run.out);
    assert!(run.out.contains("No problems"));

    let other = tempfile::tempdir().unwrap();
    let dup = project(other.path(), true);
    let run = novo(&["check", s(&dup)]);
    assert_eq!(run.code, exit::PROBLEMS);
    assert!(run.out.contains("duplicate_tag: -K1 is used by 2 objects"));
    let run = novo(&["check", "--json", s(&dup)]);
    let v: serde_json::Value = serde_json::from_str(&run.out).unwrap();
    assert_eq!(v[0]["check"], "duplicate_tag");
}

#[test]
fn bad_input_and_usage_have_their_own_codes() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing.novo");
    let run = novo(&["info", s(&missing)]);
    assert_eq!(run.code, exit::READ);
    assert!(run.err.starts_with("novo: cannot read"));
    assert!(!missing.exists(), "reading never creates the file");

    let junk = dir.path().join("junk.novo");
    std::fs::write(&junk, b"not a database").unwrap();
    assert_eq!(novo(&["info", s(&junk)]).code, exit::READ);

    assert_eq!(novo(&["frobnicate"]).code, exit::USAGE);
    assert_eq!(novo(&["export", "pdf", "x.novo"]).code, exit::USAGE);
    let help = novo(&["--help"]);
    assert_eq!(help.code, exit::OK);
    assert!(help.out.contains("Exit codes"));
}

#[test]
fn a_file_open_for_editing_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let open = ScriptProject::open(&path).unwrap();
    let run = novo(&["info", s(&path)]);
    assert_eq!(run.code, exit::READ);
    assert!(run.err.contains("already open"), "{}", run.err);
    open.close().unwrap();
}

#[test]
fn check_reports_shorts_between_potentials() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("short.novo");
    let mut p = ScriptProject::create(&path, "Short").unwrap();
    let page = p.add_page("1", "").unwrap();
    let l1 = p.add_potential("L1", "line", None).unwrap();
    let l2 = p.add_potential("L2", "line", None).unwrap();
    let a = p
        .place(page, "feed", 0.0, 0.0, 0, false, Role::Potential(l1))
        .unwrap();
    let b = p
        .place(page, "feed", 20.0, 0.0, 0, false, Role::Potential(l2))
        .unwrap();
    p.connect((a, "1"), (b, "1")).unwrap();
    p.save().unwrap();
    p.close().unwrap();

    let run = novo(&["check", s(&path)]);
    assert_eq!(run.code, exit::PROBLEMS);
    assert!(
        run.out
            .contains("potential_short: short between L1 and L2 at feed L1:1, feed L2:1"),
        "{}",
        run.out
    );
}
