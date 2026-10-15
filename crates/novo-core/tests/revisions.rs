//! Revisions: issuing one records it and freezes the drawn pages.

use novo_core::*;

fn issue(doc: &mut Document, index: &str, date: &str) -> Result<RevisionId> {
    let created = doc
        .execute(Command::IssueRevision {
            index: index.into(),
            date: date.into(),
            author: "TV".into(),
            description: "Issue".into(),
            note: String::new(),
        })?
        .created;
    match created[..] {
        [AnyId::Revision(id)] => Ok(id),
        _ => panic!("{created:?}"),
    }
}

#[test]
fn issuing_a_revision_freezes_the_drawn_pages() {
    let mut doc = Document::default();
    let (pages, _) = doc
        .transact("Pages", |tx| {
            let one = tx.add_page("1")?;
            let two = tx.add_page("2")?;
            let report = tx.add_page("20a")?;
            tx.update::<Page>(report, |p| {
                p.kind = PageKind::Generated {
                    generator: "bom".into(),
                }
            })?;
            Ok([one, two, report])
        })
        .unwrap();
    assert!(doc.project().latest_revision().is_none());

    let a = issue(&mut doc, "A", "2026-10-15").unwrap();
    let project = doc.project();
    assert_eq!(project.latest_revision().map(|r| r.id), Some(a));
    assert_eq!(project.revision_by_index("A").unwrap().author, "TV");
    for page in &pages[..2] {
        let frozen = project.page(*page).unwrap().frozen.as_ref().unwrap();
        assert_eq!(frozen.revision, "A");
    }
    // Generated sheets reflow; they are never frozen.
    assert!(project.page(pages[2]).unwrap().frozen.is_none());

    // The next revision re-takes every freeze.
    let b = issue(&mut doc, "B", "2026-10-16").unwrap();
    let project = doc.project();
    assert_eq!(project.latest_revision().map(|r| r.id), Some(b));
    assert_eq!(
        project
            .revisions()
            .map(|r| r.index.as_str())
            .collect::<Vec<_>>(),
        ["A", "B"]
    );
    let frozen = project.page(pages[0]).unwrap().frozen.as_ref().unwrap();
    assert_eq!(frozen.revision, "B");

    // One undo step takes the record and the freezes back.
    doc.undo().unwrap();
    let project = doc.project();
    assert!(project.revision_by_index("B").is_none());
    let frozen = project.page(pages[0]).unwrap().frozen.as_ref().unwrap();
    assert_eq!(frozen.revision, "A");
}

#[test]
fn what_a_revision_needs() {
    let mut doc = Document::default();
    issue(&mut doc, "A", "2026-10-15").unwrap();
    assert_eq!(
        issue(&mut doc, "A", "2026-10-16").unwrap_err(),
        ModelError::DuplicateRevision("A".into())
    );
    for (index, date) in [
        ("", "2026-10-15"),
        (" B", "2026-10-15"),
        ("B", "15-10-2026"),
        ("B", "2026-13-01"),
        ("B", "2026-10-1x"),
    ] {
        assert!(
            matches!(
                issue(&mut doc, index, date),
                Err(ModelError::InvalidValue { .. })
            ),
            "{index:?} {date:?}"
        );
    }
    assert_eq!(doc.project().revisions().count(), 1);
}

#[test]
fn an_issued_revision_is_undone_not_deleted() {
    let mut doc = Document::default();
    let a = issue(&mut doc, "A", "2026-10-15").unwrap();
    assert_eq!(
        doc.execute(Command::Remove { id: a.into() }).unwrap_err(),
        ModelError::IssuedRevision(a)
    );
    assert!(doc.project().issued_revision(a).is_some());
    doc.undo().unwrap();
    assert!(doc.project().issued_revision(a).is_none());
}

#[test]
fn a_longer_note_is_kept_with_the_revision() {
    let mut doc = Document::default();
    let issue_with = |doc: &mut Document, index: &str, note: &str| {
        let created = doc
            .execute(Command::IssueRevision {
                index: index.into(),
                date: "2026-10-15".into(),
                author: "TV".into(),
                description: "Issue".into(),
                note: note.into(),
            })
            .unwrap()
            .created;
        let [AnyId::Revision(id)] = created[..] else {
            panic!("{created:?}")
        };
        id
    };
    let a = issue_with(&mut doc, "A", "Customer asked for a second motor.");
    let b = issue_with(&mut doc, "B", "  ");
    let project = doc.project();
    assert_eq!(
        project.issued_revision(a).unwrap().note(),
        "Customer asked for a second motor."
    );
    assert_eq!(project.issued_revision(b).unwrap().note(), "");
    assert!(project.issued_revision(b).unwrap().properties.is_empty());

    // The note travels with the file; a command without one still parses.
    let back = Project::from_json(&project.to_json()).unwrap();
    assert_eq!(
        back.issued_revision(a).unwrap().note(),
        project.issued_revision(a).unwrap().note()
    );
    let old: Command =
        serde_json::from_str(r#"{"command":"issue_revision","index":"C","date":"2026-10-16"}"#)
            .unwrap();
    assert!(matches!(old, Command::IssueRevision { note, .. } if note.is_empty()));

    // One undo step takes the revision and its note.
    doc.undo().unwrap();
    assert!(doc.project().issued_revision(b).is_none());
    assert_eq!(
        doc.project().issued_revision(a).unwrap().note(),
        "Customer asked for a second motor."
    );
}
