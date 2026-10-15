use novo_core::*;
use novo_revisions::{
    ChangeKind, CompareError, Group, IndexRule, Side, compare, group_of, issue_revision,
};
use novo_storage::Session;

fn pins() -> Vec<Pin> {
    vec![Pin::new("A1", 0.0, 0.0), Pin::new("A2", 0.0, 10.0)]
}

struct Drawn {
    page: PageId,
    k1: DeviceId,
    coil: SymbolId,
    lamp: SymbolId,
    wire: WireId,
}

fn draw(doc: &mut Document) -> Drawn {
    doc.transact("Draw", |tx| {
        let page = tx.add_page("1")?;
        let k1 = tx.add_device(DeviceTag::parse("-K1").unwrap(), "Relay")?;
        let role = SymbolRole::Device {
            device: k1,
            main: true,
        };
        let coil = tx.place_symbol(page, "coil", Placement::default(), pins(), role)?;
        let lamp = tx.place_symbol(
            page,
            "lamp",
            Placement::at(Point::new(40.0, 0.0)),
            pins(),
            SymbolRole::Unassigned,
        )?;
        let wire = tx.connect(Endpoint::pin(coil, 0), Endpoint::pin(lamp, 0))?;
        Ok(Drawn {
            page,
            k1,
            coil,
            lamp,
            wire,
        })
    })
    .unwrap()
    .0
}

fn issue(doc: &mut Document, session: &mut Session, note: &str) -> RevisionId {
    issue_revision(doc, session, IndexRule::Letters, "2026-10-15", "TV", note)
        .unwrap()
        .revision
}

#[test]
fn any_two_revisions_compare_with_readable_values() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Document::default();
    let d = draw(&mut doc);
    let mut session = Session::create(dir.path().join("p.novo"), doc.project()).unwrap();
    let a = issue(&mut doc, &mut session, "First");

    doc.transact("Edit", |tx| {
        tx.update::<Device>(d.k1, |x| x.description = "Pump relay".into())?;
        tx.move_symbol(d.lamp, Placement::at(Point::new(80.0, 10.0)))?;
        Ok(())
    })
    .unwrap();
    let b = issue(&mut doc, &mut session, "Second");

    doc.transact("More", |tx| {
        tx.remove(d.wire)?;
        tx.remove(d.k1)?;
        Ok(())
    })
    .unwrap();
    let _ = (d.coil, d.page);

    let snap = |id| session.snapshot(id);
    let ab = compare(doc.project(), snap, Side::Revision(a), Side::Revision(b)).unwrap();
    let device = ab
        .rows
        .iter()
        .find(|r| r.label == "Device -K1")
        .expect("device row");
    assert_eq!(device.fields.len(), 1);
    assert_eq!(device.fields[0].name, "description");
    assert_eq!(device.fields[0].old.as_deref(), Some("Relay"));
    assert_eq!(device.fields[0].new.as_deref(), Some("Pump relay"));
    let lamp = ab
        .rows
        .iter()
        .find(|r| matches!(r.id, AnyId::Symbol(s) if s == d.lamp))
        .expect("lamp row");
    let moved = lamp.fields.iter().find(|f| f.name == "position").unwrap();
    assert_eq!(moved.old.as_deref(), Some("40, 0 mm"));
    assert_eq!(moved.new.as_deref(), Some("80, 10 mm"));
    assert_eq!(moved.note.as_deref(), Some("moved 40 mm right, 10 mm down"));

    // B against now: the wire and the device are gone; their rows read
    // from the older state, and the unnumbered wire is named by its ends.
    let snap = |id| session.snapshot(id);
    let bn = compare(doc.project(), snap, Side::Revision(b), Side::Now).unwrap();
    let labels: Vec<_> = bn
        .rows
        .iter()
        .filter(|r| r.kind == ChangeKind::Removed)
        .map(|r| r.label.as_str())
        .collect();
    assert!(labels.contains(&"Device -K1"), "{labels:?}");
    assert!(labels.contains(&"Wire -K1:A1 to lamp:A1"), "{labels:?}");
    assert_eq!(group_of(d.wire.into()), Group::Wire);

    // The other way round, removed becomes added.
    let snap = |id| session.snapshot(id);
    let nb = compare(doc.project(), snap, Side::Now, Side::Revision(b)).unwrap();
    assert!(
        nb.rows
            .iter()
            .any(|r| r.label == "Device -K1" && r.kind == ChangeKind::Added)
    );

    // Same side twice: nothing differs.
    let snap = |id| session.snapshot(id);
    let same = compare(doc.project(), snap, Side::Revision(a), Side::Revision(a)).unwrap();
    assert!(same.is_empty());
}

#[test]
fn project_info_shows_old_and_new() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Document::default();
    draw(&mut doc);
    let mut session = Session::create(dir.path().join("p.novo"), doc.project()).unwrap();
    let a = issue(&mut doc, &mut session, "First");
    doc.transact("Rename", |tx| {
        tx.set_info(|i| i.name = "Packaging line".into())
    })
    .unwrap();
    let snap = |id| session.snapshot(id);
    let c = compare(doc.project(), snap, Side::Revision(a), Side::Now).unwrap();
    let name = c.info.iter().find(|f| f.name == "name").expect("name");
    assert_eq!(name.new.as_deref(), Some("Packaging line"));
}

#[test]
fn a_missing_or_unknown_revision_is_an_error() {
    let mut doc = Document::default();
    draw(&mut doc);
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::create(dir.path().join("p.novo"), doc.project()).unwrap();
    let a = issue(&mut doc, &mut session, "First");

    let none = |_| Ok::<_, String>(None);
    let err = compare(doc.project(), none, Side::Revision(a), Side::Now).unwrap_err();
    assert!(matches!(err, CompareError::NoSnapshot { ref index } if index == "A"));

    let broken = |_| Err::<Option<Project>, _>("bad zlib stream".to_owned());
    let err = compare(doc.project(), broken, Side::Now, Side::Revision(a)).unwrap_err();
    assert!(matches!(err, CompareError::Unreadable { ref reason, .. } if reason.contains("zlib")));

    let unknown = RevisionId::from(9999_u64);
    let err = compare(doc.project(), none, Side::Revision(unknown), Side::Now).unwrap_err();
    assert!(matches!(err, CompareError::UnknownRevision(_)));
}
