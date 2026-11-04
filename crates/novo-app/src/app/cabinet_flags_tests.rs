//! Problem triangles on the Cabinet plate (lane T1): the plate, its pill, its
//! table and its inspector read the one flag store.

use eframe::egui;
use novo_checks::Summary;
use novo_core::{DeviceTag, StructureId, Value};

use super::cabinet::Ask;
use super::cabinet_flags::{self, Spot};
use super::gate1_tests::Drive;
use super::workspace::Workspace;
use crate::actions::Action;

fn project(d: &Drive) -> &novo_core::Project {
    d.app.backend.document().unwrap().project()
}

/// A cabinet with two 45 x 90 mm devices, dropped on top of each other.
fn overlapping(d: &mut Drive) -> (StructureId, u64, u64) {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
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
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    let ids: Vec<u64> = novo_layout::model::candidates(project(d), cab)
        .iter()
        .map(|c| c.id)
        .collect();
    for (id, x) in [(ids[0], 100.0), (ids[1], 110.0)] {
        let l = novo_layout::model::load(project(d), cab);
        d.app
            .cabinet_ask(Ask::Drop(id, x, 120.0), Some(cab), l, &[]);
    }
    d.frame();
    d.frame();
    (cab, ids[0], ids[1])
}

fn synced(d: &mut Drive) {
    let _ = d.app.shown_problems();
    d.app.sync_flags();
}

#[test]
fn overlapping_devices_get_a_red_triangle_on_each() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, b) = overlapping(&mut d);
    synced(&mut d);
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    let clusters = cabinet_flags::clusters(&d.app.wb.flags.store, &layout, cab);
    let spots: Vec<Spot> = clusters.iter().map(|c| c.spot).collect();
    assert!(spots.contains(&Spot::Device(a)), "{spots:?}");
    assert!(spots.contains(&Spot::Device(b)), "{spots:?}");
    assert!(clusters.iter().all(|c| c.red), "an overlap is an error");
    assert!(
        clusters[0].name.starts_with("Error: Devices overlap"),
        "{}",
        clusters[0].name
    );
}

#[test]
fn the_cabinet_count_is_the_top_bar_count_for_the_same_facts() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, _, _) = overlapping(&mut d);
    synced(&mut d);
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    let roll = cabinet_flags::roll_up(&d.app.wb.flags.store, &layout, cab);
    let top = Summary::of(d.app.problems());
    assert!(roll.total() > 0, "the overlap is counted");
    assert_eq!(roll.red, top.errors, "{roll:?} against {top:?}");
    assert_eq!(roll.yellow, top.warnings, "{roll:?} against {top:?}");
}

#[test]
fn a_waived_problem_leaves_the_plate_and_its_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, _, _) = overlapping(&mut d);
    synced(&mut d);
    d.app
        .wb
        .flags
        .store
        .waive_key(novo_checks::ProblemKind::DeviceCollision.key());
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    let store = &d.app.wb.flags.store;
    assert!(
        cabinet_flags::plate_flags(store, &layout, cab)
            .iter()
            .all(|f| f.title != "Devices overlap")
    );
}

#[test]
fn two_flags_on_one_device_share_a_triangle_with_a_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, _) = overlapping(&mut d);
    synced(&mut d);
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    let on_a = cabinet_flags::device_flags(&d.app.wb.flags.store, &layout, cab, a);
    let cluster = cabinet_flags::clusters(&d.app.wb.flags.store, &layout, cab)
        .into_iter()
        .find(|c| c.spot == Spot::Device(a))
        .unwrap();
    assert_eq!(cluster.ids.len(), on_a.len());
}

#[test]
fn the_plate_draws_its_triangles_and_locate_selects_the_device() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, _) = overlapping(&mut d);
    synced(&mut d);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
    // Locate from the schematic side lands on the plate with the device chosen.
    let flag = d
        .app
        .wb
        .flags
        .store
        .list(false)
        .into_iter()
        .find(|f| f.title == "Devices overlap")
        .cloned()
        .unwrap();
    d.app.cabinet.view.selection.clear();
    d.app.locate_on_plate(&flag);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
    assert!(d.app.cabinet_selected().contains(&a));
    let _ = (cab, egui::Pos2::ZERO);
}

/// The census of the Cabinet area (design section 12): lines that still say a
/// problem in a toast, banner or coloured sentence. The ceiling in
/// `census/cabinet.ceiling` only goes down; each lane lowers its own file.
#[test]
fn problem_census_cabinet() {
    const FILES: [&str; 9] = [
        "cabinet.rs",
        "cabinet_rules.rs",
        "cabinet_rules_panel.rs",
        "cabinet_surfaces.rs",
        "cabinet_rails.rs",
        "cabinet_rail_ends.rs",
        "cabinet_panel.rs",
        "cabinet_inspector.rs",
        "cabinet_flags.rs",
    ];
    const PATTERNS: [&str; 7] = [
        "Level::Warning",
        "Level::Error",
        "widgets::banner(",
        "warn_fg_color",
        "error_fg_color",
        ".warning)",
        ".error)",
    ];
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app");
    let mut found = 0;
    for file in FILES {
        let text = std::fs::read_to_string(dir.join(file)).unwrap();
        let code = text.split("#[cfg(test)]").next().unwrap();
        found += code
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .filter(|l| PATTERNS.iter().any(|p| l.contains(p)))
            .count();
    }
    let ceiling: usize = std::fs::read_to_string(dir.join("census/cabinet.ceiling"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        found <= ceiling,
        "{found} problem sentences in the Cabinet files, the ceiling is {ceiling}"
    );
    assert!(
        found >= ceiling,
        "{found} is below the ceiling {ceiling}: lower census/cabinet.ceiling to {found}"
    );
}
