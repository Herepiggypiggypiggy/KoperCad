//! The example project: Open example on the start panel and in the command
//! field (docs/ONBOARDING-DESIGN.md §1).

use novo_core::{DeviceTag, PageKind};

use super::example::EXAMPLE_NAME;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let d = Drive::new(&dir.path().join("untitled"));
    (dir, d)
}

fn tags(d: &Drive) -> Vec<String> {
    let mut tags: Vec<String> = d
        .doc()
        .project()
        .devices()
        .map(|dev| dev.tag.to_string())
        .collect();
    tags.sort();
    tags
}

#[test]
fn open_example_works_with_no_project_and_shows_the_motor_starter() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    assert!(d.app.document().is_none());
    assert!(d.app.is_enabled(Action::OpenExample));
    d.app.run(&ctx, Action::OpenExample);
    d.frame();

    let project = d.doc().project();
    assert_eq!(project.info().name, EXAMPLE_NAME);
    for tag in [
        "=A1+CAB-K1",
        "=A1+CAB-F2",
        "=A1+CAB-X1",
        "=A1+FLD-M1",
        "=A1+CAB-S0",
        "=A1+CAB-S1",
    ] {
        let tag = DeviceTag::parse(tag).unwrap();
        assert_eq!(project.devices_by_tag(&tag).len(), 1, "{tag}");
    }
    let drawn = project
        .pages_in_order()
        .into_iter()
        .filter(|p| !matches!(p.kind, PageKind::Generated { .. }))
        .count();
    assert_eq!(drawn, 2, "main and control circuit");
    assert!(
        project.pages().count() > drawn,
        "generated pages come with it"
    );
    assert!(d.app.current_page().is_some());
}

#[test]
fn the_example_has_no_error_level_problem() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let errors: Vec<_> = novo_checks::check(d.doc().project())
        .into_iter()
        .filter(|p| p.severity == novo_checks::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn opening_the_example_twice_gives_the_same_project() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let first = tags(&d);
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert_eq!(tags(&d), first);
}

#[test]
fn closing_the_example_untouched_leaves_nothing_under_unsaved() {
    let (dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert!(!d.app.is_dirty(), "marked saved once drawn");
    d.app.run(&ctx, Action::CloseProject);
    d.frame();
    assert!(d.app.backend.unsaved_projects().is_empty());
    let left = std::fs::read_dir(dir.path().join("untitled"))
        .map(|rd| rd.count())
        .unwrap_or(0);
    assert_eq!(left, 0, "the temporary file is gone");
}

#[test]
fn an_edited_example_is_kept_like_any_unsaved_project() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.place("mcb3", 120.0, 48.0);
    d.frame();
    assert!(d.app.is_dirty());
    d.app.run(&ctx, Action::CloseProject);
    d.frame();
    assert_eq!(d.app.backend.unsaved_projects().len(), 1);
}

#[test]
fn the_example_replaces_an_open_project() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert_eq!(d.doc().project().info().name, EXAMPLE_NAME);
    assert_eq!(tags(&d).len(), 8, "eight devices, the strip included");
}

#[test]
fn the_example_cabinet_is_complete_and_simulates() {
    use novo_core::{RailKind, Structure};
    use novo_thermal::bridge::{BridgeOptions, scenario_from_project};

    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let project = d.doc().project();

    let cab: &Structure = project
        .structures()
        .find(|s| s.enclosure)
        .expect("the example has a cabinet");
    assert!(cab.plate.is_some(), "mounting plate");
    let enclosure = novo_layout::model::enclosure_of(project, cab.id)
        .expect("interior width, height and depth are set");
    assert!(enclosure.plate_setback > 0.0, "plate setback");
    assert!(enclosure.door.is_some(), "a door with a hinge side");
    let rails = |kind| {
        project
            .all::<novo_core::Rail>()
            .filter(move |r| r.kind == kind)
    };
    assert!(rails(RailKind::DinRail).count() > 0, "DIN rails");
    assert!(rails(RailKind::Duct).count() > 0, "ducts");

    // Every device of the cabinet is placed (Place all has nothing left).
    let layout = novo_layout::model::load(project, cab.id).unwrap();
    assert_eq!(
        layout.placements.len(),
        novo_layout::model::candidates(project, cab.id).len()
    );
    assert!(layout.placements.iter().all(|p| !p.size.guessed));

    // The heat scenario builds with nothing guessed or missing.
    let bridge = scenario_from_project(project, cab.id, &BridgeOptions::default())
        .expect("no missing-interior or missing-plate error");
    assert!(bridge.report.guessed_size.is_empty(), "{:?}", bridge.report);
    assert!(bridge.report.without_loss.is_empty(), "{:?}", bridge.report);
    assert!(
        bridge.report.without_depth.is_empty(),
        "{:?}",
        bridge.report
    );
    assert!(bridge.report.unplaced.is_empty(), "{:?}", bridge.report);
    assert!(bridge.report.total_loss_w > 10.0, "{:?}", bridge.report);

    // A coarse steady solve gives a believable result: devices run warmer
    // than the surroundings, and the hottest is well below boiling.
    let result = novo_thermal::simulate_steady_with(
        &bridge.scenario,
        novo_thermal::Options {
            bands: false,
            advice: false,
        },
    )
    .expect("the solve runs");
    let hottest = result
        .devices
        .iter()
        .map(|dev| dev.surface_c)
        .fold(f64::MIN, f64::max);
    assert!(
        hottest > result.ambient_c && hottest < 100.0,
        "hottest device {hottest} C, ambient {} C",
        result.ambient_c
    );

    // The catalogue losses are planning estimates, and the heat view says so.
    let items = crate::app::heat::items_of(project, &layout, &|id| id.to_string());
    let with_loss = items.iter().filter(|i| i.loss_known).count();
    assert!(with_loss > 0);
    assert!(
        items
            .iter()
            .filter(|i| i.loss_known)
            .all(|i| i.loss_estimated),
        "every loss of the example is an estimate: {items:?}"
    );
}

#[test]
fn the_example_wires_are_numbered_and_commissioning_names_them_by_number() {
    use novo_commissioning::{Target, targets};
    use novo_reports::{Report, WireList};
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let project = d.doc().project();
    let unnumbered: Vec<_> = project
        .wires()
        .filter(|w| w.number.as_deref().is_none_or(str::is_empty))
        .map(|w| w.id)
        .collect();
    assert!(unnumbered.is_empty(), "unnumbered wires: {unnumbered:?}");
    let list = WireList.generate(project);
    let section = &list.sections[0];
    let col = section.column("number").expect("wire number column");
    assert!(!section.rows.is_empty());
    for row in &section.rows {
        assert!(
            !row.cells[col].to_string().is_empty(),
            "a wire-list row without a number: {row:?}"
        );
    }
    let wires: Vec<_> = targets(project)
        .into_iter()
        .filter_map(|t| match t.target {
            Target::Wire(id) => Some((id, t.label)),
            _ => None,
        })
        .collect();
    assert!(!wires.is_empty());
    for (id, label) in wires {
        assert_ne!(label, id.to_string(), "commissioning names a wire by id");
    }
}

/// Round trip: the example project written as PXF and read back into an
/// empty project keeps its devices, terminals, cables and pages, and says
/// what it did not keep. Passing proves the file agrees with this reader,
/// not that EPLAN accepts it.
#[test]
fn the_example_survives_a_pxf_round_trip() {
    use novo_core::Document;
    use novo_interop::eplan::pxf::{
        PxfOptions, PxfWriteOptions, plan_pxf, read_pxf_str, write_pxf,
    };

    let mut doc = Document::default();
    super::example::build_example(&mut doc).unwrap();
    let before = doc.project();
    let out = write_pxf(before, &PxfWriteOptions::default());
    let file = read_pxf_str(&out.text).unwrap();
    let mut again = Document::default();
    let plan = plan_pxf(again.project(), &file, &PxfOptions::default());
    plan.apply(&mut again).unwrap();
    let after = again.project();

    let tags = |p: &novo_core::Project| {
        let mut v: Vec<String> = p.devices().map(|d| d.tag.to_string()).collect();
        v.sort();
        v
    };
    let drawn_devices = |p: &novo_core::Project| {
        p.devices()
            .filter(|d| {
                p.symbols().any(|s| {
                    matches!(&s.role, novo_core::SymbolRole::Device { device, .. } if *device == d.id)
                })
            })
            .count()
    };
    // Devices that carry a symbol come back; the rest are only in the report.
    let missing: Vec<String> = tags(before)
        .into_iter()
        .filter(|t| !tags(after).contains(t))
        .collect();
    assert!(missing.is_empty(), "devices not back: {missing:?}");
    assert_eq!(before.cables().count(), after.cables().count());
    assert_eq!(
        before.pages().count(),
        after.pages().count(),
        "{}",
        out.report.text()
    );
    // No EPLAN symbol table was given, so the symbols are written without a
    // reference and nothing is drawn on the way back: the report says so.
    assert!(drawn_devices(before) > 0);
    assert_eq!(drawn_devices(after), 0);
    let text = out.report.text();
    assert!(text.contains("without a symbol reference"), "{text}");
    assert!(
        before.wires().count() == 0 || text.contains("wires"),
        "{text}"
    );
    let cores = |p: &novo_core::Project| p.cables().map(|c| c.cores.len()).sum::<usize>();
    assert_eq!(cores(before), cores(after));
    assert_eq!(before.terminals().count(), after.terminals().count());
}

#[test]
fn the_example_cabinet_problems_pill_can_say_what_it_counts() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let p = d.doc().project();
    let cab = p
        .structures()
        .find(|s| s.tag.to_string() == "+CAB")
        .unwrap();
    let l = novo_layout::model::load(p, cab.id).unwrap();
    let name = |id: u64| format!("#{id}");
    let lines = super::cabinet::panel_problem_lines(&l, &name);
    assert!(
        lines.iter().any(|t| t.contains("free room in front")),
        "{lines:?}"
    );
}

#[test]
fn open_example_starts_with_an_empty_undo_history() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert!(!d.app.is_enabled(Action::Undo), "nothing to undo yet");
    assert!(!d.app.is_enabled(Action::Redo));
    assert!(!d.doc().is_dirty());
    let before = tags(&d);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(tags(&d), before);
    assert!(!d.doc().is_dirty(), "Ctrl+Z must not dirty a fresh example");
}

/// The part text says "AC-3 9 A": the Inspector's AC-3 current input must
/// say the same, from the same source (audit C-49).
#[test]
fn the_example_parts_state_the_same_ac3_current_in_text_and_input() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let project = d.doc().project();
    let mut checked = 0;
    for device in project.devices() {
        let Some(part) = device.parts.first().and_then(|&p| project.part(p)) else {
            continue;
        };
        let Some(stated) = ac3_in_text(&part.description) else {
            continue;
        };
        let info = super::inspector::device_info(project, None, None, device.id).unwrap();
        let at = super::inspector::DEVICE_INPUTS
            .iter()
            .position(|(key, ..)| *key == "ac3_current_a")
            .unwrap();
        let (shown, _) = &info.calc_inputs[at];
        assert_eq!(
            shown.parse::<f64>().ok(),
            Some(stated),
            "{}: the text says AC-3 {stated} A",
            device.tag
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "the example has a contactor with an AC-3 rating"
    );
}

/// The number in "AC-3 9 A" of a part description.
fn ac3_in_text(description: &str) -> Option<f64> {
    let rest = description.split("AC-3 ").nth(1)?;
    rest.split_whitespace().next()?.parse().ok()
}

#[test]
fn reopening_a_saved_project_gets_report_pages_without_an_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    super::route_all_tests::wired_cabinet(&mut d);
    let saved_pages = d.doc().project().pages().count();
    let file = dir.path().join("Reopen.novo");
    assert!(d.app.save_to(&file));
    d.app.run(&ctx, Action::CloseProject);
    d.frame();
    // A fresh window, as after restarting the program.
    let mut e = Drive::new(&dir.path().join("untitled2"));
    e.app.open_path(&file);
    e.frame();
    let pages = e.doc().project().pages().count();
    assert!(
        pages > saved_pages,
        "opening adds the report pages: {saved_pages} -> {pages}"
    );
    assert!(
        !e.app.is_enabled(Action::Undo),
        "nothing to undo after opening, but: {:?}",
        e.doc().undo_label()
    );
    let ctx = e.ctx.clone();
    e.app.run(&ctx, Action::Undo);
    e.frame();
    assert_eq!(
        e.doc().project().pages().count(),
        pages,
        "undo does not remove them"
    );
}
