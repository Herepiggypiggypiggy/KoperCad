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
    let (land, edit) = m.step(&l, &BTreeSet::new(), drag(dev(1), (40.0, 0.0), false));
    assert!(!land.is_empty() && edit.is_none() && m.dragging());
    // The layout does not change while dragging: the origin is the press's.
    let (_, edit) = m.step(&l, &BTreeSet::new(), drag(dev(1), (60.0, 20.0), true));
    let at = l.snapped(160.0, 120.0);
    assert_eq!(edit, Some(Edit::Device(DeviceEdit::Drop(1, at.0, at.1))));
    assert!(!m.dragging());
    let (_, edit) = m.step(&l, &BTreeSet::new(), drag(dev(1), (0.5, 0.5), true));
    assert_eq!(edit, None, "snapped back to where it was");
}

#[test]
fn rails_and_ducts_move_by_the_rail_edit() {
    let l = layout();
    let mut m = MoveState::default();
    let (_, edit) = m.step(
        &l,
        &BTreeSet::new(),
        drag(ItemId::new(ItemKind::Rail, 7), (0.0, 50.0), true),
    );
    let at = l.snapped(50.0, 450.0);
    assert_eq!(edit, Some(Edit::Rail(RailEdit::MoveTo(7, at.0, at.1))));
    let (_, edit) = m.step(
        &l,
        &BTreeSet::new(),
        drag(ItemId::new(ItemKind::Duct, 8), (0.0, -50.0), true),
    );
    let at = l.snapped(50.0, 550.0);
    assert_eq!(edit, Some(Edit::Rail(RailEdit::MoveTo(8, at.0, at.1))));
}

#[test]
fn no_drag_ends_the_one_in_progress() {
    let l = layout();
    let mut m = MoveState::default();
    m.step(&l, &BTreeSet::new(), drag(dev(1), (10.0, 0.0), false));
    assert!(m.dragging());
    assert_eq!(m.step(&l, &BTreeSet::new(), None), (Vec::new(), None));
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

/// Both tests that touch the process-wide frame recorder (the Stats panel
/// reads it) take this, so neither sees the other's adapter or timings.
pub(super) static STATS_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    let (_, edit) = m.step(&l, &BTreeSet::new(), drag(dev(a), (50.0, 100.0), true));
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

#[test]
fn pressing_on_a_selected_item_moves_the_whole_selection_by_the_same_step() {
    let l = layout();
    let sel: BTreeSet<_> = [dev(1), dev(2), ItemId::new(ItemKind::Rail, 7)].into();
    let mut m = MoveState::default();
    let (lands, _) = m.step(&l, &sel, drag(dev(1), (0.0, 50.0), false));
    assert_eq!(lands.len(), 3);
    let at = |item: ItemId| lands.iter().find(|x| x.item == item).unwrap().at;
    assert_eq!(at(dev(1)), (100.0, 150.0));
    assert!(lands.iter().all(|x| !x.clash), "{lands:?}");
    assert_eq!(at(dev(2)), (300.0, 150.0));
    assert_eq!(at(ItemId::new(ItemKind::Rail, 7)), (50.0, 450.0));
    let (_, edit) = m.step(&l, &sel, drag(dev(1), (0.0, 50.0), true));
    match edit {
        Some(Edit::Group { devices, rails }) => {
            assert_eq!(devices.len(), 2);
            assert_eq!(rails, vec![(7, 50.0, 450.0)]);
        }
        other => panic!("{other:?}"),
    }
    // An unselected item moves alone.
    let (lands, _) = m.step(&l, &sel, drag(dev(2), (0.0, 0.0), false));
    assert_eq!(lands.len(), 3, "device 2 is selected too");
    m.step(&l, &sel, None);
    let (lands, _) = m.step(
        &l,
        &BTreeSet::from([dev(1)]),
        drag(dev(2), (10.0, 0.0), false),
    );
    assert_eq!(lands.len(), 1);
}

#[test]
fn a_group_clashes_when_any_member_does() {
    let l = layout();
    let sel: BTreeSet<_> = [dev(1), dev(2)].into();
    let mut m = MoveState::default();
    let (lands, _) = m.step(&l, &sel, drag(dev(1), (-95.0, 0.0), false));
    assert!(lands[0].clash || lands.iter().any(|x| x.clash), "{lands:?}");
}

#[test]
fn a_device_near_a_rail_lines_up_on_it() {
    let l = layout();
    // The rail at y = 400 runs from x = 50 for 300 mm; device 1 is at y = 100.
    let land = landing(&l, dev(1), (0.0, 296.0)).unwrap();
    assert_eq!(land.at.1, 400.0, "top snapped onto the rail line");
    assert_eq!(land.rail, Some(7));
    assert!(!land.clash);
    // Beyond the reach: no snap.
    let far = landing(&l, dev(1), (0.0, 250.0)).unwrap();
    assert_eq!(far.rail, None);
    // Beside the rail's run: no snap.
    let beside = landing(&l, dev(1), (400.0, 296.0)).unwrap();
    assert_eq!(beside.rail, None);
}

#[test]
fn the_lined_up_landing_is_green() {
    let l = layout();
    let frame = PlateFrame {
        width: 600.0,
        height: 800.0,
    };
    let land = landing(&l, dev(1), (0.0, 296.0)).unwrap();
    let set = ghost(&land, &frame, 60.0);
    let novo_view3d::Overlay::Mesh(m) = &set[0] else {
        panic!()
    };
    assert!(m.colors[0][1] > m.colors[0][0] && m.colors[0][1] > m.colors[0][2]);
}

#[test]
fn a_group_move_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, b) = cabinet_with_two_devices(&mut d);
    let (pa, pb) = (placed(&d, cab, a), placed(&d, cab, b));
    d.app.view3d_move_group(
        cab,
        vec![(a, pa.0, pa.1 + 40.0), (b, pb.0, pb.1 + 40.0)],
        vec![],
    );
    assert_eq!(placed(&d, cab, a).1, pa.1 + 40.0);
    assert_eq!(placed(&d, cab, b).1, pb.1 + 40.0);
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert_eq!((placed(&d, cab, a), placed(&d, cab, b)), (pa, pb));
}

#[test]
fn a_group_drop_in_3d_ticks_the_lesson_step_a_single_drop_ticks() {
    use super::cabinet_tests::{lesson_at, step_done};

    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    lesson_at(&mut d, dir.path(), "cabinet-layout", "drag-first");
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    let cands = novo_layout::model::candidates(project(&d), cab);
    let (a, b) = (cands[0].id, cands[1].id);
    assert!(!step_done(&mut d, "drag-first"), "nothing dropped yet");
    // Moving two devices together in 3D is a drop, like a single one.
    d.app
        .view3d_move_group(cab, vec![(a, 100.0, 100.0), (b, 300.0, 100.0)], vec![]);
    assert!(step_done(&mut d, "drag-first"));
    assert_eq!(placed(&d, cab, a).1, 100.0);
}

#[test]
fn a_group_without_devices_raises_no_drop_signal() {
    use super::cabinet_tests::{lesson_at, step_done};

    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    lesson_at(&mut d, dir.path(), "cabinet-layout", "drag-first");
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.view3d_move_group(cab, vec![], vec![]);
    assert!(!step_done(&mut d, "drag-first"));
}

#[test]
fn a_typed_rail_length_reaches_the_3d_scene() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, _b) = cabinet_with_two_devices(&mut d);
    d.app.cabinet_rail_ask(
        Some(cab),
        super::cabinet_rails::RailAsk::Add {
            kind: novo_core::RailKind::Duct,
            vertical: false,
        },
    );
    open_3d_with_selection(&mut d, a);
    let before = d.app.view3d.built.as_ref().map(|b| b.2.revision).unwrap();
    let duct = project(&d)
        .all::<novo_core::Rail>()
        .find(|r| r.kind == novo_core::RailKind::Duct)
        .map(|r| (r.id.raw(), r.x, r.y, r.width, r.height))
        .unwrap();
    d.app.cabinet_rail_ask(
        Some(cab),
        super::cabinet_rails::RailAsk::Set(super::cabinet_rails::RailEdit {
            id: duct.0,
            x: duct.1,
            y: duct.2,
            length: 123.0,
            width: duct.3,
            height: duct.4,
        }),
    );
    d.frame();
    d.frame();
    let after = d.app.view3d.built.as_ref().map(|b| b.2.revision).unwrap();
    assert!(after > before, "{before} -> {after}");
}

/// In the 3D view with a device selected, the toolbar, the Inspector, the Heat
/// panel and the statistics start clear of each other and of the view cube.
#[test]
fn the_3d_panels_start_clear_of_each_other_and_of_the_view_cube() {
    for (w, h) in [(1366.0, 768.0), (900.0, 600.0)] {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let (_, a, _) = cabinet_with_two_devices(&mut d);
        open_3d_with_selection(&mut d, a);
        let ctx = d.ctx.clone();
        d.app.run_command(&ctx, "view.3d_stats", None);
        // The Heat panel is off by default; the worst case has it open.
        d.app.view3d.controls.heat_panel = true;
        for _ in 0..12 {
            d.frame_sized(egui::vec2(w, h), Vec::new());
        }
        let rect = |id: egui::Id| ctx.memory(|m| m.area_rect(id));
        let toolbar = rect(super::panel::id("view3d_toolbar")).expect("toolbar");
        let inspector = rect(super::panel::id("cabinet_inspector")).expect("inspector");
        let heat = rect(super::panel::id("view3d_heat_panel")).expect("heat panel");
        let stats = rect(super::panel::id("view3d_stats")).expect("stats");
        // The heat key lives in the Heat strip, not in a floating area that could cover these.
        assert!(rect(egui::Id::new("heat_legend")).is_none(), "{w} x {h}");
        // The cube sits above the Heat panel, at the view's top right.
        let cube = egui::Rect::from_min_size(
            egui::pos2(heat.right() + 12.0 - 12.0 - 88.0, heat.top() - 8.0 - 88.0),
            egui::Vec2::splat(88.0),
        );
        let panels = [
            ("toolbar", toolbar),
            ("inspector", inspector),
            ("heat", heat),
            ("stats", stats),
            ("cube", cube),
        ];
        for (i, (na, ra)) in panels.iter().enumerate() {
            for (nb, rb) in &panels[i + 1..] {
                assert!(
                    !ra.intersects(*rb),
                    "{w} x {h}: {na} {ra:?} over {nb} {rb:?}"
                );
            }
        }
    }
}

// --- rails and ducts carry devices and warn --------------------------------

fn mounted() -> CabinetLayout {
    let mut l = layout();
    l.placements[0].y = 400.0;
    l.placements[0].rail = Some(7);
    l
}

fn rail(id: u64) -> ItemId {
    ItemId::new(ItemKind::Rail, id)
}

#[test]
fn devices_on_a_dragged_rail_ride_along_in_the_preview() {
    let l = mounted();
    let mut m = MoveState::default();
    let (land, _) = m.step(&l, &BTreeSet::new(), drag(rail(7), (0.0, 100.0), false));
    let rider = land.iter().find(|x| x.item == dev(1)).unwrap();
    assert!(rider.follower);
    assert_eq!(rider.at, (100.0, 500.0));
    let on_rail = land.iter().find(|x| x.item == rail(7)).unwrap();
    assert!(!on_rail.follower);
    assert!(!on_rail.clash, "{on_rail:?}");
}

#[test]
fn releasing_a_rail_asks_for_the_rail_move_only() {
    let l = mounted();
    let mut m = MoveState::default();
    let (_, edit) = m.step(&l, &BTreeSet::new(), drag(rail(7), (0.0, 100.0), true));
    assert!(matches!(edit, Some(Edit::Rail(RailEdit::MoveTo(7, _, _)))));
}

#[test]
fn a_duct_over_a_device_clashes_and_says_which_one() {
    let l = layout();
    let duct = ItemId::new(ItemKind::Duct, 8);
    let mut m = MoveState::default();
    let (land, _) = m.step(&l, &BTreeSet::new(), drag(duct, (0.0, -480.0), false));
    let d = land.iter().find(|x| x.item == duct).unwrap();
    assert!(d.clash);
    assert!(d.blockers.contains(&1), "{d:?}");
    let tag = |id: u64| format!("-K{id}");
    let text = warning(&l, &land, &tag).unwrap();
    assert!(text.starts_with("This duct would run into -K1"), "{text}");
}

#[test]
fn a_rail_that_leaves_the_plate_says_so_and_a_clean_move_says_nothing() {
    let l = mounted();
    let mut m = MoveState::default();
    let tag = |id: u64| id.to_string();
    let (land, _) = m.step(&l, &BTreeSet::new(), drag(rail(7), (0.0, 600.0), false));
    let text = warning(&l, &land, &tag).unwrap();
    assert!(text.contains("leave the plate"), "{text}");
    let (land, _) = m.step(&l, &BTreeSet::new(), drag(rail(7), (0.0, 40.0), false));
    assert_eq!(warning(&l, &land, &tag), None);
}

#[test]
fn a_rider_that_would_collide_makes_the_rail_clash() {
    let mut l = mounted();
    // Device 2 sits where the rider lands after a 200 mm move down.
    l.placements[1].x = 100.0;
    l.placements[1].y = 600.0;
    let mut m = MoveState::default();
    let (land, _) = m.step(&l, &BTreeSet::new(), drag(rail(7), (0.0, 200.0), false));
    assert!(land.iter().find(|x| x.item == dev(1)).unwrap().clash);
    assert!(land.iter().find(|x| x.item == rail(7)).unwrap().clash);
}

#[test]
fn a_group_of_a_rail_and_its_device_lands_the_device_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, _b) = cabinet_with_two_devices(&mut d);
    let ctx = d.ctx.clone();
    d.app.cabinet_rail_ask(
        Some(cab),
        super::cabinet_rails::RailAsk::Add {
            kind: novo_core::RailKind::DinRail,
            vertical: false,
        },
    );
    let r = novo_layout::model::load(project(&d), cab)
        .unwrap()
        .rails
        .iter()
        .map(|r| r.id)
        .max()
        .unwrap();
    let start = novo_layout::model::load(project(&d), cab)
        .unwrap()
        .rails
        .iter()
        .find(|x| x.id == r)
        .map(|x| (x.x, x.y))
        .unwrap();
    let pa = placed(&d, cab, a);
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Mount", |tx| {
            tx.execute(novo_core::Command::PlaceDevice {
                device: novo_core::DeviceId::from(a),
                structure: cab,
                x: pa.0,
                y: pa.1,
                rotated: false,
                rail: Some((novo_core::RailId::from(r), 0.0)),
            })?;
            Ok(())
        })
        .unwrap();
    let on = placed(&d, cab, a);
    d.app.view3d_move_group(
        cab,
        vec![(a, on.0 + 40.0, on.1 + 40.0)],
        vec![(r, start.0 + 40.0, start.1 + 40.0)],
    );
    let after = placed(&d, cab, a);
    assert_eq!((after.0, after.1), (on.0 + 40.0, on.1 + 40.0));
    d.app.run(&ctx, Action::Undo);
    assert_eq!(placed(&d, cab, a), on);
}

/// A selected rail's value card starts clear of the toolbar, the statistics,
/// the Heat panel and the view cube.
#[test]
fn the_rail_card_starts_clear_of_the_other_3d_panels() {
    let _guard = STATS_GUARD.lock().unwrap_or_else(|e| e.into_inner());
    // A fresh recorder: the Stats panel's height depends on it.
    novo_view3d::widget::register_adapter(
        "Test GPU",
        "Vulkan",
        novo_view3d::render::Tier::Discrete,
    );
    for (w, h) in [(1366.0, 768.0), (900.0, 600.0)] {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let (cab, a, _) = cabinet_with_two_devices(&mut d);
        d.app.cabinet_rail_ask(
            Some(cab),
            super::cabinet_rails::RailAsk::Add {
                kind: novo_core::RailKind::Duct,
                vertical: false,
            },
        );
        open_3d_with_selection(&mut d, a);
        let duct = project(&d)
            .all::<novo_core::Rail>()
            .find(|r| r.kind == novo_core::RailKind::Duct)
            .map(|r| r.id.raw())
            .unwrap();
        d.app.view3d.view.state.selection = BTreeSet::from([ItemId::new(ItemKind::Duct, duct)]);
        let ctx = d.ctx.clone();
        d.app.run_command(&ctx, "view.3d_stats", None);
        d.app.view3d.controls.heat_panel = true;
        for _ in 0..12 {
            d.frame_sized(egui::vec2(w, h), Vec::new());
        }
        let rect = |id: egui::Id| ctx.memory(|m| m.area_rect(id));
        let card = rect(super::panel::id("view3d_rail")).expect("rail card");
        let toolbar = rect(super::panel::id("view3d_toolbar")).expect("toolbar");
        let heat = rect(super::panel::id("view3d_heat_panel")).expect("heat panel");
        let stats = rect(super::panel::id("view3d_stats")).expect("stats");
        let cube = egui::Rect::from_min_size(
            egui::pos2(heat.right() + 12.0 - 12.0 - 88.0, heat.top() - 8.0 - 88.0),
            egui::Vec2::splat(88.0),
        );
        for (name, other) in [
            ("toolbar", toolbar),
            ("heat", heat),
            ("stats", stats),
            ("cube", cube),
        ] {
            assert!(
                !card.intersects(other),
                "{w} x {h}: rail card {card:?} over {name} {other:?}"
            );
        }
    }
}

#[test]
fn each_arrow_press_in_3d_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, _) = cabinet_with_two_devices(&mut d);
    open_3d_with_selection(&mut d, a);
    // The keys nudge while the pointer is over the view.
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(640.0, 400.0))]);
    let start = placed(&d, cab, a);
    press(&mut d, egui::Key::ArrowRight);
    d.frame();
    let one = placed(&d, cab, a);
    assert!(one.0 > start.0, "moved right");
    press(&mut d, egui::Key::ArrowRight);
    d.frame();
    let two = placed(&d, cab, a);
    assert!(two.0 > one.0, "moved again");
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert_eq!(placed(&d, cab, a), one, "one undo takes back one press");
    d.app.run(&ctx, Action::Undo);
    assert_eq!(placed(&d, cab, a), start);
}

#[test]
fn a_typed_position_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, _) = cabinet_with_two_devices(&mut d);
    let start = placed(&d, cab, a);
    let l = novo_layout::model::load(project(&d), cab).unwrap();
    let edit = super::view3d_nudge::set_position(&l, a, Some(start.0 + 30.0), Some(start.1 + 20.0))
        .expect("a different position is an edit");
    let Edit::Group { devices, rails } = edit else {
        panic!("a typed position is a group edit");
    };
    d.app.view3d_move_group(cab, devices, rails);
    assert_eq!(
        (placed(&d, cab, a).0, placed(&d, cab, a).1),
        (start.0 + 30.0, start.1 + 20.0)
    );
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert_eq!(placed(&d, cab, a), start, "X and Y come back together");
}

#[test]
fn duplicating_a_selection_of_two_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let (cab, a, b) = cabinet_with_two_devices(&mut d);
    open_3d_with_selection(&mut d, a);
    d.app.view3d.view.state.selection = BTreeSet::from([dev(a), dev(b)]);
    let count = |d: &Drive| project(d).all::<novo_core::Device>().count();
    let placements = |d: &Drive| {
        novo_layout::model::load(project(d), cab)
            .unwrap()
            .placements
            .len()
    };
    let (devices, placed_before) = (count(&d), placements(&d));
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.3d_duplicate", None);
    d.frame();
    assert_eq!(count(&d), devices + 2);
    assert_eq!(placements(&d), placed_before + 2);
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    assert!(layout.report().collisions.is_empty() && layout.report().outside.is_empty());
    assert_eq!(d.app.view3d.view.state.selection.len(), 2, "both copies");
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(count(&d), devices, "one undo takes both copies back");
}
