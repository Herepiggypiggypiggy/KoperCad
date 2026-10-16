//! Moving, turning and deleting in the 3D view: the same rules and the same
//! undo steps as the Cabinet canvas.

use std::collections::BTreeSet;

use eframe::egui;
use novo_core::{CabinetPlacement, DeviceTag, Value};
use novo_layout::state::CabinetLayout;
use novo_layout::{Duct, Margins, Placement, Plate, Rail, Size};
use novo_view3d::scene::{ItemId, ItemKind, PlateFrame};
use novo_view3d::widget::ItemDrag;

use super::cabinet::Ask;
use super::gate1_tests::Drive;
use super::view3d_move::*;
use super::workspace::Workspace;
use crate::actions::Action;

fn placement(id: u64, x: f64, y: f64) -> Placement {
    Placement {
        id,
        x,
        y,
        rotated: false,
        size: Size {
            width: 45.0,
            height: 90.0,
            guessed: false,
        },
        clearance: None,
        margins: Margins::default(),
        rail: None,
    }
}

fn layout() -> CabinetLayout {
    let mut l = CabinetLayout::new(Plate {
        width: 600.0,
        height: 800.0,
        margin: 10.0,
    });
    l.placements = vec![placement(1, 100.0, 100.0), placement(2, 300.0, 100.0)];
    l.rails = vec![Rail {
        id: 7,
        x: 50.0,
        y: 400.0,
        length: 300.0,
        vertical: false,
    }];
    l.ducts = vec![Duct {
        id: 8,
        x: 50.0,
        y: 600.0,
        length: 400.0,
        width: 40.0,
        vertical: false,
    }];
    l
}

fn dev(id: u64) -> ItemId {
    ItemId::new(ItemKind::Device, id)
}

fn drag(item: ItemId, delta: (f64, f64), done: bool) -> Option<ItemDrag> {
    Some(ItemDrag {
        id: item,
        delta,
        done,
    })
}

#[test]
fn a_device_follows_the_pointer_on_the_grid() {
    let l = layout();
    let land = landing(&l, dev(1), (52.0, 31.0)).unwrap();
    assert_eq!(land.at, l.snapped(152.0, 131.0));
    assert!(!land.clash);
    assert_eq!((land.rect.w, land.rect.h), (45.0, 90.0));
}

#[test]
fn the_landing_is_red_over_a_neighbour_and_off_the_plate() {
    let l = layout();
    let over = landing(&l, dev(1), (200.0, 0.0)).unwrap();
    assert!(over.clash, "on top of device 2");
    let off = landing(&l, dev(1), (-200.0, 0.0)).unwrap();
    assert!(off.clash, "left of the plate");
}

#[test]
fn only_things_on_the_plate_can_be_moved() {
    let l = layout();
    assert!(landing(&l, dev(99), (1.0, 1.0)).is_none());
    assert!(landing(&l, ItemId::new(ItemKind::Door, 0), (1.0, 1.0)).is_none());
}

#[test]
fn releasing_a_moved_device_asks_for_one_drop_and_a_still_one_for_nothing() {
    let l = layout();
    let mut m = MoveState::default();
    let (land, edit) = m.step(&l, drag(dev(1), (40.0, 0.0), false));
    assert!(land.is_some() && edit.is_none() && m.dragging());
    // The layout does not change while dragging: the origin is the press's.
    let (_, edit) = m.step(&l, drag(dev(1), (60.0, 20.0), true));
    let at = l.snapped(160.0, 120.0);
    assert_eq!(edit, Some(Edit::Device(DeviceEdit::Drop(1, at.0, at.1))));
    assert!(!m.dragging());
    let (_, edit) = m.step(&l, drag(dev(1), (0.5, 0.5), true));
    assert_eq!(edit, None, "snapped back to where it was");
}

#[test]
fn rails_and_ducts_move_by_the_rail_edit() {
    let l = layout();
    let mut m = MoveState::default();
    let (_, edit) = m.step(&l, drag(ItemId::new(ItemKind::Rail, 7), (0.0, 50.0), true));
    let at = l.snapped(50.0, 450.0);
    assert_eq!(edit, Some(Edit::Rail(RailEdit::MoveTo(7, at.0, at.1))));
    let (_, edit) = m.step(&l, drag(ItemId::new(ItemKind::Duct, 8), (0.0, -50.0), true));
    let at = l.snapped(50.0, 550.0);
    assert_eq!(edit, Some(Edit::Rail(RailEdit::MoveTo(8, at.0, at.1))));
}

#[test]
fn no_drag_ends_the_one_in_progress() {
    let l = layout();
    let mut m = MoveState::default();
    m.step(&l, drag(dev(1), (10.0, 0.0), false));
    assert!(m.dragging());
    assert_eq!(m.step(&l, None), (None, None));
    assert!(!m.dragging());
}

#[test]
fn keys_act_on_devices_first_then_on_rails_and_ducts() {
    let l = layout();
    let keys = ["edit.rotate".to_owned(), "edit.delete".to_owned()];
    let both: BTreeSet<_> = [dev(1), dev(2), ItemId::new(ItemKind::Rail, 7)].into();
    assert_eq!(
        key_edits(&keys, &both, &l),
        vec![
            Edit::Device(DeviceEdit::Rotate(vec![1, 2])),
            Edit::Device(DeviceEdit::Remove(vec![1, 2])),
        ]
    );
    let rails: BTreeSet<_> = [
        ItemId::new(ItemKind::Rail, 7),
        ItemId::new(ItemKind::Duct, 8),
    ]
    .into();
    assert_eq!(
        key_edits(&keys, &rails, &l),
        vec![
            Edit::Rail(RailEdit::Turn(7)),
            Edit::Rail(RailEdit::Turn(8)),
            Edit::Rail(RailEdit::Delete(7)),
            Edit::Rail(RailEdit::Delete(8)),
        ]
    );
    assert!(key_edits(&keys, &BTreeSet::new(), &l).is_empty());
    let gone: BTreeSet<_> = [dev(99)].into();
    assert!(key_edits(&keys, &gone, &l).is_empty(), "not on the plate");
    assert!(key_edits(&["edit.mirror".to_owned()], &both, &l).is_empty());
}

#[test]
fn the_landing_box_is_valid_geometry_and_red_on_a_clash() {
    let l = layout();
    let frame = PlateFrame {
        width: 600.0,
        height: 800.0,
    };
    let ok = ghost(&landing(&l, dev(1), (10.0, 0.0)).unwrap(), &frame, 60.0);
    let bad = ghost(&landing(&l, dev(1), (200.0, 0.0)).unwrap(), &frame, 60.0);
    for set in [&ok, &bad] {
        assert_eq!(set.len(), 2);
        if let novo_view3d::Overlay::Mesh(m) = &set[0] {
            assert!(m.is_valid() && m.translucent);
        } else {
            panic!("the slab comes first");
        }
    }
    let colour = |set: &[novo_view3d::Overlay]| match &set[0] {
        novo_view3d::Overlay::Mesh(m) => m.colors[0],
        _ => unreachable!(),
    };
    assert!(colour(&bad)[0] > colour(&bad)[2], "red");
    assert!(colour(&ok)[2] > colour(&ok)[0], "blue");
}

// --- in the app ---------------------------------------------------------

fn project(d: &Drive) -> &novo_core::Project {
    d.app.backend.document().unwrap().project()
}

fn cabinet_with_two_devices(d: &mut Drive) -> (novo_core::StructureId, u64, u64) {
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
    for (id, x) in [(ids[0], 100.0), (ids[1], 300.0)] {
        let l = novo_layout::model::load(project(d), cab);
        d.app
            .cabinet_ask(Ask::Drop(id, x, 120.0), Some(cab), l, &[]);
    }
    d.frame();
    (cab, ids[0], ids[1])
}

fn placed(d: &Drive, cab: novo_core::StructureId, id: u64) -> (f64, f64, bool) {
    let l = novo_layout::model::load(project(d), cab).unwrap();
    let p = l.placements.iter().find(|p| p.id == id).unwrap();
    (p.x, p.y, p.rotated)
}

fn press(d: &mut Drive, key: egui::Key) {
    d.frame_with(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
}

fn open_3d_with_selection(d: &mut Drive, a: u64) {
    let ctx = d.ctx.clone();
    d.app.cabinet_ask(Ask::Select(a), None, None, &[]);
    d.app.run_command(&ctx, "view.3d", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
    assert!(d.app.view3d.view.state.selection.contains(&dev(a)));
}

#[test]
fn a_3d_drop_is_one_undo_step_and_the_plate_shows_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, _) = cabinet_with_two_devices(&mut d);
    let before = placed(&d, cab, a);
    let l = novo_layout::model::load(project(&d), cab).unwrap();
    let mut m = MoveState::default();
    let (_, edit) = m.step(&l, drag(dev(a), (50.0, 100.0), true));
    let (ask, rail) = edit.unwrap().ask();
    assert!(rail.is_none());
    d.app.cabinet_ask(ask.unwrap(), Some(cab), Some(l), &[]);
    let after = placed(&d, cab, a);
    assert_ne!(after, before);
    assert_eq!((after.0, after.1), (before.0 + 50.0, before.1 + 100.0));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert_eq!(placed(&d, cab, a), before);
}

#[test]
fn r_in_3d_turns_the_selected_device_in_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, b) = cabinet_with_two_devices(&mut d);
    open_3d_with_selection(&mut d, a);
    press(&mut d, egui::Key::R);
    d.frame();
    assert!(placed(&d, cab, a).2, "turned");
    assert!(!placed(&d, cab, b).2, "the other stays");
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert!(!placed(&d, cab, a).2);
}

#[test]
fn delete_in_3d_takes_the_device_off_the_plate_and_keeps_it_in_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (_, a, _) = cabinet_with_two_devices(&mut d);
    let devices = project(&d).devices().count();
    open_3d_with_selection(&mut d, a);
    press(&mut d, egui::Key::Delete);
    d.frame();
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 1);
    assert_eq!(project(&d).devices().count(), devices);
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 2);
}

#[test]
fn rotate_and_delete_keys_do_not_leak_to_the_plate_afterwards() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, _) = cabinet_with_two_devices(&mut d);
    open_3d_with_selection(&mut d, a);
    press(&mut d, egui::Key::R);
    d.frame();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    d.frame();
    assert!(placed(&d, cab, a).2, "still turned once, not twice");
}
