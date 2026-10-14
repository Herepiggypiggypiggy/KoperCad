//! The hook between the shell and the project file.
//!
//! The open project is a [`novo_core::Document`]. A [`ProjectBackend`] owns it
//! and moves it to and from disk. Every edit's [`ChangeSet`] must reach
//! [`ProjectBackend::record`] so saves and autosaves include it; the shell
//! does that for undo and redo, and [`crate::OpenPage`] does it for the
//! canvas.
//!
//! [`StorageBackend`] keeps the project in a `.novo` file through
//! `novo-storage`; [`MemoryBackend`] keeps it in memory, for tests.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use novo_core::{ChangeSet, Document, ModelError, PageId, Project};
use novo_storage::{Session, StorageError};

#[derive(Debug)]
pub enum BackendError {
    /// The feature is not built yet.
    Unavailable(&'static str),
    Model(ModelError),
    Storage(StorageError),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::Unavailable(what) => write!(f, "{what} is not available yet"),
            BackendError::Model(e) => write!(f, "{e}"),
            BackendError::Storage(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BackendError {}

impl From<ModelError> for BackendError {
    fn from(e: ModelError) -> Self {
        BackendError::Model(e)
    }
}

impl From<StorageError> for BackendError {
    fn from(e: StorageError) -> Self {
        BackendError::Storage(e)
    }
}

/// Unsaved work found when a project was opened after a crash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryOffer {
    pub autosaved_at: Option<SystemTime>,
}

/// Things worth telling the user after a project opened.
#[derive(Debug, Default)]
pub struct OpenReport {
    /// A damaged recovery file was kept here instead of being deleted.
    pub set_aside: Option<PathBuf>,
}

/// Owns the open project and moves it to and from disk.
///
/// At most one project is open at a time.
pub trait ProjectBackend {
    /// Starts a new, unsaved project, replacing any open one.
    fn new_project(&mut self, name: &str) -> Result<(), BackendError>;

    /// Opens a project file, replacing any open project.
    fn open(&mut self, path: &Path) -> Result<OpenReport, BackendError>;

    /// Writes the open project to `path` and clears the unsaved flag.
    fn save(&mut self, path: &Path) -> Result<(), BackendError>;

    /// Closes the open project, dropping unsaved changes.
    fn close(&mut self) -> Result<(), BackendError>;

    fn document(&self) -> Option<&Document>;

    /// For edits only. Pass every resulting [`ChangeSet`] to [`Self::record`].
    fn document_mut(&mut self) -> Option<&mut Document>;

    /// Notes an edit so the next save and autosave include it.
    fn record(&mut self, _changes: &ChangeSet) {}

    /// True when the project has changes that are not saved.
    fn is_dirty(&self) -> bool {
        self.document().is_some_and(|d| d.is_dirty())
    }

    /// Unsaved work from a crash, waiting for [`Self::resolve_recovery`].
    fn recovery_offer(&self) -> Option<RecoveryOffer> {
        None
    }

    /// Restores (`true`) or throws away (`false`) the offered recovery.
    fn resolve_recovery(&mut self, _restore: bool) -> Result<(), BackendError> {
        Ok(())
    }

    /// Called every frame: autosave when one is due. Returns when to call
    /// again at the latest, so autosave also runs while the app is idle.
    fn tick(&mut self) -> Result<Option<Duration>, BackendError> {
        Ok(None)
    }
}

/// The real backend: one `.novo` file per project, autosaved to a recovery
/// file next to it.
#[derive(Default)]
pub struct StorageBackend {
    document: Option<Document>,
    /// `None` until a new project is first saved.
    session: Option<Session>,
    recovery: Option<novo_storage::Recovered>,
    /// The project shown is a restored recovery that differs from the file.
    restored_unsaved: bool,
}

impl StorageBackend {
    fn close_session(&mut self) -> Result<(), BackendError> {
        self.recovery = None;
        self.restored_unsaved = false;
        match self.session.take() {
            Some(session) => Ok(session.close()?),
            None => Ok(()),
        }
    }
}

impl ProjectBackend for StorageBackend {
    fn new_project(&mut self, name: &str) -> Result<(), BackendError> {
        let document = new_document(name)?;
        self.close_session()?;
        self.document = Some(document);
        Ok(())
    }

    fn open(&mut self, path: &Path) -> Result<OpenReport, BackendError> {
        let opened = Session::open(path)?;
        self.close_session()?;
        self.document = Some(Document::new(opened.project));
        self.session = Some(opened.session);
        self.recovery = opened.recovered;
        Ok(OpenReport {
            set_aside: opened.set_aside.map(|s| s.path),
        })
    }

    fn save(&mut self, path: &Path) -> Result<(), BackendError> {
        let Some(document) = self.document.as_mut() else {
            return Ok(());
        };
        match self.session.as_mut() {
            Some(session) if session.path() == path => session.save(document.project())?,
            Some(session) => session.save_as(path, document.project())?,
            None => self.session = Some(Session::create(path, document.project())?),
        }
        document.mark_saved();
        self.restored_unsaved = false;
        Ok(())
    }

    fn close(&mut self) -> Result<(), BackendError> {
        self.document = None;
        self.close_session()
    }

    fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    fn document_mut(&mut self) -> Option<&mut Document> {
        self.document.as_mut()
    }

    fn record(&mut self, changes: &ChangeSet) {
        if let Some(session) = &mut self.session {
            session.record(changes);
        }
    }

    fn is_dirty(&self) -> bool {
        self.restored_unsaved || self.document().is_some_and(|d| d.is_dirty())
    }

    fn recovery_offer(&self) -> Option<RecoveryOffer> {
        self.recovery.as_ref().map(|r| RecoveryOffer {
            autosaved_at: r.autosaved_at,
        })
    }

    fn resolve_recovery(&mut self, restore: bool) -> Result<(), BackendError> {
        let (Some(recovered), Some(session)) = (self.recovery.take(), self.session.as_mut()) else {
            return Ok(());
        };
        if restore {
            session.accept_recovery();
            self.document = Some(Document::new(recovered.project));
            self.restored_unsaved = true;
        } else {
            session.discard_recovery()?;
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<Option<Duration>, BackendError> {
        let (Some(session), Some(document)) = (self.session.as_mut(), self.document.as_ref())
        else {
            return Ok(None);
        };
        session.autosave_if_due(document.project())?;
        Ok(Some(session.autosave_interval()))
    }
}

impl Drop for StorageBackend {
    /// Closing cleanly deletes the recovery file; only a crash leaves it.
    fn drop(&mut self) {
        if let Err(e) = self.close_session() {
            log::warn!("could not close the project cleanly: {e}");
        }
    }
}

/// Keeps projects in memory and cannot read or write files.
#[derive(Default)]
pub struct MemoryBackend {
    document: Option<Document>,
}

impl ProjectBackend for MemoryBackend {
    fn new_project(&mut self, name: &str) -> Result<(), BackendError> {
        self.document = Some(new_document(name)?);
        Ok(())
    }

    fn open(&mut self, _path: &Path) -> Result<OpenReport, BackendError> {
        Err(BackendError::Unavailable("Opening project files"))
    }

    fn save(&mut self, _path: &Path) -> Result<(), BackendError> {
        Err(BackendError::Unavailable("Saving project files"))
    }

    fn close(&mut self) -> Result<(), BackendError> {
        self.document = None;
        Ok(())
    }

    fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    fn document_mut(&mut self) -> Option<&mut Document> {
        self.document.as_mut()
    }
}

/// A fresh project with one empty page, saved and with no undo history.
pub fn new_document(name: &str) -> Result<Document, ModelError> {
    let mut project = Project::new(name);
    project.transact("New project", |tx| tx.add_page("1"))?;
    Ok(Document::new(project))
}

/// Pages in display order, with the label the project panel shows.
pub fn page_labels(document: &Document) -> Vec<(PageId, String)> {
    document
        .project()
        .pages_in_order()
        .into_iter()
        .map(|p| {
            let label = if p.title.is_empty() {
                p.name.clone()
            } else {
                format!("{} {}", p.name, p.title)
            };
            (p.id, label)
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use novo_storage::autosave_path;

    fn edit(backend: &mut impl ProjectBackend, page: &str) {
        let doc = backend.document_mut().unwrap();
        let (_, changes) = doc.transact("Add page", |tx| tx.add_page(page)).unwrap();
        backend.record(&changes);
    }

    fn page_names(backend: &impl ProjectBackend) -> Vec<String> {
        let doc = backend.document().unwrap();
        doc.project()
            .pages_in_order()
            .into_iter()
            .map(|p| p.name.clone())
            .collect()
    }

    #[test]
    fn new_document_has_one_page_and_is_clean() {
        let doc = new_document("Plant").unwrap();
        assert_eq!(doc.project().info().name, "Plant");
        assert_eq!(page_labels(&doc).len(), 1);
        assert!(!doc.is_dirty());
        assert!(!doc.can_undo());
    }

    #[test]
    fn save_reopen_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");

        let mut backend = StorageBackend::default();
        backend.new_project("Plant").unwrap();
        edit(&mut backend, "2");
        assert!(backend.is_dirty());
        backend.save(&path).unwrap();
        assert!(!backend.is_dirty());

        // Later edits go through the session's incremental save.
        edit(&mut backend, "3");
        backend.save(&path).unwrap();
        backend.close().unwrap();

        let mut reopened = StorageBackend::default();
        let report = reopened.open(&path).unwrap();
        assert!(report.set_aside.is_none());
        assert!(reopened.recovery_offer().is_none());
        assert_eq!(page_names(&reopened), vec!["1", "2", "3"]);
        assert_eq!(reopened.document().unwrap().project().info().name, "Plant");
    }

    #[test]
    fn save_as_moves_to_the_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("a.novo");
        let second = dir.path().join("b.novo");

        let mut backend = StorageBackend::default();
        backend.new_project("Plant").unwrap();
        backend.save(&first).unwrap();
        edit(&mut backend, "2");
        backend.save(&second).unwrap();
        backend.close().unwrap();

        let mut a = StorageBackend::default();
        a.open(&first).unwrap();
        assert_eq!(page_names(&a), vec!["1"]);
        let mut b = StorageBackend::default();
        b.open(&second).unwrap();
        assert_eq!(page_names(&b), vec!["1", "2"]);
    }

    /// Leaves `path` with a saved project of pages 1 and 2 plus an autosave
    /// holding page 3, as a crash after autosaving would.
    pub(crate) fn crash_after_autosave(path: &Path) {
        let mut backend = StorageBackend::default();
        backend.new_project("Plant").unwrap();
        edit(&mut backend, "2");
        backend.save(path).unwrap();
        edit(&mut backend, "3");
        let session = backend.session.as_mut().unwrap();
        session
            .autosave(backend.document.as_ref().unwrap().project())
            .unwrap();
        // A crash: the session goes away without close(), so the recovery
        // file stays.
        drop(backend.session.take());
    }

    #[test]
    fn recovery_can_be_restored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        crash_after_autosave(&path);
        assert!(autosave_path(&path).exists());

        let mut backend = StorageBackend::default();
        backend.open(&path).unwrap();
        assert!(backend.recovery_offer().is_some());
        assert_eq!(page_names(&backend), vec!["1", "2"]);

        backend.resolve_recovery(true).unwrap();
        assert!(backend.recovery_offer().is_none());
        assert_eq!(page_names(&backend), vec!["1", "2", "3"]);
        assert!(backend.is_dirty(), "restored work is not in the file yet");

        backend.save(&path).unwrap();
        assert!(!backend.is_dirty());
        backend.close().unwrap();
        assert!(!autosave_path(&path).exists());

        let mut reopened = StorageBackend::default();
        reopened.open(&path).unwrap();
        assert_eq!(page_names(&reopened), vec!["1", "2", "3"]);
    }

    #[test]
    fn recovery_can_be_discarded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        crash_after_autosave(&path);

        let mut backend = StorageBackend::default();
        backend.open(&path).unwrap();
        backend.resolve_recovery(false).unwrap();
        assert_eq!(page_names(&backend), vec!["1", "2"]);
        assert!(!backend.is_dirty());
        assert!(!autosave_path(&path).exists());
    }

    #[test]
    fn a_second_window_cannot_open_the_same_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        let mut first = StorageBackend::default();
        first.new_project("Plant").unwrap();
        first.save(&path).unwrap();

        let mut second = StorageBackend::default();
        let err = second.open(&path).unwrap_err();
        assert!(
            matches!(err, BackendError::Storage(StorageError::Locked(_))),
            "{err}"
        );
    }
}
