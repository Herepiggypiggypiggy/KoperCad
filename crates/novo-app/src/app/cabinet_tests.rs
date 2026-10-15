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

fn layout_of(d: &Drive, cab: novo_core::StructureId) -> Option<novo_layout::state::CabinetLayout> {
    novo_layout::model::load(project(d), cab)
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

#[test]
fn drop_move_rotate_and_remove_are_each_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    let device = novo_layout::model::candidates(project(&d), cab)[0].id;

    let l = layout_of(&d, cab);
    d.app
        .cabinet_ask(Ask::Drop(device, 100.0, 120.0), Some(cab), l, &[]);
    let placed = |d: &Drive| {
        let l = novo_layout::model::load(d.app.backend.document().unwrap().project(), cab).unwrap();
        l.placements.first().map(|p| (p.x, p.y, p.rotated))
    };
    assert_eq!(placed(&d), Some((100.0, 120.0, false)));

    let l = layout_of(&d, cab);
    d.app
        .cabinet_ask(Ask::Rotate(vec![device]), Some(cab), l, &[]);
    assert_eq!(placed(&d), Some((100.0, 120.0, true)));

    // Moving keeps the turn.
    let l = layout_of(&d, cab);
    d.app
        .cabinet_ask(Ask::Drop(device, 200.0, 40.0), Some(cab), l, &[]);
    assert_eq!(placed(&d), Some((200.0, 40.0, true)));

    let l = layout_of(&d, cab);
    d.app
        .cabinet_ask(Ask::Remove(vec![device]), Some(cab), l, &[]);
    assert_eq!(placed(&d), None);
    d.app.run(&ctx, Action::Undo);
    assert_eq!(placed(&d), Some((200.0, 40.0, true)));
}

#[test]
fn picking_a_page_leaves_the_cabinet() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page = d.app.current_page.expect("a page is shown");
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    assert!(d.app.cabinet_open);
    d.app.show_page(page);
    assert!(!d.app.cabinet_open);
}

#[test]
fn a_ghost_is_drawn_but_never_stored() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    let device = novo_layout::model::candidates(project(&d), cab)[0].id;
    d.app.cabinet.ghost = Some((device, 100.0, 100.0));
    d.frame();
    d.frame();
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);
    // Without a hovering payload the ghost is cleared again.
    assert!(d.app.cabinet.ghost.is_none());
}

#[test]
fn the_table_face_toggles_and_problems_are_in_words() {
    use novo_layout::state::CabinetLayout;
    use novo_layout::{Plate, Size};

    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app.run_command(&ctx, "view.cabinet_table", None);
    assert!(d.app.cabinet_open && d.app.cabinet.table);
    d.frame();
    d.app.run_command(&ctx, "view.cabinet_table", None);
    assert!(d.app.cabinet_open && !d.app.cabinet.table);
    d.frame();

    // Rotate and delete from the palette go to the Cabinet, not the page.
    d.app.run_command(&ctx, "edit.delete", None);
    assert_eq!(d.app.cabinet.keys, vec!["edit.delete".to_owned()]);

    let mut l = CabinetLayout::new(Plate {
        width: 200.0,
        height: 200.0,
        margin: 10.0,
    });
    let size = Size::from_part(Some(45.0), Some(90.0));
    l.drop_device(1, size, 50.0, 50.0);
    l.drop_device(2, size, 60.0, 50.0);
    l.drop_device(3, Size::from_part(None, None), 0.0, 0.0);
    let name = |id: u64| format!("-K{id}");
    let a = super::cabinet::problem_lines(&l, 1, &name);
    assert_eq!(a[0], "Overlaps -K2");
    assert_eq!(a[1], "Only 5 mm from -K3");
    let c = super::cabinet::problem_lines(&l, 3, &name);
    assert!(c.iter().any(|t| t.contains("past the usable plate")));
    assert!(c.iter().any(|t| t.contains("guess")));
}

#[test]
fn keys_pressed_in_the_table_do_not_wait_for_the_plate() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    d.app.run_command(&ctx, "view.cabinet_table", None);
    d.frame();
    d.app.run_command(&ctx, "edit.delete", None);
    assert_eq!(d.app.cabinet.keys.len(), 1);
    d.frame();
    assert!(d.app.cabinet.keys.is_empty(), "the table drops the key");
    // Back on the plate, nothing is deleted by the stale key.
    d.app.run_command(&ctx, "view.cabinet_table", None);
    d.frame();
    assert!(!d.app.cabinet.table);
    assert!(d.app.cabinet.keys.is_empty());
}
