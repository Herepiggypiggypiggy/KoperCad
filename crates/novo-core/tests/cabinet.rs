//! Mounting plates, rails and placed devices (cabinet layout).

use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn created(doc: &mut Document, command: Command) -> AnyId {
    doc.execute(command).unwrap().created[0]
}

struct Cabinet {
    doc: Document,
    cab: StructureId,
    q1: DeviceId,
    q2: DeviceId,
}

fn cabinet() -> Cabinet {
    let mut doc = Document::default();
    let AnyId::Structure(cab) = created(
        &mut doc,
        Command::AddStructure {
            tag: tag("+K1"),
            description: String::new(),
            enclosure: true,
        },
    ) else {
        unreachable!()
    };
    let mut device = |t: &str| match created(
        &mut doc,
        Command::AddDevice {
            tag: tag(t),
            description: String::new(),
            kind: Default::default(),
        },
    ) {
        AnyId::Device(d) => d,
        other => panic!("{other:?}"),
    };
    let (q1, q2) = (device("-Q1"), device("-Q2"));
    doc.execute(Command::SetPlateSize {
        structure: cab,
        plate: Some(MountingPlate {
            width: 600.0,
            height: 800.0,
            margin: 20.0,
        }),
    })
    .unwrap();
    Cabinet { doc, cab, q1, q2 }
}

fn rail(c: &mut Cabinet) -> RailId {
    match created(
        &mut c.doc,
        Command::AddRail {
            structure: c.cab,
            x: 20.0,
            y: 100.0,
            length: 560.0,
            kind: RailKind::DinRail,
        },
    ) {
        AnyId::Rail(r) => r,
        other => panic!("{other:?}"),
    }
}

fn place(c: &mut Cabinet, device: DeviceId, x: f64, rail: Option<(RailId, f64)>) -> Outcome {
    c.doc
        .execute(Command::PlaceDevice {
            device,
            structure: c.cab,
            x,
            y: 90.0,
            rotated: false,
            rail,
        })
        .unwrap()
}

fn placements(doc: &Document) -> Vec<CabinetPlacement> {
    doc.project().all::<CabinetPlacement>().cloned().collect()
}

#[test]
fn a_device_is_placed_moved_rotated_and_removed() {
    let mut c = cabinet();
    let q1 = c.q1;
    let rail = rail(&mut c);
    let out = place(&mut c, q1, 30.0, Some((rail, 10.0)));
    let AnyId::CabinetPlacement(id) = out.created[0] else {
        panic!("{out:?}")
    };
    assert_eq!(placements(&c.doc)[0].rail_offset, Some(10.0));

    // Placing again moves it: no second placement, nothing created.
    let moved = place(&mut c, q1, 80.0, None);
    assert!(moved.created.is_empty());
    let all = placements(&c.doc);
    assert_eq!((all.len(), all[0].x, all[0].rail), (1, 80.0, None));
    assert_eq!(all[0].rail_offset, None);

    c.doc
        .execute(Command::RotatePlacement {
            placement: id,
            rotated: true,
        })
        .unwrap();
    assert!(placements(&c.doc)[0].rotated);
    c.doc.undo().unwrap();
    assert!(!placements(&c.doc)[0].rotated);

    c.doc
        .execute(Command::RemovePlacement { placement: id })
        .unwrap();
    assert!(placements(&c.doc).is_empty());
    c.doc.undo().unwrap();
    assert_eq!(placements(&c.doc).len(), 1);
}

#[test]
fn place_all_is_one_undo_step() {
    let mut c = cabinet();
    let rail = rail(&mut c);
    let command = |device, x| Command::PlaceDevice {
        device,
        structure: c.cab,
        x,
        y: 90.0,
        rotated: false,
        rail: Some((rail, x - 20.0)),
    };
    let batch = Command::Batch {
        label: "Place all".into(),
        commands: vec![command(c.q1, 20.0), command(c.q2, 60.0)],
    };
    c.doc.execute(batch).unwrap();
    assert_eq!(placements(&c.doc).len(), 2);
    c.doc.undo().unwrap();
    assert!(placements(&c.doc).is_empty());
}

#[test]
fn deleting_things_cleans_up_placements() {
    let mut c = cabinet();
    let (q1, q2) = (c.q1, c.q2);
    let rail = rail(&mut c);
    place(&mut c, q1, 30.0, Some((rail, 10.0)));
    place(&mut c, q2, 100.0, None);

    // The device stays on the plate, off the rail.
    c.doc.execute(Command::Remove { id: rail.into() }).unwrap();
    let all = placements(&c.doc);
    assert_eq!(all.len(), 2);
    assert!(
        all.iter()
            .all(|p| p.rail.is_none() && p.rail_offset.is_none())
    );

    // The placement goes with its device and with its enclosure.
    c.doc.execute(Command::Remove { id: c.q1.into() }).unwrap();
    assert_eq!(placements(&c.doc).len(), 1);
    c.doc.execute(Command::Remove { id: c.cab.into() }).unwrap();
    assert!(placements(&c.doc).is_empty());
    c.doc.undo().unwrap();
    assert_eq!(placements(&c.doc).len(), 1);
}

#[test]
fn the_model_refuses_what_cannot_be_placed() {
    let mut c = cabinet();
    let rail = rail(&mut c);
    let plate = |w, h, m| Command::SetPlateSize {
        structure: c.cab,
        plate: Some(MountingPlate {
            width: w,
            height: h,
            margin: m,
        }),
    };
    for bad in [
        plate(0.0, 800.0, 0.0),
        plate(600.0, 800.0, -1.0),
        plate(100.0, 800.0, 50.0),
    ] {
        assert!(c.doc.execute(bad).is_err());
    }
    assert!(
        c.doc
            .execute(Command::AddRail {
                structure: c.cab,
                x: 0.0,
                y: 0.0,
                length: f64::NAN,
                kind: RailKind::Duct,
            })
            .is_err()
    );
    let at = |x: f64, rail| Command::PlaceDevice {
        device: c.q1,
        structure: c.cab,
        x,
        y: 0.0,
        rotated: false,
        rail,
    };
    assert!(c.doc.execute(at(f64::INFINITY, None)).is_err());
    assert!(c.doc.execute(at(0.0, Some((RailId(9999), 0.0)))).is_err());
    assert!(c.doc.execute(at(0.0, Some((rail, f64::NAN)))).is_err());

    // Not an enclosure with a plate: no rails, no placed devices.
    let AnyId::Structure(plain) = created(
        &mut c.doc,
        Command::AddStructure {
            tag: tag("+K2"),
            description: String::new(),
            enclosure: false,
        },
    ) else {
        unreachable!()
    };
    assert!(
        c.doc
            .execute(Command::SetPlateSize {
                structure: plain,
                plate: Some(MountingPlate {
                    width: 100.0,
                    height: 100.0,
                    margin: 0.0
                })
            })
            .is_err()
    );
    // The plate cannot go while a rail is on it.
    assert!(
        c.doc
            .execute(Command::SetPlateSize {
                structure: c.cab,
                plate: None
            })
            .is_err()
    );
    assert_eq!(c.doc.project().all::<Rail>().count(), 1);
}

#[test]
fn placements_survive_json_and_old_files_load() {
    let mut c = cabinet();
    let q1 = c.q1;
    let rail = rail(&mut c);
    place(&mut c, q1, 30.0, Some((rail, 10.0)));
    let json = c.doc.project().to_json();
    let back = Project::from_json(&json).unwrap();
    assert_eq!(back.all::<CabinetPlacement>().count(), 1);
    assert_eq!(back.all::<Rail>().count(), 1);
    assert_eq!(back.to_json(), json);

    let plain = Project::default().to_json();
    assert!(!plain.contains("rails") && !plain.contains("cabinet_placements"));
    assert!(!plain.contains("plate"));
}
