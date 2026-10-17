//! The size of the shown surface in the Cabinet inspector.

use novo_core::{DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::set_enclosure;
use novo_layout::surfaces::SurfaceSize;

use super::cabinet::Ask;
use super::cabinet_surface_size::card;
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

fn prop(d: &Drive, cab: StructureId, key: &str) -> Option<String> {
    match project(d)
        .get::<novo_core::Structure>(cab)
        .unwrap()
        .properties
        .get(key)
    {
        Some(Value::Text(t)) => Some(t.clone()),
        _ => None,
    }
}

fn the_card(
    d: &Drive,
    cab: StructureId,
    name: &str,
) -> Option<super::cabinet_surface_size::SurfaceCard> {
    let sum = summarize(project(d), cab).unwrap();
    card(project(d), cab, &sum, name)
}

#[test]
fn the_plate_has_its_own_form_and_other_surfaces_get_a_card_with_planning_values() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d, true);
    assert!(the_card(&d, cab, "plate").is_none());
    assert!(the_card(&d, cab, "attic").is_none());
    let roof = the_card(&d, cab, "roof").unwrap();
    assert!(!roof.stored && !roof.profile);
    assert_eq!(roof.kind, "roof");
    assert!(roof.size.width > 0.0 && roof.size.height > 0.0);
    assert!(roof.size.origin.is_none());
}

#[test]
fn a_changed_size_is_stored_as_the_surface_property_and_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let cab = cabinet(&mut d, true);
    let before = summarize(project(&d), cab)
        .unwrap()
        .row("roof")
        .unwrap()
        .size
        .clone();
    let size = SurfaceSize {
        width: 700.0,
        height: 240.0,
        margin: 15.0,
        origin: None,
    };
    d.app.cabinet.surface = Some("roof".into());
    d.frame();
    d.app.cabinet_ask(
        Ask::SetSurfaceSize(cab, "roof".into(), Some(size)),
        Some(cab),
        None,
        &[],
    );
    assert_eq!(prop(&d, cab, "surface.roof").as_deref(), Some("700 240 15"));
    let row = summarize(project(&d), cab)
        .unwrap()
        .row("roof")
        .unwrap()
        .clone();
    assert_eq!(row.size, "700 x 240 mm");
    assert!(!row.assumed);
    assert!(the_card(&d, cab, "roof").unwrap().stored);

    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(prop(&d, cab, "surface.roof"), None);
    assert_eq!(
        summarize(project(&d), cab)
            .unwrap()
            .row("roof")
            .unwrap()
            .size,
        before
    );
}

#[test]
fn planning_values_remove_the_stored_size() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d, true);
    let size = SurfaceSize {
        width: 500.0,
        height: 200.0,
        margin: 5.0,
        origin: None,
    };
    d.app.cabinet_ask(
        Ask::SetSurfaceSize(cab, "door".into(), Some(size)),
        Some(cab),
        None,
        &[],
    );
    assert!(prop(&d, cab, "surface.door").is_some());
    d.app.cabinet_ask(
        Ask::SetSurfaceSize(cab, "door".into(), None),
        Some(cab),
        None,
        &[],
    );
    assert_eq!(prop(&d, cab, "surface.door"), None);
    assert!(!the_card(&d, cab, "door").unwrap().stored);
}

#[test]
fn a_profile_card_carries_its_corner_and_keeps_it_when_the_size_changes() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d, true);
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Profile", |tx| {
            tx.set_property(
                novo_core::AnyId::Structure(cab),
                "surface.profile1",
                Some(Value::Text("300 100 5 10 20 30".into())),
            )
        })
        .unwrap();
    let c = the_card(&d, cab, "profile1").unwrap();
    assert!(c.profile && c.stored);
    assert_eq!(c.size.origin, Some([10.0, 20.0, 30.0]));
    let mut size = c.size;
    size.width = 350.0;
    d.app.cabinet_ask(
        Ask::SetSurfaceSize(cab, "profile1".into(), Some(size)),
        Some(cab),
        None,
        &[],
    );
    assert_eq!(
        prop(&d, cab, "surface.profile1").as_deref(),
        Some("350 100 5 10 20 30")
    );
}

#[test]
fn the_size_can_be_changed_while_the_surface_is_shown() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d, true);
    d.app.cabinet.surface = Some("left".into());
    d.frame();
    d.app.messages.clear();
    let size = SurfaceSize {
        width: 250.0,
        height: 900.0,
        margin: 10.0,
        origin: None,
    };
    d.app.cabinet_ask(
        Ask::SetSurfaceSize(cab, "left".into(), Some(size)),
        Some(cab),
        None,
        &[],
    );
    assert_eq!(prop(&d, cab, "surface.left").as_deref(), Some("250 900 10"));
    assert!(d.app.messages().is_empty(), "no refusal");
}

#[test]
fn the_inspector_opens_with_the_size_card_when_a_surface_is_shown_and_nothing_is_selected() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let _ = cabinet(&mut d, true);
    d.frame();
    assert!(
        d.app.learn_anchor("cabinet.inspector").is_none(),
        "plate: no inspector"
    );
    d.app.cabinet.surface = Some("roof".into());
    d.frame();
    d.frame();
    assert!(
        d.app.learn_anchor("cabinet.inspector").is_some(),
        "roof: the size card"
    );
    d.app.cabinet.surface = None;
    d.frame();
    d.frame();
    assert!(d.app.learn_anchor("cabinet.inspector").is_none());
}
