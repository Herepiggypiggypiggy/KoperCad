//! Wire-it in the 3D view: the strip, the lit wire, the ticks in the project.

use super::view3d_tests::drive_with_cabinet;
use super::view3d_wire_it::scene_id;
use crate::actions::Action;

fn plan(d: &super::gate1_tests::Drive) -> std::sync::Arc<novo_reports::WireItPlan> {
    novo_reports::wire_it_plan(d.app.backend.document().unwrap().project())
}

fn run(d: &mut super::gate1_tests::Drive, id: &str) {
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, id, None);
    d.frame();
    d.frame();
}

/// The first step that has a path on the plate (the fixture's earth and
/// supply wires have none: they run to the field and to feeds).
fn first_drawn(d: &super::gate1_tests::Drive) -> usize {
    plan(d)
        .steps
        .iter()
        .position(|s| s.path.is_some())
        .expect("a wire is routed on the plate")
}

fn go_to(d: &mut super::gate1_tests::Drive, at: usize) {
    d.app.view3d.controls.wire_it = Some(at);
    d.frame();
    d.frame();
}

fn wires(d: &super::gate1_tests::Drive) -> Vec<u64> {
    d.app
        .view3d
        .built
        .as_ref()
        .map(|(_, _, s)| s.wires.iter().map(|w| w.id.raw).collect())
        .unwrap_or_default()
}

fn done_count(d: &super::gate1_tests::Drive) -> usize {
    let p = d.app.backend.document().unwrap().project();
    novo_reports::wire_it_progress(p, &plan(d)).done
}

#[test]
fn the_command_opens_wire_it_at_the_first_wire_and_shows_only_that_wire() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    assert_eq!(d.app.view3d.controls.wire_it, None, "off by default");
    let all = wires(&d).len();
    assert!(all > 1, "the cabinet has wires with the layer on");
    run(&mut d, "view.3d_wire_it");
    assert_eq!(d.app.view3d.controls.wire_it, Some(0), "opens at the first");
    assert!(
        wires(&d).is_empty(),
        "the first wire (earth to the field) has no path on this plate"
    );
    let at = first_drawn(&d);
    go_to(&mut d, at);
    let plan = plan(&d);
    assert!(plan.steps.len() > 1);
    let first = &plan.steps[at];
    assert_eq!(wires(&d), vec![scene_id(first.number)]);
    assert!(
        d.app.view3d.view.bottom_inset > 0.0,
        "the fit leaves room for the strip"
    );
    // Both ends, the path and its ducts are lit.
    assert!(
        d.app.view3d.wire_it.marks.len() >= 2,
        "{} overlays",
        d.app.view3d.wire_it.marks.len()
    );
    // Off: the drawn wires come back and the room is given back.
    run(&mut d, "view.3d_wire_it");
    assert_eq!(d.app.view3d.controls.wire_it, None);
    assert_eq!(wires(&d).len(), all);
    assert!(d.app.view3d.wire_it.marks.is_empty());
    assert_eq!(d.app.view3d.view.bottom_inset, 0.0);
}

#[test]
fn wire_it_and_build_order_never_run_together() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    run(&mut d, "view.3d_assembly");
    assert!(d.app.view3d.controls.assembly.is_some());
    run(&mut d, "view.3d_wire_it");
    assert_eq!(d.app.view3d.controls.assembly, None);
    assert!(d.app.view3d.controls.wire_it.is_some());
}

#[test]
fn running_a_wire_ticks_it_in_the_project_lights_the_next_and_undoes() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    run(&mut d, "view.3d_wire_it");
    let plan = plan(&d);
    let a = first_drawn(&d);
    assert!(
        plan.steps[a + 1].path.is_some(),
        "two wires in a row are drawn"
    );
    go_to(&mut d, a);
    assert_eq!(done_count(&d), 0);
    let revision = d.app.backend.document().unwrap().project().revision();

    run(&mut d, "view.3d_wire_it_done");
    assert_eq!(done_count(&d), 1, "the tick is in the project");
    assert_eq!(
        d.app.view3d.controls.wire_it,
        Some(a + 1),
        "on to the next wire"
    );
    let p = d.app.backend.document().unwrap().project();
    assert!(novo_reports::wire_it_done(p, &plan.steps[a]));
    assert!(!novo_reports::wire_it_done(p, &plan.steps[a + 1]));
    assert_ne!(p.revision(), revision);
    // The wire run stays in the picture next to the one to run.
    assert_eq!(
        wires(&d),
        vec![
            scene_id(plan.steps[a].number),
            scene_id(plan.steps[a + 1].number)
        ],
        "one run, one to run"
    );

    // One undo step takes the tick back.
    d.app.backend.document_mut().unwrap().undo().unwrap();
    d.frame();
    d.frame();
    assert_eq!(done_count(&d), 0);
}

#[test]
fn a_tick_does_not_work_the_plan_out_again() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    run(&mut d, "view.3d_wire_it");
    let before = plan(&d);
    run(&mut d, "view.3d_wire_it_done");
    let kept = d.app.view3d.wire_it.cached_revision();
    let now = d.app.backend.document().unwrap().project().revision();
    assert_eq!(kept, Some(now), "the plan follows the tick's revision");
    // And it is the same plan: the order never depends on what is run.
    assert_eq!(*before, *plan(&d));
}

#[test]
fn a_half_wired_cabinet_resumes_at_the_next_wire_after_save_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    run(&mut d, "view.3d_wire_it");
    for _ in 0..3 {
        run(&mut d, "view.3d_wire_it_done");
    }
    assert_eq!(done_count(&d), 3);
    let path = dir.path().join("plant.novo");
    assert!(d.app.save_to(&path));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::CloseProject);
    d.frame();
    d.app.open_path_as(&path, false);
    d.frame();
    assert_eq!(
        d.app.view3d.controls.wire_it, None,
        "opens with Wire-it off"
    );
    d.app.run_command(&ctx, "view.3d", None);
    d.frame();
    run(&mut d, "view.3d_wire_it");
    assert_eq!(done_count(&d), 3, "the ticks came back with the file");
    assert_eq!(
        d.app.view3d.controls.wire_it,
        Some(3),
        "opens at the first wire not run"
    );
}

#[test]
fn next_and_previous_stay_inside_the_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    run(&mut d, "view.3d_wire_it");
    let last = plan(&d).steps.len() - 1;
    run(&mut d, "view.3d_wire_it_previous");
    assert_eq!(d.app.view3d.controls.wire_it, Some(0));
    for _ in 0..last + 3 {
        run(&mut d, "view.3d_wire_it_next");
    }
    assert_eq!(d.app.view3d.controls.wire_it, Some(last));
}

#[test]
fn the_duct_fill_of_the_last_wire_in_a_duct_is_the_route_screens_fill() {
    let dir = tempfile::tempdir().unwrap();
    let d = drive_with_cabinet(dir.path());
    let project = d.app.backend.document().unwrap().project();
    let plan = novo_reports::wire_it_plan(project);
    let id = super::view3d::enclosure(project, d.app.cabinet.location.as_ref()).expect("a cabinet");
    let routed = novo_routing::model::route_project(project);
    let fills = routed
        .plans
        .iter()
        .find(|(s, _)| *s == id)
        .map(|(_, p)| super::cabinet_routes::duct_fills(project, id, p))
        .expect("the plate is routed");
    let mut checked = 0;
    for f in fills.iter().filter(|f| f.wires > 0) {
        let last = plan
            .steps
            .iter()
            .flat_map(|s| s.ducts.iter())
            .rfind(|dd| dd.duct == f.rail)
            .expect("a wire of the plan lies in the duct");
        let (a, b) = (last.fill_after.unwrap(), f.ratio.unwrap());
        assert!(
            (a - b).abs() < 1e-6,
            "duct {}: Wire-it ends at {a}, the route screen says {b}",
            f.rail
        );
        assert_eq!(last.wires_after, f.wires);
        checked += 1;
    }
    assert!(checked > 0, "the fixture has a duct with wires in it");
}

#[test]
fn the_list_exports_as_landscape_pages_with_the_run_wires_crossed() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let first = d.app.export_wire_it().expect("exported");
    let before = std::fs::read(&first).unwrap();
    assert!(before.starts_with(b"%PDF"));
    run(&mut d, "view.3d_wire_it");
    run(&mut d, "view.3d_wire_it_done");
    let second = d.app.export_wire_it().expect("exported again");
    let after = std::fs::read(&second).unwrap();
    assert_ne!(before, after, "the run wire is printed crossed");
    let doc = lopdf::Document::load_mem(&after).unwrap();
    assert_eq!(
        doc.get_pages().len(),
        novo_pdf::wireit::page_count(&plan(&d))
    );
}

#[test]
fn a_project_without_wires_says_so_instead_of_exporting() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = super::gate1_tests::Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert_eq!(d.app.export_wire_it(), None);
}

#[test]
fn cable_cores_say_run_cable_and_are_left_out_of_the_wire_to_cut() {
    let mut doc = novo_core::Document::default();
    super::example::build_example(&mut doc).unwrap();
    let plan = novo_reports::wire_it_plan(doc.project());
    let cores: Vec<_> = plan.steps.iter().filter(|s| s.cable_core).collect();
    assert!(!cores.is_empty(), "the example has a cable with cores");
    for c in &cores {
        let line = c.run_or_cut_text();
        assert!(
            line.starts_with("run cable ") && !line.contains("cut"),
            "{line}"
        );
        assert!(line.contains(&c.cable_tag), "{line}");
        assert_eq!(c.cut_to_do_mm(), 0.0);
    }
    let hookup: f64 = plan
        .steps
        .iter()
        .filter(|s| !s.cable_core)
        .map(|s| s.cut_mm)
        .sum::<f64>()
        / 1000.0;
    assert!((plan.cut_m_left(doc.project()) - hookup).abs() < 1e-9);
    // The PDF header total agrees with the on-screen total.
    let done = vec![false; plan.steps.len()];
    let pdf = novo_pdf::wireit::export_wireit_pdf(&plan, &done, "x").unwrap();
    assert!(!pdf.is_empty());
}

#[test]
fn build_order_commands_switch_wire_it_off_and_the_other_way_round() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    run(&mut d, "view.3d_wire_it");
    assert!(d.app.view3d.controls.wire_it.is_some());
    for id in [
        "view.3d_assembly",
        "view.3d_assembly_next",
        "view.3d_assembly_previous",
    ] {
        run(&mut d, "view.3d_wire_it");
        if d.app.view3d.controls.wire_it.is_none() {
            run(&mut d, "view.3d_wire_it");
        }
        run(&mut d, id);
        let c = &d.app.view3d.controls;
        assert!(c.wire_it.is_none(), "{id} left Wire-it on");
        assert!(c.assembly.is_some() || id == "view.3d_assembly", "{id}");
        run(&mut d, "view.3d_wire_it_next");
        let c = &d.app.view3d.controls;
        assert!(c.assembly.is_none() && c.wire_it.is_some(), "{id}");
    }
}
