//! The Cabinet canvas: plate, place all as one undo step.

use eframe::egui;
use novo_core::{CabinetPlacement, DeviceTag, Project, Rail, Value};

use super::cabinet::Ask;
use super::gate1_tests::Drive;
use super::workspace::Workspace;
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
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
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
    assert_eq!(d.app.workspace(), Workspace::Schematic);
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
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
    d.app.show_page(page);
    assert_eq!(d.app.workspace(), Workspace::Schematic);
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
    assert!(d.app.workspace() == Workspace::Cabinet && d.app.cabinet.table);
    d.frame();
    d.app.run_command(&ctx, "view.cabinet_table", None);
    assert!(d.app.workspace() == Workspace::Cabinet && !d.app.cabinet.table);
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
fn a_device_too_deep_for_the_door_gets_its_own_mark_and_sentence() {
    use novo_canvas::cabinet::BoxState;
    use novo_layout::enclosure::Enclosure;
    use novo_layout::state::CabinetLayout;
    use novo_layout::{Plate, Size};
    let mut l = CabinetLayout::new(Plate {
        width: 300.0,
        height: 300.0,
        margin: 10.0,
    });
    let size = Size::from_part(Some(45.0), Some(90.0));
    l.drop_device(1, size, 20.0, 20.0);
    l.drop_device(2, size, 120.0, 20.0);
    l.enclosure = Some(Enclosure {
        width: 400.0,
        height: 400.0,
        depth: 200.0,
        plate_setback: 20.0,
        door: None,
        front_clearance: None,
        hinge_side_clearance: None,
    });
    l.depths.insert(1, 220.0);
    l.depths.insert(2, 80.0);
    let name = |id: u64| format!("-K{id}");
    let view = super::cabinet::cabinet_view(&l, &name);
    let state = |id| view.boxes.iter().find(|b| b.id == id).unwrap().state;
    assert_eq!(state(1), BoxState::TooDeep);
    assert_eq!(state(2), BoxState::Ok);
    let lines = super::cabinet::problem_lines(&l, 1, &name);
    assert!(lines.iter().any(|t| t.contains("40 mm too deep")));
    assert!(super::cabinet::problem_lines(&l, 2, &name).is_empty());
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

#[test]
fn the_table_lists_every_device_and_reports_no_click_when_none_is_made() {
    use eframe::egui;
    use novo_layout::plan::Candidate;
    use novo_layout::state::CabinetLayout;
    use novo_layout::{Plate, Size};

    let mut l = CabinetLayout::new(Plate {
        width: 200.0,
        height: 200.0,
        margin: 10.0,
    });
    let size = Size::from_part(Some(45.0), Some(90.0));
    l.drop_device(1, size, 50.0, 50.0);
    let cands: Vec<Candidate> = (1..=2)
        .map(|id| Candidate {
            id,
            size,
            sort_key: String::new(),
        })
        .collect();
    let name = |id: u64| format!("-K{id}");
    let ctx = egui::Context::default();
    let mut clicked = Some(0);
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        clicked = super::cabinet::cabinet_table(ui, &l, &cands, &name, &[1]);
    });
    out.textures_delta.clear();
    assert_eq!(clicked, None);
}

#[test]

fn inspector_sets_the_plate_size_and_lists_a_device_s_problems() {
    use super::cabinet_inspector::{Facts, gather};

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

    // The plate card is off until asked for, so the plate gets the window.
    assert!(!d.app.cabinet.plate_card);
    d.app
        .cabinet_ask(Ask::TogglePlateCard, Some(cab), None, &[]);
    assert!(d.app.cabinet.plate_card);

    // The plate size comes from the inspector, as one undo step.
    let plate = novo_core::MountingPlate {
        width: 500.0,
        height: 700.0,
        margin: 15.0,
    };
    d.app
        .cabinet_ask(Ask::SetPlateSize(cab, plate), Some(cab), None, &[]);
    let stored = layout_of(&d, cab).unwrap();
    assert_eq!((stored.plate.width, stored.plate.height), (500.0, 700.0));
    d.app.run(&ctx, Action::Undo);
    assert_eq!(layout_of(&d, cab).unwrap().plate.width, 600.0);

    // Two devices on top of each other: each names the other.
    let ids: Vec<u64> = novo_layout::model::candidates(project(&d), cab)
        .iter()
        .map(|c| c.id)
        .collect();
    for id in &ids[..2] {
        let l = layout_of(&d, cab);
        d.app
            .cabinet_ask(Ask::Drop(*id, 100.0, 100.0), Some(cab), l, &[]);
    }
    let stored = layout_of(&d, cab).unwrap();
    let Facts::Devices(facts) = gather(project(&d), cab, &stored, &stored, &ids[..1], &|id| {
        id.to_string()
    }) else {
        panic!("a device is selected");
    };
    assert_eq!(facts.len(), 1);
    assert!(facts[0].problems.iter().any(|p| p.starts_with("Overlaps")));
    assert!(facts[0].part.as_deref().unwrap().starts_with("ACME"));
    assert!(matches!(
        gather(project(&d), cab, &stored, &stored, &[], &|id| id
            .to_string()),
        Facts::Plate { placed: 2, .. }
    ));
}

#[test]
fn table_rows_sort_numbers_by_value() {
    let mut tags = ["-K10", "-K2", "-Q1", "-K1"];
    tags.sort_by_key(|t| super::cabinet::natural_key(t));
    assert_eq!(tags, ["-K1", "-K2", "-K10", "-Q1"]);
}

#[test]
fn a_problem_row_selects_its_device_on_the_plate() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    d.app.cabinet.table = true;
    d.app.cabinet_ask(Ask::Select(7), None, None, &[]);
    assert!(!d.app.cabinet.table);
    assert_eq!(
        d.app
            .cabinet
            .view
            .selection
            .ids()
            .iter()
            .filter_map(|i| novo_canvas::cabinet::box_of(*i))
            .collect::<Vec<_>>(),
        vec![7]
    );
}

#[test]
fn heat_commands_open_the_cabinet_and_toggle_the_layers() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(!d.app.cabinet.heat.on);
    d.app.run_command(&ctx, "heat.toggle", None);
    assert!(d.app.workspace() == Workspace::Cabinet && d.app.cabinet.heat.on);
    d.frame();
    d.app.run_command(&ctx, "heat.airflow", None);
    assert!(d.app.cabinet.heat.airflow);
    d.frame();
    d.app.run_command(&ctx, "heat.toggle", None);
    assert!(!d.app.cabinet.heat.on);
}

#[test]
fn plate_export_says_why_when_the_enclosure_has_no_plate() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    assert!(d.app.export_plate_pdf().is_none());
    let last = d.app.messages().last().unwrap();
    assert!(last.text.contains("no mounting plate"), "{}", last.text);
    assert!(crate::app::app_runs("export.plate"));
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

#[test]
fn the_enclosure_is_set_in_one_undo_step_and_drawn_with_its_checks() {
    use novo_layout::enclosure::{Door, Enclosure, Hinge};
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
    assert!(layout_of(&d, cab).unwrap().enclosure.is_none());

    let e = Enclosure {
        width: 650.0,
        height: 850.0,
        depth: 250.0,
        plate_setback: 20.0,
        door: Some(Door {
            hinge: Hinge::Right,
            max_angle: 110.0,
            inner_depth: 0.0,
        }),
        front_clearance: Some(400.0),
        hinge_side_clearance: None,
    };
    d.app
        .cabinet_ask(Ask::SetEnclosure(e), Some(cab), None, &[]);
    // Drawing the card with the form, the checks and the top view works.
    d.app.cabinet.plate_card = true;
    d.frame();
    d.frame();
    let layout = layout_of(&d, cab).unwrap();
    assert_eq!(layout.enclosure, Some(e));
    // 650 mm door at 110 degrees: 650 in front, 400 free is 250 short.
    let report = layout.door_report().unwrap();
    assert!((report.front_short.unwrap() - 250.0).abs() < 1e-6);
    let lines = super::cabinet_door::sentences(&e, &report, &|id| format!("D{id}"));
    assert!(
        lines
            .iter()
            .any(|(bad, t)| *bad && t.contains("250 mm short"))
    );

    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert!(layout_of(&d, cab).unwrap().enclosure.is_none());
}

#[test]
fn plate_export_before_the_cabinet_was_opened_says_what_to_do() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(d.app.export_plate_pdf().is_none());
    let last = d.app.messages().last().unwrap();
    assert!(last.text.contains("Open the Cabinet"), "{}", last.text);
}

#[test]
fn space_held_and_dragged_pans_the_cabinet_not_the_hidden_schematic() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let space = |pressed| egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let at = egui::Event::PointerMoved(egui::pos2(700.0, 400.0));
    let cabinet_before = d.app.cabinet.view.viewport;
    let schematic_before = d.app.canvas.schematic_ref().unwrap().canvas.viewport;
    d.frame_with(vec![at, space(true)]);
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(760.0, 430.0))]);
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(780.0, 440.0))]);
    d.frame_with(vec![space(false)]);
    d.frame();
    assert_ne!(d.app.cabinet.view.viewport.center, cabinet_before.center);
    assert_eq!(
        d.app.canvas.schematic_ref().unwrap().canvas.viewport.center,
        schematic_before.center
    );
    assert!(!d.app.wb.field.is_open(), "a pan opens no field");
}

#[test]
fn the_3d_workspace_opens_once_the_cabinet_has_a_plate() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    assert!(d.app.workspace_unavailable(Workspace::View3d).is_some());
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    assert!(d.app.workspace_unavailable(Workspace::View3d).is_none());
    d.app.run_command(&ctx, "view.3d", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
    assert!(d.app.view3d.built.is_some(), "the scene was built");
}

#[test]
fn space_in_the_3d_workspace_leaves_the_hidden_schematic_alone_and_taps_open_the_field() {
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
    d.app.run_command(&ctx, "view.3d", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
    let space = |pressed| egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let at = egui::Event::PointerMoved(egui::pos2(700.0, 400.0));
    let before = d.app.canvas.schematic_ref().unwrap().canvas.viewport.center;
    d.frame_with(vec![at.clone(), space(true)]);
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(780.0, 440.0))]);
    d.frame_with(vec![space(false)]);
    d.frame();
    let after = d.app.canvas.schematic_ref().unwrap().canvas.viewport.center;
    assert_eq!(after, before);
    d.frame_with(vec![at, space(true)]);
    d.frame_with(vec![space(false)]);
    d.frame();
    assert!(d.app.wb.field.is_open(), "a tap opens the field");
}

/// A terminal strip made as `-X1` has no location; Place all still puts it
/// on the plate of the only cabinet. With a second cabinet it names the
/// strip and says why it is left off.
#[test]
fn place_all_places_a_terminal_strip_without_a_location_and_names_what_it_leaves() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    let strip = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("Strip", |tx| {
            tx.add_terminal_strip(DeviceTag::parse("-X1").unwrap())
        })
        .unwrap()
        .0
        .raw();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    let cands = novo_layout::model::candidates(project(&d), cab);
    d.app
        .cabinet_ask(Ask::PlaceAll, Some(cab), layout_of(&d, cab), &cands);
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();
    let layout = layout_of(&d, cab).unwrap();
    assert!(
        layout.placements.iter().any(|p| p.id == strip),
        "X1 is on the plate"
    );
    assert_eq!(layout.placements.len(), 4);
}

/// With two cabinets a strip without a location belongs to neither: Place all
/// names it and says why instead of leaving it out silently.
#[test]
fn place_all_names_a_device_no_cabinet_takes() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Strip and second cabinet", |tx| {
            tx.add_terminal_strip(DeviceTag::parse("-X1").unwrap())?;
            tx.add_structure(DeviceTag::parse("+CAB2").unwrap(), "Second", true)?;
            Ok(())
        })
        .unwrap();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d)
        .structures()
        .find(|s| s.enclosure && s.tag.to_string() == "+CAB1")
        .unwrap()
        .id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    let cands = novo_layout::model::candidates(project(&d), cab);
    d.app
        .cabinet_ask(Ask::PlaceAll, Some(cab), layout_of(&d, cab), &cands);
    let texts: Vec<_> = d.app.messages().iter().map(|m| m.text.clone()).collect();
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("Not placed: -X1") && t.contains("no location")),
        "{texts:?}"
    );
}

#[test]
fn place_all_command_shows_the_preview_from_any_workspace() {
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
    d.app.run(&ctx, Action::ShowSchematic);
    d.frame();
    assert!(d.app.cabinet.preview.is_none());

    // The command (Ctrl+Shift+P) opens the Cabinet and makes the preview on
    // the next frame; nothing is stored until Enter.
    d.app.run_command(&ctx, "cabinet.place-all", None);
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
    d.frame();
    d.frame();
    assert!(d.app.cabinet.preview.is_some());
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);
    assert_eq!(
        d.app
            .wb
            .keymap
            .shortcut_text("cabinet.place-all")
            .as_deref(),
        Some("Ctrl+Shift+P")
    );
}

/// Cabinet with one placed, selected device and the plate on screen.
fn selected_on_plate(d: &mut Drive) -> (novo_core::StructureId, u64) {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    let device = novo_layout::model::candidates(project(d), cab)[0].id;
    let l = layout_of(d, cab);
    d.app
        .cabinet_ask(Ask::Drop(device, 100.0, 120.0), Some(cab), l, &[]);
    d.frame();
    d.app.cabinet_ask(Ask::Select(device), Some(cab), None, &[]);
    d.frame();
    (cab, device)
}

fn press(d: &mut Drive, key: egui::Key) {
    d.frame_with(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
}

#[test]
fn r_on_the_plate_turns_the_selected_device_in_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, _) = selected_on_plate(&mut d);
    let size = |d: &Drive| {
        let l = layout_of(d, cab).unwrap();
        let p = &l.placements[0];
        (p.rotated, p.footprint().w, p.footprint().h)
    };
    let (r0, w0, h0) = size(&d);
    assert!(!r0);
    press(&mut d, egui::Key::R);
    d.frame();
    let (r1, w1, h1) = size(&d);
    assert!(r1, "R turned the device");
    assert_eq!((w1, h1), (h0, w0));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert_eq!(size(&d), (false, w0, h0));
}

#[test]
fn delete_on_the_plate_unplaces_the_device_and_keeps_it_in_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, device) = selected_on_plate(&mut d);
    let devices = |d: &Drive| project(d).devices().count();
    let before = devices(&d);
    press(&mut d, egui::Key::Delete);
    d.frame();
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);
    assert_eq!(devices(&d), before, "the device stays in the project");
    let cands = novo_layout::model::candidates(project(&d), cab);
    let l = layout_of(&d, cab).unwrap();
    assert!(l.unplaced(&cands).iter().any(|c| c.id == device));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 1);
}

#[test]
fn x_on_the_plate_says_why_it_does_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (_, _) = selected_on_plate(&mut d);
    press(&mut d, egui::Key::X);
    assert!(d.app.messages().iter().any(|m| m.text.contains("Mirror")));
}

#[test]
fn moving_a_device_during_the_place_all_preview_takes_the_preview_first() {
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
    let layout = layout_of(&d, cab);
    let cands = novo_layout::model::candidates(project(&d), cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.frame();
    assert!(d.app.cabinet.preview.is_some());
    let device = cands[0].id;

    let l = layout_of(&d, cab);
    d.app
        .cabinet_ask(Ask::Drop(device, 300.0, 200.0), Some(cab), l, &[]);
    d.frame();
    assert!(d.app.cabinet.preview.is_none(), "the preview is committed");
    let l = layout_of(&d, cab).unwrap();
    assert_eq!(l.placements.len(), 3);
    let p = l.placements.iter().find(|p| p.id == device).unwrap();
    assert_eq!((p.x, p.y), (300.0, 200.0));
}

#[test]
fn the_table_face_can_be_left_by_key_and_by_button() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (_, _) = selected_on_plate(&mut d);
    let ctrl_t = |d: &mut Drive| {
        d.frame_with(vec![egui::Event::Key {
            key: egui::Key::T,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }]);
        d.frame();
    };
    for _ in 0..2 {
        ctrl_t(&mut d);
        assert!(d.app.cabinet.table, "Ctrl+T shows the table");
        ctrl_t(&mut d);
        assert!(!d.app.cabinet.table, "Ctrl+T returns to the plate");
    }
    // The header button and a selected row also lead back.
    d.app.cabinet_ask(Ask::ToggleTable, None, None, &[]);
    assert!(d.app.cabinet.table);
    d.app.cabinet_ask(Ask::ToggleTable, None, None, &[]);
    assert!(!d.app.cabinet.table);
    // Leaving the Cabinet and coming back never strands the table face.
    d.app.cabinet_ask(Ask::ToggleTable, None, None, &[]);
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::ShowSchematic);
    d.frame();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    ctrl_t(&mut d);
    assert!(
        !d.app.cabinet.table || {
            ctrl_t(&mut d);
            !d.app.cabinet.table
        }
    );
}

#[test]
fn escape_and_the_cabinet_tab_also_leave_the_table() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (_, _) = selected_on_plate(&mut d);
    let ctx = d.ctx.clone();
    d.app.cabinet_ask(Ask::ToggleTable, None, None, &[]);
    d.frame();
    press(&mut d, egui::Key::Escape);
    d.frame();
    assert!(!d.app.cabinet.table, "Escape returns to the plate");
    d.app.cabinet_ask(Ask::ToggleTable, None, None, &[]);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    assert!(!d.app.cabinet.table, "the Cabinet tab returns to the plate");
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
}

/// Starts the lesson `id` and skips to the step `step`.
pub(super) fn lesson_at(d: &mut Drive, dir: &std::path::Path, id: &str, step: &str) {
    d.app.learn.reload();
    d.app.learn.use_progress_file(dir.join("learn.json"));
    d.app.start_lesson(id);
    d.frame();
    while d
        .app
        .learn
        .run
        .as_ref()
        .and_then(|r| r.step())
        .is_some_and(|s| s.id != step)
    {
        assert!(d.app.learn.run.as_mut().unwrap().skip());
    }
}

pub(super) fn step_done(d: &mut Drive, step: &str) -> bool {
    d.frame();
    d.app.learn.run.as_ref().unwrap().is_done(step)
}

#[test]
fn cabinet_actions_tick_their_lesson_steps() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    lesson_at(&mut d, dir.path(), "cabinet-layout", "drag-first");
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    let devices = novo_layout::model::candidates(project(&d), cab);
    let (a, b) = (devices[0].id, devices[1].id);

    let l = layout_of(&d, cab);
    d.app
        .cabinet_ask(Ask::Drop(a, 100.0, 100.0), Some(cab), l, &[]);
    assert!(
        step_done(&mut d, "drag-first"),
        "a drop raises cabinet_dropped"
    );

    while d
        .app
        .learn
        .run
        .as_ref()
        .unwrap()
        .step()
        .is_some_and(|s| s.id != "rotate")
    {
        d.app.learn.run.as_mut().unwrap().skip();
    }
    let l = layout_of(&d, cab);
    d.app.cabinet_ask(Ask::Rotate(vec![a]), Some(cab), l, &[]);
    assert!(step_done(&mut d, "rotate"), "a turn raises cabinet_rotated");

    while d
        .app
        .learn
        .run
        .as_ref()
        .unwrap()
        .step()
        .is_some_and(|s| s.id != "take-off")
    {
        d.app.learn.run.as_mut().unwrap().skip();
    }
    let l = layout_of(&d, cab);
    d.app.cabinet_ask(Ask::Remove(vec![a]), Some(cab), l, &[]);
    assert!(
        step_done(&mut d, "take-off"),
        "a removal raises cabinet_removed"
    );
    let _ = b;

    while d
        .app
        .learn
        .run
        .as_ref()
        .unwrap()
        .step()
        .is_some_and(|s| s.id != "accept")
    {
        d.app.learn.run.as_mut().unwrap().skip();
    }
    let layout = layout_of(&d, cab);
    let cands = novo_layout::model::candidates(project(&d), cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    assert!(
        step_done(&mut d, "accept"),
        "accepting raises cabinet_plan_accepted"
    );
}

#[test]
fn exporting_the_wire_cut_ticks_its_lesson_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    lesson_at(&mut d, dir.path(), "cut-lengths", "export-cut");
    let path = dir.path().join("cut.csv");
    assert!(d.app.export_wire_cut_to(&path).is_some());
    assert!(step_done(&mut d, "export-cut"));
}

#[test]
fn the_cabinet_registers_the_controls_and_panels_lessons_point_at() {
    use novo_layout::enclosure::{Door, Enclosure, Hinge};
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
    let e = Enclosure {
        width: 650.0,
        height: 850.0,
        depth: 250.0,
        plate_setback: 20.0,
        door: Some(Door {
            hinge: Hinge::Right,
            max_angle: 110.0,
            inner_depth: 0.0,
        }),
        front_clearance: Some(400.0),
        hinge_side_clearance: None,
    };
    d.app
        .cabinet_ask(Ask::SetEnclosure(e), Some(cab), None, &[]);
    d.app.cabinet.plate_card = true;
    d.app.cabinet.routes = true;
    d.app.cabinet.rails.open = true;
    d.app.sizing.panel_open = true;
    d.frame();
    d.frame();
    for id in [
        "cabinet.canvas",
        "cabinet.unplaced",
        "cabinet.problems",
        "cabinet.inspector",
        "cabinet.door",
        "cabinet.door.apply",
        "cabinet.routes_panel",
        "cabinet.wire_rules",
        "cabinet.sizing",
        "cabinet.rails",
        "cabinet.plate_drawing",
    ] {
        let rect = d
            .app
            .learn_anchor(id)
            .unwrap_or_else(|| panic!("`{id}` is not on screen"));
        assert!(rect.width() > 0.0 && rect.height() > 0.0, "{id}: {rect:?}");
    }
}

#[test]
fn the_selection_travels_between_the_cabinet_and_3d_and_view_commands_run() {
    use novo_view3d::scene::{ItemId, ItemKind};

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
    d.frame();

    // Selected in the Cabinet, selected in 3D.
    d.app.cabinet_ask(Ask::Select(device), None, None, &[]);
    d.app.run_command(&ctx, "view.3d", None);
    d.frame();
    assert_eq!(
        d.app
            .view3d
            .view
            .state
            .selection
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![ItemId::new(ItemKind::Device, device)]
    );

    // The view commands switch to 3D and are spent by the next frame.
    d.app.run_command(&ctx, "view.schematic", None);
    d.frame();
    d.app.run_command(&ctx, "view.iso", None);
    assert_eq!(d.app.workspace(), Workspace::View3d);
    assert!(d.app.view3d.pending.is_some());
    d.frame();
    assert!(d.app.view3d.pending.is_none());
}

/// A right-click on the plate opens a menu for what is under it; its rows
/// run the same commands as the toolbar.
#[test]
fn right_click_on_the_plate_opens_a_menu_for_what_is_under_it() {
    use super::cabinet_menu::Target;
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
    d.frame();
    d.frame();

    let right_click = |d: &mut Drive, at: egui::Pos2| {
        let ev = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        d.frame_with(vec![egui::Event::PointerMoved(at)]);
        d.frame_with(vec![ev(true)]);
        d.frame_with(vec![ev(false)]);
        d.frame();
    };

    // On the device: the menu is the device's, and the device is selected.
    let canvas = d.app.cabinet.plate_rect.expect("the plate was shown");
    let view = &d.app.cabinet.view;
    let world = novo_canvas::vec2(100.0 + 20.0, 120.0 + 40.0);
    let s = view.viewport.world_to_screen(world);
    right_click(&mut d, egui::pos2(canvas.left() + s.x, canvas.top() + s.y));
    let menu = d.app.cabinet.menu.expect("a menu opened on the device");
    assert_eq!(menu.target, Target::Device(device));
    assert_eq!(d.app.cabinet_selected(), vec![device]);

    // Escape closes it; a click on empty plate opens the plate menu.
    d.frame_with(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(d.app.cabinet.menu.is_none());
    let view = &d.app.cabinet.view;
    let s = view
        .viewport
        .world_to_screen(novo_canvas::vec2(400.0, 500.0));
    right_click(&mut d, egui::pos2(canvas.left() + s.x, canvas.top() + s.y));
    let menu = d.app.cabinet.menu.expect("a menu opened on the plate");
    assert_eq!(menu.target, Target::Plate);
}

#[test]
fn door_devices_are_swept_and_named_in_the_door_panel() {
    use novo_layout::enclosure::{Door, Enclosure, Hinge};
    use novo_layout::state::CabinetLayout;
    use novo_layout::swing::DoorDevice;
    use novo_layout::{Plate, Rect};
    let mut layout = CabinetLayout::new(Plate {
        width: 560.0,
        height: 760.0,
        margin: 10.0,
    });
    assert!(layout.swing_report().is_none());
    layout.enclosure = Some(Enclosure {
        width: 600.0,
        height: 800.0,
        depth: 250.0,
        plate_setback: 20.0,
        door: Some(Door {
            hinge: Hinge::Left,
            ..Door::default()
        }),
        front_clearance: None,
        hinge_side_clearance: None,
    });
    let tag = |id: u64| format!("-H{id}");
    let none = layout.swing_report().unwrap();
    assert!(super::cabinet_door::swing_sentences(&none, &tag).is_empty());
    layout.door_devices.push(DoorDevice {
        id: 1,
        rect: Rect {
            x: 560.0,
            y: 300.0,
            w: 39.5,
            h: 40.0,
        },
        depth: 40.0,
        mesh: None,
    });
    let lines = super::cabinet_door::swing_sentences(&layout.swing_report().unwrap(), &tag);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].0 && lines[0].1.contains("-H1") && lines[0].1.contains("[door.swing]"));
    layout.door_devices[0].rect.x = 100.0;
    layout.door_devices[0].rect.w = 20.0;
    assert!(
        super::cabinet_door::swing_sentences(&layout.swing_report().unwrap(), &tag)[0]
            .1
            .contains("clear everything")
    );
    layout.placements.push(novo_layout::Placement {
        id: 9,
        x: 50.0,
        y: 50.0,
        rotated: false,
        size: novo_layout::Size {
            width: 40.0,
            height: 40.0,
            guessed: false,
        },
        clearance: None,
        margins: Default::default(),
        rail: None,
    });
    let lines = super::cabinet_door::swing_sentences(&layout.swing_report().unwrap(), &tag);
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0]
            .1
            .contains("Not checked against the door devices: 1")
    );
    assert!(!lines[0].1.contains("clear everything"));
}
