//! The Cabinet canvas: plate, place all as one undo step.

use novo_core::{CabinetPlacement, DeviceTag, Project, Rail, Value};

use super::cabinet::Ask;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn add_cabinet(d: &mut Drive) {
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-K2", "=A1+CAB1-Q1"] {
                let device = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(device, part)?;
            }
            Ok(())
        })
        .unwrap();
}

fn project(d: &Drive) -> &Project {
    d.app.backend.document().unwrap().project()
}

#[test]
fn place_all_previews_then_accepts_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    assert!(d.app.cabinet_open);
    assert_eq!(
        d.app.cabinet.location.as_ref().unwrap().to_string(),
        "+CAB1"
    );

    // No plate yet: the first ask sets it.
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    assert!(
        project(&d)
            .get::<novo_core::Structure>(cab)
            .unwrap()
            .plate
            .is_some()
    );

    // Place all only previews.
    let layout = novo_layout::model::load(project(&d), cab);
    let cands = novo_layout::model::candidates(project(&d), cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.frame();
    assert!(d.app.cabinet.preview.is_some());
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);

    // Accepting places all three with rails, in one step.
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 3);
    assert!(project(&d).all::<Rail>().count() >= 1);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);
    assert_eq!(project(&d).all::<Rail>().count(), 0);

    d.app.run(&ctx, Action::ShowSchematic);
    assert!(!d.app.cabinet_open);
}
