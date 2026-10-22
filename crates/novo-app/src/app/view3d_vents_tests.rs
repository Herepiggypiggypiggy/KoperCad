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
