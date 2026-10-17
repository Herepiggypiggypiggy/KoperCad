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
fn the_change_report_waits_for_a_revision_then_lists_what_changed() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let moved = d.place("mcb3", 80.0, 48.0);
    d.frame();
    assert!(super::workbench::app_runs("export.change_report"));
    let out = dir.path().join("changes.pdf");
    // No revision yet: nothing to compare with, nothing written.
    assert_eq!(d.app.export_change_report(), None);
    assert!(d.app.export_change_report_to(&out).is_none());
    assert!(!out.exists());

    let issued = d
        .app
        .backend
        .issue_revision(None, "2026-10-15", "TV", "First")
        .unwrap();
    d.app.changed_outside_canvas(&issued.changes);
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Move", |tx| {
            tx.update::<SymbolInstance>(moved, |s| s.placement.origin.x += 10.0)
        })
        .unwrap();
    d.frame();
    assert_eq!(d.app.export_change_report_to(&out), Some(out.clone()));
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    // Writing again replaces our own file.
    assert_eq!(d.app.export_change_report_to(&out), Some(out.clone()));
}

/// The Changes card and the Live potentials card are panels: they start
/// at the top left without covering each other, and "Reset panel layout"
/// puts a moved one back.
#[test]
fn the_changes_and_live_cards_start_apart_and_reset_after_a_move() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let moved = d.place("mcb3", 80.0, 48.0);
    d.frame();
    let issued = d
        .app
        .backend
        .issue_revision(None, "2026-10-15", "TV", "First")
        .unwrap();
    d.app.changed_outside_canvas(&issued.changes);
    d.app.toggle_changes(None);
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Move", |tx| {
            tx.update::<SymbolInstance>(moved, |s| s.placement.origin.x += 10.0)
        })
        .unwrap();
    d.app.run_command(&ctx, "sim.live", None);
    for _ in 0..12 {
        d.frame_sized(eframe::egui::vec2(1366.0, 768.0), Vec::new());
    }
    let rect = |name: &str| ctx.memory(|m| m.area_rect(super::panel::id(name)));
    let live = rect("live_card").expect("the live card is shown");
    let changes = rect("changes_card").expect("the changes card is shown");
    assert!(!live.intersects(changes), "{live:?} over {changes:?}");
    assert!(
        live.left() < 40.0 && changes.left() < 40.0,
        "{live:?} {changes:?}"
    );
    // Dragged away, the card stays; the reset command brings it back.
    super::panel::mark_moved(&ctx, super::panel::id("changes_card"));
    assert!(super::panel::is_moved(
        &ctx,
        super::panel::id("changes_card")
    ));
    d.app.run_command(&ctx, "view.reset_panels", None);
    assert!(!super::panel::is_moved(
        &ctx,
        super::panel::id("changes_card")
    ));
}
