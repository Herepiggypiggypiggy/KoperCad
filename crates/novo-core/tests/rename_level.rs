//! Renaming a group or panel rewrites every copy of its name in one step.

use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn levels(text: &str) -> Vec<String> {
    text.split('/').map(String::from).collect()
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

fn device(doc: &mut Document, text: &str) -> DeviceId {
    let out = doc
        .execute(Command::AddDevice {
            tag: tag(text),
            description: String::new(),
            kind: Default::default(),
        })
        .unwrap();
    match out.created[0] {
        AnyId::Device(id) => id,
        other => panic!("{other:?}"),
    }
}

fn structure(doc: &mut Document, text: &str, enclosure: bool) {
    doc.execute(Command::AddStructure {
        tag: tag(text),
        description: String::new(),
        enclosure,
    })
    .unwrap();
}

fn rename(doc: &mut Document, aspect: Aspect, from: &str, to: &str) -> Result<Outcome> {
    doc.execute(Command::RenameLevel {
        aspect,
        from: levels(from),
        to: levels(to),
    })
}

#[test]
fn renaming_a_panel_rewrites_pages_devices_cables_and_structure() {
    let mut doc = Document::default();
    let p = page(&mut doc, "1", "=20D41+BAY1G1");
    let other = page(&mut doc, "1", "=20D41+K2");
    let k1 = device(&mut doc, "=20D41+BAY1G1-K1");
    let k2 = device(&mut doc, "=20D41+K2-K1");
    structure(&mut doc, "+BAY1G1", true);
    doc.execute(Command::AddCable {
        tag: tag("=20D41+BAY1G1-W1"),
        cores: Vec::new(),
    })
    .unwrap();

    rename(&mut doc, Aspect::Location, "BAY1G1", "BAY2G1").unwrap();

    let project = doc.project();
    assert_eq!(project.page(p).unwrap().structure, tag("=20D41+BAY2G1"));
    assert_eq!(project.page(other).unwrap().structure, tag("=20D41+K2"));
    assert_eq!(project.device(k1).unwrap().tag, tag("=20D41+BAY2G1-K1"));
    assert_eq!(project.device(k2).unwrap().tag, tag("=20D41+K2-K1"));
    assert!(project.cables().any(|c| c.tag == tag("=20D41+BAY2G1-W1")));
    assert!(project.structures().any(|s| s.tag == tag("+BAY2G1")));
    assert!(!project.structures().any(|s| s.tag == tag("+BAY1G1")));
}

#[test]
fn enclosure_lookups_are_the_same_before_and_after() {
    let mut doc = Document::default();
    structure(&mut doc, "+K1", true);
    structure(&mut doc, "+K2", true);
    let a = device(&mut doc, "=A+K1+DOOR-X1");
    let b = device(&mut doc, "=A+K2-X1");
    let found = |doc: &Document, id: DeviceId| {
        let project = doc.project();
        let tag = project.device(id).unwrap().tag.clone();
        project.enclosure_of(&tag).map(|s| s.tag.to_string())
    };
    assert_eq!(found(&doc, a).as_deref(), Some("+K1"));

    rename(&mut doc, Aspect::Location, "K1", "CAB1").unwrap();
    assert_eq!(found(&doc, a).as_deref(), Some("+CAB1"));
    assert_eq!(found(&doc, b).as_deref(), Some("+K2"));
    // The sub-level followed its parent.
    assert_eq!(doc.project().device(a).unwrap().tag, tag("=A+CAB1+DOOR-X1"));

    doc.undo().unwrap();
    assert_eq!(found(&doc, a).as_deref(), Some("+K1"));
}

#[test]
fn renaming_a_group_leaves_a_shared_panel_alone() {
    let mut doc = Document::default();
    page(&mut doc, "1", "=20D41+BAY1G1");
    page(&mut doc, "1", "=21D21+BAY1G1");

    rename(&mut doc, Aspect::Function, "20D41", "22D41").unwrap();

    let project = doc.project();
    assert_eq!(project.page_groups(), vec![tag("=21D21"), tag("=22D41")]);
    let panels = project.panels();
    assert_eq!(panels.len(), 1);
    assert_eq!(panels[0].pages.len(), 2);
}

#[test]
fn a_frozen_page_keeps_what_was_printed() {
    let mut doc = Document::default();
    let p = page(&mut doc, "1", "=20D41+BAY1G1");
    doc.execute(Command::FreezePage {
        page: p,
        revision: "B".into(),
    })
    .unwrap();

    rename(&mut doc, Aspect::Location, "BAY1G1", "K9").unwrap();

    let page = doc.project().page(p).unwrap().clone();
    assert_eq!(page.structure, tag("=20D41+K9"));
    assert_eq!(page.frozen.unwrap().structure, tag("=20D41+BAY1G1"));
}

#[test]
fn renaming_onto_an_existing_panel_merges_when_nothing_clashes() {
    let mut doc = Document::default();
    page(&mut doc, "1", "=A+K1");
    page(&mut doc, "1", "=B+K2");
    device(&mut doc, "=A+K1-K1");
    device(&mut doc, "=A+K2-K2");

    rename(&mut doc, Aspect::Location, "K1", "K2").unwrap();
    assert_eq!(doc.project().panels().len(), 1);
}

#[test]
fn a_clash_is_refused_and_changes_nothing() {
    let mut doc = Document::default();
    device(&mut doc, "=A+K1-K1");
    device(&mut doc, "=A+K2-K1");
    structure(&mut doc, "+K1", true);
    structure(&mut doc, "+K2", true);

    let err = rename(&mut doc, Aspect::Location, "K1", "K2").unwrap_err();
    assert!(matches!(err, ModelError::Refused { .. }), "{err}");
    assert!(err.to_string().contains("devices"), "{err}");
    assert_eq!(
        doc.project()
            .devices()
            .filter(|d| d.tag == tag("=A+K1-K1"))
            .count(),
        1
    );
}

#[test]
fn structure_nodes_may_not_collide_on_merge() {
    let mut doc = Document::default();
    structure(&mut doc, "+K1", true);
    structure(&mut doc, "+K2", true);
    let err = rename(&mut doc, Aspect::Location, "K1", "K2").unwrap_err();
    assert!(err.to_string().contains("structure nodes"), "{err}");
}

#[test]
fn nonsense_renames_are_refused() {
    let mut doc = Document::default();
    page(&mut doc, "1", "=A+CAB1");
    for (aspect, from, to) in [
        (Aspect::Product, "K1", "K2"),
        (Aspect::Location, "CAB1", "CAB1/DOOR"),
        (Aspect::Location, "CAB1", "bad name"),
    ] {
        let err = rename(&mut doc, aspect, from, to).unwrap_err();
        assert!(
            matches!(err, ModelError::Refused { .. } | ModelError::Tag(_)),
            "{err}"
        );
    }
    let err = doc
        .execute(Command::RenameLevel {
            aspect: Aspect::Location,
            from: levels("CAB1"),
            to: Vec::new(),
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::Refused { .. }));
    assert_eq!(doc.undo_depth(), 1);
}

#[test]
fn a_rename_is_one_undo_step() {
    let mut doc = Document::default();
    let p = page(&mut doc, "1", "=A+K1");
    device(&mut doc, "=A+K1-K1");
    let depth = doc.undo_depth();

    rename(&mut doc, Aspect::Location, "K1", "K7").unwrap();
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(doc.undo_label(), Some("Rename panel"));

    doc.undo().unwrap();
    assert_eq!(doc.project().page(p).unwrap().structure, tag("=A+K1"));
    assert!(doc.project().devices().all(|d| d.tag == tag("=A+K1-K1")));
}
