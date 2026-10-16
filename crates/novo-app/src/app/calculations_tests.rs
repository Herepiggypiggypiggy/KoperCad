//! The electrical inputs, the per-wire and per-device calculation inputs
//! and the Calculations view: entered in the UI, read by the checks, one
//! undo step each, and reachable from the Check menu and the palette.

use novo_checks::calc::tables::TableSet;
use novo_checks::calc_view::{StarterChoice, calc_sheet};
use novo_checks::{ProblemKind, check};
use novo_tests::{MotorStarter, motor_starter};

use super::electrical::{LIMITS, MACHINE, SUPPLY, parse_input};
use super::gate1_tests::Drive;
use super::inspector::parse_device_input;
use super::wire_panel::{WireEdit, apply, facts};
use crate::actions::Action;

fn open() -> (tempfile::TempDir, Drive, MotorStarter) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let ms = motor_starter(d.app.backend.document_mut().unwrap()).unwrap();
    d.frame();
    (dir, d, ms)
}

fn count(d: &Drive, kind: ProblemKind) -> usize {
    check(d.app.backend.document().unwrap().project())
        .into_iter()
        .filter(|p| p.kind == kind)
        .count()
}

#[test]
fn the_three_commands_exist_sit_in_the_check_menu_and_open_their_windows() {
    for id in [
        "check.calculations",
        "check.electrical_inputs",
        "check.safety",
    ] {
        assert!(novo_workflow::commands::is_known(id), "{id}");
        assert!(super::workbench::app_runs(id), "{id} is greyed");
        assert!(
            novo_workflow::menus::entries("Check")
                .iter()
                .any(|(info, _)| info.id == id),
            "{id} is not in the Check menu"
        );
    }
    let (_dir, mut d, _) = open();
    d.app.wb.queued.push("check.calculations");
    d.frame();
    assert!(d.app.wb.calculations.is_some());
    d.app.wb.queued.push("check.electrical_inputs");
    d.frame();
    assert!(d.app.wb.electrical.is_some());
    d.app.wb.queued.push("check.safety");
    d.frame();
    assert!(d.app.wb.safety.is_some());
    // The same command again closes it.
    d.app.wb.queued.push("check.calculations");
    d.frame();
    assert!(d.app.wb.calculations.is_none());
}

#[test]
fn project_inputs_are_parsed_and_refused_with_a_reason() {
    use novo_core::Value;
    let ik = &SUPPLY[1];
    assert_eq!(parse_input(ik, " 10,5 ").unwrap(), Some(Value::Float(10.5)));
    assert_eq!(parse_input(ik, "").unwrap(), None);
    assert!(
        parse_input(ik, "lots")
            .unwrap_err()
            .contains("positive number")
    );
    assert!(parse_input(ik, "-3").is_err());
    let phases = MACHINE.iter().find(|f| f.integer).unwrap();
    assert_eq!(parse_input(phases, "3").unwrap(), Some(Value::Int(3)));
    assert!(parse_input(phases, "2.5").is_err());
    assert!(LIMITS.iter().any(|f| f.key == "panel.sccr_ka"));
    assert!(LIMITS.iter().any(|f| f.key == "check.voltage_drop_pct"));
}

#[test]
fn a_supply_and_a_breaking_capacity_entered_in_the_ui_make_the_check_speak_and_undo_silences_it() {
    let (_dir, mut d, ms) = open();
    let ctx = d.ctx.clone();
    assert_eq!(
        count(&d, ProblemKind::ShortCircuit),
        0,
        "silent without inputs"
    );

    assert!(d.app.apply_electrical_input(&SUPPLY[1], "10"));
    d.app.set_device_input(ms.f1, "breaking_capacity_ka", "6");
    d.frame();
    assert_eq!(count(&d, ProblemKind::ShortCircuit), 1);

    // Each entry is one undo step.
    d.app.run(&ctx, Action::Undo);
    assert_eq!(count(&d, ProblemKind::ShortCircuit), 0);
    d.app.run(&ctx, Action::Undo);
    assert!(
        !d.app
            .backend
            .document()
            .unwrap()
            .project()
            .info()
            .properties
            .contains_key("supply.short_circuit_ka")
    );
}

#[test]
fn a_bad_entry_changes_nothing_and_tells_the_user() {
    let (_dir, mut d, ms) = open();
    assert!(!d.app.apply_electrical_input(&SUPPLY[1], "lots"));
    d.app.set_device_input(ms.f1, "trip_curve", "Z");
    d.app.set_device_input(ms.f1, "rated_current_a", "-1");
    let project = d.app.backend.document().unwrap().project();
    assert!(
        !project
            .info()
            .properties
            .contains_key("supply.short_circuit_ka")
    );
    assert!(
        !project
            .device(ms.f1)
            .unwrap()
            .properties
            .contains_key("trip_curve")
    );
    assert!(parse_device_input("trip_curve", "c").unwrap().is_some());
    assert!(parse_device_input("rated_current_a", "").unwrap().is_none());
}

#[test]
fn wire_length_and_load_are_edited_on_the_wire_and_feed_the_calculation_sheet() {
    let (_dir, mut d, ms) = open();
    let wire = ms.motor_wires[0];
    d.app.apply_electrical_input(&SUPPLY[1], "10");
    {
        let doc = d.app.backend.document_mut().unwrap();
        doc.transact("inputs", |tx| {
            apply(tx, &[wire], &WireEdit::Length("100".into()))
                .map_err(novo_core::ModelError::Format)?;
            apply(tx, &[wire], &WireEdit::Load("16".into())).map_err(novo_core::ModelError::Format)
        })
        .unwrap();
        let f = facts(doc.project(), wire).unwrap();
        assert_eq!((f.length.as_str(), f.load.as_str()), ("100", "16"));
        let bad = doc.transact("bad", |tx| {
            apply(tx, &[wire], &WireEdit::Length("far".into()))
                .map_err(novo_core::ModelError::Format)
        });
        assert!(bad.is_err());
    }
    d.app.set_device_input(ms.f1, "rated_current_a", "16");
    d.app.set_device_input(ms.f1, "trip_curve", "c");
    let project = d.app.backend.document().unwrap().project();
    let sheet = calc_sheet(
        project,
        &project.netlist(),
        StarterChoice::default(),
        &TableSet::builtin(),
    );
    assert!(sheet.supply.is_some());
    assert!(sheet.planning_values, "the planning-value warning applies");
    assert!(sheet.circuits.iter().any(|c| c.device == ms.f1));
}

#[test]
fn the_calculations_window_draws_with_and_without_inputs() {
    let (_dir, mut d, ms) = open();
    d.app.wb.queued.push("check.calculations");
    d.frame();
    d.frame();
    d.app.apply_electrical_input(&SUPPLY[1], "10");
    d.app.set_device_input(ms.f1, "rated_current_a", "16");
    d.frame();
    d.frame();
    let win = d.app.wb.calculations.as_ref().expect("window stays open");
    let sheet = win.sheet().expect("sheet is built");
    let text = super::calculations::sheet_text(sheet);
    assert!(text.contains("planning values"), "{text}");
    assert!(text.contains("Supply:"), "{text}");
}
