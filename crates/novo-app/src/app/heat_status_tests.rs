//! The shared simulation state the Cabinet and 3D views read.

use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use novo_cfd::{CfdError, CfdSnapshot, Control, Fidelity, Solver};
use novo_thermal::Scenario;

use super::heat::{HeatState, Meta};
use super::heat_run_tests::scenario;
use super::heat_status::SimStatus;

fn meta() -> Meta {
    Meta {
        scenario_key: 1,
        layout_key: 2,
        ambient: 35.0,
        plate: [20.0, 20.0, 360.0, 360.0],
        messages: Vec::new(),
        enclosure: None,
        duct_loss_w: 0.0,
        wire_notes: Vec::new(),
    }
}

/// Sets up for a while and never reports an iteration, until cancelled.
struct Setup;

impl Solver for Setup {
    fn solve(
        &self,
        _: &Scenario,
        _: Fidelity,
        control: &Control,
        _: &mut dyn FnMut(&CfdSnapshot),
    ) -> Result<CfdSnapshot, CfdError> {
        control.set_phase("Meshing");
        while !control.is_cancelled() {
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(CfdError::Cancelled)
    }
}

#[test]
fn a_new_state_is_not_run() {
    let s = HeatState::new();
    assert_eq!(s.sim_status(), SimStatus::NotRun);
    assert!(s.sim_status().text().contains("not simulated"));
    assert!(!s.can_cancel());
}

#[test]
fn a_run_in_setup_names_its_stage_and_cancel_ends_it() {
    let mut s = HeatState::new();
    s.set_solver_for_test(Arc::new(Setup));
    s.start(scenario(), meta());
    // The worker names its stage a moment after it starts.
    let ctx = egui::Context::default();
    let mut text = String::new();
    for _ in 0..200 {
        s.poll(&ctx);
        text = s.sim_status().text();
        if text.contains("meshing") {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(text.contains("meshing"), "{text}");
    match s.sim_status() {
        SimStatus::Running {
            fraction: None,
            stopping: false,
            ..
        } => {}
        other => panic!("{other:?}"),
    }
    assert!(s.can_cancel());
    s.cancel();
    assert!(matches!(
        s.sim_status(),
        SimStatus::Running { stopping: true, .. }
    ));
    assert!(!s.can_cancel());
    for _ in 0..200 {
        s.poll(&ctx);
        if !s.running() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!s.running());
}

#[test]
fn a_finished_run_is_done_with_its_label() {
    let mut s = HeatState::new();
    s.start(scenario(), meta());
    let ctx = egui::Context::default();
    for _ in 0..600 {
        s.poll(&ctx);
        if !s.running() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    match s.sim_status() {
        SimStatus::Done { label, .. } => assert!(!label.is_empty()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_failure_line_is_a_failed_status_and_stopped_is_not() {
    let mut s = HeatState::new();
    s.fail("Cannot simulate: the enclosure size is not set".into());
    assert!(matches!(s.sim_status(), SimStatus::Failed(_)));
    s.fail("Stopped: the last result is not converged".into());
    assert_eq!(s.sim_status(), SimStatus::NotRun);
}

#[test]
fn the_chip_text_says_what_each_state_is() {
    let running = SimStatus::Running {
        fraction: Some(0.4),
        label: "Coarse 3D · it 40 · 3 s".into(),
        stopping: false,
    };
    assert!(running.text().starts_with("Heat: Coarse"));
    assert!(SimStatus::OutOfDate.text().contains("out of date"));
    let done = SimStatus::Done {
        label: "Coarse 3D".into(),
        converged: false,
    };
    assert!(done.text().contains("not converged"));
}
