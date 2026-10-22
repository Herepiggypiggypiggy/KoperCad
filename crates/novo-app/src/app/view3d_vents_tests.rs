//! The Vents and fans panel of the 3D view and the openings it draws.

use eframe::egui;

use super::heat_settings::{Kind, VentSpec};
use super::view3d_tests::drive_with_cabinet;

fn enclosure(d: &super::gate1_tests::Drive) -> novo_thermal::model::Enclosure {
    let project = d.app.backend.document().unwrap().project();
    let id = project
        .structures()
        .find(|s| s.enclosure && s.plate.is_some())
        .map(|s| s.id)
        .expect("a cabinet");
    novo_thermal::bridge::scenario_from_project(project, id, &Default::default())
        .expect("scenario")
        .scenario
        .enclosure
}

fn area(ctx: &egui::Context, name: &str) -> Option<egui::Rect> {
    ctx.memory(|m| m.area_rect(super::panel::id(name)))
}

/// An opening shows on its wall as soon as it is added, before any run, and
/// the selected one is marked; the panel keeps its width with the editor open
/// and does not cover the Heat panel.
#[test]
fn an_added_opening_is_drawn_at_once_and_the_panel_keeps_its_size() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.app.view3d.controls.heat_panel = true;
    let size = egui::vec2(1366.0, 768.0);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    assert!(d.app.view3d.vent_markers.is_empty());
    let e = enclosure(&d);
    let presets = VentSpec::presets(&e);
    d.app.cabinet.heat.settings.add(presets[2].spec);
    d.app.cabinet.heat.settings.add(presets[1].spec);
    for _ in 0..6 {
        d.frame_sized(size, Vec::new());
    }
    let markers = &d.app.view3d.vent_markers;
    assert_eq!(markers.len(), 2);
    assert!(!markers[0].selected && markers[1].selected);
    assert!(!d.app.view3d.vent_marks.is_empty());
    let ctx = d.ctx.clone();
    let vents = area(&ctx, "view3d_vents").expect("the Vents and fans panel is open");
    let heat = area(&ctx, "view3d_heat_panel").expect("the Heat panel is open");
    assert!(vents.width() <= 340.0, "panel grew to {vents:?}");
    assert!(!vents.intersects(heat), "{vents:?} covers {heat:?}");
    assert!(vents.bottom() <= size.y, "{vents:?}");
}

/// With openings in the cabinet a run switches the airflow on, the fan's
/// flow is read from the result and the streamlines include the air coming
/// in at it.
#[test]
fn a_run_with_a_filter_fan_shows_the_airflow_and_reads_its_flow() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.view3d.controls.heat_panel = true;
    let size = egui::vec2(1366.0, 768.0);
    d.frame_sized(size, Vec::new());
    let e = enclosure(&d);
    let fan = VentSpec::presets(&e)[2].spec;
    assert_eq!(fan.kind, Kind::FilterFan);
    d.app.cabinet.heat.settings.add(fan);
    d.app
        .cabinet
        .heat
        .settings
        .add(VentSpec::presets(&e)[1].spec);
    assert!(!d.app.view3d.heat.lines);
    d.app.run_command(&ctx, "heat.simulate", None);
    for _ in super::test_wait::patient() {
        d.frame_sized(size, Vec::new());
        if d.app.cabinet.heat.solution().is_some() && !d.app.cabinet.heat.running() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    assert!(d.app.cabinet.heat.solution().is_some(), "the run finishes");
    assert!(
        d.app.view3d.heat.lines,
        "airflow switched on for a cabinet with openings"
    );
    let m = &d.app.view3d.vent_markers;
    assert_eq!(m.len(), 2);
    assert!(
        m.iter().all(|m| m.measured_m3h.is_some()),
        "each opening reads its flow from the result: {m:?}"
    );
    assert_eq!(m[0].role, super::heat_openings::Role::In);
}

/// A fan's editor notes wrap inside the panel instead of widening it (a grid
/// cell does not wrap; the panel grew to 640 points with a filter fan open).
#[test]
fn a_filter_fan_open_in_the_editor_keeps_the_panel_narrow() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.app.view3d.controls.heat_panel = true;
    let size = egui::vec2(1366.0, 768.0);
    d.frame_sized(size, Vec::new());
    let e = enclosure(&d);
    d.app
        .cabinet
        .heat
        .settings
        .add(VentSpec::presets(&e)[2].spec);
    for _ in 0..6 {
        d.frame_sized(size, Vec::new());
    }
    let vents = area(&d.ctx.clone(), "view3d_vents").expect("panel");
    assert!(vents.width() <= 340.0, "panel grew to {vents:?}");
}

fn stored(d: &super::gate1_tests::Drive) -> String {
    let project = d.app.backend.document().unwrap().project();
    let id = project
        .structures()
        .find(|s| s.enclosure && s.plate.is_some())
        .map(|s| s.id)
        .unwrap();
    super::heat_openings_store::stored_text(project, id)
}

/// The openings are part of the project: written when the edit ends, read
/// back when the project changes under the panel (opened, undone), and a
/// damaged text does not stop anything.
#[test]
fn openings_are_saved_in_the_project_and_follow_its_undo() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.app.view3d.controls.heat_panel = true;
    let size = egui::vec2(1366.0, 768.0);
    for _ in 0..3 {
        d.frame_sized(size, Vec::new());
    }
    assert_eq!(stored(&d), "");
    let e = enclosure(&d);
    d.app
        .cabinet
        .heat
        .settings
        .add(VentSpec::presets(&e)[2].spec);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    let text = stored(&d);
    assert!(text.contains("FilterFan"), "{text}");
    // A drag in progress is not written until it ends.
    d.app.cabinet.heat.settings.checkpoint_edit();
    d.app.cabinet.heat.settings.vents[0].u_mm = 111.0;
    let corner = egui::pos2(1300.0, 700.0);
    let press = |down| egui::Event::PointerButton {
        pos: corner,
        button: egui::PointerButton::Primary,
        pressed: down,
        modifiers: egui::Modifiers::NONE,
    };
    d.frame_sized(size, vec![egui::Event::PointerMoved(corner), press(true)]);
    d.frame_sized(size, Vec::new());
    assert_eq!(stored(&d), text, "held while the gesture goes on");
    d.frame_sized(size, vec![press(false)]);
    for _ in 0..3 {
        d.frame_sized(size, Vec::new());
    }
    assert!(stored(&d).contains("111"), "{}", stored(&d));
    // The project's own undo takes the last change back, and the panel follows.
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::Undo);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    assert_eq!(stored(&d), text);
    assert_ne!(d.app.cabinet.heat.settings.vents[0].u_mm, 111.0);
    // Undo again: no openings at all.
    d.app.run(&ctx, crate::actions::Action::Undo);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    assert_eq!(stored(&d), "");
    assert!(d.app.cabinet.heat.settings.vents.is_empty());
}

/// Dragging an opening's label slides it along its wall, in one undo step.
#[test]
fn dragging_a_label_slides_the_opening_and_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.app.view3d.controls.heat_panel = true;
    let size = egui::vec2(1366.0, 768.0);
    for _ in 0..3 {
        d.frame_sized(size, Vec::new());
    }
    let e = enclosure(&d);
    d.app
        .cabinet
        .heat
        .settings
        .add(VentSpec::presets(&e)[2].spec);
    for _ in 0..6 {
        d.frame_sized(size, Vec::new());
    }
    let before = d.app.cabinet.heat.settings.vents[0];
    // The label chip: find it from the registered interaction rectangle.
    let chip = d
        .ctx
        .read_response(egui::Id::new(("heat_opening_label", 0usize)))
        .expect("the label is on screen")
        .rect;
    let from = chip.center();
    let to = from + egui::vec2(-60.0, 0.0);
    let button = |down| egui::Event::PointerButton {
        pos: if down { from } else { to },
        button: egui::PointerButton::Primary,
        pressed: down,
        modifiers: egui::Modifiers::NONE,
    };
    d.frame_sized(size, vec![egui::Event::PointerMoved(from), button(true)]);
    for step in 1..=6 {
        let at = from + (to - from) * (step as f32 / 6.0);
        d.frame_sized(size, vec![egui::Event::PointerMoved(at)]);
    }
    d.frame_sized(size, vec![egui::Event::PointerMoved(to), button(false)]);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    let after = d.app.cabinet.heat.settings.vents[0];
    assert!(
        (after.u_mm, after.v_mm) != (before.u_mm, before.v_mm),
        "it moved: {before:?} {after:?}"
    );
    assert_eq!(after.wall, before.wall, "it stays on its wall");
    assert_eq!(after.width_mm, before.width_mm);
    d.app.cabinet.heat.settings.undo();
    let back = d.app.cabinet.heat.settings.vents[0];
    assert_eq!((back.u_mm, back.v_mm), (before.u_mm, before.v_mm));
}

/// The Ctrl+K commands open the panel in the 3D view and add the opening.
#[test]
fn the_vent_commands_open_the_panel_and_add_the_opening() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let size = egui::vec2(1366.0, 768.0);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "heat.add_filter_fan", None);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    assert!(d.app.view3d.controls.heat_panel, "the panel is open");
    let vents = &d.app.cabinet.heat.settings.vents;
    assert_eq!(vents.len(), 1);
    assert_eq!(vents[0].kind, super::heat_settings::Kind::FilterFan);
    d.app.run_command(&ctx, "heat.add_exhaust_fan", None);
    d.app.run_command(&ctx, "heat.vents", None);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    assert_eq!(d.app.cabinet.heat.settings.vents.len(), 2);
}
