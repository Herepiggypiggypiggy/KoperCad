//! Putting a placed device on another surface from the Cabinet inspector.

use novo_core::{DeviceTag, MountingPlate, Project, StructureId};
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::{place_device, set_enclosure};

use super::cabinet::Ask;
use super::cabinet_inspector::{Facts, gather};
use super::cabinet_surface_move::{annotate, keep_plate_candidates, surface_of};
use super::cabinet_surfaces::summarize;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn project(d: &Drive) -> &Project {
    d.app.backend.document().unwrap().project()
}

fn cabinet(d: &mut Drive, with_enclosure: bool) -> (StructureId, u64) {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let k1 = d
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
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
            place_device(tx, cab, k1.raw(), 20.0, 20.0, false)?;
            Ok(k1.raw())
        })
        .unwrap()
        .0;
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(d).structures().find(|s| s.enclosure).unwrap().id;
    (cab, k1)
}

fn facts_of(d: &Drive, cab: StructureId, k1: u64) -> Facts {
    let p = project(d);
    let layout = novo_layout::model::load(p, cab).unwrap();
    let tag_of = |_: u64| "-K1".to_owned();
    let mut facts = gather(p, cab, &layout, &layout, &[k1], &tag_of);
    annotate(&mut facts, summarize(p, cab).as_ref());
    facts
}

#[test]
fn a_device_goes_to_the_door_and_back_to_the_plate_as_one_undo_step_each() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, k1) = cabinet(&mut d, true);
    assert_eq!(surface_of(project(&d), cab, k1), "plate");

    d.app
        .cabinet_ask(Ask::SetSurface(k1, "door".into()), Some(cab), None, &[]);
    assert_eq!(surface_of(project(&d), cab, k1), "door");
    let sum = summarize(project(&d), cab).unwrap();
    assert_eq!(sum.row("door").unwrap().devices, 1);
    assert_eq!(sum.row("plate").unwrap().devices, 0);

    d.app
        .cabinet_ask(Ask::SetSurface(k1, "plate".into()), Some(cab), None, &[]);
    assert_eq!(surface_of(project(&d), cab, k1), "plate");
    assert!(
        !project(&d)
            .all::<novo_core::CabinetPlacement>()
            .any(|p| p.properties.contains_key("surface")),
        "the plate is the default: the property is cleared, not set to 'plate'"
    );

    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert_eq!(surface_of(project(&d), cab, k1), "door", "one undo step");
}

#[test]
fn the_device_can_be_moved_while_another_surface_is_shown() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, k1) = cabinet(&mut d, true);
    d.app.cabinet.surface = Some("roof".to_owned());
    d.frame();
    d.app
        .cabinet_ask(Ask::SetSurface(k1, "roof".into()), Some(cab), None, &[]);
    assert_eq!(surface_of(project(&d), cab, k1), "roof");
}

#[test]
fn the_inspector_offers_every_surface_with_the_current_one_marked() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, k1) = cabinet(&mut d, true);
    d.app
        .cabinet_ask(Ask::SetSurface(k1, "door".into()), Some(cab), None, &[]);
    let Facts::Devices(devices) = facts_of(&d, cab, k1) else {
        panic!("a device is selected");
    };
    let choice = devices[0].surface.as_ref().expect("a choice");
    assert_eq!(choice.current, "door");
    assert_eq!(
        choice.names,
        ["plate", "door", "left", "right", "roof", "floor"]
    );
}

#[test]
fn a_plate_only_cabinet_offers_no_choice() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, k1) = cabinet(&mut d, false);
    let Facts::Devices(devices) = facts_of(&d, cab, k1) else {
        panic!("a device is selected");
    };
    assert!(devices[0].surface.is_none());
}

#[test]
fn a_device_on_the_door_leaves_the_plate_view_and_shows_on_the_door_view() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, k1) = cabinet(&mut d, true);
    d.app
        .cabinet_ask(Ask::SetSurface(k1, "door".into()), Some(cab), None, &[]);
    d.frame();
    let drawn = |d: &Drive| -> Vec<u64> {
        d.app
            .cabinet
            .scene
            .scene()
            .iter()
            .filter_map(|(id, _)| novo_canvas::cabinet::box_of(id))
            .collect()
    };
    assert!(!drawn(&d).contains(&k1), "not drawn on the plate");
    d.app.cabinet.surface = Some("door".to_owned());
    d.frame();
    assert!(drawn(&d).contains(&k1), "drawn on the door");
}

#[test]
fn a_device_on_the_door_is_not_unplaced_and_place_all_leaves_it_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, k1) = cabinet(&mut d, true);
    // A second device with no spot yet: the only one the plate needs.
    let k2 = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("K2", |tx| {
            let k2 = tx.add_device(DeviceTag::parse("-K2")?, "")?;
            let page = tx.add_page("2")?;
            tx.place_symbol(
                page,
                "iec/contacts/no",
                novo_core::Placement::at(novo_core::Point::new(10.0, 10.0)),
                vec![novo_core::Pin::new("1", 0.0, 0.0)],
                novo_core::SymbolRole::Device {
                    device: k2,
                    main: true,
                },
            )?;
            Ok(k2.raw())
        })
        .unwrap()
        .0;
    d.app
        .cabinet_ask(Ask::SetSurface(k1, "door".into()), Some(cab), None, &[]);

    let p = project(&d);
    let mut cands = novo_layout::model::candidates(p, cab);
    assert!(cands.iter().any(|c| c.id == k1), "the model still lists K1");
    let sum = summarize(p, cab).unwrap();
    keep_plate_candidates(&mut cands, &sum);
    assert!(!cands.iter().any(|c| c.id == k1), "K1 is mounted elsewhere");

    let mut layout = novo_layout::model::load(p, cab).unwrap();
    layout.placements.retain(|pl| pl.id != k1);
    let unplaced: Vec<u64> = layout.unplaced(&cands).iter().map(|c| c.id).collect();
    assert_eq!(unplaced, [k2], "only K2 waits for a spot on the plate");
    let plan = layout.preview(&cands);
    assert!(
        !format!("{plan:?}").contains(&format!("id: {k1}")),
        "Place all does not plan K1 onto the plate"
    );
}
