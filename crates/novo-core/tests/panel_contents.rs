//! The contents list of one panel: its generator id names the panel, sorts
//! with the contents lists, and follows the panel when it is renamed.

use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn levels(text: &str) -> Vec<String> {
    text.split('/').map(String::from).collect()
}

fn add(doc: &mut Document, name: &str, structure: &str, generator: Option<&str>) -> PageId {
    let out = doc
        .execute(Command::AddPage {
            name: name.into(),
            title: String::new(),
            kind: generator.map_or(PageKind::Schematic, |g| PageKind::Generated {
                generator: g.into(),
            }),
            format: Default::default(),
            structure: tag(structure),
        })
        .unwrap();
    match out.created[0] {
        AnyId::Page(id) => id,
        other => panic!("{other:?}"),
    }
}

fn generator(doc: &Document, page: PageId) -> String {
    match &doc.project().page(page).unwrap().kind {
        PageKind::Generated { generator } => generator.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_generator_id_names_the_panel() {
    let id = panel_contents_generator(&levels("BAY1G1"));
    assert_eq!(id, "toc@+BAY1G1");
    assert_eq!(contents_scope(&id), Some(tag("+BAY1G1")));
    assert_eq!(
        contents_scope(&panel_contents_generator(&levels("CAB1/DOOR"))),
        Some(tag("+CAB1+DOOR"))
    );
    assert!(is_contents_generator("toc"));
    assert!(is_contents_generator(&id));
    // The project list has no panel, and other ids are not contents lists.
    assert_eq!(contents_scope("toc"), None);
    for other in [
        "wires",
        "toc@",
        "toc@=A1",
        "toc@+K1-X1",
        "toc+K1",
        "bom@+K1",
    ] {
        assert!(contents_scope(other).is_none(), "{other}");
        assert!(!is_contents_generator(other), "{other}");
    }
}

#[test]
fn a_panel_list_reads_with_the_contents_lists_before_drawn_pages() {
    let mut doc = Document::default();
    let drawn = add(&mut doc, "1", "=A+K1", None);
    let panel = add(&mut doc, "0", "+K1", Some("toc@+K1"));
    let project = add(&mut doc, "0", "", Some(CONTENTS_GENERATOR));
    let wires = add(&mut doc, "9", "", Some("wires"));
    let order: Vec<PageId> = doc
        .project()
        .pages_in_order()
        .iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(order, [project, panel, drawn, wires]);
}

#[test]
fn renaming_the_panel_renames_its_contents_list() {
    let mut doc = Document::default();
    let drawn = add(&mut doc, "1", "=A+CAB1", None);
    let list = add(&mut doc, "0", "+CAB1", Some("toc@+CAB1"));
    let door = add(&mut doc, "0", "+CAB1+DOOR", Some("toc@+CAB1+DOOR"));
    let other = add(&mut doc, "0", "+K9", Some("toc@+K9"));
    doc.execute(Command::RenameLevel {
        aspect: Aspect::Location,
        from: levels("CAB1"),
        to: levels("CAB2"),
    })
    .unwrap();
    assert_eq!(generator(&doc, list), "toc@+CAB2");
    assert_eq!(generator(&doc, door), "toc@+CAB2+DOOR");
    assert_eq!(generator(&doc, other), "toc@+K9");
    assert_eq!(doc.project().page(list).unwrap().structure, tag("+CAB2"));
    assert_eq!(doc.project().page(drawn).unwrap().structure, tag("=A+CAB2"));
    // One undo step puts the id back with the structure.
    doc.undo().unwrap();
    assert_eq!(generator(&doc, list), "toc@+CAB1");
    assert_eq!(doc.project().page(list).unwrap().structure, tag("+CAB1"));
}

#[test]
fn renaming_a_group_leaves_panel_lists_alone() {
    let mut doc = Document::default();
    add(&mut doc, "1", "=A+K1", None);
    let list = add(&mut doc, "0", "+K1", Some("toc@+K1"));
    doc.execute(Command::RenameLevel {
        aspect: Aspect::Function,
        from: levels("A"),
        to: levels("B"),
    })
    .unwrap();
    assert_eq!(generator(&doc, list), "toc@+K1");
}
