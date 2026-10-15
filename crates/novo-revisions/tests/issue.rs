use novo_core::*;
use novo_revisions::{IndexRule, IssueError, issue_revision, issue_revision_as};
use novo_storage::Session;

fn open(dir: &std::path::Path) -> (Document, Session) {
    let mut doc = Document::default();
    doc.transact("Pages", |tx| tx.add_page("1")).unwrap();
    let session = Session::create(dir.join("p.novo"), doc.project()).unwrap();
    (doc, session)
}

#[test]
fn issuing_proposes_the_index_and_keeps_a_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let (mut doc, mut session) = open(dir.path());

    let a = issue_revision(
        &mut doc,
        &mut session,
        IndexRule::Letters,
        "2026-10-15",
        "TV",
        "First",
    )
    .unwrap();
    assert_eq!(a.index, "A");
    assert_eq!(a.unchanged_since, None);
    let snapshot = session
        .snapshot(a.revision)
        .unwrap()
        .expect("snapshot kept");
    assert!(snapshot.issued_revision(a.revision).is_some());

    // Nothing changed since A; issuing again is allowed and says so.
    let b = issue_revision(
        &mut doc,
        &mut session,
        IndexRule::Letters,
        "2026-10-16",
        "TV",
        "",
    )
    .unwrap();
    assert_eq!(b.index, "B");
    assert_eq!(b.unchanged_since.as_deref(), Some("A"));

    // One undo step takes the revision back.
    doc.undo().unwrap();
    assert!(doc.project().issued_revision(b.revision).is_none());
}

#[test]
fn a_typed_index_and_a_taken_index() {
    let dir = tempfile::tempdir().unwrap();
    let (mut doc, mut session) = open(dir.path());
    let first = issue_revision_as(
        &mut doc,
        &mut session,
        "Rev 1".into(),
        "2026-10-15",
        "TV",
        "",
    )
    .unwrap();
    assert_eq!(first.index, "Rev 1");
    // The rule cannot continue "Rev 1".
    let err = issue_revision(
        &mut doc,
        &mut session,
        IndexRule::Letters,
        "2026-10-16",
        "TV",
        "",
    )
    .unwrap_err();
    assert!(matches!(err, IssueError::NoIndex), "{err}");
    // A duplicate typed index is refused by the model and nothing is kept.
    let err = issue_revision_as(
        &mut doc,
        &mut session,
        "Rev 1".into(),
        "2026-10-16",
        "TV",
        "",
    )
    .unwrap_err();
    assert!(matches!(err, IssueError::Model(_)), "{err}");
    assert_eq!(doc.project().revisions().count(), 1);
}
