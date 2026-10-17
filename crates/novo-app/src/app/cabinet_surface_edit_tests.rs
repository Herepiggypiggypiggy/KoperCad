//! Moving, turning and taking off a device while its surface is shown.

use novo_core::{DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::{load, place_device, set_enclosure};

use super::cabinet::Ask;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn project(d: &Drive) -> &Project {
    d.app.backend.document().unwrap().project()
}

/// A cabinet with -K1 on the plate at (20, 20) and -S1 on the door at (80, 20).
fn cabinet(d: &mut Drive) -> (StructureId, u64, u64) {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let (k1, s1) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1")?, "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 600.0,
                    height: 800.0,
                    margin: 10.0,
                }),
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 800.0,
                    height: 1000.0,
                    depth: 300.0,
                    plate_setback: 0.0,
                    door: Some(Door {
                        hinge: Hinge::Left,
                        max_angle: 90.0,
                        inner_depth: 0.0,
                    }),
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
            let s1 = tx.add_device(DeviceTag::parse("-S1")?, "")?;
            place_device(tx, cab, k1.raw(), 20.0, 20.0, false)?;
            place_device(tx, cab, s1.raw(), 80.0, 20.0, false)?;
            Ok((k1.raw(), s1.raw()))
        })
        .unwrap()
        .0;
    let cab = project(d).structures().find(|s| s.enclosure).unwrap().id;
    let id = project(d)
        .all::<novo_core::CabinetPlacement>()
        .find(|p| p.device.raw() == s1)
        .unwrap()
        .id;
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Door", |tx| {
            tx.set_property(id, "surface", Some(Value::Text("door".into())))
        })
        .unwrap();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    (cab, k1, s1)
}

fn placement(d: &Drive, cab: StructureId, device: u64) -> Option<(f64, f64, bool, Option<Value>)> {
    load(project(d), cab)?
        .placements
        .iter()
        .find(|p| p.id == device)?;
    project(d)
        .all::<novo_core::CabinetPlacement>()
        .find(|p| p.device.raw() == device)
        .map(|p| (p.x, p.y, p.rotated, p.properties.get("surface").cloned()))
}

#[test]
fn a_device_on_the_shown_surface_is_moved_and_stays_on_that_surface() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, k1, s1) = cabinet(&mut d);
    d.app.cabinet.surface = Some("door".into());
    d.frame();
    d.app
        .cabinet_ask(Ask::Drop(s1, 300.0, 400.0), Some(cab), None, &[]);
    let (x, y, _, surface) = placement(&d, cab, s1).unwrap();
    assert_eq!((x, y), (300.0, 400.0));
    assert_eq!(
        surface,
        Some(Value::Text("door".into())),
        "still on the door"
    );
    let (kx, ky, ..) = placement(&d, cab, k1).unwrap();
    assert_eq!((kx, ky), (20.0, 20.0), "the plate device did not move");
}

#[test]
fn a_device_on_the_shown_surface_is_turned_and_taken_off() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, _k1, s1) = cabinet(&mut d);
    d.app.cabinet.surface = Some("door".into());
    d.frame();
    d.app
        .cabinet_ask(Ask::Rotate(vec![s1]), Some(cab), None, &[]);
    let (_, _, rotated, surface) = placement(&d, cab, s1).unwrap();
    assert!(rotated);
    assert_eq!(surface, Some(Value::Text("door".into())));
    d.app
        .cabinet_ask(Ask::Remove(vec![s1]), Some(cab), None, &[]);
    assert!(placement(&d, cab, s1).is_none(), "taken off the door");
}

#[test]
fn a_device_on_the_door_cannot_be_dragged_in_3d_as_if_it_sat_on_the_plate() {
    use novo_layout::state::CabinetLayout;
    use novo_layout::{Margins, Placement, Plate, Size};
    use novo_view3d::scene::{ItemId, ItemKind};

    let mut l = CabinetLayout::new(Plate {
        width: 600.0,
        height: 800.0,
        margin: 10.0,
    });
    for id in [1, 2] {
        l.placements.push(Placement {
            id,
            x: 100.0 * id as f64,
            y: 100.0,
            rotated: false,
            size: Size {
                width: 45.0,
                height: 90.0,
                guessed: false,
            },
            clearance: None,
            margins: Margins::default(),
            rail: None,
        });
    }
    l.off_plate.insert(2, "door".into());
    let dev = |id| ItemId::new(ItemKind::Device, id);
    assert!(super::view3d_move::landing(&l, dev(1), (5.0, 5.0)).is_some());
    assert!(
        super::view3d_move::landing(&l, dev(2), (5.0, 5.0)).is_none(),
        "the door device is moved where the door is shown"
    );
}
