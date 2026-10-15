//! Issuing a revision in one call: the model command, the session's change
//! record and the snapshot kept in the project file (DESIGN §7). The
//! Issue revision dialog, the command line and scripts call this, so they
//! all propose the same index and keep the same snapshot.

use novo_core::{AnyId, ChangeSet, Command, Document, ModelError, Project, RevisionId};
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
    let unchanged_since = unchanged_since(doc.project(), session);
    let outcome = doc.execute(Command::IssueRevision {
        index: index.clone(),
        date: date.into(),
        author: author.into(),
        description: description.into(),
    })?;
    session.record(&outcome.changes);
    let changes = outcome.changes.clone();
    let revision = outcome
        .created
        .iter()
        .find_map(|id| match id {
            AnyId::Revision(r) => Some(*r),
            _ => None,
        })
        .expect("IssueRevision creates a revision");
    session.put_snapshot(revision, doc.project())?;
    Ok(Issued {
        revision,
        index,
        unchanged_since,
        changes,
    })
}

fn unchanged_since(project: &Project, session: &Session) -> Option<String> {
    let latest = project.latest_revision()?;
    let snapshot = session.snapshot(latest.id).ok()??;
    let changes = diff(&snapshot, project);
    // Issuing re-takes freezes, which `diff` already ignores.
    changes.is_empty().then(|| latest.index.clone())
}
