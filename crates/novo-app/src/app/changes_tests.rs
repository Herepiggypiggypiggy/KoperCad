//! The Changes list in the outline and its marks in the page list.

use novo_core::SymbolInstance;

use super::OutlineMode;
use super::gate1_tests::Drive;
use crate::actions::Action;

#[test]
fn the_changes_tab_waits_for_a_revision_then_marks_the_changed_pages() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page = d.app.current_page.expect("a page is shown");
    let moved = d.place("mcb3", 80.0, 48.0);
    d.frame();
    // Without a revision the tab has nothing to compare with and leaves
    // Changes mode off.
    d.app.outline_mode = OutlineMode::Changes;
    d.app.outline_open = true;
    d.frame();
    assert!(d.app.wb.changes.is_none());
    assert!(!d.app.page_has_changes(page));
    let issued = d
        .app
        .backend
        .issue_revision(None, "2026-10-15", "TV", "First")
        .unwrap();
    d.app.changed_outside_canvas(&issued.changes);
    d.frame();
    d.frame();
    assert!(d.app.wb.changes.is_some(), "the tab turns the mode on");
    assert!(!d.app.page_has_changes(page), "nothing changed yet");
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Move", |tx| {
            tx.update::<SymbolInstance>(moved, |s| s.placement.origin.x += 10.0)
        })
        .unwrap();
    d.frame();
    d.frame();
    assert!(d.app.page_has_changes(page), "the page is marked");
}

#[test]
fn clicking_a_change_row_opens_its_page_and_selects_the_object() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page = d.app.current_page.expect("a page is shown");
    let moved = d.place("mcb3", 80.0, 48.0);
    d.frame();
    let issued = d
        .app
        .backend
        .issue_revision(None, "2026-10-15", "TV", "First")
        .unwrap();
    d.app.changed_outside_canvas(&issued.changes);
    d.frame();
    d.app.toggle_changes(None);
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Move", |tx| {
            tx.update::<SymbolInstance>(moved, |s| s.placement.origin.x += 10.0)
        })
        .unwrap();
    d.frame();
    let row = d
        .app
        .wb
        .changes
        .as_mut()
        .map(|m| {
            let project = d.app.backend.document().unwrap().project();
            m.rows(project).to_vec()
        })
        .and_then(|rows| {
            rows.into_iter()
                .find(|r| r.id == novo_core::AnyId::Symbol(moved))
        })
        .expect("the moved symbol has a row");
    assert_eq!(row.page, Some(page));
    d.app.reveal_change(&row);
    d.frame();
    d.frame();
    assert_eq!(d.app.selection(), vec![novo_core::AnyId::Symbol(moved)]);
}

#[test]
fn next_and_previous_change_walk_the_list_and_turn_the_mode_on() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let first = d.place("mcb3", 80.0, 48.0);
    d.frame();
    let issued = d
        .app
        .backend
        .issue_revision(None, "2026-10-15", "TV", "First")
        .unwrap();
    d.app.changed_outside_canvas(&issued.changes);
    let second = d.place("mcb3", 120.0, 48.0);
    d.frame();
    assert!(super::workbench::app_runs("view.next_change"));
    assert!(super::workbench::app_runs("view.previous_change"));
    assert!(d.app.wb.changes.is_none());
    d.app.run_command(&ctx, "view.next_change", None);
    d.frame();
    d.frame();
    assert!(d.app.wb.changes.is_some(), "stepping turns the mode on");
    let selected = d.app.selection();
    assert_eq!(selected.len(), 1);
    // One more step moves on or wraps, never leaves the selection empty.
    d.app.run_command(&ctx, "view.next_change", None);
    d.frame();
    d.frame();
    assert_eq!(d.app.selection().len(), 1);
    d.app.run_command(&ctx, "view.previous_change", None);
    d.frame();
    d.frame();
    assert_eq!(d.app.selection(), selected);
    let _ = (first, second);
}
