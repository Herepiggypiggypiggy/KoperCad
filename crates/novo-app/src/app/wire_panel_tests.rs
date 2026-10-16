//! The wire panel's edits: one undo step, every selected wire, bad input
//! refused with the text it could not read.

use super::gate1_tests::Drive;
use super::wire_panel::{WireEdit, apply, facts};
use crate::actions::Action;

#[test]
fn edits_reach_every_selected_wire_and_bad_input_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let wires: Vec<_> = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .wires()
        .map(|w| w.id)
        .take(2)
        .collect();
    assert_eq!(wires.len(), 2);
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("test", |tx| {
        apply(tx, &wires, &WireEdit::Method("B2".into())).map_err(novo_core::ModelError::Format)?;
        apply(tx, &wires, &WireEdit::Section("2,5".into()))
            .map_err(novo_core::ModelError::Format)?;
        apply(tx, &wires, &WireEdit::Grouped("6".into())).map_err(novo_core::ModelError::Format)
    })
    .unwrap();
    for w in &wires {
        let f = facts(doc.project(), *w).unwrap();
        assert_eq!(f.method, "B2");
        assert_eq!(f.section, "2.5");
        assert_eq!(f.grouped, "6");
    }
    let bad = doc.transact("bad", |tx| {
        apply(tx, &wires, &WireEdit::Section("thick".into())).map_err(novo_core::ModelError::Format)
    });
    assert!(bad.is_err());
    assert_eq!(facts(doc.project(), wires[0]).unwrap().section, "2.5");
    doc.transact("clear", |tx| {
        apply(tx, &wires, &WireEdit::Method(String::new())).map_err(novo_core::ModelError::Format)
    })
    .unwrap();
    assert_eq!(facts(doc.project(), wires[0]).unwrap().method, "");
}

#[test]
fn leaving_a_field_without_an_edit_commits_nothing() {
    use super::wire_panel::should_commit;
    assert!(!should_commit(true, "BK", "BK"));
    assert!(!should_commit(true, " BK ", "BK"));
    assert!(!should_commit(false, "RD", "BK"));
    assert!(should_commit(true, "RD", "BK"));
    assert!(should_commit(true, "", "BK"));
}
