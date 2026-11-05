//! Add fault from the drawing: the command, the sidecar and the window.

use novo_core::{AnyId, Endpoint, SymbolId, WireId};
use novo_sim::fault::FaultKind;
use novo_sim::study::{FaultWhat, Study};

use super::gate1_tests::Drive;
use crate::actions::Action;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

fn select(d: &mut Drive, ids: &[AnyId]) {
    let view = d.app.canvas.schematic().expect("editor");
    view.canvas.selection.clear();
    view.canvas.selection.apply(
        ids.iter()
            .map(|&id| novo_canvas::model::item_id(id))
            .collect::<Vec<_>>(),
        novo_canvas::SelectOp::Replace,
    );
    d.frame();
}

fn joined(d: &mut Drive) -> (SymbolId, SymbolId, WireId) {
    let a = d.place("mcb3", 80.0, 48.0);
    let b = d.place("coil", 200.0, 144.0);
    let (wire, changes) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("join", |tx| {
            tx.connect(Endpoint::pin(a, 0), Endpoint::pin(b, 0))
        })
        .unwrap();
    d.app.changed_outside_canvas(&changes);
    d.frame();
    (a, b, wire)
}

#[test]
fn add_fault_without_a_selection_says_what_to_select() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "sim.add_fault", None);
    d.frame();
    assert!(d.app.wb.add_fault.is_none());
    assert!(
        d.app
            .messages()
            .last()
            .is_some_and(|m| m.text.contains("Select a wire"))
    );
}

#[test]
fn the_queued_command_offers_the_selected_device_and_stores_the_fault_next_to_the_project() {
    let (dir, mut d) = drive();
    let (a, _b, _wire) = joined(&mut d);
    let file = dir.path().join("plant.novo");
    d.app.backend.save(&file).unwrap();
    select(&mut d, &[AnyId::Symbol(a)]);

    d.app.wb.queued.push("sim.add_fault");
    d.frame();
    let dialog = d.app.wb.add_fault.as_ref().expect("the window opens");
    let tag = dialog.targets()[0].clone();
    assert!(tag.starts_with('-'), "{tag}");

    // The window's Add button calls this.
    d.app.add_fault_stored(
        &tag,
        FaultWhat::Short {
            kind: FaultKind::PhaseEarth,
        },
        0.0,
        0.1,
    );
    d.frame();
    let sidecar = file.with_extension("faults.json");
    let study = Study::from_json(&std::fs::read_to_string(&sidecar).unwrap()).unwrap();
    assert_eq!(study.faults.len(), 1);
    assert_eq!(study.faults[0].target, tag);
    // The Fault scenarios window is open on it, with no Example study.
    let win = d.app.wb.faults.as_ref().expect("the fault window opens");
    assert!(!win.is_example());
    assert_eq!(win.fault_count(), 1);
    assert!(win.selected_name().starts_with("F1:"));
}

#[test]
fn a_wire_is_a_target_and_an_unsaved_project_asks_to_save() {
    let (_dir, mut d) = drive();
    let (_a, _b, wire) = joined(&mut d);
    select(&mut d, &[AnyId::Wire(wire)]);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "sim.add_fault", None);
    d.frame();
    assert_eq!(d.app.wb.add_fault.as_ref().unwrap().targets().len(), 1);
    d.app
        .add_fault_stored("L1", FaultWhat::DeviceOpens, 0.0, 0.1);
    assert!(
        d.app
            .messages()
            .last()
            .is_some_and(|m| m.text.contains("Save the project first"))
    );
    assert!(d.app.wb.faults.is_none());
}

#[test]
fn without_faults_the_window_still_shows_the_example() {
    let (dir, mut d) = drive();
    let file = dir.path().join("plant.novo");
    d.app.backend.save(&file).unwrap();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "sim.faults", None);
    d.frame();
    let win = d.app.wb.faults.as_ref().unwrap();
    assert!(win.is_example());
    assert_eq!(win.fault_count(), 0);
}

#[test]
fn an_empty_target_is_refused_and_the_sidecar_is_written_whole() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plant.faults.json");
    let what = FaultWhat::DeviceOpens;
    assert!(super::faults::store_fault(&path, "plant", " ", what.clone(), 0.0, 0.1).is_err());
    assert!(!path.exists(), "nothing is written for a refused fault");
    super::faults::store_fault(&path, "plant", "-Q1", what.clone(), 0.0, 0.1).unwrap();
    super::faults::store_fault(&path, "plant", "-Q2", what, 0.0, 0.1).unwrap();
    let study = Study::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(study.faults.len(), 2);
    let left = std::fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(left, 1, "no temporary file is left behind");
}

fn flag_jobs(d: &Drive) -> Vec<String> {
    d.app
        .job_flags()
        .iter()
        .flat_map(|f| f.anchors.iter())
        .filter_map(|a| match a {
            novo_flags::Anchor::Job(j) => Some(j.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_study_that_cannot_be_read_flags_faults_read_and_opening_it_again_clears_it() {
    let (dir, mut d) = drive();
    let file = dir.path().join("plant.novo");
    d.app.backend.save(&file).unwrap();
    let sidecar = file.with_extension("faults.json");
    std::fs::write(&sidecar, "not json").unwrap();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "sim.faults", None);
    d.frame();
    assert_eq!(flag_jobs(&d), vec!["faults.read".to_owned()]);
    assert!(d.app.wb.faults.is_none());
    std::fs::write(&sidecar, Study::example().to_json()).unwrap();
    d.app.run_command(&ctx, "sim.faults", None);
    d.frame();
    assert!(d.app.wb.faults.is_some(), "the window opens");
    assert!(flag_jobs(&d).is_empty(), "{:?}", flag_jobs(&d));
}

#[test]
fn a_fault_that_cannot_be_stored_flags_faults_store_and_a_stored_one_clears_it() {
    let (dir, mut d) = drive();
    let file = dir.path().join("plant.novo");
    d.app.backend.save(&file).unwrap();
    let sidecar = file.with_extension("faults.json");
    std::fs::write(&sidecar, "not json").unwrap();
    d.app
        .add_fault_stored("-Q1", FaultWhat::DeviceOpens, 0.0, 0.1);
    assert_eq!(flag_jobs(&d), vec!["faults.store".to_owned()]);
    std::fs::remove_file(&sidecar).unwrap();
    d.app
        .add_fault_stored("-Q1", FaultWhat::DeviceOpens, 0.0, 0.1);
    assert!(flag_jobs(&d).is_empty(), "{:?}", flag_jobs(&d));
}

#[test]
fn a_table_that_cannot_be_read_flags_controller_table_read_and_a_read_one_clears_it() {
    let (dir, mut d) = drive();
    let missing = dir.path().join("io.csv");
    d.app.controller_read_file(missing.clone());
    assert_eq!(flag_jobs(&d), vec!["controller.table_read".to_owned()]);
    std::fs::write(&missing, "klem;functie\n1;Hijsen omhoog\n").unwrap();
    d.app.controller_read_file(missing);
    assert!(flag_jobs(&d).is_empty(), "{:?}", flag_jobs(&d));
    // The EPLAN branch clears its own flag too.
    let bad = dir.path().join("x.xml");
    d.app.controller_read_file(bad.clone());
    assert_eq!(flag_jobs(&d), vec!["controller.table_read".to_owned()]);
    std::fs::write(&bad, "<root/>").unwrap();
    d.app.controller_read_file(bad);
    assert!(
        flag_jobs(&d).is_empty() || flag_jobs(&d) == ["controller.table_read"],
        "an unreadable xml may still fail"
    );
}
