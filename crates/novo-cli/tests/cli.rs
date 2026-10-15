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
    let mut p = ScriptProject::create(&path, "Demo", false).unwrap();
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
fn export_never_overwrites_a_project() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let bytes = std::fs::read(&path).unwrap();
    let run = novo(&["export", "json", s(&path), "-o", s(&path)]);
    assert_eq!(run.code, exit::WRITE);
    assert!(run.err.contains("it is a project file"), "{}", run.err);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    // Another project, under its own name or renamed, and a recovery file.
    let other = tempfile::tempdir().unwrap();
    let plant = project(other.path(), false);
    let plant_bytes = std::fs::read(&plant).unwrap();
    let renamed = other.path().join("plant-backup.db");
    std::fs::copy(&plant, &renamed).unwrap();
    let recovery = other.path().join("plant.novo.autosave");
    for target in [&plant, &renamed, &recovery] {
        let run = novo(&["export", "json", s(&path), "-o", s(target)]);
        assert_eq!(run.code, exit::WRITE, "{}", target.display());
        let run = novo(&["export", "pdf", s(&path), "-o", s(target)]);
        assert_eq!(run.code, exit::WRITE, "{}", target.display());
    }
    assert_eq!(std::fs::read(&plant).unwrap(), plant_bytes);
    assert_eq!(std::fs::read(&renamed).unwrap(), plant_bytes);
    assert!(!recovery.exists());
}

#[test]
fn export_replaces_an_earlier_export_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let out = dir.path().join("demo.json");
    std::fs::write(&out, "old export").unwrap();
    let run = novo(&["export", "json", s(&path), "-o", s(&out)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(std::fs::read_to_string(&out).unwrap().starts_with('{'));
    let mut entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    entries.sort();
    assert_eq!(
        entries,
        ["demo.json", "demo.novo"],
        "no temporary file left"
    );

    // A failed write leaves the earlier export whole.
    let missing_dir = dir.path().join("no-such-dir").join("demo.json");
    let run = novo(&["export", "json", s(&path), "-o", s(&missing_dir)]);
    assert_eq!(run.code, exit::WRITE);
    assert!(std::fs::read_to_string(&out).unwrap().starts_with('{'));
}

#[test]
fn export_pdf_draws_every_page_or_the_ones_asked() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let out = dir.path().join("demo.pdf");
    let run = novo(&["export", "pdf", s(&path), "-o", s(&out)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    let all = std::fs::read(&out).unwrap();
    assert!(all.starts_with(b"%PDF-"));

    let one = dir.path().join("page2.pdf");
    let run = novo(&["export", "pdf", s(&path), "-o", s(&one), "--page", "2"]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    let page2 = std::fs::read(&one).unwrap();
    assert!(page2.starts_with(b"%PDF-"));
    assert!(page2.len() < all.len(), "one page is smaller than two");

    let run = novo(&["export", "pdf", s(&path), "-o", s(&one), "--page", "9"]);
    assert_eq!(run.code, exit::USAGE);
    assert!(run.err.contains("no page `9`"));
}

#[test]
fn check_runs_the_design_checks() {
    let dir = tempfile::tempdir().unwrap();
    let clean = dir.path().join("clean.novo");
    let mut p = ScriptProject::create(&clean, "Clean", false).unwrap();
    p.add_page("1", "").unwrap();
    p.add_device("-K1", "Not drawn yet", "general").unwrap();
    p.save().unwrap();
    p.close().unwrap();
    let run = novo(&["check", s(&clean)]);
    assert_eq!(run.code, exit::OK, "{}", run.out);
    assert!(run.out.contains("No issues"), "{}", run.out);
    assert!(
        run.out.contains("info    No part: -K1"),
        "infos are listed: {}",
        run.out
    );

    let other = tempfile::tempdir().unwrap();
    let dup = project(other.path(), true);
    let run = novo(&["check", s(&dup)]);
    assert_eq!(run.code, exit::PROBLEMS);
    assert!(run.out.contains("error   Duplicate tag: "), "{}", run.out);
    assert!(
        run.out
            .contains("warning Unconnected point: -K1: A1 not connected (page 1)"),
        "{}",
        run.out
    );
    let run = novo(&["check", "--json", s(&dup)]);
    let v: serde_json::Value = serde_json::from_str(&run.out).unwrap();
    let kinds: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"duplicate_tag"), "{kinds:?}");
    assert!(kinds.contains(&"unconnected_pin"), "{kinds:?}");
    let dup_tag = v
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["kind"] == "duplicate_tag")
        .unwrap();
    assert_eq!(dup_tag["severity"], "error");
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

    let usage = novo(&["frobnicate"]);
    assert_eq!(usage.code, exit::USAGE);
    assert!(!usage.err.contains('\u{1b}'), "no terminal escape codes");
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
    let mut p = ScriptProject::create(&path, "Short", false).unwrap();
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
        run.out.contains("error   Potential conflict: L1 and L2"),
        "{}",
        run.out
    );
}

#[test]
fn export_reports_as_excel_and_csv() {
    use calamine::{Reader, Xlsx, open_workbook};

    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);

    let run = novo(&["export", "devices", s(&path)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(!run.out.starts_with('\u{feff}'), "no BOM on stdout");
    assert!(run.out.contains("'-K1"), "{}", run.out);

    let csv = dir.path().join("bom.csv");
    let run = novo(&["export", "bom", s(&path), "-o", s(&csv), "--delimiter", ";"]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(text.starts_with('\u{feff}'), "Excel reads mm² right");
    assert!(text.contains(';'));

    let all = dir.path().join("reports.xlsx");
    let run = novo(&["export", "reports", s(&path), "-o", s(&all)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    let book: Xlsx<_> = open_workbook(&all).unwrap();
    assert_eq!(
        book.sheet_names(),
        [
            "Table of contents",
            "Bill of materials",
            "Device list",
            "Wire list",
            "Terminal diagram",
            "Cable diagram",
            "IO list"
        ]
    );

    let wrong = dir.path().join("reports.csv");
    assert_eq!(
        novo(&["export", "reports", s(&path), "-o", s(&wrong)]).code,
        exit::USAGE
    );
    let txt = dir.path().join("bom.txt");
    assert_eq!(
        novo(&["export", "bom", s(&path), "-o", s(&txt)]).code,
        exit::USAGE
    );
    let run = novo(&["export", "wires", s(&path), "-o", s(&path)]);
    assert_eq!(run.code, exit::WRITE);
    assert!(run.err.contains("it is a project file"));
}
