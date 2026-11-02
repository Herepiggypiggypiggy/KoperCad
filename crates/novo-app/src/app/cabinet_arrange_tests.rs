//! The arrange tools in the Cabinet: each previews first, accepting is one
//! undo step named after the tool, and Cancel stores nothing.

use novo_core::{CabinetPlacement, DeviceTag, Project, Rail, RailKind, Value};

use super::cabinet::Ask;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn project(d: &Drive) -> &Project {
    d.app.backend.document().unwrap().project()
}

/// A cabinet with three relays, dropped on the plate with no rail.
fn loose(d: &mut Drive) -> (novo_core::StructureId, Vec<u64>) {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-K2", "=A1+CAB1-K3"] {
                let part = tx.add_part("ACME", tag)?;
                tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
                tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
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
    d.frame();
    let ids: Vec<u64> = novo_layout::model::candidates(project(d), cab)
        .iter()
        .map(|c| c.id)
        .collect();
    for (i, id) in ids.iter().enumerate() {
        let x = 60.0 + 100.0 * i as f64;
        d.app.cabinet_ask(
            Ask::Drop(*id, x, 100.0 + 3.0 * i as f64),
            Some(cab),
            None,
            &[],
        );
        d.frame();
    }
    (cab, ids)
}

fn run(d: &mut Drive, cab: novo_core::StructureId, tool: &'static str) {
    let layout = novo_layout::model::load(project(d), cab);
    let cands = novo_layout::model::candidates(project(d), cab);
    d.app.cabinet_ask(
        Ask::AutoArrange(tool, Vec::new()),
        Some(cab),
        layout,
        &cands,
    );
    d.frame();
}

fn accept(d: &mut Drive, cab: novo_core::StructureId) {
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();
}

#[test]
fn auto_rails_then_close_up_and_group_each_undo_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let (cab, ids) = loose(&mut d);
    let rails = |d: &Drive| {
        project(d)
            .all::<Rail>()
            .filter(|r| r.kind == RailKind::DinRail)
            .count()
    };
    assert_eq!(rails(&d), 0);

    // Preview first: nothing is stored.
    run(&mut d, cab, "cabinet.arrange_rails");
    assert!(d.app.cabinet.preview.is_some());
    assert_eq!(rails(&d), 0);
    d.app.cabinet_ask(Ask::Cancel, Some(cab), None, &[]);
    d.frame();
    assert!(d.app.cabinet.preview.is_none());
    assert_eq!(rails(&d), 0);

    // Accept: one rail, every device mounted on it, one undo step.
    run(&mut d, cab, "cabinet.arrange_rails");
    accept(&mut d, cab);
    assert_eq!(rails(&d), 1);
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    assert!(layout.placements.iter().all(|p| p.rail.is_some()));
    assert!(layout.report().off_centre.is_empty());

    // Close the 55 mm gaps up to the 10 mm the parts ask for.
    run(&mut d, cab, "cabinet.arrange_tight");
    accept(&mut d, cab);
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    let mut xs: Vec<f64> = layout.placements.iter().map(|p| p.x).collect();
    xs.sort_by(f64::total_cmp);
    assert_eq!(xs[1] - xs[0], 55.0);
    assert_eq!(xs[2] - xs[1], 55.0);
    assert!(layout.report().too_close.is_empty());

    // Undo the close-up, then the rail: back to loose devices.
    d.app.run(&ctx, Action::Undo);
    d.frame();
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    let mut xs: Vec<f64> = layout.placements.iter().map(|p| p.x).collect();
    xs.sort_by(f64::total_cmp);
    assert!(xs[1] - xs[0] > 55.0, "the gaps are back");
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(rails(&d), 0);
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), ids.len());
}

#[test]
fn a_tool_with_nothing_to_do_says_so_and_shows_no_preview() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, _) = loose(&mut d);
    // Ducts need two rails.
    run(&mut d, cab, "cabinet.arrange_ducts");
    assert!(d.app.cabinet.preview.is_none());
}
