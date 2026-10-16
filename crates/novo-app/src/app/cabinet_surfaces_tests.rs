//! The Cabinet's Surface picker: the model behind it and its command.

use novo_core::{CabinetPlacement, DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::{place_device, set_enclosure};

use eframe::egui;

use super::cabinet_surfaces::summarize;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn project(d: &Drive) -> &Project {
    d.app.backend.document().unwrap().project()
}

fn cabinet(d: &mut Drive, with_enclosure: bool) -> StructureId {
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
            if with_enclosure {
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
            }
            Ok(())
        })
        .unwrap();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    project(d).structures().find(|s| s.enclosure).unwrap().id
}

#[test]
fn a_plate_only_cabinet_has_the_plate_as_its_only_surface() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d, false);
    let sum = summarize(project(&d), cab).unwrap();
    assert_eq!(sum.rows.len(), 1);
    assert_eq!(sum.rows[0].name, "plate");
    assert_eq!(sum.rows[0].size, "600 x 800 mm");
    assert!(sum.problems.is_empty());
}

#[test]
fn an_enclosure_lists_door_walls_roof_floor_and_counts_devices_per_surface() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d, true);
    let ((k1, s1, _q1), _) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("Devices", |tx| {
            let a = tx.add_device(DeviceTag::parse("-K1").unwrap(), "")?;
            let b = tx.add_device(DeviceTag::parse("-S1").unwrap(), "")?;
            let c = tx.add_device(DeviceTag::parse("-Q1").unwrap(), "")?;
            for (dev, x) in [(a, 20.0), (b, 80.0), (c, 140.0)] {
                place_device(tx, cab, dev.raw(), x, 20.0, false)?;
            }
            Ok((a, b, c))
        })
        .unwrap();
    // S1 sits on the door, K1 names a surface that does not exist.
    let pid = |dev| {
        project(&d)
            .all::<CabinetPlacement>()
            .find(|p| p.device == dev)
            .unwrap()
            .id
    };
    let (ps, pk) = (pid(s1), pid(k1));
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Surface", |tx| {
            tx.set_property(ps, "surface", Some(Value::Text("door".into())))?;
            tx.set_property(pk, "surface", Some(Value::Text("attic".into())))
        })
        .unwrap();
    let sum = summarize(project(&d), cab).unwrap();
    let names: Vec<&str> = sum.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["plate", "door", "left", "right", "roof", "floor"]);
    assert_eq!(sum.row("plate").unwrap().devices, 1);
    assert_eq!(sum.row("door").unwrap().devices, 1);
    assert!(sum.row("door").unwrap().assumed);
    assert_eq!(sum.problems.len(), 1);
    assert!(sum.problems[0].contains("attic"), "{:?}", sum.problems);
    assert!(sum.note("door").unwrap().contains("planning values"));
    assert_eq!(
        sum.row("plate").unwrap().label(),
        "plate  600 x 800 mm  1 device"
    );
}

#[test]
fn the_surface_command_opens_the_picker_and_has_a_menu_row() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let _ = cabinet(&mut d, true);
    d.app.run_command(&ctx, "cabinet.surface", None);
    assert!(d.app.cabinet.surface_open);
    d.frame();
    assert!(!d.app.cabinet.surface_open, "the frame took the request");
    let popup = d.app.cabinet.surface_popup.expect("the picker was drawn");
    assert!(egui::Popup::is_id_open(&ctx, popup), "the picker is open");
    assert!(d.app.learn_anchor("cabinet.surface").is_some());
    let info = novo_workflow::commands::get("cabinet.surface").expect("registered");
    assert!(novo_workflow::menus::place(info).is_some());
}

#[test]
fn the_surface_command_explains_a_plate_only_cabinet() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let _ = cabinet(&mut d, false);
    d.app.run_command(&ctx, "cabinet.surface", None);
    assert!(!d.app.cabinet.surface_open);
    d.frame();
    assert!(
        d.app.cabinet.surface_popup.is_none(),
        "no picker on a plate"
    );
    let shown = d.app.messages().last().expect("a message").text.clone();
    assert!(shown.contains("only its mounting plate"), "{shown}");
}

#[test]
fn a_picked_surface_is_drawn_from_its_own_layout_and_cannot_be_edited_yet() {
    use super::cabinet::Ask;
    use super::cabinet_surfaces::surface_layout;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let cab = cabinet(&mut d, true);
    let (k1, s1) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("Devices", |tx| {
            let a = tx.add_device(DeviceTag::parse("-K1").unwrap(), "")?;
            let b = tx.add_device(DeviceTag::parse("-S1").unwrap(), "")?;
            place_device(tx, cab, a.raw(), 20.0, 20.0, false)?;
            place_device(tx, cab, b.raw(), 80.0, 20.0, false)?;
            Ok((a, b))
        })
        .unwrap()
        .0;
    let pid = project(&d)
        .all::<novo_core::CabinetPlacement>()
        .find(|p| p.device == s1)
        .unwrap()
        .id;
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Surface", |tx| {
            tx.set_property(pid, "surface", Some(Value::Text("door".into())))
        })
        .unwrap();
    let sum = summarize(project(&d), cab).unwrap();
    let stored = novo_layout::model::load(project(&d), cab).unwrap();
    let door = surface_layout(&stored, &sum, "door").unwrap();
    assert_eq!((door.plate.width, door.plate.height), (800.0, 1000.0));
    assert_eq!(door.placements.len(), 1);
    assert_eq!(door.placements[0].id, s1.raw());
    assert!(door.rails.is_empty() && door.enclosure.is_none());
    let plate = surface_layout(&stored, &sum, "plate").unwrap();
    assert_eq!(plate.placements.len(), 1);
    assert_eq!(plate.placements[0].id, k1.raw());

    // Picked in the app: edits are refused and the stored layout is kept.
    d.app.cabinet.surface = Some("door".into());
    d.frame();
    d.app
        .cabinet_ask(Ask::Remove(vec![s1.raw()]), Some(cab), None, &[]);
    let after = novo_layout::model::load(project(&d), cab).unwrap();
    assert_eq!(after.placements.len(), 2, "nothing was removed");
    let msg = d.app.messages().last().unwrap().text.clone();
    assert!(msg.contains("view only"), "{msg}");
    let _ = &ctx;
}
