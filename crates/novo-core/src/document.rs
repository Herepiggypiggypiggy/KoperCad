//! A project with undo/redo history.

use crate::command::Command;
use crate::error::Result;
use crate::id::AnyId;
use crate::project::Project;
use crate::record::{ChangeSet, Transaction};
use crate::tx::Tx;

/// Default number of undo steps kept.
pub const DEFAULT_HISTORY_LIMIT: usize = 1000;

/// What a committed edit produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    /// Objects the edit created, in creation order (for commands).
    pub created: Vec<AnyId>,
    /// What changed, for refreshing views.
    pub changes: ChangeSet,
}

/// An open project with undo and redo: what the app, the Python API and the
/// command-line tool all edit through.
#[derive(Debug, Clone)]
pub struct Document {
    project: Project,
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    limit: usize,
    /// Undo depth at the last save; `None` once that state is unreachable.
    saved_at: Option<usize>,
}

impl Default for Document {
    fn default() -> Self {
        Document::new(Project::new(""))
    }
}

impl Document {
    /// Opens a project with empty history; it counts as saved.
    pub fn new(project: Project) -> Self {
        Document {
            project,
            undo: Vec::new(),
            redo: Vec::new(),
            limit: DEFAULT_HISTORY_LIMIT,
            saved_at: Some(0),
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn into_project(self) -> Project {
        self.project
    }

    /// Runs `edit` as one undo step. Nothing is recorded if it fails or
    /// changes nothing.
    pub fn transact<R>(
        &mut self,
        label: impl Into<String>,
        edit: impl FnOnce(&mut Tx<'_>) -> Result<R>,
    ) -> Result<(R, ChangeSet)> {
        let (value, transaction) = self.project.transact(label, edit)?;
        let changes = transaction.changes();
        self.push(transaction);
        Ok((value, changes))
    }

    /// Runs one command as one undo step.
    pub fn execute(&mut self, command: Command) -> Result<Outcome> {
        let label = command.label();
        let (created, changes) = self.transact(label, |tx| tx.execute(command))?;
        Ok(Outcome { created, changes })
    }

    fn push(&mut self, transaction: Transaction) {
        if transaction.is_empty() {
            return;
        }
        if self.saved_at.is_some_and(|s| s > self.undo.len()) {
            self.saved_at = None; // the saved state was in the redo branch
        }
        self.redo.clear();
        self.undo.push(transaction);
        if self.undo.len() > self.limit {
            let excess = self.undo.len() - self.limit;
            self.undo.drain(..excess);
            self.saved_at = self.saved_at.and_then(|s| s.checked_sub(excess));
        }
    }

    /// Undoes the last step. Returns what changed, or `None` if there was
    /// nothing to undo.
    pub fn undo(&mut self) -> Result<Option<ChangeSet>> {
        let Some(transaction) = self.undo.pop() else {
            return Ok(None);
        };
        let inverse = transaction.inverse();
        if let Err(err) = self.project.apply(&inverse) {
            self.undo.push(transaction);
            return Err(err);
        }
        self.redo.push(transaction);
        Ok(Some(inverse.changes()))
    }

    /// Redoes the last undone step.
    pub fn redo(&mut self) -> Result<Option<ChangeSet>> {
        let Some(transaction) = self.redo.pop() else {
            return Ok(None);
        };
        if let Err(err) = self.project.apply(&transaction) {
            self.redo.push(transaction);
            return Err(err);
        }
        let changes = transaction.changes();
        self.undo.push(transaction);
        Ok(Some(changes))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Label of the step `undo` would revert.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|t| t.label.as_str())
    }

    /// Label of the step `redo` would reapply.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|t| t.label.as_str())
    }

    /// Number of undo steps available.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Caps the number of undo steps kept (at least one).
    pub fn set_history_limit(&mut self, limit: usize) {
        self.limit = limit.max(1);
        if self.undo.len() > self.limit {
            let excess = self.undo.len() - self.limit;
            self.undo.drain(..excess);
            self.saved_at = self.saved_at.and_then(|s| s.checked_sub(excess));
        }
    }

    /// Drops all undo and redo history.
    pub fn clear_history(&mut self) {
        let dirty = self.is_dirty();
        self.undo.clear();
        self.redo.clear();
        self.saved_at = if dirty { None } else { Some(0) };
    }

    /// Records that the current state has been saved.
    pub fn mark_saved(&mut self) {
        self.saved_at = Some(self.undo.len());
    }

    /// Whether the project differs from the last saved state.
    pub fn is_dirty(&self) -> bool {
        self.saved_at != Some(self.undo.len())
    }
}
