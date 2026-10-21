//! The run card, the lit path, the entry marker and the length basis in the
//! 3D view, driven through the app.

use std::collections::BTreeSet;

use eframe::egui;
use novo_core::{Cable, CableCore, DeviceTag, StructureId};
use novo_view3d::scene::{ItemId, ItemKind};
use novo_view3d::{Overlay, OverlayLines};

use super::gate1_tests::Drive;
use super::view3d_tests::drive_with_cabinet;

fn lines_of(d: &Drive) -> Vec<&OverlayLines> {
    d.app
        .view3d
        .run_marks
        .iter()
        .filter_map(|o| match o {
            Overlay::Lines(l) => Some(l),
            _ => None,
        })
        .collect()
}

fn drawn(d: &Drive) -> Vec<u64> {
    d.app
        .view3d
        .built
        .as_ref()
        .expect("a scene")
        .2
        .wires
        .iter()
        .map(|w| w.id.raw)
        .collect()
}

/// Frames a second and more apart: the basis is read only once the project
/// has stayed unchanged for a moment.
fn settle(d: &mut Drive) {
    for _ in 0..3 {
        let t = d.ctx.input(|i| i.time) + 1.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            time: Some(t),
            ..Default::default()
        };
        let app = &mut d.app;
        let mut out = d.ctx.run_ui(input, |ui| app.show(ui));
        out.textures_delta.clear();
    }
}

fn pick(d: &mut Drive, wires: &[u64]) {
    d.app.view3d.view.state.selection = wires
        .iter()
        .map(|w| ItemId::new(ItemKind::Wire, *w))
        .collect::<BTreeSet<_>>();
    d.frame();
    d.frame();
    settle(d);
}

fn cabinet(d: &Drive) -> StructureId {
    d.app
        .backend
        .document()
        .unwrap()
        .project()
        .all::<novo_core::Structure>()
        .find(|s| s.plate.is_some())
        .expect("the cabinet")
        .id
}

#[test]
fn nothing_is_lit_until_a_wire_is_picked() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    assert!(
        lines_of(&d).is_empty(),
        "no entry point and no pick: no overlay"
    );
}

#[test]
fn a_picked_wire_lights_its_path_and_outlines_the_duct_it_lies_in() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let ids = drawn(&d);
    assert!(!ids.is_empty(), "the fixture draws wires");
    pick(&mut d, &ids[..1]);
    let facts = d.app.view3d.wire_facts.as_ref().expect("facts").2.clone();
    let f = &facts[&ids[0]];
    let basis = f
        .basis
        .as_ref()
        .expect("the length basis is read for a picked wire");
    assert!(basis.length_mm > 0.0 && !basis.text.is_empty());
    let all = lines_of(&d);
    assert!(
        all.iter()
            .any(|l| l.width_px == 4.0 && !l.positions.is_empty()),
        "the path"
    );
    if !f.duct_use.is_empty() {
        assert!(
            all.iter()
                .any(|l| l.width_px == 2.0 && l.starts.len() == 6 * f.duct_use.len()),
            "six polylines (two rings, four posts) per duct outline"
        );
    }
    // Letting go of the wire clears the overlay.
    pick(&mut d, &[]);
    assert!(lines_of(&d).is_empty());
}

#[test]
fn wires_with_an_end_off_the_plate_are_listed_as_not_drawn_with_their_basis() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.3d_wire_runs", None);
    d.frame();
    d.frame();
    settle(&mut d);
    let (_, _, undrawn) = d.app.view3d.undrawn.as_ref().expect("read for the list");
    assert!(
        !undrawn.is_empty(),
        "the motor and the push buttons are not on the plate"
    );
    for u in undrawn {
        assert!(
            ["estimate", "cable length"].contains(&u.text.as_str()),
            "{u:?}"
        );
        assert!(u.length_mm > 0.0);
    }
    let field = undrawn
        .iter()
        .find(|u| u.text == "cable length")
        .expect("the motor feed lies in a cable with a stated length");
    assert_eq!(field.length_mm, 12_000.0);
    assert!(field.why.contains("whole cable as bought"));
    assert!(
        undrawn
            .iter()
            .any(|u| u.text == "estimate" && u.why.starts_with("Not routed")),
        "wires between devices that are not placed stay estimates"
    );
    let drawn = drawn(&d);
    assert!(undrawn.iter().all(|u| !drawn.contains(&u.id)));
}

#[test]
fn the_entry_point_is_marked_on_the_plate_and_goes_when_it_is_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let cab = cabinet(&d);
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Entry", |tx| {
        novo_routing::entry::set_entry_point(tx, cab, Some(novo_routing::Point::new(400.0, 560.0)))
    })
    .unwrap();
    d.frame();
    d.frame();
    let marker = lines_of(&d);
    assert_eq!(marker.len(), 1, "just the entry marker");
    let m = marker[0];
    assert_eq!(
        m.starts.len(),
        4 + 1 + 5 + 1,
        "diamond, two cross bars, the post, the stub to the edge, a dashed arrow (the wall is not stated) and its head"
    );
    // The marker sits at the entry: plate y 560 is 40 mm from the bottom edge.
    let (lo, hi) = m
        .positions
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p.x), hi.max(p.x))
        });
    assert!(
        (lo - 386.0).abs() < 0.01 && (hi - 414.0).abs() < 0.01,
        "{lo} {hi}"
    );
    // Stating the wall draws the arrow solid: one polyline instead of five dashes.
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Wall", |tx| {
        novo_routing::entry::set_wall(tx, cab, Some(novo_routing::entry::Wall::Bottom))
    })
    .unwrap();
    d.frame();
    d.frame();
    assert_eq!(lines_of(&d)[0].starts.len(), 4 + 1 + 1 + 1);
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Entry", |tx| {
        novo_routing::entry::set_entry_point(tx, cab, None)
    })
    .unwrap();
    d.frame();
    d.frame();
    assert!(lines_of(&d).is_empty());
}

#[test]
fn the_cores_of_a_cable_share_the_cable_and_one_click_picks_them_all() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let ids = drawn(&d);
    assert!(ids.len() >= 2);
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Cable", |tx| {
        let cable = tx.add_cable(
            DeviceTag::parse("-W1")?,
            vec![
                CableCore::new("BK"),
                CableCore::new("1"),
                CableCore::new("2"),
            ],
        )?;
        tx.update::<Cable>(cable, |c| c.length_m = Some(8.0))?;
        tx.assign_core(cable, 0, Some(novo_core::WireId::from(ids[0])))?;
        tx.assign_core(cable, 1, Some(novo_core::WireId::from(ids[1])))?;
        Ok(())
    })
    .unwrap();
    d.frame();
    pick(&mut d, &ids[..1]);
    let facts = d.app.view3d.wire_facts.as_ref().expect("facts").2.clone();
    let cable = facts[&ids[0]]
        .basis
        .as_ref()
        .and_then(|b| b.cable.clone())
        .expect("the wire is a core of -W1");
    assert_eq!(cable.tag, "-W1");
    assert!(cable.stated);
    assert_eq!(cable.length_mm, 8000.0);
    assert_eq!((cable.cores_used, cable.cores_total), (2, 3));
    assert_eq!(
        cable.line(),
        "Cable -W1: 2 of 3 cores wired, 8.00 m as bought, stated"
    );
    let mut wires = cable.wires.clone();
    wires.sort();
    let mut want = vec![ids[0], ids[1]];
    want.sort();
    assert_eq!(wires, want);
}

#[test]
fn a_run_that_leaves_the_cabinet_is_drawn_to_the_entry_and_keeps_its_cable_length() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let before = drawn(&d);
    let cab = cabinet(&d);
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Entry", |tx| {
        novo_routing::entry::set_entry_point(tx, cab, Some(novo_routing::Point::new(400.0, 580.0)))
    })
    .unwrap();
    d.frame();
    d.frame();
    let after = drawn(&d);
    let new: Vec<u64> = after
        .iter()
        .copied()
        .filter(|w| !before.contains(w))
        .collect();
    assert!(
        !new.is_empty(),
        "the field wires are now drawn up to the entry"
    );
    pick(&mut d, &new[..1]);
    let facts = d.app.view3d.wire_facts.as_ref().expect("facts").2.clone();
    let f = &facts[&new[0]];
    let entry = f.entry.as_ref().expect("drawn to the entry");
    assert_eq!(entry.at, Some(novo_routing::Point::new(400.0, 580.0)));
    assert!(entry.outside_mm.is_none());
    let basis = f.basis.as_ref().expect("basis");
    let lines = super::view3d_runs::card_lines(f);
    if basis.text == "cable length" {
        // The wire list's figure is the whole cable; the card also gives the
        // part inside the cabinet.
        assert_eq!(basis.length_mm, 12_000.0);
        assert!(
            lines
                .iter()
                .any(|(_, t)| t.starts_with("Inside the cabinet: "))
        );
        assert!(lines.iter().any(|(_, t)| t.starts_with("Cable -W")));
    }
    assert!(
        lines
            .iter()
            .any(|(_, t)| t.starts_with("Drawn to the cabinet entry at x 400, y 580 mm"))
    );
    // The marker and the lit path are both there.
    assert!(
        lines_of(&d).iter().any(|l| l.width_px == 2.5),
        "entry marker"
    );
    assert!(lines_of(&d).iter().any(|l| l.width_px == 4.0), "lit path");
}

#[test]
fn a_duct_that_is_too_small_is_called_out_on_every_run_in_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let rail = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .all::<novo_core::Rail>()
        .find(|r| r.kind == novo_core::RailKind::Duct)
        .map(|r| (r.id, r.length))
        .expect("the fixture's duct");
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Tiny duct", |tx| tx.set_rail_size(rail.0, rail.1, 6.0, 6.0))
        .unwrap();
    d.frame();
    let ids = drawn(&d);
    let in_duct = {
        pick(&mut d, &ids);
        let facts = &d.app.view3d.wire_facts.as_ref().expect("facts").2;
        *ids.iter()
            .find(|w| !facts[w].duct_use.is_empty())
            .expect("a wire lies in the duct")
    };
    pick(&mut d, &[in_duct]);
    let facts = d.app.view3d.wire_facts.as_ref().expect("facts").2.clone();
    let f = &facts[&in_duct];
    assert_eq!(f.duct_use[0].tone, super::cabinet_routes::Tone::Over);
    assert!(
        f.duct_use[0].place.starts_with("Duct at y "),
        "{:?}",
        f.duct_use[0]
    );
    assert!(
        f.problems
            .iter()
            .any(|(t, s)| *t == super::cabinet_routes::Tone::Over
                && s.contains("move wires out of it")),
        "{:?}",
        f.problems
    );
    assert!(
        f.lines().iter().any(|l| l.contains("% full of")),
        "the hover card names it"
    );
}

#[test]
fn while_the_project_keeps_changing_the_basis_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let ids = drawn(&d);
    pick(&mut d, &ids[..1]);
    let cab = cabinet(&d);
    // An edit: the basis of the new revision is not there until it settles.
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Entry", |tx| {
        novo_routing::entry::set_entry_point(tx, cab, Some(novo_routing::Point::new(100.0, 500.0)))
    })
    .unwrap();
    d.frame();
    let facts = &d.app.view3d.wire_facts.as_ref().unwrap().2;
    assert!(
        facts[&ids[0]].basis.is_none(),
        "not read in the frame of the edit"
    );
    settle(&mut d);
    let facts = &d.app.view3d.wire_facts.as_ref().unwrap().2;
    assert!(facts[&ids[0]].basis.is_some(), "read once it has settled");
}

#[test]
fn the_wire_runs_panel_and_the_inspector_never_share_screen_space() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.3d_wire_runs", None);
    d.frame();
    d.frame();
    let ids = drawn(&d);
    pick(&mut d, &ids[..1]);
    d.app.run_command(&ctx, "view.3d_select_devices", None);
    for size in [egui::vec2(1366.0, 768.0), egui::vec2(1920.0, 1080.0)] {
        for _ in 0..3 {
            d.frame_sized(size, Vec::new());
        }
        let area = |name: &str| {
            ctx.memory(|m| m.area_rect(super::panel::id(name)))
                .unwrap_or_else(|| panic!("{name} is open at {size:?}"))
        };
        let (runs, inspector) = (area("view3d_wire_runs"), area("cabinet_inspector"));
        assert!(
            !runs.intersects(inspector),
            "{size:?}: runs {runs:?} meets inspector {inspector:?}"
        );
        assert!(runs.left() > size.x / 2.0, "the list hangs on the right");
    }
}

#[test]
fn the_wire_runs_panel_keeps_clear_of_the_heat_panel_even_before_there_is_a_result() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.view3d.controls.heat_panel = true;
    d.app.run_command(&ctx, "view.3d_wire_runs", None);
    for size in [egui::vec2(1366.0, 768.0), egui::vec2(1920.0, 1080.0)] {
        for _ in 0..4 {
            d.frame_sized(size, Vec::new());
        }
        let area = |name: &str| {
            ctx.memory(|m| m.area_rect(super::panel::id(name)))
                .unwrap_or_else(|| panic!("{name} is open at {size:?}"))
        };
        let (runs, heat) = (area("view3d_wire_runs"), area("view3d_heat_panel"));
        assert!(
            !runs.intersects(heat),
            "{size:?}: runs {runs:?} meets heat {heat:?}"
        );
    }
    // With the Heat panel closed, the stale rectangle it left is not used:
    // the panel starts under the view cube again.
    d.app.view3d.controls.heat_panel = false;
    for _ in 0..4 {
        d.frame_sized(egui::vec2(1366.0, 768.0), Vec::new());
    }
    let runs = ctx
        .memory(|m| m.area_rect(super::panel::id("view3d_wire_runs")))
        .unwrap();
    assert!(
        runs.top() < 200.0,
        "starts under the cube, not low: {runs:?}"
    );
}

#[test]
fn the_card_labels_the_bend_and_heat_checks_as_models() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let ids = drawn(&d);
    pick(&mut d, &ids[..1]);
    let facts = d.app.view3d.wire_facts.as_ref().expect("facts").2.clone();
    let f = &facts[&ids[0]];
    let lines = super::view3d_runs::card_lines(f);
    assert!(
        lines
            .iter()
            .any(|(_, s)| s.starts_with("Bend radius") || s.contains("bend radius is not checked")),
        "{lines:?}"
    );
    assert!(
        lines
            .last()
            .unwrap()
            .1
            .starts_with("Not checked: how stiff this cable is"),
        "{lines:?}"
    );
}

#[test]
fn the_devices_a_wire_ends_at_are_not_close_hot_neighbours() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let id = cabinet(&d);
    let doc = d.app.backend.document_mut().unwrap();
    // Every device loses 40 W and any distance counts as close.
    let devices: Vec<_> = doc
        .project()
        .all::<novo_core::CabinetPlacement>()
        .filter(|p| p.structure == id)
        .map(|p| p.device)
        .collect();
    doc.transact("Hot devices", |tx| {
        for dev in &devices {
            tx.set_property(*dev, "power_loss_w", Some(novo_core::Value::Float(40.0)))?;
        }
        novo_routing::project_rules::set_rule(
            tx,
            novo_routing::project_rules::HOT_GAP_MM,
            Some(500.0),
        )
    })
    .unwrap();
    d.frame();
    let ids = drawn(&d);
    pick(&mut d, &ids);
    let facts = d.app.view3d.wire_facts.as_ref().expect("facts").2.clone();
    for f in facts.values() {
        let c = f.checks.as_ref().expect("checks");
        assert_eq!(c.others_hot, 0, "the only hot devices are its own");
        // The fixture has two devices and every wire joins them.
        assert!(c.neighbours.is_empty(), "{:?}", c.neighbours);
    }
}

// ----- the checks fixture ----------------------------------------------------

/// The committed project the QA scenario opens: the example motor starter with
/// sized wires (35 mm2 round the corner of two ducts, 2.5 mm2 on another run
/// through them), a power loss on three devices (one estimated, one maker
/// figure, one left without) and the gap rule widened so the run passes
/// close to one.
fn checks_fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../qa/fixtures/cable-checks.novo")
}

fn build_checks_fixture(doc: &mut novo_core::Document) {
    super::example::build_example(doc).unwrap();
    let routed = novo_routing::model::route_project(doc.project());
    let mut cornered: Vec<u64> = routed
        .routes
        .iter()
        .filter(|(_, r)| r.ducts.len() >= 2)
        .map(|(id, _)| *id)
        .collect();
    cornered.sort_unstable();
    let devices: Vec<(String, novo_core::DeviceId)> = doc
        .project()
        .devices()
        .map(|d| (d.tag.to_string(), d.id))
        .collect();
    let device = |tag: &str| {
        devices
            .iter()
            .find(|(t, _)| t.ends_with(tag))
            .map(|(_, id)| *id)
            .unwrap_or_else(|| panic!("no device {tag}"))
    };
    let wires: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    doc.transact("Checks fixture", |tx| {
        for w in &wires {
            let small = cornered.get(1).is_some_and(|id| *id == w.raw());
            tx.update::<novo_core::Wire>(*w, |wire| {
                wire.cross_section_mm2 = Some(if small { 2.5 } else { 35.0 });
            })?;
        }
        tx.set_property(
            device("-K1"),
            "power_loss_w",
            Some(novo_core::Value::Float(30.0)),
        )?;
        tx.set_property(
            device("-F3"),
            "power_loss_w",
            Some(novo_core::Value::Float(12.0)),
        )?;
        tx.set_property(
            device("-F3"),
            "power_loss_source",
            Some(novo_core::Value::Text("estimated".into())),
        )?;
        novo_routing::project_rules::set_rule(
            tx,
            novo_routing::project_rules::HOT_GAP_MM,
            Some(80.0),
        )
    })
    .unwrap();
}

/// Rewrites the fixture: `cargo test -p novo-app write_checks_fixture -- --ignored`.
#[test]
#[ignore = "writes qa/fixtures/cable-checks.novo"]
fn write_checks_fixture() {
    use crate::backend::ProjectBackend;
    let mut backend = crate::backend::StorageBackend::default();
    backend.new_project("Cable checks (example)").unwrap();
    build_checks_fixture(backend.document_mut().unwrap());
    let path = checks_fixture_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);
    backend.save(&path).unwrap();
}

#[test]
fn the_fixture_has_a_tight_corner_a_fitting_one_and_a_close_hot_device() {
    use crate::backend::ProjectBackend;
    let mut backend = crate::backend::StorageBackend::default();
    backend.new_project("x").unwrap();
    build_checks_fixture(backend.document_mut().unwrap());
    let project = backend.document().unwrap().project();
    let routed = novo_routing::model::route_project(project);
    let cab = project
        .all::<novo_core::Structure>()
        .find(|s| s.plate.is_some())
        .unwrap()
        .id;
    let on: BTreeSet<u64> = routed.routes.keys().copied().collect();
    let hot = super::view3d::hot_devices_for_tests(project, cab);
    let facts = super::view3d_wires::facts(project, cab, &routed, &on, &hot);
    let tight = facts
        .values()
        .filter(|f| f.checks.as_ref().is_some_and(|c| !c.corners.is_empty()))
        .count();
    let fitting = facts
        .values()
        .filter(|f| {
            f.checks
                .as_ref()
                .is_some_and(|c| c.corners.is_empty() && f.ducts >= 2)
        })
        .count();
    let near = facts
        .values()
        .filter(|f| f.checks.as_ref().is_some_and(|c| !c.neighbours.is_empty()))
        .count();
    eprintln!("tight {tight} fitting {fitting} near {near}");
    assert!(
        tight > 0,
        "a 35 mm2 wire round two 25 mm ducts is too tight"
    );
    assert!(fitting > 0, "a 2.5 mm2 wire takes the same corner");
    assert!(near > 0, "a run passes within 80 mm of a hot device");
}

#[test]
fn picking_a_duct_lights_every_run_in_it_and_lists_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let ids = drawn(&d);
    pick(&mut d, &ids);
    let facts = d.app.view3d.wire_facts.as_ref().expect("facts").2.clone();
    let rail = facts
        .values()
        .find_map(|f| f.duct_use.first().map(|u| u.rail))
        .expect("a wire lies in a duct");
    let inside: Vec<u64> = facts
        .values()
        .filter(|f| f.duct_use.iter().any(|u| u.rail == rail))
        .map(|f| f.id)
        .collect();
    // Let go of the wires, pick the duct alone.
    d.app.view3d.view.state.selection = BTreeSet::from([ItemId::new(ItemKind::Duct, rail)]);
    d.frame();
    d.frame();
    settle(&mut d);
    let runs = super::view3d_runs::duct_runs(&facts, &d.app.view3d.view.state.selection);
    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0].wires.iter().map(|f| f.id).collect::<BTreeSet<_>>(),
        inside.iter().copied().collect()
    );
    assert!(runs[0].fill.is_some());
    let lit = lines_of(&d)
        .into_iter()
        .find(|l| l.width_px == 4.0)
        .expect("the runs in the duct are lit");
    assert_eq!(lit.starts.len(), inside.len(), "one lit path per run in it");
}

#[test]
fn a_duct_without_wires_says_so() {
    let facts = std::collections::BTreeMap::new();
    let sel = BTreeSet::from([ItemId::new(ItemKind::Duct, 99)]);
    let runs = super::view3d_runs::duct_runs(&facts, &sel);
    assert_eq!(runs.len(), 1);
    assert!(runs[0].wires.is_empty() && runs[0].fill.is_none());
}
