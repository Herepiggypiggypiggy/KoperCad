//! Reordering the pages of a group and panel changes their reading order
//! without touching names.

use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn page(doc: &mut Document, name: &str, structure: &str) -> PageId {
    let out = doc
        .execute(Command::AddPage {
            name: name.into(),
            title: String::new(),
            kind: Default::default(),
            format: Default::default(),
            structure: tag(structure),
        })
        .unwrap();
    match out.created[0] {
        AnyId::Page(id) => id,
        other => panic!("{other:?}"),
    }
}

fn names(doc: &Document) -> Vec<String> {
    doc.project()
        .pages_in_order()
        .iter()
        .map(|p| p.name.clone())
        .collect()
}

fn reorder(doc: &mut Document, pages: &[PageId]) -> Result<Outcome> {
    doc.execute(Command::ReorderPages {
        pages: pages.to_vec(),
    })
}

fn four() -> (Document, [PageId; 4]) {
    let mut doc = Document::default();
    let ids = ["1", "2", "3", "4"].map(|n| page(&mut doc, n, "=A+K1"));
    (doc, ids)
}

#[test]
fn an_untouched_project_keeps_the_order_by_name() {
    let (doc, _) = four();
    assert_eq!(names(&doc), ["1", "2", "3", "4"]);
    assert!(
        doc.project()
            .pages()
            .all(|p| !p.properties.contains_key(PAGE_ORDER))
    );
}

#[test]
fn reordering_changes_the_reading_order_and_leaves_names_alone() {
    let (mut doc, [a, b, c, d]) = four();
    reorder(&mut doc, &[c, a, d, b]).unwrap();
    assert_eq!(names(&doc), ["3", "1", "4", "2"]);
    assert_eq!(doc.project().page(c).unwrap().name, "3");
    assert_eq!(doc.project().sibling_pages(b), [c, a, d, b]);
    // Undo is one step and brings the old order back.
    doc.undo().unwrap();
    assert_eq!(names(&doc), ["1", "2", "3", "4"]);
    doc.redo().unwrap();
    assert_eq!(names(&doc), ["3", "1", "4", "2"]);
}

#[test]
fn a_page_added_later_goes_last_and_other_groups_do_not_move() {
    let (mut doc, [a, b, c, d]) = four();
    let other = page(&mut doc, "1", "=B+K1");
    reorder(&mut doc, &[d, c, b, a]).unwrap();
    let new = page(&mut doc, "0", "=A+K1");
    assert_eq!(doc.project().sibling_pages(a), [d, c, b, a, new]);
    // =B sorts after =A whatever the order inside =A.
    assert_eq!(doc.project().pages_in_order().last().unwrap().id, other);
    assert!(
        !doc.project()
            .page(other)
            .unwrap()
            .properties
            .contains_key(PAGE_ORDER)
    );
}

#[test]
fn the_same_order_changes_nothing() {
    let (mut doc, ids) = four();
    reorder(&mut doc, &ids).unwrap();
    let out = reorder(&mut doc, &ids).unwrap();
    assert!(out.changes.is_empty(), "{:?}", out.changes);
}

#[test]
fn a_bad_list_is_refused_and_nothing_changes() {
    let (mut doc, [a, b, c, d]) = four();
    let other = page(&mut doc, "1", "=B+K1");
    let before = names(&doc);
    // Missing a page of the group, a page twice, a page of another group,
    // and no pages at all.
    for bad in [
        vec![a, b, c],
        vec![a, b, c, d, d],
        vec![a, b, c, d, other],
        vec![other, a],
        vec![],
    ] {
        let err = reorder(&mut doc, &bad).unwrap_err();
        assert!(matches!(err, ModelError::Refused { .. }), "{err:?}");
    }
    assert_eq!(names(&doc), before);
    assert!(
        doc.project()
            .pages()
            .all(|p| !p.properties.contains_key(PAGE_ORDER))
    );
}

#[test]
fn contents_and_generated_pages_keep_their_sections() {
    let (mut doc, [a, b, c, d]) = four();
    let contents = doc
        .execute(Command::AddPage {
            name: "0".into(),
            title: String::new(),
            kind: PageKind::Generated {
                generator: CONTENTS_GENERATOR.into(),
            },
            format: Default::default(),
            structure: tag("=A+K1"),
        })
        .unwrap();
    let AnyId::Page(contents) = contents.created[0] else {
        panic!()
    };
    // The contents page is not a sibling of the drawn pages, so a list that
    // mixes them is refused, and reordering the drawn ones leaves it first.
    assert!(reorder(&mut doc, &[contents, a]).is_err());
    reorder(&mut doc, &[d, c, b, a]).unwrap();
    assert_eq!(doc.project().pages_in_order()[0].id, contents);
    assert_eq!(doc.project().sibling_pages(a), [d, c, b, a]);
}

#[test]
fn renaming_the_panel_keeps_the_order_and_a_merge_stays_stable() {
    let (mut doc, [a, b, c, d]) = four();
    reorder(&mut doc, &[d, c, b, a]).unwrap();
    doc.execute(Command::RenameLevel {
        aspect: Aspect::Location,
        from: vec!["K1".into()],
        to: vec!["K2".into()],
    })
    .unwrap();
    assert_eq!(doc.project().sibling_pages(a), [d, c, b, a]);
    // Merging a panel that has its own order into this one does not lose
    // pages; equal order numbers fall back to the name.
    let e = page(&mut doc, "1", "=A+K3");
    reorder(&mut doc, &[e]).unwrap();
    doc.execute(Command::RenameLevel {
        aspect: Aspect::Location,
        from: vec!["K3".into()],
        to: vec!["K2".into()],
    })
    .unwrap();
    assert_eq!(doc.project().sibling_pages(a).len(), 5);
}
