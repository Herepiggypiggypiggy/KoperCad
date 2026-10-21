//! Restore: put a project, or some of its pages, back to the state kept
//! with an issued revision. It is one undo step, so the person can take the
//! restore back; the revision history itself is never rewritten.

use std::collections::BTreeSet;

use novo_core::{AnyId, ChangeSet, Document, ModelError, PageId, Project, Record, Tx};

/// What to put back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreScope {
    /// Everything in the project except the revisions themselves.
    Project,
    /// The drawing of these pages: their symbols, junctions, wires, texts
    /// and block instances, and the page settings. Devices, terminals and
    /// parts the restored drawing needs and that were deleted since come
    /// back; ones that still exist stay as they are now.
    Pages(Vec<PageId>),
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq)]
pub struct Restored {
    pub inserted: usize,
    pub replaced: usize,
    pub removed: usize,
    /// For views to refresh from.
    pub changes: ChangeSet,
}

impl Restored {
    /// Whether the restore changed nothing (the state already matched).
    pub fn is_empty(&self) -> bool {
        self.inserted + self.replaced + self.removed == 0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    /// The model refused part of the restore; nothing was changed.
    #[error(transparent)]
    Model(#[from] ModelError),
}

/// What [`restore`] would do, without doing it: the counts a confirmation
/// shows before the person agrees.
pub fn preview(
    doc: &Document,
    snapshot: &Project,
    scope: &RestoreScope,
) -> Result<Restored, RestoreError> {
    restore(&mut doc.clone(), snapshot, scope)
}

enum Step {
    Insert(Record),
    Replace(Record),
}

/// Puts `scope` of `doc` back to `snapshot` (the state kept with a revision).
/// Locked pages are restored too: the lock protects against edits, and a
/// restore is the way back to what was issued. Revisions are never touched.
pub fn restore(
    doc: &mut Document,
    snapshot: &Project,
    scope: &RestoreScope,
) -> Result<Restored, RestoreError> {
    let current = doc.project();
    let in_scope = |record: &Record| -> bool {
        if matches!(record, Record::Revision(_)) {
            return false;
        }
        match scope {
            RestoreScope::Project => true,
            RestoreScope::Pages(pages) => {
                let page = match record {
                    Record::BlockInstance(b) => Some(b.page),
                    other => other.page(),
                };
                page.is_some_and(|p| pages.contains(&p))
            }
        }
    };

    let mut steps: Vec<Step> = Vec::new();
    let mut planned: BTreeSet<AnyId> = BTreeSet::new();
    for record in snapshot.records().filter(|r| in_scope(r)) {
        match current.record(record.id()) {
            None => steps.push(Step::Insert(record)),
            Some(now) if now != record => steps.push(Step::Replace(record)),
            Some(_) => continue,
        }
        planned.insert(steps_id(&steps));
    }
    // Records the restored drawing refers to that no longer exist come back
    // with it, and whatever they refer to in turn.
    let mut at = 0;
    while at < steps.len() {
        let refs = match &steps[at] {
            Step::Insert(r) | Step::Replace(r) => r.refs(),
        };
        for id in refs {
            if !current.contains(id)
                && !planned.contains(&id)
                && let Some(record) = snapshot.record(id)
                && !matches!(record, Record::Revision(_))
            {
                planned.insert(id);
                steps.push(Step::Insert(record));
            }
        }
        at += 1;
    }
    // What was added after the revision goes. Newest kinds first, so a
    // symbol is removed before the device it draws.
    let mut removes: Vec<AnyId> = current
        .records()
        .filter(|r| in_scope(r) && !snapshot.contains(r.id()))
        .map(|r| r.id())
        .collect();
    removes.reverse();
    let info = matches!(scope, RestoreScope::Project) && snapshot.info() != current.info();
    let snapshot_info = snapshot.info().clone();

    let mut counts = (0, 0, 0);
    let ((), changes) = doc.transact("Restore revision", |tx| {
        tx.allow_locked_pages();
        counts = apply(tx, steps, removes)?;
        if info {
            tx.set_info(|i| *i = snapshot_info)?;
        }
        Ok(())
    })?;
    Ok(Restored {
        inserted: counts.0,
        replaced: counts.1,
        removed: counts.2,
        changes,
    })
}

fn steps_id(steps: &[Step]) -> AnyId {
    match steps.last().expect("just pushed") {
        Step::Insert(r) | Step::Replace(r) => r.id(),
    }
}

/// Applies the steps in whatever order the model accepts them: records
/// refer to each other, so one that is refused for a missing or leftover
/// reference waits until the others have gone through.
fn apply(
    tx: &mut Tx<'_>,
    steps: Vec<Step>,
    removes: Vec<AnyId>,
) -> Result<(usize, usize, usize), ModelError> {
    let (mut inserted, mut replaced, mut removed) = (0, 0, 0);
    let mut pending = steps;
    let mut removes = removes;
    loop {
        let mut progress = false;
        let mut waiting = Vec::new();
        let mut last: Option<(AnyId, ModelError)> = None;
        for step in pending {
            let (id, result) = match &step {
                Step::Insert(r) => (r.id(), tx.insert_record(r.clone()).map(|_| ())),
                Step::Replace(r) => (r.id(), tx.replace(r.clone())),
            };
            match result {
                Ok(()) => {
                    progress = true;
                    match step {
                        Step::Insert(_) => inserted += 1,
                        Step::Replace(_) => replaced += 1,
                    }
                }
                Err(e) => {
                    last = Some((id, e));
                    waiting.push(step);
                }
            }
        }
        pending = waiting;
        if pending.is_empty() {
            break;
        }
        if !progress {
            // Something old may be in the way (a terminal with the same
            // name, a record that still refers to what comes back).
            if !removes.is_empty() {
                let id = removes.remove(0);
                if tx.project().contains(id) {
                    tx.remove(id)?;
                    removed += 1;
                }
                continue;
            }
            let (id, source) = last.expect("a step failed");
            return Err(ModelError::Refused {
                what: id.to_string(),
                reason: source.to_string(),
            });
        }
    }
    for id in removes {
        if tx.project().contains(id) {
            tx.remove(id)?;
            removed += 1;
        }
    }
    Ok((inserted, replaced, removed))
}
