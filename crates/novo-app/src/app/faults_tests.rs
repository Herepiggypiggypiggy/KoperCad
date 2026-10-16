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
