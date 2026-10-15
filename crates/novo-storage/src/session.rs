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

use novo_core::{ChangeSet, Project, RevisionId};

use crate::error::{Result, StorageError};
use crate::file::{ProjectFile, Role, remove_if_exists, sibling};

/// How often [`Session::autosave_if_due`] writes, by default.
pub const DEFAULT_AUTOSAVE_INTERVAL: Duration = Duration::from_secs(30);

/// Suffix of the recovery file written next to a project file.
pub const AUTOSAVE_SUFFIX: &str = ".autosave";

/// The recovery file for the project file at `path`, next to it. This is
/// where a session without a recovery folder keeps it, and where older
/// versions did.
pub fn autosave_path(path: impl AsRef<Path>) -> PathBuf {
    sibling(path.as_ref(), AUTOSAVE_SUFFIX)
}

/// The recovery file for the project file at `path` when recovery files are
/// kept in `dir` (a per-user folder): `<name>-<hash of the full path>.autosave`.
/// The hash keeps two projects of the same name in different folders apart.
pub fn recovery_path_in(dir: impl AsRef<Path>, path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let full = path.canonicalize().unwrap_or_else(|_| {
        // The file may not exist yet; resolve its folder instead.
        match (path.parent().map(Path::canonicalize), path.file_name()) {
            (Some(Ok(parent)), Some(name)) => parent.join(name),
            _ => path.to_owned(),
        }
    });
    // FNV-1a, stable across runs and versions (std's hasher is not).
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in full.to_string_lossy().to_lowercase().bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    let stem: String = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    dir.as_ref()
        .join(format!("{stem}-{hash:016x}{AUTOSAVE_SUFFIX}"))
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

/// A recovery file that could not be read. It is renamed aside (to
/// `<file>.autosave.broken`) rather than deleted, so the work in it is not
/// lost; tell the user where it is.
#[derive(Debug)]
pub struct SetAside {
    pub path: PathBuf,
    pub reason: StorageError,
}

/// What [`Session::open`] found.
#[derive(Debug)]
pub struct Opened {
    pub session: Session,
    /// The project as last saved.
    pub project: Project,
    /// Unsaved work from an earlier run that did not close cleanly. Show it
    /// to the user, then call [`Session::accept_recovery`] (and use this
    /// project) or [`Session::discard_recovery`]. Autosave waits until one
    /// of them is called, so the recovery file is kept meanwhile.
    pub recovered: Option<Recovered>,
    /// A recovery file that was there but could not be read.
    pub set_aside: Option<SetAside>,
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
    /// A recovered project was offered and the app has not decided yet.
    recovery_pending: bool,
    last_autosave: Instant,
    autosave_interval: Duration,
    /// The per-user folder for recovery files; `None` keeps them beside
    /// the project file.
    recovery_dir: Option<PathBuf>,
}

impl Session {
    /// Saves `project` as a new project file at `path` and opens it. A file
    /// open elsewhere is not replaced: [`StorageError::Locked`].
    pub fn create(path: impl AsRef<Path>, project: &Project) -> Result<Session> {
        Session::create_in(path, project, None)
    }

    /// Like [`Session::create`], keeping the recovery file in `recovery_dir`
    /// (a per-user folder, see [`recovery_path_in`]) instead of beside the
    /// project. If that folder cannot be created, the recovery file goes
    /// beside the project as before.
    pub fn create_in(
        path: impl AsRef<Path>,
        project: &Project,
        recovery_dir: Option<&Path>,
    ) -> Result<Session> {
        Ok(Session::with_file(
            ProjectFile::create(path, project)?,
            recovery_dir,
        ))
    }

    /// Opens the project file at `path`, and its recovery file if the last
    /// session did not close cleanly.
    pub fn open(path: impl AsRef<Path>) -> Result<Opened> {
        Session::open_in(path, None)
    }

    /// Like [`Session::open`], with the recovery file in `recovery_dir`.
    /// A recovery file beside the project, left by an older version, is
    /// still offered.
    pub fn open_in(path: impl AsRef<Path>, recovery_dir: Option<&Path>) -> Result<Opened> {
        let file = ProjectFile::open(path.as_ref())?;
        let project = file.load()?;
        let mut session = Session::with_file(file, recovery_dir);
        let (recovered, set_aside) = session.read_recovery()?;
        let file = &mut session.file;
        if recovered.is_none() {
            // Nothing can redo an undone revision any more; a recovered
            // project may still hold one, so then they wait for the next
            // open.
            file.drop_orphan_snapshots(&project)?;
        }
        session.recovery_pending = recovered.is_some();
        Ok(Opened {
            session,
            project,
            recovered,
            set_aside,
        })
    }

    fn with_file(file: ProjectFile, recovery_dir: Option<&Path>) -> Session {
        // A folder that cannot be created falls back to beside the project.
        let recovery_dir = recovery_dir
            .filter(|dir| std::fs::create_dir_all(dir).is_ok())
            .map(Path::to_owned);
        Session {
            recovery_dir,
            file,
            unsaved: Pending::Nothing,
            autosave: None,
            unautosaved: Pending::Nothing,
            recovery_pending: false,
            last_autosave: Instant::now(),
            autosave_interval: DEFAULT_AUTOSAVE_INTERVAL,
        }
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// Moves recovery files to `dir` (`None`: beside the project) from now
    /// on, for a session that moved from a temporary file to a user's file
    /// by [`Session::save_as`]. Any open recovery file is removed first;
    /// the next autosave writes a fresh one. A folder that cannot be
    /// created means beside the project.
    pub fn set_recovery_dir(&mut self, dir: Option<&Path>) {
        self.drop_autosave_quietly();
        self.recovery_dir = dir
            .filter(|dir| std::fs::create_dir_all(dir).is_ok())
            .map(Path::to_owned);
    }

    /// Where this session's recovery file is (or will be) written.
    pub fn recovery_path(&self) -> PathBuf {
        match &self.recovery_dir {
            Some(dir) => recovery_path_in(dir, self.file.path()),
            None => autosave_path(self.file.path()),
        }
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

    /// Keeps `project` as the state issued with `revision`. It is written
    /// to the project file at once, beside the records; an unsaved issue
    /// leaves an orphan snapshot that the next open drops.
    pub fn put_snapshot(&mut self, revision: RevisionId, project: &Project) -> Result<()> {
        self.file.put_snapshot(revision, project)
    }

    /// The project as it was issued with `revision`, if a snapshot is kept.
    pub fn snapshot(&self, revision: RevisionId) -> Result<Option<Project>> {
        self.file.snapshot(revision)
    }

    /// Keeps a document beside the model in the project file, at once (see
    /// [`ProjectFile::put_sidecar`]).
    pub fn put_sidecar(&mut self, key: &str, data: &str) -> Result<()> {
        self.file.put_sidecar(key, data)
    }

    /// The document kept under `key`, if any.
    pub fn sidecar(&self, key: &str) -> Result<Option<String>> {
        self.file.sidecar(key)
    }

    /// Whether anything changed since the last save.
    pub fn has_unsaved_changes(&self) -> bool {
        self.unsaved != Pending::Nothing
    }

    /// Writes `project` to the project file. Only the recorded changes are
    /// written. The recovery file is no longer needed and is deleted.
    pub fn save(&mut self, project: &Project) -> Result<()> {
        self.unsaved.write(&mut self.file, project)?;
        self.saved();
        Ok(())
    }

    /// Saves `project` to the file at `path` and continues there. The old
    /// file is left as it was last saved. A file open elsewhere is not
    /// replaced: [`StorageError::Locked`].
    pub fn save_as(&mut self, path: impl AsRef<Path>, project: &Project) -> Result<()> {
        let path = path.as_ref();
        if same_file(path, self.file.path()) {
            return self.save(project);
        }
        let mut file = ProjectFile::create(path, project)?;
        // Snapshots of issued revisions travel with the project.
        self.file.copy_snapshots_to(&mut file)?;
        self.file.copy_sidecars_to(&mut file)?;
        self.drop_autosave_quietly();
        let old = std::mem::replace(&mut self.file, file);
        let _ = old.close();
        self.saved();
        Ok(())
    }

    /// After a successful save: nothing is pending, and the recovery file
    /// is obsolete. Failing to delete it is harmless, as it refers to an
    /// older save and is discarded at the next open.
    fn saved(&mut self) {
        self.unsaved = Pending::Nothing;
        self.unautosaved = Pending::Nothing;
        self.recovery_pending = false;
        self.drop_autosave_quietly();
    }

    /// Whether there are changes the recovery file does not hold yet and the
    /// autosave interval has passed. Never while a recovery decision is
    /// pending.
    pub fn autosave_due(&self) -> bool {
        !self.recovery_pending
            && self.unautosaved != Pending::Nothing
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
    /// Does nothing if there are none, or while a recovery decision is
    /// pending (the recovery file still holds the offered work).
    pub fn autosave(&mut self, project: &Project) -> Result<()> {
        if self.recovery_pending || self.unautosaved == Pending::Nothing {
            return Ok(());
        }
        match &mut self.autosave {
            Some(file) => self.unautosaved.write(file, project)?,
            None => {
                let base = self.file.save_seq()?.to_string();
                let base_file = self.file.file_id()?.unwrap_or_default();
                self.autosave = Some(ProjectFile::create_as(
                    &self.recovery_path(),
                    project,
                    Role::Autosave,
                    |tx| {
                        let mut set =
                            tx.prepare("INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)")?;
                        set.execute(["base_seq", &base])?;
                        set.execute(["base_file", &base_file])?;
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
    /// now edits it. The next save writes all of it, and the next autosave
    /// writes a fresh recovery file; until then the old one stays.
    pub fn accept_recovery(&mut self) {
        self.recovery_pending = false;
        self.unsaved = Pending::Everything;
        self.unautosaved = Pending::Everything;
    }

    /// Throws the recovered work away and deletes the recovery file.
    pub fn discard_recovery(&mut self) -> Result<()> {
        self.recovery_pending = false;
        self.drop_autosave()
    }

    /// Closes the project. Unsaved changes are dropped (ask the user to save
    /// first) and the recovery file is deleted, except while a recovery
    /// decision is still pending: then the offered work is kept for the next
    /// open.
    pub fn close(mut self) -> Result<()> {
        if !self.recovery_pending {
            self.drop_autosave()?;
        }
        self.file.close()
    }

    fn drop_autosave(&mut self) -> Result<()> {
        if let Some(file) = self.autosave.take() {
            file.close()?;
        }
        remove_if_exists(&self.recovery_path())?;
        // A leftover from an older version, beside the project.
        remove_if_exists(&autosave_path(self.file.path()))
    }

    fn drop_autosave_quietly(&mut self) {
        let _ = self.drop_autosave();
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

impl Session {
    /// Reads the recovery file, and one beside the project left by an older
    /// version. The first that is valid is offered.
    fn read_recovery(&self) -> Result<(Option<Recovered>, Option<SetAside>)> {
        let legacy = autosave_path(self.file.path());
        let mut aside = None;
        let mut paths = vec![self.recovery_path()];
        if paths[0] != legacy {
            paths.push(legacy);
        }
        for path in paths {
            let (recovered, set_aside) = read_recovery_file(&self.file, &path)?;
            aside = aside.or(set_aside);
            if recovered.is_some() {
                return Ok((recovered, aside));
            }
        }
        Ok((None, aside))
    }
}

/// Reads the recovery file at `path` for `file`. One made against an older
/// save of the project, or against a different file at the same path, is
/// deleted; one that cannot be read is renamed aside.
fn read_recovery_file(
    file: &ProjectFile,
    path: &Path,
) -> Result<(Option<Recovered>, Option<SetAside>)> {
    if !path.exists() {
        return Ok((None, None));
    }
    let read = (|| {
        let autosave = ProjectFile::open_as(path, Role::Autosave)?;
        let base = autosave
            .meta("base_seq")?
            .and_then(|v| v.parse::<u64>().ok());
        let base_file = autosave.meta("base_file")?;
        if base != Some(file.save_seq()?) || base_file.is_none() || base_file != file.file_id()? {
            autosave.close()?;
            return Ok(None);
        }
        let recovered = Recovered {
            project: autosave.load()?,
            autosaved_at: autosave.saved_at()?,
        };
        autosave.close()?;
        Ok(Some(recovered))
    })();
    match read {
        Ok(Some(recovered)) => Ok((Some(recovered), None)),
        Ok(None) => {
            remove_if_exists(path)?;
            Ok((None, None))
        }
        Err(StorageError::Locked(p)) => Err(StorageError::Locked(p)),
        Err(reason) => {
            let aside = sibling(path, ".broken");
            remove_if_exists(&aside)?;
            std::fs::rename(path, &aside)?;
            Ok((
                None,
                Some(SetAside {
                    path: aside,
                    reason,
                }),
            ))
        }
    }
}
