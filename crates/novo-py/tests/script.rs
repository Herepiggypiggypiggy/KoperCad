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
    let mut p = ScriptProject::create(&path, "DOL")?;
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
    let mut p = ScriptProject::create(&path, "Undo")?;
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
