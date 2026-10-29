//! Tests for the defects card of the live view (lane F1), headless and through
//! `LiveView::solved`, the path the live view draws from.

use novo_core::{Document, Value};
use novo_sim::inject::{Defect, DefectKind};
use novo_sim::walkthrough::{Action, Check, MotorIs, Walkthrough};
use novo_sim::{MotorRun, device_states, rest_state};
use novo_tests::motor_starter;

use super::defects_card::{PROPERTY, operate_steps};
use super::live::LiveView;

fn starter() -> (Document, LiveView) {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    let mut live = LiveView::default();
    live.reset_poles(doc.project());
    live.solved(doc.project(), 0.0);
    (doc, live)
}

fn motor(live: &mut LiveView, doc: &Document) -> MotorRun {
    let solved = live.solved(doc.project(), 0.05);
    device_states(doc.project(), &solved.netlist, &solved.settled)
        .motors
        .values()
        .copied()
        .next()
        .expect("the starter has a motor")
}

/// Holds the start button for a few frames, then lets go.
fn start(live: &mut LiveView, doc: &Document) {
    let s1 = doc
        .project()
        .devices()
        .find(|d| d.tag.to_string().ends_with("-S1"))
        .unwrap()
        .id;
    let symbol = doc.project().symbols_of_device(s1)[0].id;
    live.press(doc.project(), symbol, 0);
    for _ in 0..3 {
        live.solved(doc.project(), 0.05);
    }
    live.release(doc.project());
    for _ in 0..3 {
        live.solved(doc.project(), 0.05);
    }
}

#[test]
fn a_welded_contactor_keeps_the_motor_running_in_the_live_view() {
    let (doc, mut live) = starter();
    start(&mut live, &doc);
    assert!(matches!(motor(&mut live, &doc), MotorRun::Running { .. }));
    live.defects
        .inject(DefectKind::ContactWelded.build("-K1", ""));
    // The stop button no longer stops it: the main contact conducts on its own.
    let s0 = doc
        .project()
        .devices()
        .find(|d| d.tag.to_string().ends_with("-S0"))
        .unwrap()
        .id;
    let symbol = doc.project().symbols_of_device(s0)[0].id;
    live.press(doc.project(), symbol, 0);
    for _ in 0..3 {
        live.solved(doc.project(), 0.05);
    }
    live.release(doc.project());
    assert!(matches!(motor(&mut live, &doc), MotorRun::Running { .. }));
    assert!(live.defects.active());
    // The welded seal-in contact re-latched the coil when stop was let go, so after the repair
    // the coil still holds itself: a stop press now stops the healthy plant.
    live.defects.repair_all();
    live.solved(doc.project(), 0.05);
    assert!(matches!(motor(&mut live, &doc), MotorRun::Running { .. }));
    live.press(doc.project(), symbol, 0);
    for _ in 0..3 {
        live.solved(doc.project(), 0.05);
    }
    live.release(doc.project());
    assert_eq!(motor(&mut live, &doc), MotorRun::Stopped);
}

#[test]
fn leaving_the_live_view_repairs_everything() {
    let (doc, mut live) = starter();
    live.defects.inject(DefectKind::CoilOpen.build("-K1", ""));
    assert!(live.defects.active());
    live.leave();
    assert!(!live.defects.active());
    start(&mut live, &doc);
    live.on = true;
    assert!(matches!(motor(&mut live, &doc), MotorRun::Running { .. }));
}

#[test]
fn a_cut_wire_is_solved_on_its_own_netlist_and_stays_stable() {
    let (doc, mut live) = starter();
    live.defects.inject(Defect::ConductorOpen {
        at: "-S0:12".into(),
    });
    start(&mut live, &doc);
    // The stop chain is open: the start button cannot pull the coil in.
    assert_eq!(motor(&mut live, &doc), MotorRun::Stopped);
    // Solving again without any change does not rebuild (the faulted netlist is not "stale").
    let first = live.solved(doc.project(), 0.0).netlist.clone();
    let second = live.solved(doc.project(), 0.0).netlist.clone();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn operator_moves_become_walkthrough_steps() {
    let (doc, mut live) = starter();
    let rest = rest_state(doc.project());
    let before = live.poles.clone();
    let s1 = doc
        .project()
        .devices()
        .find(|d| d.tag.to_string().ends_with("-S1"))
        .unwrap()
        .id;
    let symbol = doc.project().symbols_of_device(s1)[0].id;
    live.press(doc.project(), symbol, 0);
    let steps = operate_steps(doc.project(), &rest, &before, &live.poles);
    assert_eq!(
        steps,
        vec![Action::Operate {
            device: "=A1+CAB-S1".into(),
            pin: None,
            operated: true
        }]
    );
    let held = live.poles.clone();
    live.release(doc.project());
    let steps = operate_steps(doc.project(), &rest, &held, &live.poles);
    assert!(matches!(
        steps.as_slice(),
        [Action::Operate {
            operated: false,
            ..
        }]
    ));
    assert!(operate_steps(doc.project(), &rest, &live.poles, &live.poles).is_empty());
}

#[test]
fn saved_walkthroughs_are_read_from_the_project_property() {
    let (mut doc, mut live) = starter();
    let w = Walkthrough::new("stop must stop").at(
        1.0,
        Action::Expect {
            check: Check::Motor {
                device: "-M1".into(),
                is: MotorIs::Stopped,
            },
        },
    );
    let text = serde_json::to_string(&vec![w.clone()]).unwrap();
    doc.transact("Walk-throughs", |tx| {
        tx.set_info(|i| {
            i.properties
                .insert(PROPERTY.to_owned(), Value::Text(text.clone()));
        })
    })
    .unwrap();
    live.defects.sync(doc.project());
    let report = novo_sim::walkthrough::run(doc.project(), &w);
    assert!(report.ok(), "{report:?}");
}
