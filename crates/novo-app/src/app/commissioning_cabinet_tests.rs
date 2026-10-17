//! Redline marks on the Cabinet plate.

use eframe::egui;
use novo_commissioning::{Change, CommissioningStore, Redline, RedlineState, Target, targets};
use novo_layout::state::CabinetLayout;
use novo_layout::{Margins, Placement, Plate, Size};

use super::commissioning_cabinet::{draw, marks};
use super::gate1_tests::Drive;
use crate::actions::Action;

fn redline(target: Target, change: Change, note: &str) -> Redline {
    Redline {
        id: 0,
        target,
        label: "L".into(),
        change,
        note: note.into(),
        by: "tv".into(),
        at: "2026-10-17T08:00:00Z".into(),
        state: RedlineState::Open,
    }
}

fn layout_with(device: u64) -> CabinetLayout {
    let mut l = CabinetLayout::new(Plate {
        width: 600.0,
        height: 800.0,
        margin: 10.0,
    });
    l.placements = vec![Placement {
        id: device,
        x: 100.0,
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
    }];
    l
}

#[test]
fn open_redlines_on_a_placed_device_or_its_terminals_are_marked_once_with_every_line() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let project = d.doc().project();
    let all = targets(project);
    let terminal = all
        .iter()
        .find(|t| matches!(t.target, Target::Terminal(_)))
        .expect("a terminal");
    let wire = all
        .iter()
        .find(|t| matches!(t.target, Target::Wire(_)))
        .expect("a wire");
    let Target::Terminal(id) = terminal.target else {
        unreachable!()
    };
    let strip = project.terminal(id).unwrap().strip;
    let layout = layout_with(strip.raw());

    let mut store = CommissioningStore::new();
    store
        .add_redline(redline(terminal.target, Change::Note, "terminal 3 is a PE"))
        .unwrap();
    store
        .add_redline(redline(
            Target::Device(strip),
            Change::Note,
            "strip is shorter",
        ))
        .unwrap();
    store
        .add_redline(redline(wire.target, Change::WireNumber("7".into()), ""))
        .unwrap();
    let found = marks(&store, project, &layout);
    assert_eq!(found.len(), 1, "one outline for the strip; wires have none");
    assert_eq!(found[0].footprint, (100.0, 100.0, 45.0, 90.0));
    assert!(
        found[0].text.contains("terminal 3 is a PE"),
        "{}",
        found[0].text
    );
    assert!(
        found[0].text.contains("strip is shorter"),
        "{}",
        found[0].text
    );
    assert_eq!(found[0].text.lines().count(), 2);

    // a device that is not on the plate has no mark; applied ones go
    assert!(marks(&store, project, &layout_with(strip.raw() + 1000)).is_empty());
    for r in store.redlines().map(|r| r.id).collect::<Vec<_>>() {
        store.set_redline_state(r, RedlineState::Applied).unwrap();
    }
    assert!(marks(&store, project, &layout).is_empty());
}

#[test]
fn the_outline_reports_its_text_only_while_the_pointer_is_over_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let project = d.doc().project();
    let device = targets(project)
        .into_iter()
        .find_map(|t| match t.target {
            Target::Device(id) => Some(id),
            _ => None,
        })
        .expect("a device");
    let mut store = CommissioningStore::new();
    store
        .add_redline(redline(Target::Device(device), Change::Note, "other make"))
        .unwrap();
    let found = marks(&store, project, &layout_with(device.raw()));

    let egui_ctx = egui::Context::default();
    let painter = egui_ctx.layer_painter(egui::LayerId::background());
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    // the default viewport centres world (0,0) on screen centre at zoom 2
    let view = novo_canvas::Viewport::default();
    let at = |x: f32, y: f32| {
        let s = view.world_to_screen(novo_canvas::vec2(x, y));
        egui::pos2(s.x, s.y)
    };
    let over = draw(
        &painter,
        rect,
        &view,
        &found,
        egui::Color32::YELLOW,
        Some(at(120.0, 130.0)),
    );
    assert!(over.is_some_and(|t| t.contains("other make")));
    let away = draw(
        &painter,
        rect,
        &view,
        &found,
        egui::Color32::YELLOW,
        Some(at(400.0, 400.0)),
    );
    assert!(away.is_none());
    assert!(draw(&painter, rect, &view, &found, egui::Color32::YELLOW, None).is_none());
}

#[test]
fn the_plate_draws_with_an_open_redline_on_a_placed_device() {
    use super::cabinet::Ask;
    use super::cabinet_tests::add_cabinet;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
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
    d.frame();
    let project = d.app.backend.document().unwrap().project();
    let layout = novo_layout::model::load(project, cab);
    let cands = novo_layout::model::candidates(project, cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.frame();
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &cands);
    d.frame();
    let project = d.app.backend.document().unwrap().project();
    let layout = novo_layout::model::load(project, cab).expect("a layout");
    let device = layout.placements[0].id;
    d.app.load_commissioning();
    let mode = d.app.wb.commissioning.as_mut().unwrap();
    mode.store
        .add_redline(redline(
            Target::Device(novo_core::DeviceId::from(device)),
            Change::Note,
            "other make",
        ))
        .unwrap();
    d.frame();
    d.frame();
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    let project = d.app.backend.document().unwrap().project();
    assert_eq!(marks(&mode.store, project, &layout).len(), 1);
    assert!(d.app.cabinet.plate_rect.is_some(), "the plate was drawn");

    // a project reopened straight into the Cabinet: the mode holds nothing
    // yet, and drawing the plate reads the saved redline back
    let json = d.app.wb.commissioning.as_ref().unwrap().store.to_json();
    d.app.backend.set_commissioning_json(&json);
    d.app.wb.commissioning = None;
    d.frame();
    let mode = d
        .app
        .wb
        .commissioning
        .as_ref()
        .expect("loaded by the plate");
    assert_eq!(mode.store.redlines().count(), 1);
}

#[test]
fn a_device_on_the_door_has_no_outline_on_the_plate() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let project = d.doc().project();
    let device = targets(project)
        .into_iter()
        .find_map(|t| match t.target {
            Target::Device(id) => Some(id),
            _ => None,
        })
        .expect("a device");
    let mut store = CommissioningStore::new();
    store
        .add_redline(redline(Target::Device(device), Change::Note, "door lamp"))
        .unwrap();
    let mut layout = layout_with(device.raw());
    assert_eq!(marks(&store, project, &layout).len(), 1);
    layout.off_plate.insert(device.raw(), "Door".into());
    assert!(marks(&store, project, &layout).is_empty());
}
