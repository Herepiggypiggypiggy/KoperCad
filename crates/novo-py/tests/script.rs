//! The scripting API from Rust: the same calls the Python module makes.

use novo_core::{DeviceTag, Document};
use novo_py::{Role, ScriptError, ScriptProject};
use novo_storage::{Session, autosave_path};
use serde_json::Value;

fn net_with<'a>(nets: &'a Value, pin: &str) -> &'a Value {
    let found: Vec<&Value> = nets
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["pins"].as_array().unwrap().iter().any(|p| p == pin))
        .collect();
    assert_eq!(found.len(), 1, "{pin} is in {} nets", found.len());
    found[0]
}

/// L1 -> -Q1 -> -K1 -> -M1, the coil on L/N: a small DOL circuit.
fn build(p: &mut ScriptProject) -> Result<(), ScriptError> {
    let page = p.add_page("1", "Main circuit")?;
    let l1 = p.add_potential("L1", "line", Some(400.0))?;
    let l = p.add_potential("L", "line", Some(230.0))?;
    let n = p.add_potential("N", "neutral", None)?;
    let q1 = p.add_device("-Q1", "Breaker", "general")?;
    let k1 = p.add_device("-K1", "Contactor", "general")?;
    let m1 = p.add_device("-M1", "Motor", "general")?;
    let dev = |device| Role::Device { device, main: true };

    let feed = p.place(page, "feed", 40.0, 0.0, 0, false, Role::Potential(l1))?;
    let q1_s = p.place(page, "mcb3", 40.0, 20.0, 0, false, dev(q1))?;
    let k1_s = p.place(
        page,
        "iec/contactors/main_3p",
        40.0,
        60.0,
        0,
        false,
        dev(k1),
    )?;
    let m1_s = p.place(page, "m3", 40.0, 120.0, 0, false, dev(m1))?;
    p.connect((feed, "1"), (q1_s, "1"))?;
    p.connect((q1_s, "2"), (k1_s, "1"))?;
    p.connect((k1_s, "2"), (m1_s, "U1"))?;

    let lf = p.place(page, "feed", 120.0, 0.0, 0, false, Role::Potential(l))?;
    let nf = p.place(page, "feed", 120.0, 80.0, 0, false, Role::Potential(n))?;
    let coil = p.place(page, "coil", 120.0, 40.0, 0, false, dev(k1))?;
    p.connect((lf, "1"), (coil, "A1"))?;
    p.connect((coil, "A2"), (nf, "1"))?;
    Ok(())
}

#[test]
fn dol_circuit_renamed_saved_and_reopened() -> Result<(), ScriptError> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dol.novo");
    let mut p = ScriptProject::create(&path, "DOL", false)?;
    build(&mut p)?;
    let k1 = p.device_by_tag("-K1")?.unwrap();
    p.set_tag(k1, "=A1+CAB-K1")?;
    assert_eq!(p.undo_label(), Some("Rename device"));
    p.save()?;
    let before = p.nets();
    p.close()?;

    let p = ScriptProject::open(&path)?;
    assert_eq!(p.nets(), before);
    let nets = p.nets();
    assert_eq!(net_with(&nets, "-Q1:2")["potentials"][0], "L1");
    let motor = net_with(&nets, "-M1:U1");
    assert!(
        motor["pins"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "=A1+CAB-K1:2")
    );
    assert_eq!(motor["potentials"][0], "L1", "L1 passes the contactor");
    assert_eq!(net_with(&nets, "=A1+CAB-K1:A2")["potentials"][0], "N");
    assert!(
        net_with(&nets, "feed N:1")["pins"]
            .as_array()
            .unwrap()
            .len()
            == 2
    );
    assert_eq!(p.wires().as_array().unwrap().len(), 5);
    assert!(!p.can_undo());
    p.close()
}

#[test]
fn every_call_is_one_undo_step_and_saves_follow_undo() -> Result<(), ScriptError> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("undo.novo");
    let mut p = ScriptProject::create(&path, "Undo", false)?;
    p.add_page("1", "")?;
    let k1 = p.add_device("-K1", "", "general")?;
    p.set_tag(k1, "-K2")?;
    assert!(p.undo()?);
    assert!(p.undo()?);
    assert!(p.redo()?);
    // Saved state: one page, -K1 (rename undone).
    p.save()?;
    p.close()?;

    let p = ScriptProject::open(&path)?;
    assert_eq!(p.device_by_tag("-K1")?, Some(k1));
    assert_eq!(p.device_by_tag("-K2")?, None);
    assert_eq!(p.pages().as_array().unwrap().len(), 1);
    p.close()
}

#[test]
fn refused_calls_change_nothing() {
    let mut p = ScriptProject::new("Refusals");
    let page = p.add_page("1", "").unwrap();
    let fuse = p
        .place(page, "fuse", 0.0, 0.0, 0, false, Role::Unassigned)
        .unwrap();
    let before = p.to_json();
    assert!(matches!(
        p.place(page, "nope", 0.0, 0.0, 0, false, Role::Unassigned),
        Err(ScriptError::UnknownSymbol(_))
    ));
    assert!(matches!(
        p.place(page, "fuse", 0.0, 0.0, 45, false, Role::Unassigned),
        Err(ScriptError::UnknownValue { .. })
    ));
    assert!(matches!(
        p.connect((fuse, "1"), (fuse, "X9")),
        Err(ScriptError::UnknownPin { .. })
    ));
    assert!(matches!(
        p.add_device("K1 bad", "", "general"),
        Err(ScriptError::Tag(_))
    ));
    assert!(matches!(
        p.add_device("-K1", "", "robot"),
        Err(ScriptError::UnknownValue { .. })
    ));
    assert!(matches!(
        p.execute_json(r#"{"command": "fly"}"#),
        Err(ScriptError::Command(_))
    ));
    assert!(matches!(p.save(), Err(ScriptError::NoFile)));
    assert_eq!(p.to_json(), before);
    assert_eq!(p.undo_label(), Some("Place symbol"));
}

#[test]
fn json_commands_run_through_the_same_door() -> Result<(), ScriptError> {
    let mut p = ScriptProject::new("Json");
    let created = p.execute_json(
        r#"{"command": "batch", "label": "Two pages", "commands": [
            {"command": "add_page", "name": "1"},
            {"command": "add_page", "name": "2"}]}"#,
    )?;
    assert_eq!(created.len(), 2);
    assert_eq!(p.undo_label(), Some("Two pages"));
    p.undo()?;
    assert_eq!(p.pages().as_array().unwrap().len(), 0);
    Ok(())
}

#[test]
fn a_file_with_recovered_work_is_left_alone() -> Result<(), ScriptError> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crashed.novo");
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project())?;
    let (_, changes) = doc.transact("Add", |tx| {
        tx.add_device(DeviceTag::parse("-K1")?, "unsaved")
    })?;
    session.record(&changes);
    session.autosave(doc.project())?;
    drop(session); // the app "crashes": no close, the recovery file stays

    let recovery = autosave_path(&path);
    assert!(recovery.exists());
    assert!(matches!(
        ScriptProject::open(&path),
        Err(ScriptError::RecoveryPending(_))
    ));
    assert!(recovery.exists(), "the recovered work is kept for the app");
    Ok(())
}

/// Leaves `path` as the app does after a crash: saved once, then unsaved
/// work in the recovery file next to it.
fn crash_with_unsaved_work(path: &std::path::Path) -> Result<(), ScriptError> {
    let mut doc = Document::default();
    let mut session = Session::create(path, doc.project())?;
    let (_, changes) = doc.transact("Add", |tx| {
        tx.add_device(DeviceTag::parse("-K9")?, "unsaved")
    })?;
    session.record(&changes);
    session.autosave(doc.project())?;
    drop(session);
    assert!(autosave_path(path).exists());
    Ok(())
}

#[test]
fn create_and_save_as_never_replace_a_file_with_recovered_work() -> Result<(), ScriptError> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crashed.novo");
    crash_with_unsaved_work(&path)?;
    let saved = std::fs::read(&path).unwrap();

    assert!(matches!(
        ScriptProject::create(&path, "Generated", true),
        Err(ScriptError::RecoveryPending(_))
    ));
    let mut other = ScriptProject::create(dir.path().join("other.novo"), "Other", false)?;
    assert!(matches!(
        other.save_as(&path, true),
        Err(ScriptError::RecoveryPending(_))
    ));
    let mut fresh = ScriptProject::new("Fresh");
    assert!(matches!(
        fresh.save_as(&path, true),
        Err(ScriptError::RecoveryPending(_))
    ));

    assert!(autosave_path(&path).exists(), "the recovered work is kept");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        saved,
        "the file is untouched"
    );
    assert_eq!(other.path(), Some(dir.path().join("other.novo").as_path()));
    other.close()
}

#[test]
fn save_as_onto_its_own_file_still_saves() -> Result<(), ScriptError> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("own.novo");
    let mut p = ScriptProject::create(&path, "Own", false)?;
    p.add_page("1", "")?;
    p.save_as(&path, false)?;
    assert!(!p.is_dirty());
    p.close()?;
    let p = ScriptProject::open(&path)?;
    assert_eq!(p.pages().as_array().unwrap().len(), 1);
    p.close()
}

#[test]
fn an_existing_project_is_replaced_only_on_request() -> Result<(), ScriptError> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plant.novo");
    let mut real = ScriptProject::create(&path, "Real plant", false)?;
    real.add_page("1", "")?;
    real.save()?;
    real.close()?;
    let saved = std::fs::read(&path).unwrap();

    assert!(matches!(
        ScriptProject::create(&path, "Typo", false),
        Err(ScriptError::Exists(_))
    ));
    let mut other = ScriptProject::new("Other");
    assert!(matches!(
        other.save_as(&path, false),
        Err(ScriptError::Exists(_))
    ));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        saved,
        "the file is untouched"
    );

    let replaced = ScriptProject::create(&path, "Replacement", true)?;
    replaced.close()?;
    let p = ScriptProject::open(&path)?;
    assert_eq!(p.project().info().name, "Replacement");
    p.close()
}

#[test]
fn dol_bom_exports_to_excel() -> Result<(), ScriptError> {
    use calamine::{Data, Reader, Xlsx, open_workbook};

    let dir = tempfile::tempdir().unwrap();
    let mut p = ScriptProject::create(dir.path().join("dol.novo"), "DOL", false)?;
    build(&mut p)?;
    let contactor = p.add_part(
        "Siemens",
        "3RT2015-1BB41",
        "3RT2015",
        "Contactor 3 kW 24 V DC",
    )?;
    let breaker = p.add_part(
        "Siemens",
        "3RV2011-1EA10",
        "3RV2011",
        "Motor breaker 2.8-4 A",
    )?;
    let k1 = p.device_by_tag("-K1")?.unwrap();
    let q1 = p.device_by_tag("-Q1")?.unwrap();
    p.assign_part(k1, contactor)?;
    p.assign_part(q1, breaker)?;
    p.set_tag(k1, "=A1+CAB-K1")?;

    let bom = p.report("bom")?;
    let parts = &bom["sections"][0];
    assert_eq!(parts["key"], "parts");
    let order_numbers: Vec<&str> = parts["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[3].as_str().unwrap())
        .collect();
    assert_eq!(order_numbers, ["3RT2015-1BB41", "3RV2011-1EA10"]);

    let xlsx = dir.path().join("bom.xlsx");
    p.export_reports(&["bom"], &xlsx)?;
    let mut book: Xlsx<_> = open_workbook(&xlsx).unwrap();
    assert_eq!(book.sheet_names(), ["Bill of materials"]);
    let sheet = book.worksheet_range("Bill of materials").unwrap();
    let cells: Vec<String> = sheet.cells().map(|(_, _, c)| c.to_string()).collect();
    assert!(cells.iter().any(|c| c == "3RT2015-1BB41"), "{cells:?}");
    assert!(cells.iter().any(|c| c == "=A1+CAB-K1"), "{cells:?}");
    assert!(
        sheet
            .cells()
            .any(|(_, _, c)| *c == Data::Float(1.0) || *c == Data::Int(1))
    );

    let csv = dir.path().join("bom.csv");
    p.export_reports(&["bom"], &csv)?;
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(text.contains("3RV2011-1EA10"));

    // Exports never land on a project file.
    assert!(matches!(
        p.export_reports(&["bom"], dir.path().join("dol.novo")),
        Err(ScriptError::ProjectFileTarget(_))
    ));
    assert!(matches!(
        p.export_reports(&[], dir.path().join("all.csv")),
        Err(ScriptError::UnknownValue { .. })
    ));
    assert!(matches!(
        p.report("nope"),
        Err(ScriptError::UnknownValue { .. })
    ));
    p.close()
}
