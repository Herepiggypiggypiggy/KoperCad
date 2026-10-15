//! Page groups and panels: pages in different groups under the same
//! location are one panel.

use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn add(doc: &mut Document, name: &str, structure: &str) -> PageId {
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

#[test]
fn pages_of_one_panel_in_two_groups_share_the_panel() {
    let mut doc = Document::default();
    let a1 = add(&mut doc, "1", "=10D41+K1B1G1");
    let a2 = add(&mut doc, "2", "=10D41+K1B1G1");
    let b1 = add(&mut doc, "1", "=11D21+K1B1G1");
    let b2 = add(&mut doc, "2", "=11D21+K1B1G1");
    let other = add(&mut doc, "1", "=11D21+K2");
    let project = doc.project();

    let panels = project.panels();
    assert_eq!(panels.len(), 2);
    assert_eq!(panels[0].location, tag("+K1B1G1"));
    assert_eq!(panels[0].pages, vec![a1, a2, b1, b2]);
    assert_eq!(panels[0].groups, vec![tag("=10D41"), tag("=11D21")]);
    assert_eq!(panels[1].location, tag("+K2"));
    assert_eq!(panels[1].pages, vec![other]);
    assert_eq!(project.page_groups(), vec![tag("=10D41"), tag("=11D21")]);
}

#[test]
fn a_page_moves_between_groups_and_panels_and_undoes() {
    let mut doc = Document::default();
    let page = add(&mut doc, "1", "=10D41+K1B1G1");
    doc.execute(Command::SetPageStructure {
        page,
        structure: tag("=11D21+K2"),
    })
    .unwrap();
    assert_eq!(
        doc.project().page(page).unwrap().structure,
        tag("=11D21+K2")
    );
    doc.undo().unwrap();
    assert_eq!(
        doc.project().page(page).unwrap().structure,
        tag("=10D41+K1B1G1")
    );
    let err = doc
        .execute(Command::SetPageStructure {
            page,
            structure: tag("=A-K1"),
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidValue { .. }));
}

#[test]
fn empty_panels_and_next_page_names() {
    let mut doc = Document::default();
    doc.execute(Command::AddStructure {
        tag: tag("+K9"),
        description: String::new(),
        enclosure: true,
    })
    .unwrap();
    add(&mut doc, "1", "=10D41+K1B1G1");
    add(&mut doc, "2", "=10D41+K1B1G1");
    let project = doc.project();
    assert!(
        project
            .panels()
            .iter()
            .any(|p| p.location == tag("+K9") && p.pages.is_empty())
    );
    assert_eq!(project.next_page_name(&tag("=10D41+K1B1G1")), "3");
    assert_eq!(project.next_page_name(&tag("=11D21+K1B1G1")), "1");
}

#[test]
fn a_page_renames_and_undoes_but_not_to_nothing() {
    let mut doc = Document::default();
    let page = add(&mut doc, "1", "=10D41+K1B1G1");
    doc.execute(Command::RenamePage {
        page,
        name: " 4a ".into(),
    })
    .unwrap();
    assert_eq!(doc.project().page(page).unwrap().name, "4a");
    doc.undo().unwrap();
    assert_eq!(doc.project().page(page).unwrap().name, "1");
    let err = doc
        .execute(Command::RenamePage {
            page,
            name: "  ".into(),
        })
        .unwrap_err();
    assert!(matches!(
        err,
        ModelError::InvalidValue { field: "name", .. }
    ));
}
