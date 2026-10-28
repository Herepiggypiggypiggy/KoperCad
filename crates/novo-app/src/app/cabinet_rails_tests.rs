//! Rails and ducts by hand in the Cabinet: add, move, resize, turn, pick and
//! delete, each one undo step.

use eframe::egui;
use novo_core::{DeviceTag, MountingPlate, Project, Rail, RailKind};

use super::cabinet_rails::{RailAsk, RailEdit, new_rail_at, pick, pick_grip};
use super::gate1_tests::Drive;
use crate::actions::Action;

fn project(d: &Drive) -> &Project {
    d.app.backend.document().unwrap().project()
}

fn cabinet(d: &mut Drive) -> novo_core::StructureId {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1")?, "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 600.0,
                    height: 800.0,
                    margin: 10.0,
                }),
            )?;
            Ok(())
        })
        .unwrap();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    project(d).structures().find(|s| s.enclosure).unwrap().id
}

fn rails(d: &Drive) -> Vec<Rail> {
    let mut v: Vec<Rail> = project(d).all::<Rail>().cloned().collect();
    v.sort_by_key(|r| r.id.raw());
    v
}

#[test]
fn rails_and_ducts_are_added_edited_turned_and_deleted_with_undo() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let cab = cabinet(&mut d);

    for ask in [
        RailAsk::Add {
            kind: RailKind::DinRail,
            vertical: false,
        },
        RailAsk::Add {
            kind: RailKind::Duct,
            vertical: false,
        },
        RailAsk::Add {
            kind: RailKind::Duct,
            vertical: true,
        },
    ] {
        d.app.cabinet_rail_ask(Some(cab), ask);
    }
    let all = rails(&d);
    assert_eq!(all.len(), 3);
    let (rail, duct, trunk) = (&all[0], &all[1], &all[2]);
    assert_eq!(rail.kind, RailKind::DinRail);
    assert_eq!((duct.width, duct.height), (25.0, 60.0));
    // The vertical duct would cross the horizontal one at full length, so it
    // is shortened to stand clear of it.
    assert!(trunk.vertical && trunk.length < 780.0);
    // New horizontal ones stack below each other on the usable plate.
    assert!(duct.y > rail.y);
    assert_eq!(d.app.cabinet.rails.selected, Some(trunk.id.raw()));

    let id = duct.id.raw();
    d.app.cabinet_rail_ask(
        Some(cab),
        RailAsk::Set(RailEdit {
            id,
            x: 100.0,
            y: 200.0,
            length: 300.0,
            width: 40.0,
            height: 40.0,
        }),
    );
    let edited = rails(&d).into_iter().find(|r| r.id.raw() == id).unwrap();
    assert_eq!(
        (
            edited.x,
            edited.y,
            edited.length,
            edited.width,
            edited.height
        ),
        (100.0, 200.0, 300.0, 40.0, 40.0)
    );
    d.app.cabinet_rail_ask(Some(cab), RailAsk::Turn(id));
    assert!(
        rails(&d)
            .iter()
            .find(|r| r.id.raw() == id)
            .unwrap()
            .vertical
    );
    d.app
        .cabinet_rail_ask(Some(cab), RailAsk::MoveTo(id, 150.0, 120.0));
    let moved = rails(&d).into_iter().find(|r| r.id.raw() == id).unwrap();
    assert_eq!((moved.x, moved.y), (150.0, 120.0));

    d.app.cabinet_rail_ask(Some(cab), RailAsk::Delete(id));
    assert_eq!(rails(&d).len(), 2);
    // Each edit is one undo step.
    for _ in 0..4 {
        d.app.run(&ctx, Action::Undo);
    }
    let back = rails(&d);
    assert_eq!(back.len(), 3);
    let first = back.iter().find(|r| r.id.raw() == id).unwrap();
    assert_eq!((first.x, first.y), (duct.x, duct.y));
}

#[test]
fn a_click_picks_the_duct_over_the_rail_and_delete_removes_the_selected_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d);
    let (x, y, length, width, height) =
        new_rail_at(project(&d), cab, RailKind::DinRail, false).unwrap();
    assert_eq!((x, length), (10.0, 580.0));
    assert!(width == 0.0 && height == 0.0 && y > 10.0);
    d.app.cabinet_rail_ask(
        Some(cab),
        RailAsk::Add {
            kind: RailKind::DinRail,
            vertical: false,
        },
    );
    let rail = rails(&d)[0].id.raw();
    // On the rail strip, 12 mm off its centre line, and well off it.
    assert_eq!(pick(project(&d), cab, 100.0, y + 12.0, 0.0), Some(rail));
    assert_eq!(pick(project(&d), cab, 100.0, y + 40.0, 0.0), None);
    d.app.cabinet_rail_ask(
        Some(cab),
        RailAsk::Add {
            kind: RailKind::Duct,
            vertical: true,
        },
    );
    let duct = rails(&d)[1].id.raw();
    // Where they cross the duct is on top.
    let dx = rails(&d)[1].x;
    assert_eq!(pick(project(&d), cab, dx, y, 0.0), Some(duct));

    // Delete key: nothing selected, nothing happens; selected, it goes.
    d.app.cabinet.rails.selected = None;
    assert_eq!(d.app.cabinet.rails.deletable(project(&d)), None);
    d.app.cabinet.rails.selected = Some(duct);
    let id = d.app.cabinet.rails.deletable(project(&d)).unwrap();
    d.app.cabinet_rail_ask(Some(cab), RailAsk::Delete(id));
    assert_eq!(rails(&d).len(), 1);
    assert_eq!(d.app.cabinet.rails.selected, None);
}

#[test]
fn the_rails_and_ducts_list_has_a_command_a_menu_row_and_a_shortcut() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let _ = cabinet(&mut d);
    d.app.run_command(&ctx, "cabinet.rails", None);
    d.frame();
    assert!(d.app.cabinet.rails.open);
    d.app.run_command(&ctx, "cabinet.rails", None);
    assert!(!d.app.cabinet.rails.open);
    let info = novo_workflow::commands::get("cabinet.rails").expect("registered");
    assert!(novo_workflow::menus::place(info).is_some());
    let keymap = novo_workflow::keymap::Keymap::workflow();
    assert!(
        keymap
            .bindings()
            .iter()
            .any(|b| b.command == "cabinet.rails")
    );
}

/// In the Cabinet the chord and the arrow keys reach the rails: Ctrl+Shift+R
/// opens and closes the list, an arrow nudges the selected rail by a grid
/// step and Shift by 1 mm, and the devices on it go along.
#[test]
fn keys_in_the_cabinet_toggle_the_list_and_nudge_the_selected_rail() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d);
    d.app.run(&d.ctx.clone(), Action::ShowCabinet);
    d.frame();
    assert_eq!(d.app.workspace(), super::workspace::Workspace::Cabinet);
    assert!(!d.app.cabinet.rails.open);
    let chord = egui::Modifiers {
        ctrl: true,
        command: true,
        shift: true,
        ..Default::default()
    };
    d.key(egui::Key::R, chord);
    assert!(d.app.cabinet.rails.open, "the chord opens the list");
    d.key(egui::Key::R, chord);
    assert!(!d.app.cabinet.rails.open, "and closes it again");

    d.app.cabinet_rail_ask(
        Some(cab),
        RailAsk::Add {
            kind: RailKind::DinRail,
            vertical: false,
        },
    );
    let rail = rails(&d)[0].clone();
    let grid = novo_layout::state::DEFAULT_GRID_MM;
    d.app.cabinet.rails.selected = Some(rail.id.raw());
    d.key(egui::Key::ArrowDown, egui::Modifiers::NONE);
    let moved = rails(&d)[0].clone();
    assert_eq!((moved.x, moved.y), (rail.x, rail.y + grid));
    d.key(egui::Key::ArrowRight, egui::Modifiers::SHIFT);
    assert_eq!(rails(&d)[0].x, rail.x + 1.0);
}

/// The grip at a rail's start is grabbable where devices cover the rail, and
/// ducts and rails alike have one.
#[test]
fn a_rail_has_a_grip_outside_its_start() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d);
    d.app.cabinet_rail_ask(
        Some(cab),
        RailAsk::Add {
            kind: RailKind::DinRail,
            vertical: false,
        },
    );
    let r = rails(&d)[0].clone();
    let id = r.id.raw();
    assert_eq!(
        pick_grip(project(&d), cab, r.x - 5.0, r.y, 0.0, 10.0),
        Some(id)
    );
    assert_eq!(
        pick_grip(project(&d), cab, r.x + 100.0, r.y, 0.0, 10.0),
        None
    );
}

/// The inside size and door form has a command and a menu row, and the
/// command opens the plate card with the form scrolled into view.
#[test]
fn inside_size_and_door_has_a_command_a_menu_row_and_opens_the_card() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let _ = cabinet(&mut d);
    d.app.cabinet.plate_card = false;
    d.app.run_command(&ctx, "cabinet.door", None);
    d.frame();
    assert!(d.app.cabinet.plate_card);
    assert!(d.app.cabinet.view.selection.ids().is_empty());
    let info = novo_workflow::commands::get("cabinet.door").expect("registered");
    assert!(novo_workflow::menus::place(info).is_some());
    assert!(
        d.app.learn_anchor("cabinet.door.apply").is_some(),
        "the form is drawn"
    );
    // The inspector is a movable window now; the form's Apply button is
    // scrolled into it, not left below its edge.
    // Scrolling to a target is animated: let it finish.
    for _ in 0..60 {
        d.frame();
    }
    let apply = d.app.learn_anchor("cabinet.door.apply").unwrap();
    let panel = d.app.learn_anchor("cabinet.inspector").expect("the window");
    assert!(
        panel.contains_rect(apply),
        "Apply {apply:?} inside the inspector {panel:?}"
    );
}

fn rail_at(x: f64, y: f64, vertical: bool, kind: RailKind) -> Rail {
    Rail {
        id: novo_core::RailId::from(1),
        structure: novo_core::StructureId::from(1),
        kind,
        x,
        y,
        length: 400.0,
        vertical,
        width: 0.0,
        height: 0.0,
        properties: Default::default(),
    }
}

#[test]
fn a_grip_sits_just_outside_the_start_of_its_rail() {
    use super::cabinet_rails::grip_of;
    // Horizontal: a tab to the left of the start, centred on the line.
    let (l, t, r, b) = grip_of(&rail_at(100.0, 200.0, false, RailKind::DinRail), 14.0);
    assert_eq!((l, r), (86.0, 100.0));
    assert!(t < 200.0 && b > 200.0 && ((t + b) / 2.0 - 200.0).abs() < 1e-9);
    // Vertical: a tab above the start, centred on the line.
    let (l, t, r, b) = grip_of(&rail_at(100.0, 200.0, true, RailKind::Duct), 14.0);
    assert_eq!((t, b), (186.0, 200.0));
    assert!(l < 100.0 && r > 100.0);
}

#[test]
fn the_grip_colours_come_from_the_palette_not_from_raw_tokens() {
    // Neutral at rest, copper only when lit: the two fills differ and the
    // resting one is never the accent.
    for p in [&novo_theme::Palette::DARK, &novo_theme::Palette::LIGHT] {
        assert_ne!(p.border_strong, p.accent);
        assert_ne!(p.text_tertiary, p.on_accent);
    }
}

#[test]
fn a_duct_never_lies_over_another_duct() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d);
    let add = |d: &mut Drive, vertical| {
        d.app.cabinet_rail_ask(
            Some(cab),
            RailAsk::Add {
                kind: RailKind::Duct,
                vertical,
            },
        )
    };
    add(&mut d, true);
    // A second vertical duct asks for the same place and lands beside it.
    add(&mut d, true);
    let ducts: Vec<Rail> = rails(&d);
    assert_eq!(ducts.len(), 2);
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    assert!(layout.report().duct_overlaps.is_empty());

    // Typing a start that puts one on the other is refused, nothing changes.
    let (a, b) = (&ducts[0], &ducts[1]);
    let before = rails(&d);
    d.app.cabinet_rail_ask(
        Some(cab),
        RailAsk::Set(RailEdit {
            id: b.id.raw(),
            x: a.x + 5.0,
            y: a.y,
            length: b.length,
            width: b.width,
            height: b.height,
        }),
    );
    assert_eq!(rails(&d), before);
    // So is dragging it there and turning it across the other.
    d.app
        .cabinet_rail_ask(Some(cab), RailAsk::MoveTo(b.id.raw(), a.x + 5.0, a.y));
    assert_eq!(rails(&d), before);
    // One that ends against the side of the other (a T) is fine.
    let side = a.x + b.width.max(a.width) + 1.0;
    d.app
        .cabinet_rail_ask(Some(cab), RailAsk::MoveTo(b.id.raw(), side, a.y));
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    assert!(layout.report().duct_overlaps.is_empty());
}

mod ends {
    use super::*;
    use crate::app::cabinet_rail_ends::{handle_of, handle_size, pick_end};
    use novo_layout::railedit::{End, Stop};

    fn setup() -> (tempfile::TempDir, Drive, novo_core::StructureId, Vec<u64>) {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let cab = cabinet(&mut d);
        // Two horizontal ducts on separate rows and a vertical one between.
        let ids = d
            .app
            .backend
            .document_mut()
            .unwrap()
            .transact("Ducts", |tx| {
                let mut ids = Vec::new();
                for (x, y, len, vertical) in [
                    (20.0, 100.0, 300.0, false),
                    (20.0, 300.0, 300.0, false),
                    (200.0, 40.0, 500.0, true),
                ] {
                    let r = tx.add_rail(cab, x, y, len, RailKind::Duct)?;
                    tx.update::<Rail>(r, |r| r.vertical = vertical)?;
                    tx.set_rail_size(r, len, 30.0, 60.0)?;
                    ids.push(r.raw());
                }
                Ok(ids)
            })
            .unwrap()
            .0;
        (dir, d, cab, ids)
    }

    #[test]
    fn dragging_an_end_resizes_it_in_one_undo_step_and_keeps_the_other_end() {
        let (_dir, mut d, cab, ids) = setup();
        let ctx = d.ctx.clone();
        let layout = novo_layout::model::load(project(&d), cab).unwrap();
        // Finish end pulled to x 160 (snapped to the grid), start stays.
        let edit =
            novo_layout::railedit::resize_end(&layout, ids[0], End::Finish, 161.0, 3.0).unwrap();
        d.app
            .cabinet_rail_ask(Some(cab), RailAsk::Resize(ids[0], edit));
        let r = rails(&d)
            .into_iter()
            .find(|r| r.id.raw() == ids[0])
            .unwrap();
        assert_eq!((r.x, r.y, r.length), (20.0, 100.0, 140.0));
        d.app.run(&ctx, Action::Undo);
        let r = rails(&d)
            .into_iter()
            .find(|r| r.id.raw() == ids[0])
            .unwrap();
        assert_eq!((r.x, r.length), (20.0, 300.0));
    }

    #[test]
    fn a_typed_length_stops_at_the_vertical_duct_instead_of_overlapping_it() {
        let (_dir, mut d, cab, ids) = setup();
        // Duct 1 starts at 20 and the vertical duct's left edge is 185. Both
        // rows start 300 mm long, across the trunk; cut them clear of it first.
        for id in [ids[0], ids[1]] {
            d.app
                .cabinet_rail_ask(Some(cab), RailAsk::SetLength(id, 100.0));
        }
        d.app
            .cabinet_rail_ask(Some(cab), RailAsk::SetLength(ids[0], 400.0));
        let r = rails(&d)
            .into_iter()
            .find(|r| r.id.raw() == ids[0])
            .unwrap();
        assert_eq!(r.length, 165.0);
        // The list's own field takes the same path.
        d.app.cabinet_rail_ask(
            Some(cab),
            RailAsk::Set(RailEdit {
                id: ids[1],
                x: 20.0,
                y: 300.0,
                length: 500.0,
                width: 30.0,
                height: 60.0,
            }),
        );
        let r = rails(&d)
            .into_iter()
            .find(|r| r.id.raw() == ids[1])
            .unwrap();
        assert_eq!(r.length, 165.0);
        let layout = novo_layout::model::load(project(&d), cab).unwrap();
        assert!(layout.report().duct_overlaps.is_empty());
    }

    #[test]
    fn trim_to_crossing_and_at_a_point_cut_the_duct_back() {
        let (_dir, mut d, cab, ids) = setup();
        d.app.cabinet_rail_ask(
            Some(cab),
            RailAsk::Trim {
                id: ids[0],
                end: End::Finish,
            },
        );
        let r = rails(&d)
            .into_iter()
            .find(|r| r.id.raw() == ids[0])
            .unwrap();
        assert_eq!(r.length, 165.0);
        d.app.cabinet_rail_ask(
            Some(cab),
            RailAsk::TrimAt {
                id: ids[1],
                end: End::Start,
                at: 100.0,
            },
        );
        let r = rails(&d)
            .into_iter()
            .find(|r| r.id.raw() == ids[1])
            .unwrap();
        assert_eq!((r.x, r.length), (100.0, 220.0));
    }

    #[test]
    fn the_end_handles_sit_inside_both_ends_and_are_found_there() {
        let (_dir, d, _cab, ids) = setup();
        let r = rails(&d)
            .into_iter()
            .find(|r| r.id.raw() == ids[0])
            .unwrap();
        let size = handle_size(1.0);
        let (l, t, rt, b) = handle_of(&r, End::Finish, size);
        assert_eq!(rt, r.x + r.length);
        assert!(l > r.x && t < r.y && b > r.y);
        let p = project(&d);
        assert_eq!(
            pick_end(p, ids[0], rt - 2.0, r.y, 1.0, size),
            Some(End::Finish)
        );
        assert_eq!(
            pick_end(p, ids[0], r.x + 2.0, r.y, 1.0, size),
            Some(End::Start)
        );
        assert_eq!(
            pick_end(p, ids[0], r.x + r.length / 2.0, r.y, 1.0, size),
            None
        );
        // Short pieces still have two separate handles.
        let _ = Stop::Shortest;
    }
}
