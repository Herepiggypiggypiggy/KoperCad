//! The Compare revisions window: command, sides, and what it lists.

use novo_core::SymbolInstance;
use novo_revisions::{ChangeKind, Side};

use super::gate1_tests::Drive;
use crate::actions::Action;

#[test]
fn compare_lists_what_changed_between_two_revisions_and_now() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let moved = d.place("mcb3", 80.0, 48.0);
    d.frame();

    // Without a revision there is nothing to compare.
    assert!(super::workbench::app_runs("revision.compare"));
    d.app.run_command(&ctx, "revision.compare", None);
    assert!(d.app.wb.compare.is_none());

    let a = d
        .app
        .backend
        .issue_revision(None, "2026-10-15", "TV", "First")
        .unwrap();
    d.app.changed_outside_canvas(&a.changes);
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Move", |tx| {
            tx.update::<SymbolInstance>(moved, |s| s.placement.origin.x += 10.0)
        })
        .unwrap();
    let b = d
        .app
        .backend
        .issue_revision(None, "2026-10-16", "TV", "Second")
        .unwrap();
    d.app.changed_outside_canvas(&b.changes);
    d.frame();

    // Default: the latest revision against now, which differ in nothing.
    d.app.run_command(&ctx, "revision.compare", None);
    d.frame();
    let win = d.app.wb.compare.as_ref().expect("the window opens");
    let (_, from, to, result) = win.shown.as_ref().expect("compared");
    assert_eq!((*from, *to), (Side::Revision(b.revision), Side::Now));
    assert!(result.as_ref().unwrap().is_empty());

    // A against B names the moved symbol with its old and new position.
    d.app.run_command(&ctx, "revision.compare", Some("A B"));
    d.frame();
    let win = d.app.wb.compare.as_ref().unwrap();
    let (_, _, _, result) = win.shown.as_ref().unwrap();
    let c = result.as_ref().unwrap();
    let row = c
        .rows
        .iter()
        .find(|r| matches!(r.kind, ChangeKind::Changed { .. }))
        .expect("a changed row");
    let position = row.fields.iter().find(|f| f.name == "position").unwrap();
    assert_eq!(position.note.as_deref(), Some("moved 10 mm right"));
    assert!(super::compare::field_line(position).contains("→"));

    // An unknown revision keeps the window as it was and says so.
    d.app.run_command(&ctx, "revision.compare", Some("Q"));
    assert!(
        d.app
            .messages()
            .iter()
            .any(|m| m.text.contains("no revision Q") || m.text.contains("There is no revision Q"))
    );
}
