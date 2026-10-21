//! Issuing a revision in one call: the model command, the session's change
//! record and the snapshot kept in the project file (DESIGN §7). The
//! Issue revision dialog, the command line and scripts call this, so they
//! all propose the same index and keep the same snapshot.

use novo_core::{AnyId, ChangeSet, Command, Document, ModelError, PageId, Project, RevisionId, Tx};
use novo_storage::{Session, StorageError};

use crate::{IndexRule, diff};

/// Why a revision was not issued.
#[derive(Debug, thiserror::Error)]
pub enum IssueError {
    /// The rule has no index after the latest one (it does not follow the
    /// rule, for example a hand-typed `Rev 1`). Type the index instead.
    #[error("no next index follows the latest revision under this rule")]
    NoIndex,
    #[error(transparent)]
    Model(#[from] ModelError),
    /// The edit was made, but the snapshot could not be stored; undo the
    /// issue (the orphan snapshot is dropped on the next open).
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// The revision was issued, but locking the pages failed; lock them
    /// from File > Revisions > Lock page.
    #[error("the revision was issued, but locking the pages failed: {reason}")]
    LockFailed {
        reason: ModelError,
        /// The issue that did happen; its snapshot is kept.
        issued: Box<Issued>,
    },
}

/// What an issue did.
#[derive(Debug, Clone, PartialEq)]
pub struct Issued {
    pub revision: RevisionId,
    pub index: String,
    /// The index of the previous revision when nothing in the model
    /// changed since it: the command line says "no changes since B".
    /// Reissuing is allowed. Generated sheets are not compared here.
    pub unchanged_since: Option<String>,
    /// What the issue changed in the model (the new revision and the
    /// freezes), for views to refresh from.
    pub changes: ChangeSet,
}

/// Issues the next revision: the index comes from `rule`, `date` is
/// `YYYY-MM-DD`, and the project is snapshotted into the session's file.
/// One undo step; the caller refreshes its views from the document as for
/// any edit.
pub fn issue_revision(
    doc: &mut Document,
    session: &mut Session,
    rule: IndexRule,
    date: &str,
    author: &str,
    description: &str,
) -> Result<Issued, IssueError> {
    let index = rule.next(doc.project()).ok_or(IssueError::NoIndex)?;
    issue_revision_as(doc, session, index, date, author, description)
}

/// Like [`issue_revision`] with an index the user typed.
pub fn issue_revision_as(
    doc: &mut Document,
    session: &mut Session,
    index: String,
    date: &str,
    author: &str,
    description: &str,
) -> Result<Issued, IssueError> {
    issue_revision_noted(doc, session, index, date, author, description, "")
}

/// Like [`issue_revision_as`] with a longer note on why (kept as the
/// revision's `note`, see `Revision::note`; blank keeps none).
pub fn issue_revision_noted(
    doc: &mut Document,
    session: &mut Session,
    index: String,
    date: &str,
    author: &str,
    description: &str,
    note: &str,
) -> Result<Issued, IssueError> {
    issue(
        doc,
        session,
        index,
        date,
        author,
        description,
        note,
        false,
        lock_all,
    )
}

/// Like [`issue_revision_noted`], and locks every page with the new
/// revision in the same undo step: the released sheets cannot be edited
/// until unlocked (`Command::UnlockPage`), and the kept snapshot holds the
/// locks, so restoring a revision brings its locks back.
pub fn issue_revision_locked(
    doc: &mut Document,
    session: &mut Session,
    index: String,
    date: &str,
    author: &str,
    description: &str,
    note: &str,
) -> Result<Issued, IssueError> {
    issue(
        doc,
        session,
        index,
        date,
        author,
        description,
        note,
        true,
        lock_all,
    )
}

/// Locks every page in `pages` with `index`.
fn lock_all(tx: &mut Tx<'_>, pages: &[PageId], index: &str) -> Result<(), ModelError> {
    for &page in pages {
        tx.lock_page(page, index)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn issue(
    doc: &mut Document,
    session: &mut Session,
    index: String,
    date: &str,
    author: &str,
    description: &str,
    note: &str,
    lock: bool,
    locker: fn(&mut Tx<'_>, &[PageId], &str) -> Result<(), ModelError>,
) -> Result<Issued, IssueError> {
    let unchanged_since = unchanged_since(doc.project(), session);
    let outcome = doc.execute(Command::IssueRevision {
        index: index.clone(),
        date: date.into(),
        author: author.into(),
        description: description.into(),
        note: note.into(),
    })?;
    session.record(&outcome.changes);
    let mut changes = outcome.changes.clone();
    let mut lock_failed = None;
    if lock {
        let pages: Vec<_> = doc.project().pages().map(|p| p.id).collect();
        match doc.amend_last("Lock released pages", |tx| locker(tx, &pages, &index)) {
            Ok((_, locked)) => {
                session.record(&locked);
                changes.merge(&locked);
            }
            Err(e) => lock_failed = Some(e),
        }
    }
    let revision = outcome
        .created
        .iter()
        .find_map(|id| match id {
            AnyId::Revision(r) => Some(*r),
            _ => None,
        })
        .expect("IssueRevision creates a revision");
    session.put_snapshot(revision, doc.project())?;
    let issued = Issued {
        revision,
        index,
        unchanged_since,
        changes,
    };
    match lock_failed {
        None => Ok(issued),
        Some(reason) => Err(IssueError::LockFailed {
            reason,
            issued: Box::new(issued),
        }),
    }
}

fn unchanged_since(project: &Project, session: &Session) -> Option<String> {
    let latest = project.latest_revision()?;
    let snapshot = session.snapshot(latest.id).ok()??;
    let changes = diff(&snapshot, project);
    // Issuing re-takes freezes, which `diff` already ignores.
    changes.is_empty().then(|| latest.index.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_lock_still_keeps_the_issued_revision_and_its_snapshot() {
        let mut doc = Document::default();
        doc.transact("p", |tx| tx.add_page("1")).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut session = Session::create(dir.path().join("p.novo"), doc.project()).unwrap();
        let refuse: fn(&mut Tx<'_>, &[PageId], &str) -> Result<(), ModelError> = |_, _, _| {
            Err(ModelError::Refused {
                what: "test".into(),
                reason: "no".into(),
            })
        };
        let err = issue(
            &mut doc,
            &mut session,
            "A".into(),
            "2026-10-21",
            "TV",
            "",
            "",
            true,
            refuse,
        )
        .unwrap_err();
        let IssueError::LockFailed { issued, .. } = err else {
            panic!("expected LockFailed");
        };
        assert!(doc.project().issued_revision(issued.revision).is_some());
        assert!(session.snapshot(issued.revision).unwrap().is_some());
        assert!(!issued.changes.is_empty());
    }
}
