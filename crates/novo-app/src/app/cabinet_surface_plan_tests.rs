//! Place all with surface rules: the plan covers the door as well, and
//! Enter stores everything as one step.

use novo_core::{CabinetPlacement, DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::{candidates, load, set_enclosure};

use super::cabinet::Ask;
use super::cabinet_surface_move::surface_of;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn project(d: &Drive) -> &Project {
    d.app.backend.document().unwrap().project()
}

fn cabinet(d: &mut Drive) -> StructureId {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app
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
            tx.set_property(cab, "surface_rules", Some(Value::Text("door: H".into())))?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(22.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(22.0)))?;
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-H1", "=A1+CAB1-H2"] {
                let dev = tx.add_device(DeviceTag::parse(tag)?, "")?;
                tx.assign_part(dev, part)?;
            }
            Ok(())
        })
        .unwrap();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    project(d).structures().find(|s| s.enclosure).unwrap().id
}

fn device(d: &Drive, tag: &str) -> u64 {
    project(d)
        .devices()
        .find(|dev| dev.tag.to_string().ends_with(tag))
        .unwrap()
        .id
        .raw()
}

#[test]
fn place_all_plans_the_door_devices_and_enter_stores_them_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d);
    let (k1, h1, h2) = (device(&d, "K1"), device(&d, "H1"), device(&d, "H2"));

    let layout = load(project(&d), cab);
    let cands = candidates(project(&d), cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.frame();
    assert!(d.app.cabinet.preview.is_some());
    assert_eq!(d.app.cabinet.surface_preview.len(), 1);
    assert_eq!(d.app.cabinet.surface_preview[0].0, "door");
    assert_eq!(d.app.cabinet.surface_preview[0].1.placements.len(), 2);
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);
    let said = d
        .app
        .messages()
        .iter()
        .any(|m| m.text.contains("go on the door"));
    assert!(said, "{:?}", d.app.messages());

    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 3);
    assert_eq!(surface_of(project(&d), cab, k1), "plate");
    assert_eq!(surface_of(project(&d), cab, h1), "door");
    assert_eq!(surface_of(project(&d), cab, h2), "door");
    let stored = load(project(&d), cab).unwrap();
    assert_eq!(stored.door_devices.len(), 2);
    assert_eq!(stored.on_plate().len(), 1);
    assert!(d.app.cabinet.surface_preview.is_empty());
}

#[test]
fn cancel_drops_the_door_part_of_the_plan() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d);
    let layout = load(project(&d), cab);
    let cands = candidates(project(&d), cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.app.cabinet_ask(Ask::Cancel, Some(cab), None, &[]);
    assert!(d.app.cabinet.preview.is_none() && d.app.cabinet.surface_preview.is_empty());
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);
}
