use novo_core::{CabinetPlacement, DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::{load, place_device, set_enclosure, set_plate};

fn project(door: bool) -> (Project, StructureId, u64, u64) {
    let mut p = Project::new("t");
    let (r, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 560.0,
                    height: 760.0,
                    margin: 10.0,
                },
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 600.0,
                    height: 800.0,
                    depth: 250.0,
                    plate_setback: 20.0,
                    door: door.then(|| Door {
                        hinge: Hinge::Left,
                        ..Door::default()
                    }),
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            let part = tx.add_part("ACME", "P1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(40.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(40.0)))?;
            tx.set_property(part, "depth_mm", Some(Value::Float(40.0)))?;
            let k1 = tx.add_device(DeviceTag::parse("=A1+CAB1-K1").unwrap(), "")?;
            let h1 = tx.add_device(DeviceTag::parse("=A1+CAB1-H1").unwrap(), "")?;
            tx.assign_part(k1, part)?;
            tx.assign_part(h1, part)?;
            // The same spot on two different surfaces.
            place_device(tx, cab, k1.raw(), 100.0, 100.0, false)?;
            place_device(tx, cab, h1.raw(), 100.0, 100.0, false)?;
            Ok((cab, k1.raw(), h1.raw()))
        })
        .unwrap();
    let (cab, k1, h1) = r;
    (p, cab, k1, h1)
}

fn put_on(p: &mut Project, device: u64, surface: &str) {
    let id = p
        .all::<CabinetPlacement>()
        .find(|c| c.device.raw() == device)
        .unwrap()
        .id;
    p.transact("surface", |tx| {
        tx.set_property(id, "surface", Some(Value::Text(surface.into())))
    })
    .unwrap();
}

#[test]
fn a_placement_on_the_door_becomes_a_door_device_and_leaves_the_plate_checks() {
    let (mut p, cab, k1, h1) = project(true);
    let before = load(&p, cab).unwrap();
    assert_eq!(
        before.report().collisions.len(),
        1,
        "same spot, both on the plate"
    );
    put_on(&mut p, h1, "Door");
    let l = load(&p, cab).unwrap();
    assert_eq!(l.door_devices.len(), 1);
    let d = &l.door_devices[0];
    assert_eq!((d.id, d.depth), (h1, 40.0));
    assert_eq!(
        (d.rect.x, d.rect.y, d.rect.w, d.rect.h),
        (100.0, 100.0, 40.0, 40.0)
    );
    assert_eq!(l.off_plate.get(&h1).map(String::as_str), Some("door"));
    assert!(l.report().collisions.is_empty());
    assert_eq!(l.on_plate().len(), 1);
    assert_eq!(l.on_plate()[0].id, k1);
    // Plate device 100 deep reaches the door device only at the right spot:
    // here it does not, so the sweep is clean and nothing is unchecked.
    let s = l.swing_report().unwrap();
    assert!(s.is_clean() && s.swept == 1 && s.unchecked_plate.is_empty());
}

#[test]
fn a_door_device_near_the_strike_wall_gives_a_real_swing_hit() {
    let (mut p, cab, _k1, h1) = project(true);
    // Move it so its right edge is 0.5 mm from the right wall, then put it
    // on the door.
    p.transact("move", |tx| place_device(tx, cab, h1, 559.5, 100.0, false))
        .unwrap();
    put_on(&mut p, h1, "door");
    let s = load(&p, cab).unwrap().swing_report().unwrap();
    assert_eq!(s.hits.len(), 1, "{s:?}");
    assert_eq!(s.hits[0].door_device, h1);
}

#[test]
fn without_a_door_the_placement_stays_on_the_plate() {
    let (mut p, cab, _k1, h1) = project(false);
    put_on(&mut p, h1, "door");
    let l = load(&p, cab).unwrap();
    assert!(l.door_devices.is_empty() && l.off_plate.is_empty());
    assert_eq!(l.report().collisions.len(), 1);
}

#[test]
fn other_surfaces_leave_the_plate_checks_but_are_not_door_devices() {
    let (mut p, cab, _k1, h1) = project(true);
    put_on(&mut p, h1, "left");
    let l = load(&p, cab).unwrap();
    assert!(l.door_devices.is_empty());
    assert_eq!(l.off_plate.get(&h1).map(String::as_str), Some("left"));
    assert!(l.report().collisions.is_empty());
}

#[test]
fn a_door_device_does_not_push_the_plate_proposal_down() {
    use novo_layout::Size;
    use novo_layout::plan::Candidate;
    let (mut p, cab, _k1, h1) = project(true);
    let cands = [Candidate {
        id: 99,
        size: Size {
            width: 40.0,
            height: 40.0,
            guessed: false,
        },
        sort_key: "Z".into(),
    }];
    let before = load(&p, cab).unwrap().preview(&cands);
    p.transact("move", |tx| place_device(tx, cab, h1, 100.0, 700.0, false))
        .unwrap();
    put_on(&mut p, h1, "door");
    let after = load(&p, cab).unwrap().preview(&cands);
    assert_eq!(before.placements.len(), 1);
    assert_eq!(after.placements[0].y, before.placements[0].y);
}
