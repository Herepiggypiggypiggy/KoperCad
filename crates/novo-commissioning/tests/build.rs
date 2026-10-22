//! Build mode: the guided plan, the ticks and how they travel.

use novo_commissioning::*;
use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, StructureId};
use novo_tests::motor_starter::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let m = motor_starter(&mut doc).expect("fixture builds");
    (doc, m)
}

/// The starter with a cabinet `+CAB`: a plate, a rail, a duct, F1 and K1 on
/// the rail, F2 free on the plate.
fn cabinet(doc: &mut Document, h: &MotorStarter) -> StructureId {
    doc.transact("Cabinet", |tx| {
        let cab = tx.add_structure(DeviceTag::parse("+CAB")?, "Cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 800.0,
                height: 600.0,
                margin: 10.0,
            }),
        )?;
        let r1 = tx.add_rail(cab, 50.0, 100.0, 400.0, RailKind::DinRail)?;
        let duct = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
        tx.set_rail_size(duct, 700.0, 50.0, 60.0)?;
        tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, Some((r1, 50.0)))?;
        tx.place_device(
            h.k1,
            cab,
            Point::new(200.0, 100.0),
            false,
            Some((r1, 150.0)),
        )?;
        tx.place_device(h.f2, cab, Point::new(80.0, 400.0), true, None)?;
        Ok(cab)
    })
    .expect("layout")
    .0
}

fn plan_of(doc: &Document) -> BuildPlan {
    BuildPlan::of(doc.project())
}

fn phases(c: &CabinetPlan) -> Vec<BuildPhase> {
    let mut v: Vec<BuildPhase> = c.steps.iter().map(|s| s.phase).collect();
    v.dedup();
    v
}

#[test]
fn a_project_without_a_cabinet_has_no_plan() {
    let (doc, _) = starter();
    assert!(plan_of(&doc).is_empty());
}

#[test]
fn the_plan_runs_prepare_drill_rails_devices_wires_labels_check() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    assert_eq!(plan.cabinets.len(), 1);
    let c = &plan.cabinets[0];
    assert_eq!(c.tag, "+CAB");
    let got = phases(c);
    assert_eq!(got[0], BuildPhase::Prepare);
    assert_eq!(*got.last().unwrap(), BuildPhase::Check);
    assert!(
        got.windows(2).all(|w| w[0] < w[1]),
        "phases only go forward: {got:?}"
    );
    for want in [
        BuildPhase::Drill,
        BuildPhase::Rails,
        BuildPhase::Ducts,
        BuildPhase::Devices,
        BuildPhase::Labels,
    ] {
        assert!(got.contains(&want), "missing {want:?}");
    }
}

#[test]
fn every_step_comes_after_what_it_needs_and_numbers_run_one_to_n() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    for (i, s) in c.steps.iter().enumerate() {
        assert_eq!(s.number, i + 1);
        assert!(
            s.after.iter().all(|&a| a < s.number),
            "{} {:?}",
            s.title,
            s.after
        );
    }
    // Keys are unique, so a tick belongs to exactly one step.
    let keys = build_keys(&plan);
    let mut seen = std::collections::BTreeMap::new();
    for s in &c.steps {
        if let Some(other) = seen.insert(s.key, s.number) {
            panic!(
                "steps {other} and {} share {:?}: {}",
                s.number, s.key, s.title
            );
        }
    }
    assert_eq!(keys.len(), c.steps.len());
}

#[test]
fn holes_are_drilled_before_rails_and_wires_wait_for_every_device() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let drill: Vec<usize> = c
        .steps
        .iter()
        .filter(|s| s.phase == BuildPhase::Drill)
        .map(|s| s.number)
        .collect();
    assert!(!drill.is_empty(), "rail holes give at least one drill step");
    let rail = c
        .steps
        .iter()
        .find(|s| s.phase == BuildPhase::Rails)
        .unwrap();
    for d in &drill {
        assert!(rail.after.contains(d), "rail waits for drilling step {d}");
    }
    let devices: Vec<usize> = c
        .steps
        .iter()
        .filter(|s| s.phase == BuildPhase::Devices)
        .map(|s| s.number)
        .collect();
    assert_eq!(devices.len(), 3);
    for w in c.steps.iter().filter(|s| s.phase == BuildPhase::Wires) {
        for d in &devices {
            assert!(
                w.after.contains(d),
                "wire {} waits for device step {d}",
                w.mark
            );
        }
    }
    // A clip-on device waits for its rail.
    let k1 = c
        .steps
        .iter()
        .find(|s| s.phase == BuildPhase::Devices && s.mark.ends_with("K1"))
        .unwrap();
    assert!(k1.after.contains(&rail.number));
}

#[test]
fn the_instructions_say_cut_and_drill_by_hand() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let drill = c
        .steps
        .iter()
        .find(|s| s.phase == BuildPhase::Drill)
        .unwrap();
    assert!(drill.title.starts_with("Drill "), "{}", drill.title);
    assert!(drill.caution.as_deref().unwrap().contains("by hand"));
    let wire = c
        .steps
        .iter()
        .find(|s| s.phase == BuildPhase::Wires)
        .unwrap();
    assert!(wire.title.starts_with("Cut "), "{}", wire.title);
}

#[test]
fn power_wires_come_before_control_and_earth_last() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let words: Vec<&str> = c
        .steps
        .iter()
        .filter(|s| s.phase == BuildPhase::Wires)
        .map(|s| {
            ["power", "control", "earth"]
                .into_iter()
                .find(|w| s.title.contains(&format!("the {w} wire")))
                .expect("a circuit in the title")
        })
        .collect();
    let rank = |w: &&str| {
        ["power", "control", "earth"]
            .iter()
            .position(|x| x == w)
            .unwrap()
    };
    assert!(
        words.windows(2).all(|p| rank(&p[0]) <= rank(&p[1])),
        "{words:?}"
    );
}

#[test]
fn a_guessed_size_is_said_on_the_device_step() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    // The fixture has no part sizes: every footprint is a guess, and so the
    // prepare step and each device step say so.
    let prepare = &c.steps[0];
    assert!(prepare.caution.is_some());
    assert!(
        prepare.details.iter().any(|d| d.contains("guess")),
        "{:?}",
        prepare.details
    );
    let dev = c
        .steps
        .iter()
        .find(|s| s.phase == BuildPhase::Devices)
        .unwrap();
    assert!(dev.caution.as_deref().unwrap().contains("guess"));
}

#[test]
fn next_walks_the_plan_and_finishes() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let mut store = CommissioningStore::new();
    assert_eq!(c.progress(&store).done, 0);
    let mut order = Vec::new();
    loop {
        match c.next(&store) {
            Next::Ready(n) => {
                order.push(n);
                mark_done(&mut store, c.step(n).unwrap(), "tv", "2026-10-22T08:00:00Z");
            }
            Next::Finished => break,
            Next::Blocked { .. } => panic!("nothing is parked, nothing can be blocked"),
        }
    }
    assert_eq!(order.len(), c.steps.len());
    assert_eq!(
        order,
        (1..=c.steps.len()).collect::<Vec<_>>(),
        "plan order is the walk order"
    );
    let p = c.progress(&store);
    assert_eq!(
        (p.done, p.total, p.open()),
        (c.steps.len(), c.steps.len(), 0)
    );
    assert!((p.fraction() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn several_steps_are_ready_at_once_after_the_rail() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let mut store = CommissioningStore::new();
    // Do everything up to and including the rails and ducts.
    for s in c.steps.iter().filter(|s| s.phase < BuildPhase::Devices) {
        mark_done(&mut store, s, "tv", "t1");
    }
    let ready = c.ready(&store);
    assert_eq!(
        ready.len(),
        3,
        "the three devices can be fitted in any order: {ready:?}"
    );
}

#[test]
fn a_problem_parks_the_step_raises_a_punch_item_and_blocks_what_depends_on_it() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let mut store = CommissioningStore::new();
    for s in c.steps.iter().filter(|s| s.phase < BuildPhase::Devices) {
        mark_done(&mut store, s, "tv", "t1");
    }
    let dev = c
        .steps
        .iter()
        .find(|s| s.phase == BuildPhase::Devices)
        .unwrap();
    let punch = mark_problem(
        &mut store,
        &c.tag,
        dev,
        "Rail is 5 mm too short",
        "tv",
        "t2",
    );
    assert_eq!(c.status(&store, dev), StepStatus::Problem);
    let item = store.punch(punch).unwrap();
    assert!(item.title.contains("+CAB"));
    assert_eq!(item.note, "Rail is 5 mm too short");
    assert_eq!(
        item.target,
        Some(Target::Device(novo_core::DeviceId::from(
            dev.device.unwrap()
        )))
    );
    // The other devices go on; once they are done the wires wait for the
    // parked one and the next step is reported as blocked.
    for s in c.steps.iter().filter(|s| s.phase == BuildPhase::Devices) {
        if s.number != dev.number {
            mark_done(&mut store, s, "tv", "t3");
        }
    }
    // Labels do not wait for the parked device's neighbours: work goes on
    // there. Once they are done, only blocked steps remain.
    for s in c.steps.iter().filter(|s| s.phase == BuildPhase::Labels) {
        if s.device != dev.device {
            mark_done(&mut store, s, "tv", "t3");
        }
    }
    match c.next(&store) {
        Next::Blocked { waits_for, .. } => assert_eq!(waits_for, dev.number),
        other => panic!("expected blocked, got {other:?}"),
    }
    assert_eq!(c.progress(&store).problems, 1);
    // Undo takes the tick back; the punch item stays.
    undo_build(&mut store, dev.key);
    assert_eq!(c.status(&store, dev), StepStatus::Open);
    assert!(store.punch(punch).is_some());
}

#[test]
fn a_ticked_step_that_changed_is_open_again() {
    let (mut doc, h) = starter();
    let cab = cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let mut store = CommissioningStore::new();
    let f2 = c
        .steps
        .iter()
        .find(|s| s.phase == BuildPhase::Devices && s.mark.ends_with("F2"))
        .unwrap();
    mark_done(&mut store, f2, "tv", "t1");
    assert_eq!(c.status(&store, f2), StepStatus::Done);
    // Move F2: the screw position in the instruction changes.
    doc.transact("Move", |tx| {
        tx.place_device(h.f2, cab, Point::new(120.0, 420.0), true, None)?;
        Ok(())
    })
    .unwrap();
    let again = plan_of(&doc);
    let c2 = &again.cabinets[0];
    let f2b = c2.find(f2.key).expect("same key after a move");
    assert_ne!(f2b.title, f2.title);
    assert_eq!(c2.status(&store, f2b), StepStatus::Changed);
    assert_eq!(c2.progress(&store).changed, 1);
    assert_eq!(
        c2.progress(&store).done,
        0,
        "changed does not count as done"
    );
}

#[test]
fn ticks_survive_the_project_file_and_old_files_still_load() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let mut store = CommissioningStore::new();
    mark_done(&mut store, &c.steps[0], "tv", "t1");
    let back = CommissioningStore::from_json(&store.to_json()).unwrap();
    assert_eq!(back, store);
    assert!(!back.is_empty());
    // A store written before Build mode has no `build` key.
    let old = CommissioningStore::from_json(r#"{"checks":[],"punch":[]}"#).unwrap();
    assert!(old.is_empty());
    // And a store without ticks does not write the key.
    assert!(!CommissioningStore::new().to_json().contains("build"));
}

fn package(doc: &Document, store: &CommissioningStore) -> FieldPackage {
    FieldPackage::new(
        "Starter",
        "tablet",
        "2026-10-22T10:00:00Z",
        store,
        doc.project(),
    )
}

#[test]
fn ticks_travel_in_the_field_file_and_the_newer_one_wins() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let (a, b) = (&c.steps[0], &c.steps[1]);

    // On the tablet: both ticked.
    let mut tablet = CommissioningStore::new();
    mark_done(&mut tablet, a, "tablet", "2026-10-22T09:00:00Z");
    mark_done(&mut tablet, b, "tablet", "2026-10-22T09:05:00Z");
    let json = package(&doc, &tablet).to_json();

    // At the desk: step a was already ticked later, as a problem.
    let mut desk = CommissioningStore::new();
    mark_problem(
        &mut desk,
        &c.tag,
        a,
        "wrong rail",
        "desk",
        "2026-10-22T11:00:00Z",
    );
    let report = FieldPackage::from_json(&json)
        .unwrap()
        .merge_into(&mut desk, doc.project());
    assert_eq!(report.build_added, 1, "b is new");
    assert_eq!(
        report.checks_kept, 1,
        "the desk's later decision on a stays"
    );
    assert_eq!(desk.build_tick(a.key).unwrap().state, TickState::Problem);
    assert_eq!(desk.build_tick(b.key).unwrap().by, "tablet");
    assert!(report.summary().contains("1 new and 0 changed build steps"));

    // Merging again changes nothing.
    let again = FieldPackage::from_json(&json)
        .unwrap()
        .merge_into(&mut desk, doc.project());
    assert_eq!(again.build_added + again.build_updated, 0);
}

#[test]
fn ticks_for_another_project_or_a_deleted_part_are_skipped() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let mut tablet = CommissioningStore::new();
    let mut foreign = c.steps[0].clone();
    // The same key, but another mark: another project's step.
    foreign.mark = "+OTHER".into();
    mark_done(&mut tablet, &foreign, "tablet", "t1");
    let mut gone = c.steps[1].clone();
    gone.key.subject = 999_999;
    mark_done(&mut tablet, &gone, "tablet", "t1");
    let mut desk = CommissioningStore::new();
    let report = package(&doc, &tablet).merge_into(&mut desk, doc.project());
    assert_eq!(report.build_added, 0);
    assert_eq!(report.mismatched_targets, 1);
    assert_eq!(report.unknown_targets, 1);
    assert!(desk.is_empty());
}

#[test]
fn stale_ticks_are_pruned_only_when_asked() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let mut store = CommissioningStore::new();
    mark_done(&mut store, &c.steps[0], "tv", "t1");
    let mut gone = c.steps[1].clone();
    gone.key.subject = 424_242;
    mark_done(&mut store, &gone, "tv", "t1");
    assert_eq!(store.build_ticks().count(), 2);
    store.retain_build(&build_keys(&plan));
    assert_eq!(store.build_ticks().count(), 1);
}

#[test]
fn a_problem_parked_twice_reuses_its_punch_item() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let plan = plan_of(&doc);
    let c = &plan.cabinets[0];
    let mut store = CommissioningStore::new();
    let a = mark_problem(&mut store, &c.tag, &c.steps[1], "first", "tv", "t1");
    undo_build(&mut store, c.steps[1].key);
    let b = mark_problem(&mut store, &c.tag, &c.steps[1], "second", "tv", "t2");
    assert_eq!(a, b);
    assert_eq!(store.punch_items().count(), 1);
    assert_eq!(store.punch(a).unwrap().note, "second");
}
