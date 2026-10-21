use novo_core::*;
use novo_revisions::{IndexRule, RestoreScope, issue_revision, issue_revision_locked, restore};
use novo_storage::Session;

struct Fixture {
    doc: Document,
    session: Session,
    page1: PageId,
    page2: PageId,
    _dir: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let mut doc = Document::default();
    let (page1, page2) = doc
        .transact("Pages", |tx| Ok((tx.add_page("1")?, tx.add_page("2")?)))
        .unwrap()
        .0;
    let dir = tempfile::tempdir().unwrap();
    let session = Session::create(dir.path().join("p.novo"), doc.project()).unwrap();
    Fixture {
        doc,
        session,
        page1,
        page2,
        _dir: dir,
    }
}

fn text(doc: &mut Document, page: PageId, s: &str) -> TextId {
    doc.execute(Command::AddText {
        page,
        content: TextContent::Literal { text: s.into() },
        placement: Default::default(),
        attached_to: None,
    })
    .map(|o| match o.created[0] {
        AnyId::Text(t) => t,
        _ => unreachable!(),
    })
    .unwrap()
}

#[test]
fn a_locked_page_refuses_edits_until_unlocked() {
    let mut f = fixture();
    let issued = issue_revision_locked(
        &mut f.doc,
        &mut f.session,
        "A".into(),
        "2026-10-21",
        "TV",
        "First",
        "",
    )
    .unwrap();
    assert_eq!(issued.index, "A");
    assert_eq!(
        f.doc.project().page(f.page1).unwrap().locked.as_deref(),
        Some("A")
    );

    let err = f
        .doc
        .execute(Command::AddText {
            page: f.page1,
            content: TextContent::Literal {
                text: "late edit".into(),
            },
            placement: Default::default(),
            attached_to: None,
        })
        .unwrap_err();
    assert!(err.to_string().contains("locked with revision A"), "{err}");
    assert!(
        f.doc.project().texts().next().is_none(),
        "nothing was added"
    );
    assert!(
        f.doc
            .execute(Command::RenamePage {
                page: f.page1,
                name: "x".into()
            })
            .is_err()
    );

    // A new page is open; unlocking opens the old one; the whole step undoes.
    let new_page = f.doc.transact("p", |tx| tx.add_page("3")).unwrap().0;
    text(&mut f.doc, new_page, "fine");
    // Work elsewhere in the project goes on: devices, new pages.
    f.doc
        .transact("d", |tx| {
            tx.add_device(DeviceTag::parse("-Q9").unwrap(), "added after the issue")
        })
        .unwrap();
    f.doc
        .execute(Command::UnlockPage { page: f.page1 })
        .unwrap();
    text(&mut f.doc, f.page1, "now allowed");
}

#[test]
fn issuing_the_next_revision_over_locked_pages_is_allowed() {
    let mut f = fixture();
    for index in ["A", "B"] {
        issue_revision_locked(
            &mut f.doc,
            &mut f.session,
            index.into(),
            "2026-10-21",
            "TV",
            "",
            "",
        )
        .unwrap();
    }
    assert_eq!(
        f.doc.project().page(f.page2).unwrap().locked.as_deref(),
        Some("B")
    );
}

#[test]
fn restoring_a_page_brings_back_the_issued_drawing_and_is_one_undo_step() {
    let mut f = fixture();
    let kept = text(&mut f.doc, f.page1, "kept");
    let edited = text(&mut f.doc, f.page1, "to be edited");
    let issued = issue_revision(
        &mut f.doc,
        &mut f.session,
        IndexRule::Letters,
        "2026-10-21",
        "TV",
        "",
    )
    .unwrap();
    let snapshot = f.session.snapshot(issued.revision).unwrap().unwrap();
    let before_texts = f.doc.project().texts().count();

    // After A: edit one text, delete one, add one, and change page 2.
    f.doc
        .transact("edit", |tx| {
            tx.update::<Text>(edited, |t| {
                t.content = TextContent::Literal {
                    text: "changed".into(),
                }
            })
        })
        .unwrap();
    f.doc.execute(Command::Remove { id: kept.into() }).unwrap();
    let added = text(&mut f.doc, f.page1, "added later");
    let other = text(&mut f.doc, f.page2, "page two");

    let restored = restore(&mut f.doc, &snapshot, &RestoreScope::Pages(vec![f.page1])).unwrap();
    assert_eq!(
        (restored.inserted, restored.replaced, restored.removed),
        (1, 1, 1)
    );
    let project = f.doc.project();
    assert_eq!(
        project.texts().count(),
        before_texts + 1,
        "page two keeps its text"
    );
    assert!(project.text(kept).is_some());
    assert_eq!(project.text(edited), snapshot.text(edited));
    assert!(project.text(added).is_none());
    assert!(project.text(other).is_some(), "other pages are untouched");

    // Restoring again changes nothing; undo takes the restore back.
    let again = restore(&mut f.doc, &snapshot, &RestoreScope::Pages(vec![f.page1])).unwrap();
    assert!(again.is_empty());
    f.doc.undo().unwrap();
    assert!(f.doc.project().text(added).is_some());
}

#[test]
fn restoring_the_project_keeps_the_revision_history_and_works_on_locked_pages() {
    let mut f = fixture();
    let a = issue_revision_locked(
        &mut f.doc,
        &mut f.session,
        "A".into(),
        "2026-10-21",
        "TV",
        "",
        "",
    )
    .unwrap();
    let snapshot = f.session.snapshot(a.revision).unwrap().unwrap();
    f.doc
        .execute(Command::UnlockPage { page: f.page1 })
        .unwrap();
    text(&mut f.doc, f.page1, "after A");
    f.doc
        .execute(Command::SetProjectInfo {
            info: ProjectInfo {
                name: "renamed".into(),
                ..f.doc.project().info().clone()
            },
        })
        .unwrap();
    issue_revision(
        &mut f.doc,
        &mut f.session,
        IndexRule::Letters,
        "2026-10-22",
        "TV",
        "",
    )
    .unwrap();

    let restored = restore(&mut f.doc, &snapshot, &RestoreScope::Project).unwrap();
    assert!(!restored.is_empty());
    let project = f.doc.project();
    assert_eq!(project.revisions().count(), 2, "history is not rewritten");
    assert!(project.texts().next().is_none());
    assert_eq!(project.info().name, snapshot.info().name);
    assert_eq!(project.page(f.page1).unwrap().locked.as_deref(), Some("A"));
}

#[test]
fn restoring_the_project_brings_back_a_deleted_device() {
    let mut f = fixture();
    let device = f
        .doc
        .transact("d", |tx| {
            tx.add_device(DeviceTag::parse("-Q1").unwrap(), "breaker")
        })
        .unwrap()
        .0;
    let issued = issue_revision(
        &mut f.doc,
        &mut f.session,
        IndexRule::Letters,
        "2026-10-21",
        "TV",
        "",
    )
    .unwrap();
    let snapshot = f.session.snapshot(issued.revision).unwrap().unwrap();
    f.doc
        .execute(Command::Remove { id: device.into() })
        .unwrap();
    let restored = restore(&mut f.doc, &snapshot, &RestoreScope::Project).unwrap();
    assert_eq!(restored.inserted, 1);
    assert!(f.doc.project().device(device).is_some());
}

#[test]
fn a_preview_counts_what_a_restore_would_do_and_changes_nothing() {
    let mut f = fixture();
    let issued = issue_revision(
        &mut f.doc,
        &mut f.session,
        IndexRule::Letters,
        "2026-10-21",
        "TV",
        "",
    )
    .unwrap();
    let snapshot = f.session.snapshot(issued.revision).unwrap().unwrap();
    text(&mut f.doc, f.page1, "added later");
    let before = f.doc.project().clone();
    let p =
        novo_revisions::preview(&f.doc, &snapshot, &RestoreScope::Pages(vec![f.page1])).unwrap();
    assert_eq!((p.inserted, p.replaced, p.removed), (0, 0, 1));
    assert_eq!(f.doc.project().revision(), before.revision());
    assert_eq!(f.doc.project().texts().count(), before.texts().count());
}
