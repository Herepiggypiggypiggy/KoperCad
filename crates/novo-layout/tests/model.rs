use novo_core::{DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::model::{
    apply_plan, candidates, device_size, load, place_device, rotate_device, set_plate,
    unplace_device,
};

fn project() -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(90)))?;
            for tag in ["=A1+CAB1-K1", "=A2+CAB1-K2", "=A1+CAB1-Q1", "=A1+FIELD-M1"] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                if tag != "=A2+CAB1-K2" {
                    tx.assign_part(d, part)?;
                }
            }
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                },
            )?;
            Ok(cab)
        })
        .unwrap();
    (p, cab)
}

#[test]
fn candidates_are_the_devices_at_the_location_across_function_groups() {
    let (p, cab) = project();
    let c = candidates(&p, cab);
    assert_eq!(c.len(), 3); // not the FIELD motor
    let guessed = c.iter().filter(|c| c.size.guessed).count();
    assert_eq!(guessed, 1); // K2 has no part
    let d = p.all::<novo_core::Device>().next().unwrap();
    assert!(!device_size(&p, d).guessed);
}

#[test]
fn no_plate_means_no_layout() {
    let mut p = Project::new("t");
    let (s, _) = p
        .transact("s", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB2").unwrap(), "", true)
        })
        .unwrap();
    assert!(load(&p, s).is_none());
}

#[test]
fn place_all_is_one_undo_step_and_reloads_as_the_planned_layout() {
    let (mut p, cab) = project();
    let layout = load(&p, cab).unwrap();
    let cands = candidates(&p, cab);
    let plan = layout.preview(&cands);
    assert_eq!(plan.placements.len(), 3);
    let (_, tx) = p
        .transact("Place all", |tx| apply_plan(tx, cab, &plan))
        .unwrap();

    let after = load(&p, cab).unwrap();
    assert_eq!(after.placements.len(), 3);
    assert!(!after.rails.is_empty());
    assert!(after.placements.iter().all(|q| q.rail.is_some()));
    assert!(after.report().is_clean());
    assert!(after.preview(&cands).placements.is_empty());

    p.apply(&tx.inverse()).unwrap();
    let undone = load(&p, cab).unwrap();
    assert!(undone.placements.is_empty() && undone.rails.is_empty());
}

#[test]
fn a_second_place_all_skips_placed_devices_and_stays_below_them() {
    let (mut p, cab) = project();
    let cands = candidates(&p, cab);
    let first = load(&p, cab).unwrap().preview(&cands[..1]);
    p.transact("one", |tx| apply_plan(tx, cab, &first)).unwrap();
    let layout = load(&p, cab).unwrap();
    let placed_bottom = layout.placements[0].footprint().bottom();
    let plan = layout.preview(&cands);
    assert_eq!(plan.placements.len(), 2);
    assert!(plan.placements.iter().all(|q| q.y >= placed_bottom));
    p.transact("rest", |tx| apply_plan(tx, cab, &plan)).unwrap();
    assert!(load(&p, cab).unwrap().report().is_clean());
}

#[test]
fn part_clearances_reach_the_clearance_check_and_a_device_can_override_them() {
    use novo_core::{Command, Device};
    let (mut p, cab) = project();
    let part = p.all::<novo_core::Part>().next().unwrap().id;
    let devices: Vec<_> = p
        .all::<Device>()
        .filter(|d| d.tag.location == ["CAB1"] && !d.parts.is_empty())
        .map(|d| d.id)
        .collect();
    let (k1, q1) = (devices[0], devices[1]);
    p.transact("place", |tx| {
        for (device, x) in [(k1, 10.0), (q1, 75.0)] {
            // 20 mm between the 45 mm wide footprints.
            tx.execute(Command::PlaceDevice {
                device,
                structure: cab,
                x,
                y: 10.0,
                rotated: false,
                rail: None,
            })?;
        }
        Ok(())
    })
    .unwrap();
    // Without data the default 10 mm gap is kept.
    assert!(load(&p, cab).unwrap().report().too_close.is_empty());

    // The part asks for 40 mm at its right; the left neighbour is its own.
    p.transact("part data", |tx| {
        tx.set_property(part, "clearance_right_mm", Some(Value::Int(40)))
    })
    .unwrap();
    let too_close = load(&p, cab).unwrap().report().too_close;
    assert_eq!(too_close.len(), 1);
    assert_eq!(too_close[0].2, 20.0);

    // A device can say it needs less than its part does (and a negative
    // value is ignored, falling back to the part).
    p.transact("override", |tx| {
        tx.set_property(k1, "clearance_right_mm", Some(Value::Float(-5.0)))?;
        tx.set_property(q1, "clearance_left_mm", Some(Value::Int(0)))
    })
    .unwrap();
    assert_eq!(load(&p, cab).unwrap().report().too_close.len(), 1);
    p.transact("override", |tx| {
        tx.set_property(k1, "clearance_right_mm", Some(Value::Int(15)))
    })
    .unwrap();
    assert!(load(&p, cab).unwrap().report().too_close.is_empty());
}

#[test]
fn place_rotate_move_and_remove_a_device() {
    let (mut p, cab) = project();
    let cands = candidates(&p, cab);
    let id = cands[0].id;
    p.transact("drop", |tx| place_device(tx, cab, id, 50.0, 60.0, false))
        .unwrap();
    let l = load(&p, cab).unwrap();
    assert_eq!(
        (
            l.placements[0].x,
            l.placements[0].y,
            l.placements[0].rotated
        ),
        (50.0, 60.0, false)
    );
    p.transact("turn", |tx| rotate_device(tx, id)).unwrap();
    assert!(load(&p, cab).unwrap().placements[0].rotated);
    p.transact("move", |tx| place_device(tx, cab, id, 100.0, 120.0, true))
        .unwrap();
    let l = load(&p, cab).unwrap();
    assert_eq!(l.placements.len(), 1);
    assert_eq!((l.placements[0].x, l.placements[0].y), (100.0, 120.0));
    p.transact("remove", |tx| unplace_device(tx, id)).unwrap();
    assert!(load(&p, cab).unwrap().placements.is_empty());
    // Turning or removing a device that is not placed does nothing.
    p.transact("noop", |tx| {
        rotate_device(tx, id)?;
        unplace_device(tx, id)
    })
    .unwrap();
}
