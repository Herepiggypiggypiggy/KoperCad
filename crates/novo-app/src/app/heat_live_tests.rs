//! The heat result follows the cabinet: moving a hot device changes the
//! solved field, marks the old one out of date and, with auto on, solves
//! again.

use std::time::Duration;

use eframe::egui;
use novo_cfd::{CabinetSolver, Control, Fidelity, Solver};
use novo_core::{DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::enclosure::Enclosure;
use novo_layout::model::{place_device, set_enclosure, set_plate};
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};

use super::heat::HeatState;

/// A cabinet with one 200 W device and one cool one.
fn fixture() -> (Project, StructureId, u64) {
    let mut p = Project::new("t");
    let mut hot = 0;
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 400.0,
                    height: 600.0,
                    margin: 10.0,
                },
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 500.0,
                    height: 700.0,
                    depth: 250.0,
                    plate_setback: 0.0,
                    door: None,
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            let part = tx.add_part("ACME", "HOT")?;
            for (k, v) in [("width_mm", 60.0), ("height_mm", 90.0), ("depth_mm", 70.0)] {
                tx.set_property(part, k, Some(Value::Float(v)))?;
            }
            tx.set_property(part, "power_loss_w", Some(Value::Float(200.0)))?;
            for (i, tag) in ["=A1+CAB1-K1", "=A1+CAB1-K2"].into_iter().enumerate() {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
                if i == 0 {
                    hot = d.raw();
                    tx.set_property(d, "power_loss_w", Some(Value::Float(200.0)))?;
                } else {
                    tx.set_property(d, "power_loss_w", Some(Value::Float(1.0)))?;
                }
                place_device(tx, cab, d.raw(), 20.0 + 200.0 * i as f64, 100.0, false)?;
            }
            Ok(cab)
        })
        .unwrap();
    (p, cab, hot)
}

fn move_hot(p: &mut Project, cab: StructureId, hot: u64, x: f64, y: f64) {
    p.transact("move", |tx| place_device(tx, cab, hot, x, y, false))
        .unwrap();
}

fn scenario(p: &Project, cab: StructureId) -> novo_thermal::Scenario {
    scenario_from_project(p, cab, &BridgeOptions::default())
        .unwrap()
        .scenario
}

#[test]
fn moving_a_hot_device_changes_the_scenario_and_the_solved_field() {
    let (mut p, cab, hot) = fixture();
    let before = scenario(&p, cab);
    move_hot(&mut p, cab, hot, 300.0, 400.0);
    let after = scenario(&p, cab);
    assert_ne!(before, after, "the move reaches the thermal scenario");

    let solve = |s: &novo_thermal::Scenario| {
        CabinetSolver::default()
            .solve(s, Fidelity::Coarse, &Control::new(), &mut |_| {})
            .unwrap()
    };
    let a = solve(&before);
    let b = solve(&after);
    // The grids differ (the mesh follows the devices), so compare where the
    // hottest air is, in mm.
    let hottest = |s: &novo_cfd::CfdSnapshot| {
        let (n, _) = s
            .temperature_c
            .iter()
            .enumerate()
            .max_by(|x, y| x.1.total_cmp(y.1))
            .unwrap();
        let g = &s.grid;
        let (i, j, k) = (n % g.nx, (n / g.nx) % g.ny, n / (g.nx * g.ny));
        g.centre_mm(i, j, k)
    };
    let (pa, pb) = (hottest(&a), hottest(&b));
    let moved = ((pa[0] - pb[0]).powi(2) + (pa[1] - pb[1]).powi(2)).sqrt();
    assert!(
        moved > 100.0,
        "the hot spot moved with the device: {pa:?} to {pb:?}"
    );
}

fn frame(state: &mut HeatState, ctx: &egui::Context, p: &Project, cab: StructureId, now: f64) {
    state.poll(ctx);
    state.sync_live(p, cab, 35.0, false, now, ctx);
}

fn settle(state: &mut HeatState, ctx: &egui::Context, p: &Project, cab: StructureId, now: f64) {
    for _ in 0..4000 {
        frame(state, ctx, p, cab, now);
        if !state.running() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the run did not end");
}

#[test]
fn an_edit_marks_the_result_out_of_date_and_auto_solves_again() {
    let (mut p, cab, hot) = fixture();
    let ctx = egui::Context::default();
    let mut state = HeatState::new();
    frame(&mut state, &ctx, &p, cab, 0.0);
    state.start_current();
    settle(&mut state, &ctx, &p, cab, 0.0);
    let first = state.solution().expect("a result");
    assert!(!state.live.is_stale());

    // An edit that does not touch the thermal model changes nothing.
    p.transact("rename", |tx| {
        tx.add_structure(DeviceTag::parse("+OTHER").unwrap(), "Other", false)?;
        Ok(())
    })
    .unwrap();
    frame(&mut state, &ctx, &p, cab, 1.0);
    assert!(!state.live.is_stale());

    // A move marks it at once, but waits out the quiet time before solving.
    move_hot(&mut p, cab, hot, 300.0, 400.0);
    frame(&mut state, &ctx, &p, cab, 2.0);
    assert!(state.live.is_stale());
    assert!(!state.running(), "debounced");
    frame(&mut state, &ctx, &p, cab, 2.1);
    assert!(!state.running(), "still inside the quiet time");
    frame(&mut state, &ctx, &p, cab, 2.6);
    assert!(state.running(), "auto solves after the quiet time");
    settle(&mut state, &ctx, &p, cab, 3.0);
    assert!(!state.live.is_stale(), "the new result matches the cabinet");
    let second = state.solution().unwrap();
    assert_ne!(first.id, second.id);
    assert_ne!(first.scenario_key, second.scenario_key);
}

#[test]
fn with_auto_off_the_result_only_goes_stale() {
    let (mut p, cab, hot) = fixture();
    let ctx = egui::Context::default();
    let mut state = HeatState::new();
    frame(&mut state, &ctx, &p, cab, 0.0);
    state.start_current();
    settle(&mut state, &ctx, &p, cab, 0.0);
    state.live.set_auto(false);
    move_hot(&mut p, cab, hot, 300.0, 400.0);
    frame(&mut state, &ctx, &p, cab, 5.0);
    frame(&mut state, &ctx, &p, cab, 9.0);
    assert!(state.live.is_stale());
    assert!(!state.running());
}

struct Fails(std::sync::atomic::AtomicUsize);

impl Solver for Fails {
    fn solve(
        &self,
        _: &novo_thermal::Scenario,
        _: Fidelity,
        _: &Control,
        _: &mut dyn FnMut(&novo_cfd::CfdSnapshot),
    ) -> Result<novo_cfd::CfdSnapshot, novo_cfd::CfdError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(novo_cfd::CfdError::Cancelled)
    }
}

#[test]
fn a_run_that_fails_is_not_started_again_every_frame() {
    let (mut p, cab, hot) = fixture();
    let ctx = egui::Context::default();
    let mut state = HeatState::new();
    frame(&mut state, &ctx, &p, cab, 0.0);
    state.start_current();
    settle(&mut state, &ctx, &p, cab, 0.0);
    move_hot(&mut p, cab, hot, 300.0, 400.0);
    // From here every run fails before its first snapshot.
    let fails = std::sync::Arc::new(Fails(Default::default()));
    state.set_solver_for_test(fails.clone());
    for t in 1..60 {
        frame(&mut state, &ctx, &p, cab, f64::from(t));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(fails.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(state.live.is_stale());
}

#[test]
fn a_move_during_a_run_cancels_it_and_starts_the_next() {
    let (mut p, cab, hot) = fixture();
    let ctx = egui::Context::default();
    let mut state = HeatState::new();
    frame(&mut state, &ctx, &p, cab, 0.0);
    state.start_current();
    assert!(state.running());
    move_hot(&mut p, cab, hot, 300.0, 400.0);
    // The old run is superseded; no "stopped" warning is left behind.
    for t in 1..4000 {
        frame(&mut state, &ctx, &p, cab, f64::from(t));
        std::thread::sleep(Duration::from_millis(5));
        if !state.running() && !state.live.is_stale() && state.solution().is_some() {
            break;
        }
    }
    let sol = state.solution().expect("a result for the moved cabinet");
    assert_eq!(
        sol.scenario_key,
        super::heat_live::scenario_key(&scenario(&p, cab))
    );
    assert!(!state.live.is_stale());
}
