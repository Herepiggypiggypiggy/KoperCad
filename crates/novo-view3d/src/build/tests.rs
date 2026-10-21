use super::*;
use novo_layout::{Placement, Plate, Rail, Size};

fn layout() -> CabinetLayout {
    let mut l = CabinetLayout::new(Plate {
        width: 400.0,
        height: 500.0,
        margin: 10.0,
    });
    l.rails.push(Rail {
        id: 2,
        x: 20.0,
        y: 100.0,
        length: 300.0,
        vertical: false,
    });
    l.placements.push(Placement {
        id: 5,
        x: 30.0,
        y: 80.0,
        rotated: false,
        size: Size::from_part(Some(45.0), Some(90.0)),
        clearance: None,
        margins: Default::default(),
        rail: Some(2),
    });
    l
}

fn inputs() -> Inputs {
    let mut i = Inputs::default();
    i.depths.insert(
        5,
        Depth {
            mm: 70.0,
            guessed: false,
        },
    );
    i.ducts.push(DuctShape {
        id: 9,
        x: 20.0,
        y: 250.0,
        length: 300.0,
        vertical: false,
        width: 40.0,
        height: 60.0,
    });
    i
}

fn id(kind: ItemKind, raw: u64) -> ItemId {
    ItemId::new(kind, raw)
}

fn build() -> Scene {
    from_layout(&layout(), &inputs(), &Options::default())
}

#[test]
fn plate_rail_and_device_stack_by_depth() {
    let s = build();
    // Plate origin (back face middle) is 3 mm behind the surface.
    let plate = s.get(id(ItemKind::Plate, 0)).unwrap();
    assert_eq!(plate.pose.translation, Vec3::new(200.0, 250.0, -3.0));
    assert_eq!(plate.size.z, 3.0);
    // Rail centred on its line: centre x = 20 + 150, 2D y = 100 -> world 400.
    let rail = s.get(id(ItemKind::Rail, 2)).unwrap();
    assert_eq!(rail.pose.translation, Vec3::new(170.0, 400.0, 0.0));
    assert_eq!(rail.size, Vec3::new(300.0, 35.0, 7.5));
    assert_eq!(rail.shape, Shape::Profile(Profile::TopHat35x7_5));
    // Device: footprint centre (52.5, 125), back on the rail top.
    let dev = s.get(id(ItemKind::Device, 5)).unwrap();
    assert_eq!(dev.pose.translation, Vec3::new(52.5, 375.0, 7.5));
    assert_eq!(dev.size, Vec3::new(45.0, 90.0, 70.0));
    assert_eq!(dev.mark, Mark::None);
}

#[test]
fn rotated_device_keeps_its_size_and_turns_clockwise() {
    let mut l = layout();
    l.placements[0].rotated = true;
    l.placements[0].rail = None;
    let s = from_layout(&l, &inputs(), &Options::default());
    let dev = s.get(id(ItemKind::Device, 5)).unwrap();
    assert_eq!(dev.size, Vec3::new(45.0, 90.0, 70.0));
    assert_eq!(dev.pose.rotation, Quat::from_rotation_z(-FRAC_PI_2));
    // Footprint is 90 x 45 at (30, 80): centre (75, 102.5), off the rail.
    assert_eq!(dev.pose.translation, Vec3::new(75.0, 397.5, 0.0));
    // World bounds match the 2D footprint.
    let b = s.world_bounds(dev);
    assert!((b.size().x - 90.0).abs() < 1e-3 && (b.size().y - 45.0).abs() < 1e-3);
}

#[test]
fn unknown_depth_and_duct_section_are_marked_not_zero() {
    let mut i = inputs();
    i.depths.clear();
    i.ducts[0].height = 0.0;
    let s = from_layout(&layout(), &i, &Options::default());
    let dev = s.get(id(ItemKind::Device, 5)).unwrap();
    assert_eq!(dev.size.z, DEFAULT_DEPTH_MM as f32);
    assert_eq!(dev.mark, Mark::GuessedSize);
    let duct = s.get(id(ItemKind::Duct, 9)).unwrap();
    assert_eq!(duct.size.z, DEFAULT_DUCT_MM.1 as f32);
    assert_eq!(duct.mark, Mark::GuessedSize);
}

#[test]
fn marks_follow_the_layout_report() {
    let mut l = layout();
    l.placements.push(Placement {
        id: 6,
        x: 60.0, // overlaps device 5
        y: 80.0,
        rotated: false,
        size: Size::from_part(Some(45.0), Some(90.0)),
        clearance: None,
        margins: Default::default(),
        rail: None,
    });
    l.placements.push(Placement {
        id: 7,
        x: 380.0, // leaves the usable plate
        y: 300.0,
        rotated: false,
        size: Size::from_part(Some(45.0), Some(90.0)),
        clearance: None,
        margins: Default::default(),
        rail: None,
    });
    let s = from_layout(&l, &inputs(), &Options::default());
    assert_eq!(
        s.get(id(ItemKind::Device, 5)).unwrap().mark,
        Mark::Collision
    );
    assert_eq!(
        s.get(id(ItemKind::Device, 6)).unwrap().mark,
        Mark::Collision
    );
    assert_eq!(s.get(id(ItemKind::Device, 7)).unwrap().mark, Mark::Outside);
}

#[test]
fn the_door_hangs_on_a_hinge_on_the_chosen_side() {
    let mut o = Options::default();
    o.shell.door = Some(DoorSpec {
        hinge: HingeSide::Right,
        open_deg: 90.0,
    });
    let s = from_layout(&layout(), &inputs(), &o);
    let door = s.get(id(ItemKind::Door, 0)).unwrap();
    assert_eq!(door.group, Some(DOOR_HINGE));
    assert!(door.translucent);
    assert_eq!(door.pose.translation.z, DEFAULT_ENCLOSURE_DEPTH_MM as f32);
    let g = s.group(DOOR_HINGE).unwrap();
    assert_eq!(g.pivot, Vec3::new(400.0, 0.0, 250.0));
    // Open 90 degrees: the free (left) edge swings out towards +Z.
    let b = s.world_bounds(door);
    assert!(b.max.z > 600.0, "door should reach out, got {:?}", b);

    o.shell.door = Some(DoorSpec {
        hinge: HingeSide::Left,
        open_deg: 90.0,
    });
    let s = from_layout(&layout(), &inputs(), &o);
    let b = s.world_bounds(s.get(id(ItemKind::Door, 0)).unwrap());
    assert!(b.max.z > 600.0, "left hinge also swings out, got {:?}", b);

    o.shell.door = None;
    let s = from_layout(&layout(), &inputs(), &o);
    assert!(s.get(id(ItemKind::Door, 0)).is_none() && s.groups.is_empty());
}

#[test]
fn a_device_that_reaches_the_door_is_reported() {
    let mut o = Options::default();
    o.shell.depth = 80.0; // device front at 7.5 + 70 = 77.5
    assert!(reaching_door(&from_layout(&layout(), &inputs(), &o)).is_empty());
    o.shell.depth = 77.5;
    assert_eq!(
        reaching_door(&from_layout(&layout(), &inputs(), &o)),
        vec![(5, 0.0)]
    );
    o.shell.depth = 60.0;
    assert_eq!(
        reaching_door(&from_layout(&layout(), &inputs(), &o)),
        vec![(5, 17.5)]
    );
}

#[test]
fn wires_are_runs_mid_duct_or_in_free_air() {
    let o = Options {
        wires: vec![
            WireInput {
                id: 1,
                kind: WireKind::Ducted,
                points: vec![(20.0, 250.0), (120.0, 250.0), (120.0, 200.0)],
                ducts: vec![9],
                diameter_mm: Some(4.0),
                colour: None,
            },
            WireInput {
                id: 2,
                kind: WireKind::FreeAir,
                points: vec![(0.0, 0.0), (0.0, 10.0)],
                ducts: vec![],
                diameter_mm: None,
                colour: None,
            },
        ],
        ..Options::default()
    };
    let s = from_layout(&layout(), &inputs(), &o);
    let wire = |n: u64| {
        s.wires
            .iter()
            .find(|w| w.id == id(ItemKind::Wire, n))
            .unwrap()
    };
    // Ducted: half way up the 60 mm duct, radius from the 4 mm diameter.
    let a = wire(1);
    assert_eq!(a.radius, 2.0);
    assert_eq!(a.mark, Mark::None);
    assert_eq!(a.points[0], Vec3::new(20.0, 250.0, 30.0));
    assert_eq!(a.points[2], Vec3::new(120.0, 300.0, 30.0));
    // Free air: flagged, default diameter, 15 mm up.
    let b = wire(2);
    assert_eq!(b.mark, Mark::Outside);
    assert_eq!(b.radius, WIRE_DIAMETER_MM as f32 / 2.0);
    assert_eq!(b.points[0].z, 15.0);
    assert_eq!(
        s.items
            .iter()
            .filter(|i| i.id.kind == ItemKind::Wire)
            .count(),
        0
    );
}

#[test]
fn the_same_inputs_give_the_same_scene() {
    assert_eq!(build(), build());
}

#[test]
fn bounds_cover_shell_plate_and_closed_door() {
    let b = build().bounds();
    assert_eq!(b.min.z, -3.0);
    assert_eq!(
        b.max.z,
        DEFAULT_ENCLOSURE_DEPTH_MM as f32 + DOOR_THICKNESS_MM as f32
    );
}

#[test]
fn refresh_bumps_the_right_counter() {
    let mut s = build();
    refresh(&mut s, build());
    assert_eq!((s.revision, s.pose_revision), (0, 0));

    // Move a device: items changed.
    let mut l = layout();
    l.placements[0].x += 10.0;
    refresh(&mut s, from_layout(&l, &inputs(), &Options::default()));
    assert_eq!((s.revision, s.pose_revision), (1, 0));

    // Only the door angle: pose revision.
    let mut o = Options::default();
    o.shell.door = Some(DoorSpec {
        hinge: HingeSide::Left,
        open_deg: 45.0,
    });
    refresh(&mut s, from_layout(&l, &inputs(), &o));
    assert_eq!((s.revision, s.pose_revision), (1, 1));
}

#[test]
fn a_vertical_rail_runs_down_the_plate() {
    let mut l = layout();
    l.rails[0].vertical = true;
    let s = from_layout(&l, &inputs(), &Options::default());
    let rail = s.get(id(ItemKind::Rail, 2)).unwrap();
    // Starts at 2D (20, 100) and runs 300 down: centre 2D (20, 250) -> world y 250.
    assert_eq!(rail.pose.translation, Vec3::new(20.0, 250.0, 0.0));
    assert_eq!(rail.pose.rotation, Quat::from_rotation_z(FRAC_PI_2));
    let b = s.world_bounds(rail);
    assert!((b.size().y - 300.0).abs() < 1e-3);
}

#[test]
fn a_device_takes_its_body_colour_from_its_family() {
    let mut inp = inputs();
    inp.classes.insert(5, DeviceClass::Switching);
    let s = from_layout(&layout(), &inp, &Options::default());
    let dev = s.get(id(ItemKind::Device, 5)).unwrap();
    assert_eq!(dev.tint, Some(DeviceClass::Switching.tint()));
    // No class known: the kind's own colour applies.
    assert_eq!(build().get(id(ItemKind::Device, 5)).unwrap().tint, None);
}

#[test]
fn a_device_of_a_known_family_gets_its_typical_shape_at_its_own_size() {
    let mut inp = inputs();
    inp.classes.insert(5, DeviceClass::Protection);
    let s = from_layout(&layout(), &inp, &Options::default());
    let dev = s.get(id(ItemKind::Device, 5)).unwrap();
    let Shape::Part(k) = dev.shape else {
        panic!("a typical shape")
    };
    assert_eq!(
        crate::device_shape::spec(k),
        Some((DeviceClass::Protection, dev.size))
    );
    // A generic or unknown device stays a box.
    inp.classes.insert(5, DeviceClass::Generic);
    let s = from_layout(&layout(), &inp, &Options::default());
    assert_eq!(s.get(id(ItemKind::Device, 5)).unwrap().shape, Shape::Box);
    assert_eq!(
        build().get(id(ItemKind::Device, 5)).unwrap().shape,
        Shape::Box
    );
}

#[test]
fn a_turned_device_hands_the_scene_its_needs_on_the_sides_of_the_plate() {
    use novo_layout::Margins;
    let mut l = layout();
    l.placements[0].rotated = true;
    l.placements[0].margins = Margins {
        above: Some(30.0),
        below: Some(40.0),
        left: Some(10.0),
        right: Some(20.0),
    };
    let s = from_layout(&l, &inputs(), &Options::default());
    let item = s.get(id(ItemKind::Device, 5)).unwrap();
    // The data sheet's sides turn with the part, 90 degrees clockwise: what it
    // calls below is now on the left, above on the right, left above, right below.
    assert_eq!(item.needs, Some([40.0, 30.0, 10.0, 20.0]));
    // The zone then grows the turned footprint by those, in world sides.
    let b = s.world_bounds(item);
    let sel = std::collections::BTreeSet::from([item.id]);
    let zone = crate::dimension::clearance_zones(&s, &sel);
    assert!((zone[0][0].x - (b.min.x - 40.0)).abs() < 0.01);
    assert!((zone[0][2].x - (b.max.x + 30.0)).abs() < 0.01);
    assert!((zone[0][0].y - (b.min.y - 20.0)).abs() < 0.01);
    assert!((zone[0][2].y - (b.max.y + 10.0)).abs() < 0.01);
}
