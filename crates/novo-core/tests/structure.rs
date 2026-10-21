//! Structure nodes (plant sections, locations, enclosures) and dismissed
//! proposals.

use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

#[test]
fn enclosures_are_found_from_a_device_tag() {
    let mut doc = Document::default();
    doc.transact("Structure", |tx| {
        tx.add_structure(tag("+CAB1"), "Main cabinet", true)?;
        tx.add_structure(tag("=A2+CAB1"), "Section A2 side of the cabinet", true)?;
        tx.add_structure(tag("+FIELD"), "Machine", false)?;
        Ok(())
    })
    .unwrap();
    let project = doc.project();
    let found = |text: &str| project.enclosure_of(&tag(text)).map(|s| s.tag.to_string());
    assert_eq!(found("+CAB1-X1").as_deref(), Some("+CAB1"));
    // A sub-location is inside its parent enclosure.
    assert_eq!(found("+CAB1+DOOR-S1").as_deref(), Some("+CAB1"));
    // The same function wins over a function-less node.
    assert_eq!(found("=A2+CAB1-K1").as_deref(), Some("=A2+CAB1"));
    assert_eq!(found("=A3+CAB1-K1").as_deref(), Some("+CAB1"));
    // Not an enclosure, or not described at all: field side.
    assert_eq!(found("+FIELD-M1"), None);
    assert_eq!(found("+JB7-X1"), None);
    assert_eq!(found("-K1"), None);

    // Tags are unique, and a node has no product aspect.
    let err = doc
        .transact("Twice", |tx| tx.add_structure(tag("+CAB1"), "", false))
        .unwrap_err();
    assert!(matches!(err, ModelError::DuplicateStructure(_)));
    let err = doc
        .transact("Product", |tx| tx.add_structure(tag("+CAB2-K1"), "", false))
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidValue { field: "tag", .. }));

    let json = doc.project().to_json();
    assert_eq!(json.matches("\"enclosure\": true").count(), 2);
    let loaded = Project::from_json(&json).unwrap();
    assert_eq!(loaded.structures().count(), 3);
}

#[test]
fn dismissals_stick_and_go_with_their_subjects() {
    let mut doc = Document::default();
    let ((a, b), _) = doc
        .transact("Two symbols", |tx| {
            let page = tx.add_page("1")?;
            let mut place = |x: f64| {
                tx.place_symbol(
                    page,
                    "x",
                    Placement::at(Point::new(x, 0.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Unassigned,
                )
            };
            Ok((place(0.0)?, place(20.0)?))
        })
        .unwrap();
    let subjects = vec![AnyId::from(b), AnyId::from(a)];
    let created = doc
        .execute(Command::Dismiss {
            proposal: "connect.facing".into(),
            subjects: subjects.clone(),
        })
        .unwrap()
        .created;
    let [AnyId::Dismissal(dismissal)] = created[..] else {
        panic!("{created:?}")
    };
    let project = doc.project();
    assert!(project.is_dismissed("connect.facing", &[a.into(), b.into()]));
    assert!(!project.is_dismissed("connect.facing", &[a.into()]));
    assert!(!project.is_dismissed("terminal.suggest", &subjects));
    // Dismissing again finds the same record.
    let (again, _) = doc
        .transact("Again", |tx| tx.dismiss("connect.facing", subjects.clone()))
        .unwrap();
    assert_eq!(again, dismissal);
    assert_eq!(doc.project().dismissals().count(), 1);

    // It survives a save and goes when a subject is deleted.
    let loaded = Project::from_json(&doc.project().to_json()).unwrap();
    assert!(loaded.is_dismissed("connect.facing", &subjects));
    doc.transact("Delete", |tx| tx.remove(a)).unwrap();
    assert_eq!(doc.project().dismissals().count(), 0);
    doc.undo().unwrap();
    assert!(doc.project().is_dismissed("connect.facing", &subjects));
}

#[test]
fn a_plain_location_becomes_the_cabinet_without_a_second_node() {
    let mut doc = Document::default();
    let (id, _) = doc
        .transact("Import", |tx| tx.add_structure(tag("+K1"), "", false))
        .unwrap();
    assert!(doc.project().enclosure_of(&tag("+K1-X1")).is_none());
    doc.transact("Create cabinet", |tx| tx.set_enclosure(id, true))
        .unwrap();
    assert_eq!(doc.project().structures().count(), 1);
    assert!(doc.project().enclosure_of(&tag("+K1-X1")).is_some());
    // A plate keeps the node a cabinet.
    doc.transact("Plate", |tx| {
        tx.set_plate(
            id,
            Some(MountingPlate {
                width: 600.0,
                height: 800.0,
                margin: 0.0,
            }),
        )
    })
    .unwrap();
    assert!(
        doc.transact("Off", |tx| tx.set_enclosure(id, false))
            .is_err()
    );
}

#[test]
fn set_enclosure_on_an_unknown_node_is_not_found() {
    let mut doc = Document::default();
    let err = doc
        .transact("Create cabinet", |tx| {
            tx.set_enclosure(StructureId(9999), true)
        })
        .unwrap_err();
    assert_eq!(err, ModelError::NotFound(StructureId(9999).into()));
}
