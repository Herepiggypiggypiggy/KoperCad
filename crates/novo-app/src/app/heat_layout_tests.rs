//! The cooler-layout search of the Cabinet Heat view: it runs from the
//! scenario the view follows, ends with a finding, says so when there is
//! nothing to move, and its moves apply as the engine promised.

use std::time::Duration;

use eframe::egui;
use novo_core::{
    Command, DeviceId, DeviceTag, MountingPlate, Project, RailId, RailKind, StructureId, Value,
};
use novo_layout::enclosure::Enclosure;
use novo_layout::model::{load, set_enclosure};
use novo_thermal::placement::{Move, Suggestion};

use super::heat::HeatState;

fn cabinet(with_rails: bool) -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 1100.0,
                    margin: 10.0,
                }),
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 900.0,
                    height: 1200.0,
                    depth: 300.0,
                    plate_setback: 0.0,
                    door: None,
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            let mut rails = Vec::new();
            if with_rails {
                for row in 0..3 {
                    rails.push(tx.add_rail(
                        cab,
                        20.0,
                        40.0 + 350.0 * f64::from(row),
                        760.0,
                        RailKind::DinRail,
                    )?);
                }
            }
            for (tag, x, row, w, h, loss, limit) in [
                ("=A1+CAB1-T1", 30.0, 2, 120.0, 220.0, 150.0, 40.0),
                ("=A1+CAB1-K1", 40.0, 1, 50.0, 90.0, 4.0, 45.0),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                for (k, v) in [
                    ("width_mm", w),
                    ("height_mm", h),
                    ("depth_mm", 80.0),
                    ("power_loss_w", loss),
                    ("max_ambient_c", limit),
                ] {
                    tx.set_property(d, k, Some(Value::Float(v)))?;
                }
                tx.execute(Command::PlaceDevice {
                    device: d,
                    structure: cab,
                    x,
                    y: 40.0 + 350.0 * f64::from(row),
                    rotated: false,
                    rail: rails.get(row as usize).map(|r| (*r, x - 20.0)),
                })?;
            }
            Ok(cab)
        })
        .unwrap();
    (p, cab)
}

fn tags(p: &Project) -> impl Fn(u64) -> String + '_ {
    move |id| {
        p.get::<novo_core::Device>(novo_core::DeviceId::from(id))
            .map_or_else(String::new, |d| d.tag.to_string())
    }
}

fn run(state: &mut HeatState, p: &Project, cab: StructureId) {
    let ctx = egui::Context::default();
    state.sync_live(p, cab, 35.0, false, 0.0, &ctx);
    state.command("heat.cooler_layout");
    assert!(state.on, "the command shows the Heat view");
    assert!(state.layout.take_request());
    let scenario = state.current_scenario();
    state.layout.start(p, cab, scenario, &tags(p));
    for _ in super::test_wait::patient() {
        state.layout.poll(&ctx);
        if !state.layout.running() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the search did not end");
}

fn moves_of(state: &HeatState) -> (Suggestion, StructureId) {
    let f = state.layout.found.as_ref().expect("a finding");
    (f.suggestion.clone(), f.structure)
}

#[test]
fn the_search_finds_a_cooler_place_and_the_moves_apply_on_the_rails() {
    let (mut p, cab) = cabinet(true);
    let mut state = HeatState::new();
    run(&mut state, &p, cab);
    let (s, structure) = moves_of(&state);
    assert_eq!(structure, cab);
    assert!(!s.moves.is_empty(), "{:?}", s.notes);
    assert!(s.after.cost < s.before.cost);
    let moves: Vec<Move> = s.moves.clone();
    p.transact("Cooler layout", |tx| {
        novo_thermal::placement_project::apply_moves(tx, cab, &moves).map(|_| ())
    })
    .unwrap();
    let layout = load(&p, cab).unwrap();
    for m in &moves {
        let placed = layout.placements.iter().find(|q| q.id == m.id).unwrap();
        assert_eq!(placed.rail, Some(m.to_row));
    }
    // The cabinet changed, so the view's key differs from the search's.
    let ctx = egui::Context::default();
    state.sync_live(&p, cab, 35.0, false, 1.0, &ctx);
    let (_, now) = state.current_scenario().unwrap();
    assert_ne!(now, state.layout.found.as_ref().unwrap().scenario_key());
}

#[test]
fn without_rails_it_says_there_is_nothing_to_move_along() {
    let (p, cab) = cabinet(false);
    let mut state = HeatState::new();
    run(&mut state, &p, cab);
    assert!(state.layout.found.is_none());
    assert!(
        state
            .layout
            .notice()
            .is_some_and(|n| n.contains("No device stands on a rail")),
        "{:?}",
        state.layout.notice()
    );
}

#[test]
fn closing_drops_the_finding_and_stops_a_run() {
    let (p, cab) = cabinet(true);
    let mut state = HeatState::new();
    run(&mut state, &p, cab);
    assert!(state.layout.found.is_some());
    state.layout.close();
    assert!(state.layout.found.is_none());
    assert!(!state.layout.running());
}

#[test]
fn a_finding_is_stale_unless_the_view_follows_the_same_cabinet() {
    assert!(!super::heat_layout::is_stale(Some(7), 7));
    assert!(super::heat_layout::is_stale(Some(8), 7));
    assert!(super::heat_layout::is_stale(None, 7));
}

#[test]
fn apply_keeps_the_rotation_of_a_moved_device() {
    let (mut p, cab) = cabinet(true);
    let mut state = HeatState::new();
    run(&mut state, &p, cab);
    let (s, _) = moves_of(&state);
    let first = s.moves[0].clone();
    let at = load(&p, cab)
        .unwrap()
        .placements
        .iter()
        .find(|q| q.id == first.id)
        .map(|q| (q.x, q.y, q.rail))
        .unwrap();
    p.transact("Turn", |tx| {
        tx.execute(Command::PlaceDevice {
            device: DeviceId::from(first.id),
            structure: cab,
            x: at.0,
            y: at.1,
            rotated: true,
            rail: at.2.map(|r| (RailId::from(r), 0.0)),
        })
    })
    .unwrap();
    let moves = s.moves.clone();
    p.transact("Cooler layout", |tx| {
        novo_thermal::placement_project::apply_moves(tx, cab, &moves).map(|_| ())
    })
    .unwrap();
    let placed = load(&p, cab).unwrap();
    assert!(
        placed
            .placements
            .iter()
            .find(|q| q.id == first.id)
            .unwrap()
            .rotated
    );
}

#[test]
fn a_gone_rail_applies_nothing_not_half_a_swap() {
    let (mut p, cab) = cabinet(true);
    let mut state = HeatState::new();
    run(&mut state, &p, cab);
    let (s, _) = moves_of(&state);
    assert!(!s.moves.is_empty());
    let before = load(&p, cab).unwrap().placements;
    let mut moves = s.moves.clone();
    moves.last_mut().unwrap().to_row = u64::MAX - 1;
    let mut skipped = Vec::new();
    p.transact("Cooler layout", |tx| {
        skipped = novo_thermal::placement_project::apply_moves(tx, cab, &moves)?;
        Ok(())
    })
    .unwrap();
    assert!(!skipped.is_empty());
    assert_eq!(load(&p, cab).unwrap().placements, before);
}
