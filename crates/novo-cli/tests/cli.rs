//! The `kopercad-cli` tool, driven through `novo_cli::run` like the binary.

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
        std::iter::once("kopercad-cli").chain(args.iter().copied()),
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
fn export_eplan_writes_a_pxf_file_and_never_replaces_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let target = dir.path().join("demo.epj");
    let run = novo(&["export", "eplan", s(&path), "-o", s(&target)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    let text = std::fs::read_to_string(&target).unwrap();
    assert!(text.contains("<EplanPxfRoot") && text.contains("PxfVersion=\"1.23\""));
    assert!(run.out.contains("2 pages"), "{}", run.out);
    assert!(
        run.out.contains("not been opened in a real EPLAN"),
        "{}",
        run.out
    );
    let again = novo(&["export", "eplan", s(&path), "-o", s(&target)]);
    assert_eq!(again.code, exit::WRITE, "{}", again.err);
}

#[test]
fn export_dxf_writes_one_file_or_one_per_page() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);

    let folder = dir.path().join("dxf");
    let run = novo(&["export", "dxf", s(&path), "-o", s(&folder)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    for name in ["1.dxf", "2.dxf"] {
        let text = std::fs::read_to_string(folder.join(name)).unwrap();
        assert!(text.contains("SECTION") && text.contains("EOF"), "{name}");
    }

    let one = dir.path().join("page1.dxf");
    let run = novo(&["export", "dxf", s(&path), "-o", s(&one), "--page", "1"]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert_eq!(
        std::fs::read(&one).unwrap(),
        std::fs::read(folder.join("1.dxf")).unwrap()
    );

    // Two pages cannot share one .dxf file.
    let run = novo(&["export", "dxf", s(&path), "-o", s(&one)]);
    assert_eq!(run.code, exit::USAGE);
    assert!(run.err.contains("--page"));
    let run = novo(&["export", "dxf", s(&path), "-o", s(&one), "--page", "9"]);
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
fn export_wires_and_cut_list_carry_cut_lengths() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);

    let run = novo(&["export", "wires", s(&path)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(
        run.out.contains("Route length mm,Length basis"),
        "{}",
        run.out
    );

    let run = novo(&["export", "cutlist", s(&path)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    // This project has no wire between two device pins, so no groups.
    assert!(run.out.is_empty(), "{}", run.out);

    // The wire production job file is always there, with its header.
    let run = novo(&["export", "wirecut", s(&path)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(
        run.out
            .starts_with("No,Wire no.,Qty,mm2,Colour,Cut mm,Wire mm,"),
        "{}",
        run.out
    );
}

#[test]
fn export_labels_in_two_layouts() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);

    let run = novo(&["export", "labels", s(&path), "--kind", "devices"]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(run.out.starts_with("Line 1,Line 2,Line 3"), "{}", run.out);
    assert!(run.out.contains("Contactor"), "{}", run.out);

    let run = novo(&[
        "export",
        "labels",
        s(&path),
        "--kind",
        "devices",
        "--layout",
        "brother",
    ]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(
        run.out.starts_with("'-K1\tContactor\t") || run.out.starts_with("-K1\tContactor\t"),
        "{}",
        run.out
    );

    let run = novo(&["export", "labels", s(&path), "--layout", "zebra"]);
    assert_eq!(run.code, exit::USAGE);
    let run = novo(&["export", "labels", s(&path), "--kind", "pens"]);
    assert_eq!(run.code, exit::USAGE);
}

#[test]
fn reports_print_in_the_chosen_language() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);

    let english = novo(&["export", "devices", s(&path)]);
    assert_eq!(english.code, exit::OK, "{}", english.err);
    assert!(english.out.contains("Description"), "{}", english.out);

    let dutch = novo(&["export", "devices", s(&path), "--language", "nl"]);
    assert_eq!(dutch.code, exit::OK, "{}", dutch.err);
    assert!(dutch.out.contains("Omschrijving"), "{}", dutch.out);
    assert!(!dutch.out.contains("Description"));
    assert!(dutch.out.contains("'-K1"), "cell values stay as they are");

    let bad = novo(&["export", "devices", s(&path), "--language", "xx"]);
    assert_eq!(bad.code, exit::USAGE);

    let commissioning = novo(&["commissioning-report", s(&path), "--language", "nl"]);
    assert_eq!(commissioning.code, exit::OK, "{}", commissioning.err);
    let bad = novo(&["commissioning-report", s(&path), "--language", "xx"]);
    assert_eq!(
        bad.code,
        exit::USAGE,
        "unknown code is a usage error here too"
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
            "Cut list",
            "Wire production",
            "Terminal diagram",
            "Cable diagram",
            "IO list",
            "Cable schedule",
            "Plug connection list",
            "Labels",
            "Single-line diagram"
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

/// A project with revisions A and B (a device description changed in
/// between) and a rename after B, saved in `dir`.
fn revised(dir: &Path) -> PathBuf {
    use novo_core::{Device, DeviceTag, Document};
    use novo_revisions::{IndexRule, issue_revision};
    use novo_storage::Session;

    let path = dir.join("revised.novo");
    let mut doc = Document::default();
    let (k1, _) = doc
        .transact("Draw", |tx| {
            tx.add_page("1")?;
            tx.add_device(DeviceTag::parse("-K1").unwrap(), "Relay")
        })
        .unwrap();
    let mut session = Session::create(&path, doc.project()).unwrap();
    let rule = IndexRule::Letters;
    issue_revision(&mut doc, &mut session, rule, "2026-10-15", "TV", "First").unwrap();
    let (_, changes) = doc
        .transact("Edit", |tx| {
            tx.update::<Device>(k1, |d| d.description = "Pump relay".into())
        })
        .unwrap();
    session.record(&changes);
    issue_revision(&mut doc, &mut session, rule, "2026-10-16", "TV", "Second").unwrap();
    let (_, changes) = doc
        .transact("Rename", |tx| {
            tx.update::<Device>(k1, |d| d.description = "Pump relay 2".into())
        })
        .unwrap();
    session.record(&changes);
    session.save(doc.project()).unwrap();
    session.close().unwrap();
    path
}

#[test]
fn revisions_list_and_diff() {
    let dir = tempfile::tempdir().unwrap();
    let file = revised(dir.path());
    let file = file.to_str().unwrap();
    let before = std::fs::read(file).unwrap();

    let list = novo(&["revisions", "list", file]);
    assert_eq!(list.code, exit::OK, "{}", list.err);
    assert!(
        list.out.contains("A  2026-10-15  TV  First"),
        "{}",
        list.out
    );
    assert!(
        list.out.contains("B  2026-10-16  TV  Second"),
        "{}",
        list.out
    );

    let diff = novo(&["revisions", "diff", file, "A", "B"]);
    assert_eq!(diff.code, exit::OK, "{}", diff.err);
    assert!(diff.out.contains("changed Device -K1"), "{}", diff.out);
    assert!(
        diff.out.contains("description: Relay -> Pump relay"),
        "{}",
        diff.out
    );

    let now = novo(&["revisions", "diff", file, "B", "now", "--json"]);
    assert_eq!(now.code, exit::OK, "{}", now.err);
    let json: serde_json::Value = serde_json::from_str(&now.out).unwrap();
    assert_eq!(json["rows"][0]["label"], "Device -K1");
    assert_eq!(json["rows"][0]["fields"][0]["new"], "Pump relay 2");

    let same = novo(&["revisions", "diff", file, "B", "B"]);
    assert_eq!(same.code, exit::OK);
    assert!(same.out.contains("No differences"), "{}", same.out);

    let unknown = novo(&["revisions", "diff", file, "Q", "now"]);
    assert_eq!(unknown.code, exit::USAGE);
    assert!(unknown.err.contains("no revision `Q`"), "{}", unknown.err);

    // Read only: the file is unchanged.
    assert_eq!(std::fs::read(file).unwrap(), before);
}

#[test]
fn commissioning_report_writes_pdf_csv_and_xlsx() {
    let dir = tempfile::tempdir().unwrap();
    let file = project(dir.path(), false);
    let file = file.to_str().unwrap();

    // Nothing ticked yet: every check is pending, and the report says so.
    let csv = novo(&["commissioning-report", file]);
    assert_eq!(csv.code, exit::OK, "{}", csv.err);
    assert!(csv.out.contains("Progress per cabinet"), "{}", csv.out);
    assert!(csv.out.contains("Pending"));
    assert!(csv.out.contains("Customer"));

    for name in ["report.pdf", "report.xlsx", "report.csv"] {
        let target = dir.path().join(name);
        let run = novo(&["commissioning-report", file, "-o", target.to_str().unwrap()]);
        assert_eq!(run.code, exit::OK, "{name}: {}", run.err);
        assert!(std::fs::metadata(&target).unwrap().len() > 0, "{name}");
    }
    let pdf = std::fs::read(dir.path().join("report.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF"));

    let bad = novo(&[
        "commissioning-report",
        file,
        "-o",
        dir.path().join("report.txt").to_str().unwrap(),
    ]);
    assert_eq!(bad.code, exit::USAGE);
}

#[test]
fn export_terminals_writes_the_strip_diagram_with_wire_and_bridge_columns() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("strip.novo");
    let mut p = ScriptProject::create(&path, "Strip", false).unwrap();
    p.add_page("1", "Main").unwrap();
    let x1 = p.add_device("-X1", "Strip", "terminal_strip").unwrap();
    p.add_terminal(x1, "1").unwrap();
    p.save().unwrap();
    p.close().unwrap();

    let run = novo(&["export", "terminals", s(&path)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(
        run.out
            .contains("Internal,Wire in,Terminal,Kind,Bridged to,Wire out,External"),
        "{}",
        run.out
    );
}

#[test]
fn export_plc_writes_the_address_plan_and_refuses_what_it_cannot() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let out = dir.path().join("plc");

    let run = novo(&["export", "plc", s(&path), "-o", s(&out)]);
    assert_eq!(run.code, exit::USAGE, "{}", run.err);
    assert!(run.err.contains("no PLC cards"), "{}", run.err);

    let mut p = ScriptProject::open(&path).unwrap();
    p.add_device("-K9", "CPU", "plc").unwrap();
    p.save().unwrap();
    p.close().unwrap();

    let run = novo(&[
        "export",
        "plc",
        s(&path),
        "--format",
        "hardware",
        "-o",
        s(&out),
    ]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    let text = std::fs::read_to_string(out.join("demo PLC address plan.csv")).unwrap();
    assert!(text.contains("Station;Slot;Card;Channels"), "{text}");
    assert!(text.contains("-K9"), "{text}");

    let run = novo(&["export", "plc", s(&path), "--format", "tia", "-o", s(&out)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);

    let run = novo(&["export", "plc", s(&path), "--format", "s7", "-o", s(&out)]);
    assert_eq!(run.code, exit::USAGE);
    assert!(run.err.contains("unknown format"), "{}", run.err);
}

#[test]
fn calc_judges_circuits_and_exits_one_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let sample =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/protection-sample.json");
    let r = novo(&["calc", sample.to_str().unwrap()]);
    assert_eq!(r.code, exit::PROBLEMS, "{}", r.err);
    assert!(r.out.contains("Circuit Q1"));
    assert!(r.out.contains("FAIL    Voltage drop"));
    assert!(r.out.contains("UNKNOWN Short-circuit (maximum)"));
    assert!(r.out.contains("not checked against the standard"));

    // JSON output parses and carries the statuses.
    let j = novo(&["calc", sample.to_str().unwrap(), "--json"]);
    let v: serde_json::Value = serde_json::from_str(&j.out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 3);
    assert_eq!(v[2]["rows"][1]["status"], "fail");

    // A licensed table is reported as such, and a bad file is a usage error.
    let csv = dir.path().join("tables.csv");
    std::fs::write(&csv, "ampacity,PVC,C,3,2.5,24\n").unwrap();
    let l = novo(&[
        "calc",
        sample.to_str().unwrap(),
        "--tables",
        csv.to_str().unwrap(),
    ]);
    assert!(l.out.contains("licensed copy tables.csv"));
    std::fs::write(&csv, "ampacity,PVC,C,3,2.5\n").unwrap();
    let bad = novo(&[
        "calc",
        sample.to_str().unwrap(),
        "--tables",
        csv.to_str().unwrap(),
    ]);
    assert_eq!(bad.code, exit::USAGE);
    assert!(bad.err.contains("line 1"));

    // Maker back-up table turns the failed breaking capacity into a pass.
    let pairs = dir.path().join("pairs.csv");
    std::fs::write(&pairs, "backup,MCCB160,MCB16C10,25\n").unwrap();
    let mut spec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&sample).unwrap()).unwrap();
    spec["circuits"][0]["device"]["id"] = "MCCB160".into();
    spec["circuits"][1]["device"]["id"] = "MCB16C10".into();
    let file = dir.path().join("plant.json");
    std::fs::write(&file, spec.to_string()).unwrap();
    let b = novo(&[
        "calc",
        file.to_str().unwrap(),
        "--pairs",
        pairs.to_str().unwrap(),
    ]);
    assert!(b.out.contains("backed up by Q1"), "{}", b.out);
}

#[test]
fn export_plate_writes_the_plate_pdf_of_the_enclosure() {
    use novo_core::{DeviceTag, MountingPlate, Project};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cab.novo");
    let mut p = Project::new("Cab");
    p.transact("setup", |tx| {
        let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
        novo_layout::model::set_plate(
            tx,
            cab,
            MountingPlate {
                width: 400.0,
                height: 400.0,
                margin: 10.0,
            },
        )
    })
    .unwrap();
    novo_storage::ProjectFile::create(&path, &p).unwrap();

    let out = dir.path().join("plate.pdf");
    let run = novo(&["export", "plate", s(&path), "-o", s(&out)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(std::fs::read(&out).unwrap().starts_with(b"%PDF"));

    let run = novo(&[
        "export",
        "plate",
        s(&path),
        "-o",
        s(&out),
        "--enclosure",
        "+NOPE",
    ]);
    assert_eq!(run.code, exit::USAGE);
    assert!(run.err.contains("no enclosure"), "{}", run.err);
}

/// A saved project with a cabinet: an enclosure with a plate and two devices.
fn cabinet_project(dir: &Path) -> PathBuf {
    use novo_core::{DeviceTag, MountingPlate, Project};
    let path = dir.join("cab.novo");
    let mut p = Project::new("Cab");
    p.transact("setup", |tx| {
        let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 600.0,
                height: 800.0,
                margin: 10.0,
            }),
        )?;
        tx.add_device(DeviceTag::parse("=A1+CAB1-K1").unwrap(), "Contactor")?;
        Ok(())
    })
    .unwrap();
    novo_storage::ProjectFile::create(&path, &p)
        .unwrap()
        .close()
        .unwrap();
    path
}

#[test]
fn export_step_writes_the_cabinet_and_says_what_it_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let path = cabinet_project(dir.path());
    let out = dir.path().join("cab.stp");
    let run = novo(&["export", "step", s(&path), "-o", s(&out)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.starts_with("ISO-10303-21;"));
    assert!(text.contains("'Mounting plate'"));
    assert!(run.out.contains("solids"), "{}", run.out);

    // A directory gets one file per enclosure.
    let folder = dir.path().join("steps");
    let run = novo(&["export", "step", s(&path), "-o", s(&folder)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(folder.join("+CAB1.stp").is_file());

    // A tag that is not an enclosure is a usage error, not an empty file.
    let run = novo(&[
        "export",
        "step",
        s(&path),
        "-o",
        s(&out),
        "--enclosure",
        "+NOPE",
    ]);
    assert_eq!(run.code, exit::USAGE);
}

#[test]
fn export_step_needs_a_mounting_plate() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let run = novo(&[
        "export",
        "step",
        s(&path),
        "-o",
        s(&dir.path().join("x.stp")),
    ]);
    assert_eq!(run.code, exit::USAGE);
    assert!(run.err.contains("mounting plate"), "{}", run.err);
}

#[test]
fn compare_says_when_two_projects_match_and_where_they_differ() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let one = project(a.path(), false);
    let same = project(b.path(), false);
    let run = novo(&["compare", s(&one), s(&same)]);
    assert_eq!(run.code, exit::OK, "{}{}", run.out, run.err);
    assert!(run.out.contains("Nothing differs"), "{}", run.out);
    let c = tempfile::tempdir().unwrap();
    let other = project(c.path(), true);
    let run = novo(&["compare", s(&one), s(&other)]);
    assert_eq!(run.code, exit::PROBLEMS, "{}{}", run.out, run.err);
    assert!(run.out.contains("device"), "{}", run.out);
}

#[test]
fn export_aml_writes_caex_and_lists_what_the_format_cannot_carry() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let out = dir.path().join("demo.aml");
    let run = novo(&["export", "aml", s(&path), "-o", s(&out)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("<CAEXFile"));
    assert!(text.contains("Name=\"-K1\""));
    assert!(run.out.contains("1 device"), "{}", run.out);
    assert!(run.out.contains("2 pages"), "{}", run.out);
    assert!(run.out.contains("page graphics"), "{}", run.out);
}

/// An edited export of `project`: the first device's description set to
/// `text`, saved as `edited.xlsx` next to it.
fn edited_sheet(project: &Path, text: &str) -> PathBuf {
    use novo_interop::sheets::{Cell, export_project, xlsx_bytes};
    let loaded = novo_storage::ProjectFile::read(project).unwrap();
    let mut book = export_project(&loaded);
    let col = book.tables[0].column("Description").unwrap();
    book.tables[0].rows[0].cells[col] = Cell::text(text);
    let path = project.with_file_name("edited.xlsx");
    std::fs::write(&path, xlsx_bytes(&book).unwrap()).unwrap();
    path
}

#[test]
fn import_sheet_previews_and_never_touches_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let sheet = edited_sheet(&path, "Main contactor");
    let before = std::fs::read(&path).unwrap();
    let r = novo(&["import", "sheet", s(&path), s(&sheet)]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    assert!(
        r.out.contains("Description = Contactor -> Main contactor"),
        "{}",
        r.out
    );
    assert!(r.out.contains("preview only"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let r = novo(&["import", "sheet", s(&path), s(&sheet), "--json"]);
    let v: serde_json::Value = serde_json::from_str(&r.out).unwrap();
    assert_eq!(v["changes"][0]["after"], "Main contactor");
    assert_eq!(v["written"], false);
}

#[test]
fn import_sheet_output_writes_a_new_project_and_never_replaces_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let sheet = edited_sheet(&path, "Main contactor");
    let before = std::fs::read(&path).unwrap();
    let out_path = dir.path().join("imported.novo");
    let r = novo(&["import", "sheet", s(&path), s(&sheet), "-o", s(&out_path)]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    let imported = novo_storage::ProjectFile::read(&out_path).unwrap();
    assert_eq!(
        imported.devices().next().unwrap().description,
        "Main contactor"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    // Over the input, over an existing file, or to a non-project name.
    for target in [path.clone(), out_path.clone(), dir.path().join("x.txt")] {
        let r = novo(&["import", "sheet", s(&path), s(&sheet), "-o", s(&target)]);
        assert_eq!(r.code, exit::WRITE, "{}", target.display());
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn import_sheet_skip_and_rejected_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let sheet = edited_sheet(&path, "Main contactor");
    let out_path = dir.path().join("skipped.novo");
    let r = novo(&[
        "import",
        "sheet",
        s(&path),
        s(&sheet),
        "-o",
        s(&out_path),
        "--skip",
        "Devices:2",
    ]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    assert!(r.out.contains("skipped:"), "{}", r.out);
    assert!(r.out.contains("no file written"));
    assert!(!out_path.exists());
    let r = novo(&["import", "sheet", s(&path), s(&sheet), "--skip", "nonsense"]);
    assert_eq!(r.code, exit::USAGE);

    let csv = dir.path().join("gone.csv");
    std::fs::write(&csv, "Id,Tag,Description\n999999,-K9,Ghost\n").unwrap();
    let r = novo(&["import", "sheet", s(&path), s(&csv)]);
    assert_eq!(r.code, exit::PROBLEMS, "{}{}", r.out, r.err);
    assert!(r.out.contains("rejected: Devices row"), "{}", r.out);
    let r = novo(&[
        "import",
        "sheet",
        s(&path),
        s(&dir.path().join("none.xlsx")),
    ]);
    assert_eq!(r.code, exit::READ);
}

#[test]
fn import_sheet_lists_and_writes_new_devices() {
    use novo_interop::sheets::{Cell, export_project, xlsx_bytes};
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let loaded = novo_storage::ProjectFile::read(&path).unwrap();
    let mut book = export_project(&loaded);
    let table = &mut book.tables[0];
    let mut cells: Vec<Cell> = table.headers.iter().map(|_| Cell::text("")).collect();
    cells[table.column("Tag").unwrap()] = Cell::text("-M1");
    cells[table.column("Description").unwrap()] = Cell::text("Pump motor");
    table.push(cells);
    let sheet = dir.path().join("new.xlsx");
    std::fs::write(&sheet, xlsx_bytes(&book).unwrap()).unwrap();
    let out_path = dir.path().join("with-motor.novo");
    let r = novo(&["import", "sheet", s(&path), s(&sheet), "-o", s(&out_path)]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    assert!(r.out.contains("new: Devices row"), "{}", r.out);
    let imported = novo_storage::ProjectFile::read(&out_path).unwrap();
    assert!(imported.devices().any(|d| d.description == "Pump motor"));
}

fn pxf_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../novo-interop/tests/fixtures/pxf/synthetic-small.epj")
}

#[test]
fn import_eplan_previews_with_a_counted_not_carried_list_and_writes_nothing() {
    let pxf = pxf_fixture();
    let r = novo(&["import", "eplan", s(&pxf)]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    assert!(r.out.contains("2 pages"), "{}", r.out);
    assert!(r.out.contains("symbol drawings"), "{}", r.out);
    let r = novo(&["import", "eplan", s(&pxf), "--json"]);
    assert!(r.out.contains("\"not_carried\""), "{}", r.out);
    assert!(r.out.contains("\"guessed\""), "{}", r.out);
}

fn crane_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../novo-interop/tests/fixtures/crane/output")
}

#[test]
fn import_crane_reads_the_newest_version_and_quotes_the_configurator() {
    let r = novo(&["import", "crane", s(&crane_fixture())]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    assert!(r.out.contains("version 2"), "{}", r.out);
    assert!(r.out.contains("GOED MET OPMERKINGEN"), "{}", r.out);
    assert!(r.out.contains("generated report pages"), "{}", r.out);
    assert!(r.out.contains("Inferred"), "{}", r.out);
}

#[test]
fn import_crane_output_writes_a_new_project_and_never_replaces_one() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("crane.novo");
    let r = novo(&["import", "crane", s(&crane_fixture()), "-o", s(&target)]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    let imported = novo_storage::ProjectFile::read(&target).unwrap();
    assert_eq!(imported.cables().count(), 2);
    assert_eq!(imported.info().name, "DEMO-1 Demo slewing crane");
    let again = novo(&["import", "crane", s(&crane_fixture()), "-o", s(&target)]);
    assert_ne!(again.code, exit::OK);
}

#[test]
fn import_crane_says_when_the_folder_holds_no_project() {
    let dir = tempfile::tempdir().unwrap();
    let r = novo(&["import", "crane", s(dir.path())]);
    assert_eq!(r.code, exit::READ);
    assert!(r.err.contains("no .epj file"), "{}", r.err);
}

#[test]
fn eplan_survey_prints_names_and_shapes_but_no_values() {
    let r = novo(&["eplan", "survey", s(&pxf_fixture())]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    assert!(r.out.contains("O17  x4"), "{}", r.out);
    assert!(r.out.contains("A762"), "{}", r.out);
    assert!(
        !r.out.contains("DEMO") && !r.out.contains("Breaker"),
        "{}",
        r.out
    );
}

#[test]
fn import_eplan_output_writes_a_new_project_and_never_replaces_one() {
    let dir = tempfile::tempdir().unwrap();
    let pxf = pxf_fixture();
    let target = dir.path().join("eplan.novo");
    let r = novo(&["import", "eplan", s(&pxf), "-o", s(&target)]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    let imported = novo_storage::ProjectFile::read(&target).unwrap();
    assert_eq!(imported.pages().count(), 2);
    assert_eq!(imported.cables().count(), 1);
    let again = novo(&["import", "eplan", s(&pxf), "-o", s(&target)]);
    assert_ne!(again.code, exit::OK);
}

#[test]
fn import_eplan_refuses_a_file_that_is_not_pxf() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("x.epj");
    std::fs::write(&bad, "<project/>").unwrap();
    let r = novo(&["import", "eplan", s(&bad)]);
    assert_eq!(r.code, exit::READ);
    assert!(r.err.contains("not a PXF file"), "{}", r.err);
}

#[test]
fn eplan_script_is_written_and_an_export_is_checked() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("script");
    let r = novo(&[
        "eplan",
        "script",
        "-o",
        target.to_str().unwrap(),
        "--show",
        "nl",
    ]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    assert!(r.out.contains("Stappen"));
    for name in [
        "KoperCAD_ExportFromEPLAN.cs",
        "README-EN.txt",
        "README-NL.txt",
        "kopercad-selftest.epj",
    ] {
        assert!(target.join(name).is_file(), "{name}");
    }
    // The self-test file passes the check; no manifest means unknown version.
    let r = novo(&[
        "eplan",
        "check",
        target.join("kopercad-selftest.epj").to_str().unwrap(),
    ]);
    assert_eq!(r.code, exit::OK, "{}", r.err);
    assert!(r.out.contains("self-test"));
    // An export folder from an older script is flagged.
    let export = dir.path().join("export");
    std::fs::create_dir(&export).unwrap();
    std::fs::copy(
        target.join("kopercad-selftest.epj"),
        export.join("project.epj"),
    )
    .unwrap();
    std::fs::write(
        export.join("kopercad-export.xml"),
        "<KoperCadEplanExport scriptVersion=\"0\"/>",
    )
    .unwrap();
    let r = novo(&["eplan", "check", export.to_str().unwrap()]);
    assert_eq!(r.code, exit::PROBLEMS);
    assert!(r.out.contains("Get the script again"));
    let r = novo(&[
        "eplan",
        "script",
        "-o",
        target.to_str().unwrap(),
        "--show",
        "fr",
    ]);
    assert_eq!(r.code, exit::USAGE);
}

#[test]
fn export_the_single_line_schedule() {
    let board = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/distribution-board.novo");
    let run = novo(&["export", "single-line", s(&board)]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(
        run.out.contains("'-Q1") || run.out.contains("=A1+CAB-Q1"),
        "{}",
        run.out
    );
    assert!(run.out.contains("Iz A"), "{}", run.out);
}
