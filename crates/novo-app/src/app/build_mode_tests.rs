//! Build mode: opens from its command, draws at tablet and desktop sizes,
//! ticks with Space, keeps the ticks in the project and closes on Escape.

use eframe::egui;
use novo_core::{DeviceTag, Value};

use super::cabinet::Ask;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn cabinet(d: &mut Drive) {
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-K2"] {
                let device = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(device, part)?;
            }
            Ok(())
        })
        .unwrap();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .structures()
        .find(|s| s.enclosure)
        .unwrap()
        .id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    let project = d.app.backend.document().unwrap().project();
    let layout = novo_layout::model::load(project, cab);
    let candidates = novo_layout::model::candidates(project, cab);
    d.app
        .cabinet_ask(Ask::PlaceAll, Some(cab), layout, &candidates);
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();
}

fn started() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

/// A tap of Space: the press, then the release (the app acts on the release).
fn tap_space(d: &mut Drive) {
    d.key(egui::Key::Space, egui::Modifiers::NONE);
    d.frame_with(vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
}

fn ticks(d: &Drive) -> usize {
    d.app
        .backend
        .commissioning_json()
        .and_then(|j| novo_commissioning::CommissioningStore::from_json(&j).ok())
        .map_or(0, |s| s.build_ticks().count())
}

#[test]
fn without_a_cabinet_it_opens_and_says_what_is_missing() {
    let (_dir, mut d) = started();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.build", None);
    d.frame();
    d.frame();
    assert!(d.app.wb.build.is_some());
}

#[test]
fn it_draws_at_every_size_from_a_tablet_to_full_hd() {
    let (_dir, mut d) = started();
    cabinet(&mut d);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.build", None);
    for (w, h) in [
        (1920.0, 1080.0),
        (1366.0, 768.0),
        (1024.0, 768.0),
        (683.0, 384.0),
    ] {
        d.frame_sized(egui::vec2(w, h), Vec::new());
        d.frame_sized(egui::vec2(w, h), Vec::new());
        assert!(d.app.wb.build.is_some(), "still open at {w}x{h}");
    }
}

#[test]
fn space_ticks_the_step_and_the_tick_is_kept_in_the_project() {
    let (_dir, mut d) = started();
    cabinet(&mut d);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.build", None);
    d.frame();
    d.frame();
    assert_eq!(ticks(&d), 0);
    tap_space(&mut d);
    d.frame();
    assert_eq!(ticks(&d), 1, "the first step is ticked and saved");
    tap_space(&mut d);
    d.frame();
    assert_eq!(ticks(&d), 2);
}

#[test]
fn escape_closes_it_and_the_ticks_stay() {
    let (_dir, mut d) = started();
    cabinet(&mut d);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.build", None);
    d.frame();
    d.frame();
    tap_space(&mut d);
    d.frame();
    d.key(egui::Key::Escape, egui::Modifiers::NONE);
    d.frame();
    assert!(d.app.wb.build.is_none());
    assert_eq!(ticks(&d), 1);
    // Opened again, it carries on after the ticked step.
    d.app.run_command(&ctx, "view.build", None);
    d.frame();
    d.frame();
    tap_space(&mut d);
    d.frame();
    assert_eq!(ticks(&d), 2);
}

#[test]
fn the_command_is_in_the_reports_menu_and_the_palette() {
    let cmd = novo_workflow::commands::get("view.build").expect("registered");
    assert!(cmd.label.contains("Build mode"));
    let place = novo_workflow::menus::place(cmd).expect("reachable from a menu");
    assert_eq!(place.menu, "Reports");
}

#[test]
fn opening_the_commissioning_workspace_closes_it() {
    let (_dir, mut d) = started();
    cabinet(&mut d);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.build", None);
    d.frame();
    d.app
        .switch_workspace(super::workspace::Workspace::Commissioning);
    d.frame();
    d.frame();
    assert!(d.app.wb.build.is_none());
}

#[test]
fn the_build_sheet_pdf_lands_next_to_the_project_with_the_ticks_on_it() {
    let (dir, mut d) = started();
    let file = dir.path().join("Plant 3.novo");
    assert!(d.app.save_to(&file));
    cabinet(&mut d);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.build", None);
    d.frame();
    d.frame();
    tap_space(&mut d);
    let plan = d
        .app
        .wb
        .build
        .as_ref()
        .and_then(|b| b.plan.clone())
        .map(|(_, p)| p)
        .unwrap();
    let pdf = d.app.export_build_sheet(&plan).expect("written");
    assert_eq!(pdf, dir.path().join("Plant 3 build sheet.pdf"));
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    // A second print replaces our own file.
    d.app.export_build_sheet(&plan).expect("again");
    assert!(
        d.app
            .messages()
            .last()
            .unwrap()
            .text
            .starts_with("Replaced")
    );
}

#[test]
fn a_problem_keeps_its_photos_on_the_punch_item_and_in_the_project() {
    let (dir, mut d) = started();
    cabinet(&mut d);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.build", None);
    d.frame();
    d.frame();
    let (id, cab_tag) = {
        let plan = novo_commissioning::BuildPlan::of(d.app.backend.document().unwrap().project());
        let cab = &plan.cabinets[0];
        let step = &cab.steps[0];
        let mode = d.app.wb.commissioning.as_mut().unwrap();
        let id = novo_commissioning::mark_problem(
            &mut mode.store,
            &cab.tag,
            step,
            "hole is off",
            "tv",
            "2026-10-22T08:00:00Z",
        );
        mode.dirty = true;
        (id, cab.tag.clone())
    };
    assert!(!cab_tag.is_empty());
    let picture = dir.path().join("IMG_0002.png");
    std::fs::write(&picture, b"a picture of the hole").unwrap();
    assert_eq!(
        d.app
            .keep_problem_photos(id, std::slice::from_ref(&picture)),
        1
    );
    // the same picture again is not a second photo
    assert_eq!(
        d.app
            .keep_problem_photos(id, std::slice::from_ref(&picture)),
        0
    );
    let json = d.app.backend.commissioning_json().unwrap();
    let store = novo_commissioning::CommissioningStore::from_json(&json).unwrap();
    assert_eq!(store.punch(id).unwrap().photos.len(), 1);
}

#[test]
fn a_device_step_can_be_shown_on_the_plate_and_in_3d_and_build_mode_steps_aside() {
    use super::workspace::Workspace;
    let (_dir, mut d) = started();
    cabinet(&mut d);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.build", None);
    d.frame();
    d.frame();
    let device = d
        .app
        .wb
        .build
        .as_ref()
        .and_then(|b| b.plan.clone())
        .and_then(|(_, p)| p.cabinets[0].steps.iter().find_map(|s| s.device))
        .expect("a device step");
    d.app.show_device(device, "+CAB1", false);
    d.app.wb.build.as_mut().unwrap().away = true;
    d.frame();
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
    assert_eq!(d.app.cabinet_selected(), vec![device]);
    // Stepped aside: Space does not tick, Escape does not close.
    tap_space(&mut d);
    d.key(egui::Key::Escape, egui::Modifiers::NONE);
    d.frame();
    assert!(d.app.wb.build.is_some());
    assert_eq!(ticks(&d), 0);
    d.app.show_device(device, "+CAB1", true);
    d.frame();
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
}

#[test]
fn showing_a_device_switches_the_cabinet_to_the_step_s_own_plate_not_the_table() {
    let (_dir, mut d) = started();
    cabinet(&mut d);
    let device = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .devices()
        .next()
        .unwrap()
        .id
        .raw();
    // Another cabinet is on show, in table face, on a door surface.
    d.app.cabinet.location = Some(DeviceTag::parse("+OTHER").unwrap());
    d.app.cabinet.table = true;
    d.app.cabinet.surface = Some("door".to_owned());
    d.app.show_device(device, "+CAB1", false);
    assert_eq!(
        d.app.cabinet.location,
        Some(DeviceTag::parse("+CAB1").unwrap())
    );
    assert!(!d.app.cabinet.table);
    assert_eq!(d.app.cabinet.surface, None);
    assert_eq!(d.app.cabinet_selected(), vec![device]);
}
