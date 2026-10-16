//! Rails and ducts by hand in the Cabinet: add, move, resize, turn, pick and
//! delete, each one undo step.

use novo_core::{DeviceTag, MountingPlate, Project, Rail, RailKind};

use super::cabinet_rails::{RailAsk, RailEdit, new_rail_at, pick};
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
    assert!(trunk.vertical && trunk.length == 780.0);
    // New horizontal ones stack below each other on the usable plate.
    assert!(duct.y > rail.y);
    assert_eq!(d.app.cabinet.rails.selected, Some(trunk.id.raw()));

    let id = duct.id.raw();
    d.app.cabinet_rail_ask(
        Some(cab),
        RailAsk::Set(RailEdit {
            id,
            x: 30.0,
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
        (30.0, 200.0, 300.0, 40.0, 40.0)
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
        .cabinet_rail_ask(Some(cab), RailAsk::MoveTo(id, 100.0, 120.0));
    let moved = rails(&d).into_iter().find(|r| r.id.raw() == id).unwrap();
    assert_eq!((moved.x, moved.y), (100.0, 120.0));

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
