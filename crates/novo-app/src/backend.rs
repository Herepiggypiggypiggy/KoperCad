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
    /// The open project could not be written before closing it, so it was
    /// left open (see [`ProjectBackend::close`]).
    NotWritten(StorageError),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::Unavailable(what) => write!(f, "{what} is not available yet"),
            BackendError::Model(e) => write!(f, "{e}"),
            BackendError::Storage(e) => write!(f, "{e}"),
            BackendError::NotWritten(e) => {
                write!(
                    f,
                    "the open project could not be written ({e}), so it stays open"
                )
            }
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
    /// Starts a new, unsaved project, replacing any open one. Fails, leaving
    /// the open project as it was, if that one cannot be written first.
    fn new_project(&mut self, name: &str) -> Result<(), BackendError>;

    /// Opens a project file, replacing any open project. A file from
    /// [`Self::unsaved_projects`] opens as an untitled project. Fails,
    /// leaving the open project as it was, if that one cannot be written
    /// first.
    fn open(&mut self, path: &Path) -> Result<OpenReport, BackendError>;

    /// Writes the open project to `path` and clears the unsaved flag.
    fn save(&mut self, path: &Path) -> Result<(), BackendError>;

    /// Closes the open project without losing work: a saved project is
    /// written to its file first, and an untitled one with work is kept
    /// among [`Self::unsaved_projects`]. If that write fails, the project
    /// stays open (still bound to its file) and the error is returned.
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

    /// Writes a saved project's changes to its file, and autosaves an
    /// untitled one when due. Returns when to call again at the latest, so
    /// this also runs while the app is idle.
    fn tick(&mut self) -> Result<Option<Duration>, BackendError> {
        Ok(None)
    }

    /// The file holding the open project, temporary ones included.
    fn file(&self) -> Option<PathBuf> {
        None
    }

    /// The open project has never been saved under a name, such as one
    /// opened from [`Self::unsaved_projects`].
    fn is_untitled(&self) -> bool {
        false
    }

    /// Untitled projects closed with work in them, newest first, with when
    /// they last changed. Not the open one.
    fn unsaved_projects(&self) -> Vec<(PathBuf, SystemTime)> {
        Vec::new()
    }

    /// Reopens an unsaved new project a crash left behind, with its work
    /// offered through [`Self::recovery_offer`]. Returns whether it found one.
    fn restore_untitled(&mut self) -> Result<bool, BackendError> {
        Ok(false)
    }
}

/// The real backend: one `.novo` file per project. A saved project is
/// written to its file as it changes (each [`ProjectBackend::tick`] and on
/// close), so there is nothing to lose and no "save changes?" question.
///
/// With an untitled folder set, a new project is kept in a file there from
/// the start, autosaved to a recovery file so its work survives a crash.
/// The first save moves it to the chosen file. Closing it with work keeps
/// it there as `kept-*.novo`, offered through `unsaved_projects`; closing
/// it untouched deletes it.
#[derive(Default)]
pub struct StorageBackend {
    document: Option<Document>,
    /// `None` for a new project until it is first saved, unless untitled
    /// projects are kept on disk.
    session: Option<Session>,
    recovery: Option<novo_storage::Recovered>,
    /// The project shown is a restored recovery that differs from the file.
    restored_unsaved: bool,
    /// Where unsaved new projects are kept.
    untitled_dir: Option<PathBuf>,
    /// The open project's file is a temporary one in `untitled_dir`.
    untitled: Option<PathBuf>,
    /// The open untitled project came from a kept file, so it is kept
    /// again on close even without new changes.
    keep: bool,
    /// Tests: writes to the project file fail, as for a locked file or a
    /// drive that went away.
    #[cfg(test)]
    fail_writes: bool,
}

/// Prefix of untitled projects closed with work in them.
const KEPT: &str = "kept-";

impl StorageBackend {
    /// Keeps unsaved new projects in `dir`, which is created when needed.
    pub fn with_untitled_dir(dir: impl Into<PathBuf>) -> Self {
        let mut backend = Self::default();
        backend.untitled_dir = Some(dir.into());
        backend
    }

    /// Writes and closes the open project's file. If the write fails,
    /// nothing is closed: the project stays open and bound to its file (with
    /// its work also put in the recovery file) and the error is returned, so
    /// the caller must not go on to replace or drop the project.
    fn close_session(&mut self) -> Result<(), BackendError> {
        // Undecided recovered work stays on disk (the session keeps its
        // recovery file), so an untitled project's file must stay with it
        // to be offered again on the next start.
        let undecided = self.recovery.is_some();
        let changed = self.restored_unsaved || self.document().is_some_and(|d| d.is_dirty());
        let keep_untitled = self.untitled.is_some() && !undecided && (changed || self.keep);
        let write = !undecided && changed && (self.untitled.is_none() || keep_untitled);
        let fail = self.writes_fail();
        if write
            && let (Some(session), Some(document)) = (self.session.as_mut(), &self.document)
            && let Err(e) = Self::write(session, document.project(), fail)
        {
            // Keep everything as it is so later edits are still written,
            // and put the work in the recovery file in case of a crash.
            if let Err(e) = session.autosave(document.project()) {
                log::error!("could not keep the unsaved work: {e}");
            }
            return Err(BackendError::NotWritten(e));
        }
        self.recovery = None;
        self.restored_unsaved = false;
        self.keep = false;
        let untitled = self.untitled.take();
        let Some(session) = self.session.take() else {
            return Ok(());
        };
        let closed = session.close();
        if let Some(path) = untitled
            && !undecided
        {
            if !keep_untitled {
                remove_file_if_exists(&path);
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && !name.starts_with(KEPT)
            {
                let kept = path.with_file_name(format!("{KEPT}{name}"));
                if let Err(e) = std::fs::rename(&path, &kept) {
                    log::warn!("could not keep {}: {e}", path.display());
                }
            }
        }
        Ok(closed?)
    }

    /// Writes the open project to its file.
    fn write(session: &mut Session, project: &Project, fail: bool) -> Result<(), StorageError> {
        if fail {
            return Err(std::io::Error::other("the file cannot be written").into());
        }
        session.save(project)
    }

    fn writes_fail(&self) -> bool {
        #[cfg(test)]
        return self.fail_writes;
        #[cfg(not(test))]
        false
    }

    fn in_untitled_dir(&self, path: &Path) -> bool {
        self.untitled_dir
            .as_deref()
            .is_some_and(|dir| path.parent() == Some(dir))
    }

    /// Starts a temporary file for a new project, if untitled projects are
    /// kept on disk. Failing that, the project still works, unprotected.
    fn start_untitled(&mut self) {
        let (Some(dir), Some(document)) = (&self.untitled_dir, &self.document) else {
            return;
        };
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let path = dir.join(format!(
            "untitled-{stamp}-{}.{}",
            std::process::id(),
            novo_storage::PROJECT_EXTENSION
        ));
        let created = std::fs::create_dir_all(dir)
            .map_err(StorageError::from)
            .and_then(|()| Session::create(&path, document.project()));
        match created {
            Ok(session) => {
                self.session = Some(session);
                self.untitled = Some(path);
            }
            Err(e) => log::warn!("unsaved new projects are not protected from crashes: {e}"),
        }
    }
}

fn remove_file_if_exists(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("could not delete {}: {e}", path.display()),
    }
}

impl ProjectBackend for StorageBackend {
    fn new_project(&mut self, name: &str) -> Result<(), BackendError> {
        let document = new_document(name)?;
        self.close_session()?;
        self.document = Some(document);
        self.start_untitled();
        Ok(())
    }

    fn open(&mut self, path: &Path) -> Result<OpenReport, BackendError> {
        let opened = Session::open(path)?;
        self.close_session()?;
        self.document = Some(Document::new(opened.project));
        self.session = Some(opened.session);
        self.recovery = opened.recovered;
        if self.in_untitled_dir(path) {
            self.untitled = Some(path.to_owned());
            self.keep = true;
        }
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
        if let Some(temp) = self.untitled.take() {
            remove_file_if_exists(&temp);
        }
        self.keep = false;
        Ok(())
    }

    fn close(&mut self) -> Result<(), BackendError> {
        self.close_session()?;
        self.document = None;
        Ok(())
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
        } else if self.untitled.is_some() {
            // An untitled project is nothing without its unsaved work.
            session.discard_recovery()?;
            self.close()?;
        } else {
            session.discard_recovery()?;
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<Option<Duration>, BackendError> {
        let fail = self.writes_fail();
        let (Some(session), Some(document)) = (self.session.as_mut(), self.document.as_mut())
        else {
            return Ok(None);
        };
        let changed = self.restored_unsaved || document.is_dirty();
        if self.untitled.is_none() && self.recovery.is_none() && changed {
            if let Err(e) = Self::write(session, document.project(), fail) {
                // The file cannot be written now (locked, gone, read-only):
                // keep the recovery file current so a crash loses nothing,
                // and report the failure so the caller retries later.
                if let Err(e) = session.autosave_if_due(document.project()) {
                    log::error!("could not keep the unsaved work: {e}");
                }
                return Err(e.into());
            }
            document.mark_saved();
            self.restored_unsaved = false;
        } else {
            session.autosave_if_due(document.project())?;
        }
        Ok(Some(session.autosave_interval()))
    }

    fn file(&self) -> Option<PathBuf> {
        self.session.as_ref().map(|s| s.path().to_owned())
    }

    fn is_untitled(&self) -> bool {
        self.document.is_some() && (self.session.is_none() || self.untitled.is_some())
    }

    fn unsaved_projects(&self) -> Vec<(PathBuf, SystemTime)> {
        let Some(entries) = self
            .untitled_dir
            .as_ref()
            .and_then(|d| std::fs::read_dir(d).ok())
        else {
            return Vec::new();
        };
        let mut kept: Vec<(PathBuf, SystemTime)> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e == novo_storage::PROJECT_EXTENSION)
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with(KEPT))
                    && self.untitled.as_deref() != Some(p.as_path())
            })
            .map(|p| {
                let modified = p
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                (p, modified)
            })
            .collect();
        kept.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
        kept
    }

    fn restore_untitled(&mut self) -> Result<bool, BackendError> {
        let Some(dir) = self.untitled_dir.clone() else {
            return Ok(false);
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Ok(false);
        };
        let mut leftovers: Vec<(SystemTime, PathBuf)> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e == novo_storage::PROJECT_EXTENSION)
            })
            .map(|p| {
                let modified = p
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                (modified, p)
            })
            .collect();
        // Newest first; older ones are offered on later starts.
        leftovers.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
        for (_, path) in leftovers {
            let opened = match Session::open(&path) {
                Ok(opened) => opened,
                // Another window is using it.
                Err(StorageError::Locked(_)) => continue,
                Err(e) => {
                    log::warn!("leaving unreadable {}: {e}", path.display());
                    continue;
                }
            };
            let kept = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(KEPT));
            if opened.recovered.is_none() && kept {
                // Closed with work in it: offered in the unsaved list.
                if let Err(e) = opened.session.close() {
                    log::warn!("could not close {}: {e}", path.display());
                }
                continue;
            }
            if opened.recovered.is_none() {
                // Nothing was autosaved before the crash: nothing to offer.
                if let Some(aside) = &opened.set_aside {
                    log::warn!(
                        "unreadable unsaved work from an untitled project kept at {}",
                        aside.path.display()
                    );
                }
                if let Err(e) = opened.session.close() {
                    log::warn!("could not close {}: {e}", path.display());
                }
                remove_file_if_exists(&path);
                continue;
            }
            self.close_session()?;
            self.document = Some(Document::new(opened.project));
            self.session = Some(opened.session);
            self.recovery = opened.recovered;
            self.untitled = Some(path);
            self.keep = kept;
            return Ok(true);
        }
        Ok(false)
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

    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|d| {
                d.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn a_new_project_is_kept_on_disk_until_saved() {
        let dir = tempfile::tempdir().unwrap();
        let untitled = dir.path().join("untitled");
        let path = dir.path().join("plant.novo");

        let mut backend = StorageBackend::with_untitled_dir(&untitled);
        backend.new_project("Plant").unwrap();
        assert_eq!(files_in(&untitled).len(), 1);
        edit(&mut backend, "2");
        backend.save(&path).unwrap();
        assert!(files_in(&untitled).is_empty(), "{:?}", files_in(&untitled));
        backend.close().unwrap();

        let mut reopened = StorageBackend::default();
        reopened.open(&path).unwrap();
        assert_eq!(page_names(&reopened), vec!["1", "2"]);
    }

    #[test]
    fn closing_an_untouched_new_project_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let mut backend = StorageBackend::with_untitled_dir(dir.path());
        backend.new_project("Plant").unwrap();
        backend.close().unwrap();
        assert!(files_in(dir.path()).is_empty());
        assert!(backend.unsaved_projects().is_empty());
    }

    #[test]
    fn closing_a_new_project_with_work_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut backend = StorageBackend::with_untitled_dir(dir.path());
        backend.new_project("Plant").unwrap();
        edit(&mut backend, "2");
        backend.close().unwrap();
        assert!(!backend.restore_untitled().unwrap(), "not a crash");
        let kept = backend.unsaved_projects();
        assert_eq!(kept.len(), 1, "{:?}", files_in(dir.path()));

        backend.open(&kept[0].0).unwrap();
        assert!(backend.is_untitled());
        assert_eq!(page_names(&backend), vec!["1", "2"]);
        assert!(
            backend.unsaved_projects().is_empty(),
            "the open one is not listed"
        );
        backend.close().unwrap();
        assert_eq!(backend.unsaved_projects().len(), 1, "kept again");

        backend.open(&kept[0].0).unwrap();
        let saved = tempfile::tempdir().unwrap();
        backend.save(&saved.path().join("plant.novo")).unwrap();
        assert!(!backend.is_untitled());
        backend.close().unwrap();
        assert!(
            files_in(dir.path()).is_empty(),
            "{:?}",
            files_in(dir.path())
        );
    }

    #[test]
    fn a_saved_project_is_written_as_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        let mut backend = StorageBackend::default();
        backend.new_project("Plant").unwrap();
        backend.save(&path).unwrap();

        edit(&mut backend, "2");
        assert!(backend.is_dirty());
        backend.tick().unwrap();
        assert!(!backend.is_dirty());

        // Closing writes what the last tick did not.
        edit(&mut backend, "3");
        backend.close().unwrap();
        let mut reopened = StorageBackend::default();
        let report = reopened.open(&path).unwrap();
        assert!(report.set_aside.is_none());
        assert!(reopened.recovery_offer().is_none());
        assert_eq!(page_names(&reopened), vec!["1", "2", "3"]);
    }

    #[test]
    fn a_failed_write_keeps_the_project_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        let other = dir.path().join("other.novo");
        let mut backend = StorageBackend::default();
        backend.new_project("Other").unwrap();
        backend.save(&other).unwrap();
        backend.new_project("Plant").unwrap();
        backend.save(&path).unwrap();
        edit(&mut backend, "2");

        backend.fail_writes = true;
        let err = backend.new_project("Next").unwrap_err();
        assert!(matches!(err, BackendError::NotWritten(_)), "{err:?}");
        let err = backend.open(&other).unwrap_err();
        assert!(matches!(err, BackendError::NotWritten(_)), "{err:?}");
        let err = backend.close().unwrap_err();
        assert!(matches!(err, BackendError::NotWritten(_)), "{err:?}");

        // Still open, still bound to its file, with the work in the
        // recovery file in case of a crash.
        assert_eq!(page_names(&backend), vec!["1", "2"]);
        assert_eq!(backend.file().as_deref(), Some(path.as_path()));
        assert!(!backend.is_untitled());
        assert!(autosave_path(&path).exists());
        // The other file was not left open.
        StorageBackend::default().open(&other).unwrap();

        // Edits made meanwhile are written once writing works again.
        edit(&mut backend, "3");
        backend.fail_writes = false;
        backend.tick().unwrap();
        assert!(!backend.is_dirty());
        backend.open(&other).unwrap();
        assert_eq!(backend.file().as_deref(), Some(other.as_path()));
        backend.close().unwrap();

        let mut reopened = StorageBackend::default();
        reopened.open(&path).unwrap();
        assert!(reopened.recovery_offer().is_none());
        assert_eq!(page_names(&reopened), vec!["1", "2", "3"]);
    }

    #[test]
    fn a_failing_write_still_keeps_a_recovery_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        let mut backend = StorageBackend::default();
        backend.new_project("Plant").unwrap();
        backend.save(&path).unwrap();
        backend
            .session
            .as_mut()
            .unwrap()
            .set_autosave_interval(Duration::ZERO);

        backend.fail_writes = true;
        edit(&mut backend, "2");
        assert!(backend.tick().is_err());
        assert!(backend.is_dirty());
        assert!(autosave_path(&path).exists());
        edit(&mut backend, "3");
        assert!(backend.tick().is_err());

        // A crash now: the work comes back from the recovery file.
        drop(backend.session.take());
        let mut reopened = StorageBackend::default();
        reopened.open(&path).unwrap();
        assert!(reopened.recovery_offer().is_some());
        reopened.resolve_recovery(true).unwrap();
        assert_eq!(page_names(&reopened), vec!["1", "2", "3"]);
    }

    /// An untitled project with page 2 autosaved, then a crash.
    pub(crate) fn crash_untitled(dir: &Path) {
        let mut backend = StorageBackend::with_untitled_dir(dir);
        backend.new_project("Plant").unwrap();
        edit(&mut backend, "2");
        let session = backend.session.as_mut().unwrap();
        session
            .autosave(backend.document.as_ref().unwrap().project())
            .unwrap();
        drop(backend.session.take());
        backend.untitled = None;
    }

    #[test]
    fn an_untitled_project_survives_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        crash_untitled(dir.path());

        let mut backend = StorageBackend::with_untitled_dir(dir.path());
        assert!(backend.restore_untitled().unwrap());
        assert!(backend.recovery_offer().is_some());
        backend.resolve_recovery(true).unwrap();
        assert_eq!(page_names(&backend), vec!["1", "2"]);
        assert!(backend.is_dirty());

        let saved = tempfile::tempdir().unwrap();
        backend.save(&saved.path().join("plant.novo")).unwrap();
        backend.close().unwrap();
        assert!(
            files_in(dir.path()).is_empty(),
            "{:?}",
            files_in(dir.path())
        );
    }

    #[test]
    fn discarding_a_crashed_untitled_project_deletes_it() {
        let dir = tempfile::tempdir().unwrap();
        crash_untitled(dir.path());

        let mut backend = StorageBackend::with_untitled_dir(dir.path());
        assert!(backend.restore_untitled().unwrap());
        backend.resolve_recovery(false).unwrap();
        assert!(backend.document().is_none());
        assert!(
            files_in(dir.path()).is_empty(),
            "{:?}",
            files_in(dir.path())
        );
        assert!(!backend.restore_untitled().unwrap());
    }

    #[test]
    fn quitting_before_answering_keeps_a_crashed_untitled_project() {
        let dir = tempfile::tempdir().unwrap();
        crash_untitled(dir.path());
        let mut backend = StorageBackend::with_untitled_dir(dir.path());
        assert!(backend.restore_untitled().unwrap());
        backend.close().unwrap();
        assert_eq!(
            files_in(dir.path())
                .iter()
                .filter(|f| f.ends_with(".novo"))
                .count(),
            1,
            "{:?}",
            files_in(dir.path())
        );
    }

    #[test]
    fn an_untitled_project_open_in_another_window_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = StorageBackend::with_untitled_dir(dir.path());
        first.new_project("Plant").unwrap();
        let mut second = StorageBackend::with_untitled_dir(dir.path());
        assert!(!second.restore_untitled().unwrap());
        assert_eq!(files_in(dir.path()).len(), 1);
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
