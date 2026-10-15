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
fn panel_data_edits_are_one_undo_step_and_bad_input_changes_nothing() {
    use super::cabinet_panel::PanelEdit;
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

    let set = |d: &mut Drive, key: &'static str, text: &str| {
        let l = layout_of(d, cab);
        d.app.cabinet_ask(
            Ask::SetPanelValue(PanelEdit {
                device,
                key,
                text: text.into(),
            }),
            Some(cab),
            l,
            &[],
        );
    };
    let margins = |d: &Drive| {
        let l = layout_of(d, cab).unwrap();
        l.placements.first().map(|p| p.margins)
    };
    set(&mut d, "clearance_above_mm", "40");
    assert_eq!(margins(&d).unwrap().above, Some(40.0));
    // A negative number or an unknown mounting type is refused.
    set(&mut d, "clearance_below_mm", "-3");
    assert_eq!(margins(&d).unwrap().below, None);
    set(&mut d, "mounting", "glue");
    let device_id = novo_core::DeviceId::from(device);
    assert!(
        !project(&d)
            .device(device_id)
            .unwrap()
            .properties
            .contains_key("mounting")
    );
    set(&mut d, "mounting", "din_rail");
    set(&mut d, "power_loss_w", "4,5");
    let panel =
        super::cabinet_panel::panel_data(project(&d), project(&d).device(device_id).unwrap());
    assert_eq!(panel.power_loss_w, Some(4.5));
    assert_eq!(panel.mounting, Some(novo_parts::Mounting::DinRail));
    d.app.run(&ctx, Action::Undo);
    let panel =
        super::cabinet_panel::panel_data(project(&d), project(&d).device(device_id).unwrap());
    assert_eq!(panel.power_loss_w, None);
    assert_eq!(panel.mounting, Some(novo_parts::Mounting::DinRail));
}

#[test]
fn the_panel_data_section_draws_and_reads_the_devices_values() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    let device = project(&d).all::<novo_core::Device>().next().unwrap().id;
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("loss", |tx| {
            tx.set_property(device, "power_loss_w", Some(Value::Float(7.5)))
        })
        .unwrap();
    let mut fields = super::cabinet_panel::PanelFields::default();
    let egui_ctx = d.ctx.clone();
    let mut edit = None;
    for _ in 0..2 {
        let mut out = egui_ctx.run_ui(Default::default(), |ui| {
            edit = super::cabinet_panel::show(ui, &mut fields, project(&d), device.raw());
        });
        out.textures_delta.clear();
    }
    assert!(edit.is_none(), "drawing alone changes nothing");
}
