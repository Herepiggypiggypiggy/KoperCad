//! The `page.*` commands: the outline's page actions reachable from the
//! palette and the File > Pages menu, and the right-click Make cable row's
//! editor hook.

use novo_core::{Endpoint, Pin, Placement, Point, SymbolRole};

use super::gate1_tests::Drive;
use crate::actions::Action;

fn open() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

fn drawn_pages(d: &Drive) -> usize {
    d.doc()
        .project()
        .pages()
        .filter(|p| !matches!(p.kind, novo_core::PageKind::Generated { .. }))
        .count()
}

#[test]
fn the_page_commands_run_and_sit_in_the_pages_menu() {
    for id in [
        "page.rename",
        "page.duplicate",
        "page.delete",
        "page.move_up",
        "page.move_down",
    ] {
        assert!(super::workbench::app_runs(id), "{id}");
        let info = novo_workflow::commands::COMMANDS
            .iter()
            .find(|c| c.id == id)
            .unwrap();
        let place = novo_workflow::menus::place(info).expect("in a menu");
        assert_eq!((place.menu, place.submenu), ("File", Some("Pages")), "{id}");
    }
}

#[test]
fn duplicate_and_delete_act_on_the_open_page_as_single_undo_steps() {
    let (_dir, mut d) = open();
    let ctx = d.ctx.clone();
    let before = drawn_pages(&d);
    d.app.run_command(&ctx, "page.duplicate", None);
    d.frame();
    assert_eq!(drawn_pages(&d), before + 1);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(drawn_pages(&d), before, "one undo step");
    d.app.run_command(&ctx, "page.duplicate", None);
    d.frame();
    // The copy is the open page now.
    d.app.run_command(&ctx, "page.delete", None);
    d.frame();
    assert_eq!(drawn_pages(&d), before);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(drawn_pages(&d), before + 1);
}

#[test]
fn rename_opens_the_title_box_and_move_reorders() {
    let (_dir, mut d) = open();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "page.duplicate", None);
    d.frame();
    let open_page = d.app.current_page.unwrap();
    let order = |d: &Drive| d.doc().project().sibling_pages(open_page);
    let at = |d: &Drive| order(d).iter().position(|p| *p == open_page).unwrap();
    let start = at(&d);
    assert!(start > 0, "the copy comes after the original");
    d.app.run_command(&ctx, "page.move_up", None);
    d.frame();
    assert_eq!(at(&d), start - 1);
    d.app.run_command(&ctx, "page.move_down", None);
    d.frame();
    assert_eq!(at(&d), start);
    d.app.run_command(&ctx, "page.rename", None);
    d.frame();
    assert!(d.app.ol.rename.is_some(), "the title is being edited");
}

#[test]
fn make_cable_from_the_right_click_menu_hands_the_wires_to_the_app() {
    let (_dir, mut d) = open();
    let page = d.doc().project().pages().next().unwrap().id;
    let wire = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("setup", |tx| {
            let pins = || vec![Pin::new("1", 0.0, 0.0)];
            let a = tx.place_symbol(
                page,
                "x",
                Placement::at(Point::new(40.0, 20.0)),
                pins(),
                SymbolRole::Unassigned,
            )?;
            let b = tx.place_symbol(
                page,
                "x",
                Placement::at(Point::new(40.0, 90.0)),
                pins(),
                SymbolRole::Unassigned,
            )?;
            tx.connect(Endpoint::pin(a, 0), Endpoint::pin(b, 0))
        })
        .unwrap()
        .0;
    let view = d.app.canvas.schematic().expect("editor");
    view.editor.request_cable(vec![]);
    assert!(view.editor.take_cable_request().is_none(), "nothing asked");
    view.editor.request_cable(vec![wire]);
    assert_eq!(view.editor.take_cable_request(), Some(vec![wire]));
}
