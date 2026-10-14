//! An open project as the app sees it: the project file, autosave and crash
//! recovery.
//!
//! The app keeps its [`novo_core::Document`] in memory and tells the session
//! what each edit changed ([`Session::record`]). Saving writes the project
//! file; between saves, autosave writes a recovery file next to it
//! (`motor.novo.autosave`). A clean close deletes the recovery file, so one
//! that is still there at the next open means the app stopped without
//! saving, and [`Session::open`] offers its contents back.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use novo_core::{ChangeSet, Project};

use crate::error::{Result, StorageError};
use crate::file::{ProjectFile, Role, remove_if_exists, sibling};

/// How often [`Session::autosave_if_due`] writes, by default.
pub const DEFAULT_AUTOSAVE_INTERVAL: Duration = Duration::from_secs(30);

/// Suffix of the recovery file written next to a project file.
pub const AUTOSAVE_SUFFIX: &str = ".autosave";

/// The recovery file for the project file at `path`.
pub fn autosave_path(path: impl AsRef<Path>) -> PathBuf {
    sibling(path.as_ref(), AUTOSAVE_SUFFIX)
}

/// Changes not yet written to some file.
#[derive(Debug, Clone, PartialEq)]
enum Pending {
    Nothing,
    Changes(ChangeSet),
    /// Unknown changes: write everything.
    Everything,
}

impl Pending {
    fn add(&mut self, changes: &ChangeSet) {
        if changes.is_empty() {
            return;
        }
        match self {
            Pending::Nothing => *self = Pending::Changes(changes.clone()),
            Pending::Changes(set) => set.merge(changes),
            Pending::Everything => {}
        }
    }

    fn write(&self, file: &mut ProjectFile, project: &Project) -> Result<()> {
        match self {
            Pending::Nothing => Ok(()),
            Pending::Changes(set) => file.save_changes(project, set),
            Pending::Everything => file.save(project),
        }
    }
}

/// A project recovered from the autosave file after the app stopped
/// without saving.
#[derive(Debug)]
pub struct Recovered {
    /// The project as last autosaved.
    pub project: Project,
    /// When that autosave happened.
    pub autosaved_at: Option<SystemTime>,
}

/// What [`Session::open`] found.
#[derive(Debug)]
pub struct Opened {
    pub session: Session,
    /// The project as last saved.
    pub project: Project,
    /// Unsaved work from an earlier run that did not close cleanly. Show it
    /// to the user, then call [`Session::accept_recovery`] (and use this
    /// project) or [`Session::discard_recovery`].
    pub recovered: Option<Recovered>,
}

/// An open project file with autosave.
#[derive(Debug)]
pub struct Session {
    file: ProjectFile,
    /// Changes since the last save to the project file.
    unsaved: Pending,
    /// The open recovery file, once the first autosave has written it.
    autosave: Option<ProjectFile>,
    /// Changes since the last write to the recovery file.
    unautosaved: Pending,
    last_autosave: Instant,
    autosave_interval: Duration,
}

impl Session {
    /// Saves `project` as a new project file at `path` and opens it.
    pub fn create(path: impl AsRef<Path>, project: &Project) -> Result<Session> {
        let path = path.as_ref();
        remove_if_exists(&autosave_path(path))?;
        Ok(Session::with_file(ProjectFile::create(path, project)?))
    }

    /// Opens the project file at `path`, and its recovery file if the last
    /// session did not close cleanly.
    pub fn open(path: impl AsRef<Path>) -> Result<Opened> {
        let file = ProjectFile::open(path.as_ref())?;
        let project = file.load()?;
        let recovered = read_recovery(&file)?;
        let mut session = Session::with_file(file);
        if recovered.is_some() {
            // The next autosave starts a fresh recovery file.
            session.unautosaved = Pending::Everything;
        }
        Ok(Opened {
            session,
            project,
            recovered,
        })
    }

    fn with_file(file: ProjectFile) -> Session {
        Session {
            file,
            unsaved: Pending::Nothing,
            autosave: None,
            unautosaved: Pending::Nothing,
            last_autosave: Instant::now(),
            autosave_interval: DEFAULT_AUTOSAVE_INTERVAL,
        }
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    pub fn autosave_interval(&self) -> Duration {
        self.autosave_interval
    }

    pub fn set_autosave_interval(&mut self, interval: Duration) {
        self.autosave_interval = interval;
    }

    /// Notes what an edit, undo or redo changed. Call it after every change
    /// to the project, with the change set the model returned.
    pub fn record(&mut self, changes: &ChangeSet) {
        self.unsaved.add(changes);
        self.unautosaved.add(changes);
    }

    /// Whether anything changed since the last save.
    pub fn has_unsaved_changes(&self) -> bool {
        self.unsaved != Pending::Nothing
    }

    /// Writes `project` to the project file. Only the recorded changes are
    /// written. The recovery file is no longer needed and is deleted.
    pub fn save(&mut self, project: &Project) -> Result<()> {
        self.unsaved.write(&mut self.file, project)?;
        self.unsaved = Pending::Nothing;
        self.drop_autosave()?;
        Ok(())
    }

    /// Saves `project` to a new file at `path` and continues there. The old
    /// file is left as it was last saved.
    pub fn save_as(&mut self, path: impl AsRef<Path>, project: &Project) -> Result<()> {
        let path = path.as_ref();
        remove_if_exists(&autosave_path(path))?;
        let file = ProjectFile::create(path, project)?;
        self.drop_autosave()?;
        std::mem::replace(&mut self.file, file).close()?;
        self.unsaved = Pending::Nothing;
        Ok(())
    }

    /// Whether there are changes the recovery file does not hold yet and the
    /// autosave interval has passed.
    pub fn autosave_due(&self) -> bool {
        self.unautosaved != Pending::Nothing
            && self.last_autosave.elapsed() >= self.autosave_interval
    }

    /// Autosaves if [`Session::autosave_due`]. Returns whether it wrote.
    /// Call it from the app's idle loop.
    pub fn autosave_if_due(&mut self, project: &Project) -> Result<bool> {
        if !self.autosave_due() {
            return Ok(false);
        }
        self.autosave(project)?;
        Ok(true)
    }

    /// Writes the changes since the last autosave to the recovery file now.
    /// Does nothing if there are none.
    pub fn autosave(&mut self, project: &Project) -> Result<()> {
        if self.unautosaved == Pending::Nothing {
            return Ok(());
        }
        match &mut self.autosave {
            Some(file) => self.unautosaved.write(file, project)?,
            None => {
                let base = self.file.save_seq()?.to_string();
                self.autosave = Some(ProjectFile::create_as(
                    &autosave_path(self.file.path()),
                    project,
                    Role::Autosave,
                    |tx| {
                        tx.execute(
                            "INSERT OR REPLACE INTO meta (key, value) VALUES ('base_seq', ?1)",
                            [&base],
                        )?;
                        Ok(())
                    },
                )?);
            }
        }
        self.unautosaved = Pending::Nothing;
        self.last_autosave = Instant::now();
        Ok(())
    }

    /// Keeps the recovered project offered by [`Session::open`]: the app
    /// now edits it, and the next save writes all of it.
    pub fn accept_recovery(&mut self) {
        self.unsaved = Pending::Everything;
    }

    /// Throws the recovered work away and deletes the recovery file.
    pub fn discard_recovery(&mut self) -> Result<()> {
        self.drop_autosave()?;
        if self.unautosaved == Pending::Everything && self.unsaved == Pending::Nothing {
            self.unautosaved = Pending::Nothing;
        }
        Ok(())
    }

    /// Closes the project. Unsaved changes are dropped (ask the user to save
    /// first) and the recovery file is deleted.
    pub fn close(mut self) -> Result<()> {
        self.drop_autosave()?;
        self.file.close()
    }

    fn drop_autosave(&mut self) -> Result<()> {
        if let Some(file) = self.autosave.take() {
            file.close()?;
        }
        remove_if_exists(&autosave_path(self.file.path()))
    }
}

/// Reads the recovery file next to `file`, if there is a usable one. A
/// recovery file made against an older save of the project, or one that
/// cannot be read, is deleted.
fn read_recovery(file: &ProjectFile) -> Result<Option<Recovered>> {
    let path = autosave_path(file.path());
    if !path.exists() {
        return Ok(None);
    }
    let autosave = match ProjectFile::open_as(&path, Role::Autosave) {
        Ok(autosave) => autosave,
        Err(StorageError::Locked(path)) => return Err(StorageError::Locked(path)),
        Err(_) => {
            remove_if_exists(&path)?;
            return Ok(None);
        }
    };
    let base = autosave
        .meta("base_seq")?
        .and_then(|v| v.parse::<u64>().ok());
    let current = base == Some(file.save_seq()?);
    let recovered = if current {
        match autosave.load() {
            Ok(project) => Some(Recovered {
                project,
                autosaved_at: autosave.saved_at()?,
            }),
            Err(_) => None,
        }
    } else {
        None
    };
    autosave.close()?;
    if recovered.is_none() {
        remove_if_exists(&path)?;
    }
    Ok(recovered)
}
