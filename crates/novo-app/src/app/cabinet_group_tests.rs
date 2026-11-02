//! Batch drag, align and select all on the Cabinet plate.

use super::cabinet::Ask;
use super::cabinet_tests::add_cabinet;
use super::gate1_tests::Drive;
use super::view3d_align::Op;
use crate::actions::Action;

fn setup(d: &mut Drive) -> (novo_core::StructureId, Vec<u64>) {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let project = |d: &Drive| d.app.backend.document().unwrap().project().clone();
    let _ = project;
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
    let ids: Vec<u64> =
        novo_layout::model::candidates(d.app.backend.document().unwrap().project(), cab)
            .iter()
            .map(|c| c.id)
            .collect();
    (cab, ids)
}

fn layout(d: &Drive, cab: novo_core::StructureId) -> novo_layout::state::CabinetLayout {
    novo_layout::model::load(d.app.backend.document().unwrap().project(), cab).unwrap()
}

fn at(d: &Drive, cab: novo_core::StructureId, id: u64) -> (f64, f64) {
    let l = layout(d, cab);
    let p = l.placements.iter().find(|p| p.id == id).unwrap();
    (p.x, p.y)
}

#[test]
fn a_group_move_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, ids) = setup(&mut d);
    let ctx = d.ctx.clone();
    for (n, id) in ids.iter().enumerate() {
        let l = Some(layout(&d, cab));
        d.app.cabinet_ask(
            Ask::Drop(*id, 100.0 + 100.0 * n as f64, 100.0),
            Some(cab),
            l,
            &[],
        );
    }
    let l = Some(layout(&d, cab));
    d.app.cabinet_ask(
        Ask::MoveGroup(vec![(ids[0], 100.0, 300.0), (ids[1], 200.0, 300.0)]),
        Some(cab),
        l,
        &[],
    );
    assert_eq!(at(&d, cab, ids[0]), (100.0, 300.0));
    assert_eq!(at(&d, cab, ids[1]), (200.0, 300.0));
    assert_eq!(at(&d, cab, ids[2]), (300.0, 100.0), "the rest stays");
    d.app.run(&ctx, Action::Undo);
    assert_eq!(at(&d, cab, ids[0]), (100.0, 100.0));
    assert_eq!(at(&d, cab, ids[1]), (200.0, 100.0));
}

#[test]
fn align_lines_the_selection_up_and_select_all_picks_every_device() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, ids) = setup(&mut d);
    for (n, id) in ids.iter().enumerate() {
        let l = Some(layout(&d, cab));
        d.app.cabinet_ask(
            Ask::Drop(*id, 100.0 + 100.0 * n as f64, 100.0 + 20.0 * n as f64),
            Some(cab),
            l,
            &[],
        );
    }
    d.app.cabinet.keys.push("edit.select_all".to_owned());
    d.frame();
    assert_eq!(d.app.cabinet_selected().len(), 3, "Ctrl+A selects all");
    let l = Some(layout(&d, cab));
    d.app.cabinet_ask(Ask::Arrange(Op::Top), Some(cab), l, &[]);
    for id in &ids {
        assert_eq!(at(&d, cab, *id).1, 100.0);
    }
}
