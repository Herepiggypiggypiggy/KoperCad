//! Wire-it: the bench sequence against the Gate 1 motor starter in a cabinet.

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, Value, Wire};
use novo_reports::{
    WIREIT_DONE_KEY, WireClass, WireItPlan, wire_it_done, wire_it_orphaned, wire_it_plan,
    wire_it_progress, wire_it_set_done,
};
use novo_tests::{MotorStarter, motor_starter};

fn cabinet() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).expect("fixture builds");
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
        tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, None)?;
        tx.place_device(h.k1, cab, Point::new(400.0, 100.0), false, None)?;
        let duct = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
        tx.set_rail_size(duct, 700.0, 50.0, 60.0)?;
        Ok(())
    })
    .expect("layout");
    let wires: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    doc.transact("Sizes", |tx| {
        for (i, id) in wires.into_iter().enumerate() {
            tx.update::<Wire>(id, |w| {
                // A mix: heavy black, green-yellow and thin blue wires.
                match i % 3 {
                    0 => {
                        w.cross_section_mm2 = Some(4.0);
                        w.color = Some("BK".into());
                    }
                    1 => {
                        w.cross_section_mm2 = Some(1.5);
                        w.color = Some("GNYE".into());
                    }
                    _ => {
                        w.cross_section_mm2 = Some(0.75);
                        w.color = Some("BU".into());
                    }
                }
            })?;
        }
        Ok(())
    })
    .expect("sizes");
    (doc, h)
}

fn tick(doc: &mut Document, plan: &WireItPlan, step: usize, on: bool) {
    let s = plan.steps[step].clone();
    doc.transact("Tick", |tx| wire_it_set_done(tx, &s, on))
        .expect("tick");
}

#[test]
fn every_connection_is_one_numbered_step() {
    let (doc, _) = cabinet();
    let jobs = novo_reports::wire_cut_jobs(doc.project());
    let plan = wire_it_plan(doc.project());
    assert!(!plan.steps.is_empty());
    assert_eq!(plan.steps.len(), jobs.len());
    for (i, s) in plan.steps.iter().enumerate() {
        assert_eq!(s.number, i + 1);
    }
    // A step is its two pins: the keys are all different.
    let keys: std::collections::BTreeSet<_> = plan.steps.iter().map(|s| &s.key).collect();
    assert_eq!(keys.len(), plan.steps.len());
}

#[test]
fn passes_run_earth_then_power_then_signals() {
    let (doc, _) = cabinet();
    let plan = wire_it_plan(doc.project());
    let classes: Vec<WireClass> = plan.steps.iter().map(|s| s.class).collect();
    let mut sorted = classes.clone();
    sorted.sort();
    assert_eq!(classes, sorted, "classes never go backwards");
    // The fixture mixes them, so more than one pass exists.
    assert!(classes.first() != classes.last());
    // Every green-yellow wire is in the earth pass, wherever it was drawn.
    for s in &plan.steps {
        if s.colour == "GNYE" {
            assert_eq!(s.class, WireClass::Earth, "{}", s.from);
        }
    }
}

#[test]
fn inside_a_track_short_wires_come_before_long_ones() {
    let (doc, _) = cabinet();
    let plan = wire_it_plan(doc.project());
    for class in [
        WireClass::Earth,
        WireClass::Power,
        WireClass::Supply,
        WireClass::Signal,
    ] {
        let steps: Vec<_> = plan.of_class(class).collect();
        for pair in steps.windows(2) {
            if pair[0].track == pair[1].track {
                assert!(
                    pair[0].cut_mm <= pair[1].cut_mm,
                    "{} then {} in {}",
                    pair[0].cut_mm,
                    pair[1].cut_mm,
                    pair[0].track
                );
            }
        }
        // A track is visited once: it never comes back after another.
        let mut left: Vec<&str> = Vec::new();
        for s in &steps {
            if left.last() != Some(&s.track.as_str()) {
                assert!(
                    !left.contains(&s.track.as_str()),
                    "track {} visited twice in {:?}",
                    s.track,
                    class
                );
                left.push(&s.track);
            }
        }
    }
}

#[test]
fn duct_fill_only_rises_and_ends_where_the_wires_leave_it() {
    let (doc, _) = cabinet();
    let plan = wire_it_plan(doc.project());
    let mut last: std::collections::BTreeMap<u64, f64> = Default::default();
    let mut routed = 0;
    for s in &plan.steps {
        for d in &s.ducts {
            routed += 1;
            let after = d.fill_after.expect("the duct has a size");
            let before = d.fill_before.expect("the duct has a size");
            assert!(after > before, "a wire in a duct adds to it");
            let prev = last.insert(d.duct, after).unwrap_or(0.0);
            assert!((before - prev).abs() < 1e-9, "before is the last after");
            assert!(d.wires_after >= 1);
        }
    }
    assert!(routed > 0, "the fixture routes wires through the duct");
    assert!(
        plan.steps
            .iter()
            .any(|s| s.fill_text().contains(" now ") && s.fill_text().contains(" %")),
        "{}",
        plan.steps[0].fill_text()
    );
}

#[test]
fn progress_is_kept_on_the_wires_and_never_changes_the_order() {
    let (mut doc, _) = cabinet();
    let plan = wire_it_plan(doc.project());
    let before = wire_it_progress(doc.project(), &plan);
    assert_eq!(before.done, 0);
    assert_eq!(before.next, Some(0));
    tick(&mut doc, &plan, 0, true);
    tick(&mut doc, &plan, 1, true);
    let again = wire_it_plan(doc.project());
    assert_eq!(*plan, *again, "ticking changes nothing about the order");
    let p = wire_it_progress(doc.project(), &again);
    assert_eq!((p.done, p.next), (2, Some(2)));
    assert!(wire_it_done(doc.project(), &again.steps[0]));
    // Un-ticking the first step makes it the next one again.
    tick(&mut doc, &again, 0, false);
    let p = wire_it_progress(doc.project(), &again);
    assert_eq!((p.done, p.next), (1, Some(0)));
    // Undo goes back through the tick.
    doc.undo().expect("undo");
    let p = wire_it_progress(doc.project(), &again);
    assert_eq!(p.done, 2);
}

#[test]
fn legs_of_one_drawn_net_are_ticked_one_at_a_time() {
    let (mut doc, _) = cabinet();
    let plan = wire_it_plan(doc.project());
    // A T-junction is one drawn net and several connections.
    let shared = plan.steps.iter().enumerate().find_map(|(i, s)| {
        plan.steps
            .iter()
            .position(|t| t.number != s.number && t.wires == s.wires)
            .map(|j| (i, j))
    });
    let (a, b) = shared.expect("the motor starter has a net with two legs");
    tick(&mut doc, &plan, a, true);
    assert!(wire_it_done(doc.project(), &plan.steps[a]));
    assert!(
        !wire_it_done(doc.project(), &plan.steps[b]),
        "the other leg of the same drawn wires is not run"
    );
    tick(&mut doc, &plan, b, true);
    tick(&mut doc, &plan, a, false);
    assert!(!wire_it_done(doc.project(), &plan.steps[a]));
    assert!(wire_it_done(doc.project(), &plan.steps[b]));
}

#[test]
fn a_tick_whose_connection_is_gone_is_counted_not_lost() {
    let (mut doc, _) = cabinet();
    let plan = wire_it_plan(doc.project());
    assert_eq!(wire_it_orphaned(doc.project(), &plan), 0);
    let w = plan.steps[0].wires[0];
    doc.transact("stale tick", |tx| {
        tx.set_property(
            w,
            WIREIT_DONE_KEY,
            Some(Value::Text("9999.0>9998.1".into())),
        )
    })
    .unwrap();
    let again = wire_it_plan(doc.project());
    assert_eq!(wire_it_orphaned(doc.project(), &again), 1);
    // And it does not count as progress.
    assert_eq!(wire_it_progress(doc.project(), &again).done, 0);
}

#[test]
fn the_cut_and_end_figures_are_the_wire_production_figures() {
    let (doc, _) = cabinet();
    let jobs = novo_reports::wire_cut_jobs(doc.project());
    let plan = wire_it_plan(doc.project());
    for s in &plan.steps {
        let j = &jobs[jobs
            .iter()
            .position(|j| {
                j.end_a.designation == s.from && j.end_b.designation == s.to && j.wires == s.wires
            })
            .expect("step has a job")];
        assert_eq!(s.cut_mm, j.cut_mm);
        assert_eq!(s.strip_from_mm, j.end_a.strip_mm);
        assert_eq!(s.fitting_from, j.end_a.sleeve_text());
        assert_eq!(s.label_to, j.end_b.label);
        assert!((2.0 * s.allowance_mm + s.length_mm - s.cut_mm).abs() < 1e-6);
        assert_eq!(s.track, j.track);
    }
}

#[test]
fn a_routed_step_carries_its_path_on_the_plate_and_an_unrouted_one_does_not() {
    let (doc, _) = cabinet();
    let plan = wire_it_plan(doc.project());
    let cab = doc
        .project()
        .structures()
        .find(|s| s.tag.to_string().contains("CAB") && s.plate.is_some())
        .expect("the cabinet")
        .id;
    let routed: Vec<_> = plan.steps.iter().filter(|s| s.path.is_some()).collect();
    assert!(!routed.is_empty(), "wires between F1 and K1 are routed");
    for s in &routed {
        let p = s.path.as_ref().unwrap();
        assert_eq!(p.plate, cab);
        assert!(p.points.len() >= 2, "{:?}", p.points);
        assert!(p.points.iter().all(|(x, y)| x.is_finite() && y.is_finite()));
        // A wire in a duct is not a free-air wire.
        assert_eq!(p.free_air, s.ducts.is_empty(), "{}", s.track);
    }
    // A feed or an unplaced device has no path.
    assert!(plan.steps.iter().any(|s| s.path.is_none()));
}
