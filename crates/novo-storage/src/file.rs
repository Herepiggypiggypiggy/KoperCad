//! One project file: a single SQLite database holding a whole project.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use novo_core::{
    AnyId, ChangeSet, EntityKind, JSON_FORMAT_VERSION, Project, ProjectInfo, Record, RevisionId,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};

use crate::error::{Result, StorageError, is_busy, is_not_a_database};
use crate::schema::{self, APPLICATION_ID, MIGRATIONS, Migration};
use crate::session::AUTOSAVE_SUFFIX;

/// File name extension of project files.
pub const PROJECT_EXTENSION: &str = "novo";

/// What a file is for. Autosave files share the project file's schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Project,
    Autosave,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Project => "project",
            Role::Autosave => "autosave",
        }
    }
}

/// An open project file.
///
/// The file is locked for as long as this value lives, so a second window
/// or program opening it gets [`StorageError::Locked`]. Every write is one
/// SQLite transaction: a crash or power cut during a save leaves the file as
/// it was before the save, never half written.
///
/// The lock is SQLite's file lock. It is reliable on local disks; network
/// shares (SMB, NFS) may not honour it, so two machines opening the same
/// file on a share are not guaranteed to be kept apart.
///
/// While a file is open, SQLite keeps an empty `<file>-journal` next to it
/// (exclusive locking blanks the journal instead of deleting it). It is
/// removed when the file is closed and is harmless if copied or synced.
#[derive(Debug)]
pub struct ProjectFile {
    conn: Connection,
    path: PathBuf,
}

impl ProjectFile {
    /// Writes `project` to a new file at `path`, replacing any file there.
    ///
    /// The project is written to a temporary file next to `path` and moved
    /// into place only once complete, so an existing file is never left half
    /// overwritten. A file that is open elsewhere is not replaced:
    /// [`StorageError::Locked`].
    ///
    /// On Unix, another program could open the new file in the instant
    /// between moving it into place and opening it here; the file is then
    /// written but this returns `Locked`.
    pub fn create(path: impl AsRef<Path>, project: &Project) -> Result<ProjectFile> {
        Self::create_as(path.as_ref(), project, Role::Project, |_| Ok(()))
    }

    /// Opens an existing project file, upgrading its schema if it was written
    /// by an older version.
    pub fn open(path: impl AsRef<Path>) -> Result<ProjectFile> {
        Self::open_as(path.as_ref(), Role::Project)
    }

    /// Reads the project at `path` without writing to it, for tools that
    /// only read (such as the command-line exporter). Works on read-only
    /// media; a file from an older version is upgraded in memory only. A file
    /// open in the app gives [`StorageError::Locked`].
    pub fn read(path: impl AsRef<Path>) -> Result<Project> {
        Self::read_only(path)?.load()
    }

    /// Like [`ProjectFile::read`] but keeps the file, so the snapshots of
    /// issued revisions can be read too ([`ProjectFile::snapshot`]), for
    /// comparing revisions from the command line. Never writes.
    pub fn read_only(path: impl AsRef<Path>) -> Result<ProjectFile> {
        let path = path.as_ref();
        ensure_exists(path)?;
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(path, flags)?;
        let version = check_existing(&conn, path, schema::SCHEMA_VERSION)?;
        let conn = if version < schema::SCHEMA_VERSION {
            let mut memory = Connection::open_in_memory()?;
            // rusqlite refuses -1 ("all pages") here; one step of any
            // realistic file size copies everything.
            rusqlite::backup::Backup::new(&conn, &mut memory)?.run_to_completion(
                i32::MAX,
                Duration::ZERO,
                None,
            )?;
            schema::migrate(&mut memory, path, MIGRATIONS, false)?;
            memory
        } else {
            conn
        };
        let file = ProjectFile {
            conn,
            path: path.to_owned(),
        };
        file.check_role(Role::Project)?;
        Ok(file)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn create_as(
        path: &Path,
        project: &Project,
        role: Role,
        extra: impl FnOnce(&Transaction<'_>) -> Result<()>,
    ) -> Result<ProjectFile> {
        // Hold the target's lock while preparing, so a file open in another
        // window is never replaced and its recovery file never touched.
        let lock = lock_existing(path)?;
        let temp = sibling(path, ".saving");
        remove_if_exists(&temp)?;
        let written = (|| {
            let mut file = Self::connect(&temp, true, MIGRATIONS)?;
            let tx = file.conn.transaction()?;
            set_meta(&tx, "role", role.as_str())?;
            set_meta(&tx, "created_at", &now_millis().to_string())?;
            set_meta(&tx, "file_id", &new_file_id())?;
            write_all(&tx, project)?;
            finish_save(&tx)?;
            extra(&tx)?;
            tx.commit()?;
            // Close before moving: Windows cannot rename an open file.
            file.close()?;
            replace(&temp, path, lock)
        })();
        if let Err(err) = written {
            let _ = std::fs::remove_file(&temp);
            return Err(err);
        }
        if role == Role::Project {
            // A recovery file left by a crash belonged to the replaced file.
            // It names that file's id, so it would never be offered against
            // the new one; this only tidies it away.
            let _ = remove_if_exists(&sibling(path, AUTOSAVE_SUFFIX));
        }
        Self::open_as(path, role)
    }

    pub(crate) fn open_as(path: &Path, role: Role) -> Result<ProjectFile> {
        ensure_exists(path)?;
        let file = Self::connect(path, false, MIGRATIONS)?;
        file.check_role(role)?;
        Ok(file)
    }

    fn check_role(&self, role: Role) -> Result<()> {
        if self.meta("role")?.as_deref() != Some(role.as_str()) {
            return Err(StorageError::NotAProject(self.path.clone()));
        }
        Ok(())
    }

    /// Opens (and with `create`, creates) a database, takes the exclusive
    /// lock, checks it is ours and runs pending migrations. Recovery files
    /// are upgraded without a backup copy.
    fn connect(path: &Path, create: bool, migrations: &[Migration]) -> Result<ProjectFile> {
        let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        if create {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        let mut conn = Connection::open_with_flags(path, flags)?;
        // EXCLUSIVE locking keeps the write lock between transactions, so the
        // file stays ours until this connection closes.
        conn.execute_batch(
            "PRAGMA locking_mode = EXCLUSIVE;
             PRAGMA journal_mode = DELETE;
             PRAGMA synchronous = FULL;",
        )
        .map_err(|e| classify(e, path))?;
        conn.execute_batch("BEGIN EXCLUSIVE; COMMIT;")
            .map_err(|e| classify(e, path))?;

        if create {
            let app_id = schema::application_id(&conn).map_err(|e| classify(e, path))?;
            if schema::user_version(&conn)? != 0 || app_id != 0 {
                return Err(StorageError::NotAProject(path.to_owned()));
            }
        } else {
            check_existing(&conn, path, schema::latest(migrations))?;
        }
        let backup = !create && !is_autosave(path);
        schema::migrate(&mut conn, path, migrations, backup)?;
        Ok(ProjectFile {
            conn,
            path: path.to_owned(),
        })
    }

    /// Closes the file and releases its lock, reporting any error.
    pub fn close(self) -> Result<()> {
        self.conn.close().map_err(|(_, e)| e.into())
    }

    /// Reads the whole project, checking every model integrity rule.
    pub fn load(&self) -> Result<Project> {
        let corrupt = |reason: String| StorageError::Corrupt {
            path: self.path.clone(),
            reason,
        };
        let format: u32 = self
            .meta("record_format")?
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| corrupt("missing record format".into()))?;
        // A change to novo-core's record JSON comes with a schema migration
        // that rewrites the rows, so after migrating the formats match.
        if format > JSON_FORMAT_VERSION {
            return Err(StorageError::NewerFormat {
                path: self.path.clone(),
                found: format,
                supported: JSON_FORMAT_VERSION,
            });
        }
        if format != JSON_FORMAT_VERSION {
            return Err(corrupt(format!(
                "records use format {format} but no migration upgraded them to {JSON_FORMAT_VERSION}"
            )));
        }
        let info: ProjectInfo = serde_json::from_str(
            &self
                .meta("info")?
                .ok_or_else(|| corrupt("missing project info".into()))?,
        )
        .map_err(|e| corrupt(format!("project info: {e}")))?;
        let next_id: u64 = self
            .meta("next_id")?
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| corrupt("missing next id".into()))?;

        let mut stmt = self.conn.prepare("SELECT id, kind, data FROM records")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut records = Vec::new();
        for row in rows {
            let (id, kind, data) = row?;
            let record = decode(&kind, &data).map_err(|e| corrupt(format!("object {id}: {e}")))?;
            if record.id().raw() != id as u64 {
                return Err(corrupt(format!("object {id} is stored under the wrong id")));
            }
            records.push(record);
        }
        Ok(Project::from_records(info, next_id, records)?)
    }

    /// Replaces the file's contents with `project`.
    pub fn save(&mut self, project: &Project) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM records", [])?;
        write_all(&tx, project)?;
        finish_save(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// Writes only the objects in `changes`, which must cover every change
    /// since the file was last written (merge successive change sets with
    /// [`ChangeSet::merge`]). Much faster than [`ProjectFile::save`] for
    /// small edits to a large project.
    pub fn save_changes(&mut self, project: &Project, changes: &ChangeSet) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut upsert = tx.prepare_cached(
                "INSERT OR REPLACE INTO records (id, kind, page, data) VALUES (?1, ?2, ?3, ?4)",
            )?;
            let mut delete = tx.prepare_cached("DELETE FROM records WHERE id = ?1")?;
            let ids = changes
                .added
                .iter()
                .chain(&changes.modified)
                .chain(&changes.removed);
            for &id in ids {
                match project.record(id) {
                    Some(record) => write_record(&mut upsert, &record)?,
                    None => {
                        delete.execute([raw(id)])?;
                    }
                }
            }
        }
        set_meta(&tx, "info", &to_json(project.info()))?;
        set_meta(&tx, "next_id", &project.next_id().to_string())?;
        finish_save(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// Keeps `project` as the state issued with `revision` (DESIGN §7,
    /// Revisions), replacing any snapshot already kept for it.
    pub fn put_snapshot(&mut self, revision: RevisionId, project: &Project) -> Result<()> {
        use std::io::Write as _;
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(project.to_json().as_bytes())?;
        let data = encoder.finish()?;
        self.conn.execute(
            "INSERT OR REPLACE INTO snapshots (revision, format, data) VALUES (?1, ?2, ?3)",
            params![raw(revision.into()), JSON_FORMAT_VERSION, data],
        )?;
        Ok(())
    }

    /// The project as it was issued with `revision`, if a snapshot is kept.
    pub fn snapshot(&self, revision: RevisionId) -> Result<Option<Project>> {
        use std::io::Read as _;
        let corrupt = |reason: String| StorageError::Corrupt {
            path: self.path.clone(),
            reason: format!("snapshot of {revision}: {reason}"),
        };
        let Some((format, data)) = self
            .conn
            .query_row(
                "SELECT format, data FROM snapshots WHERE revision = ?1",
                [raw(revision.into())],
                |r| Ok((r.get::<_, u32>(0)?, r.get::<_, Vec<u8>>(1)?)),
            )
            .optional()?
        else {
            return Ok(None);
        };
        if format != JSON_FORMAT_VERSION {
            return Err(corrupt(format!(
                "uses format {format} but no migration upgraded it to {JSON_FORMAT_VERSION}"
            )));
        }
        let mut json = String::new();
        flate2::read::ZlibDecoder::new(data.as_slice())
            .read_to_string(&mut json)
            .map_err(|e| corrupt(e.to_string()))?;
        Ok(Some(Project::from_json(&json)?))
    }

    /// The revisions a snapshot is kept for, in id order.
    pub fn snapshot_revisions(&self) -> Result<Vec<RevisionId>> {
        let mut stmt = self
            .conn
            .prepare("SELECT revision FROM snapshots ORDER BY revision")?;
        let ids = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        let mut out = Vec::new();
        for id in ids {
            out.push(RevisionId(id? as u64));
        }
        Ok(out)
    }

    /// Drops the snapshots of revisions `project` no longer has (issued and
    /// then undone). Call it on open or close only: while the undo history
    /// can still redo a revision, its snapshot must stay.
    pub fn drop_orphan_snapshots(&mut self, project: &Project) -> Result<usize> {
        let mut dropped = 0;
        for revision in self.snapshot_revisions()? {
            if project.issued_revision(revision).is_none() {
                dropped += self.conn.execute(
                    "DELETE FROM snapshots WHERE revision = ?1",
                    [raw(revision.into())],
                )?;
            }
        }
        Ok(dropped)
    }

    /// Keeps `data` (JSON text) under `key`, replacing what was there. It is
    /// written at once, like a snapshot, not with the next save.
    pub fn put_sidecar(&mut self, key: &str, data: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sidecar (key, data) VALUES (?1, ?2)",
            params![key, data],
        )?;
        Ok(())
    }

    /// The document kept under `key`, if any.
    pub fn sidecar(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT data FROM sidecar WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    /// Drops the document kept under `key`.
    pub fn remove_sidecar(&mut self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM sidecar WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Copies every sidecar document into `other` (save as).
    pub(crate) fn copy_sidecars_to(&self, other: &mut ProjectFile) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT key, data FROM sidecar")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let tx = other.conn.transaction()?;
        for row in rows {
            let (key, data) = row?;
            tx.execute(
                "INSERT OR REPLACE INTO sidecar (key, data) VALUES (?1, ?2)",
                params![key, data],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Copies every kept snapshot into `other` as stored (save as).
    pub(crate) fn copy_snapshots_to(&self, other: &mut ProjectFile) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT revision, format, data FROM snapshots")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })?;
        let tx = other.conn.transaction()?;
        for row in rows {
            let (revision, format, data) = row?;
            tx.execute(
                "INSERT OR REPLACE INTO snapshots (revision, format, data) VALUES (?1, ?2, ?3)",
                params![revision, format, data],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// When the file was last written.
    pub fn saved_at(&self) -> Result<Option<SystemTime>> {
        Ok(self
            .meta("saved_at")?
            .and_then(|v| v.parse::<u64>().ok())
            .map(|ms| UNIX_EPOCH + Duration::from_millis(ms)))
    }

    /// A random id given to the file when it is created, so a recovery file
    /// can tell this file from another one written to the same path.
    pub(crate) fn file_id(&self) -> Result<Option<String>> {
        self.meta("file_id")
    }

    /// Counts the saves made to this file; identifies a saved state.
    pub(crate) fn save_seq(&self) -> Result<u64> {
        Ok(self
            .meta("save_seq")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0))
    }

    pub(crate) fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }
}

fn ensure_exists(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("{} does not exist", path.display()),
    )
    .into())
}

fn is_autosave(path: &Path) -> bool {
    path.as_os_str()
        .to_string_lossy()
        .ends_with(AUTOSAVE_SUFFIX)
}

/// Checks an existing database is a project file this version can read.
/// Returns its schema version.
fn check_existing(conn: &Connection, path: &Path, supported: u32) -> Result<u32> {
    let app_id = schema::application_id(conn).map_err(|e| classify(e, path))?;
    let version = schema::user_version(conn)?;
    if app_id != APPLICATION_ID || version == 0 {
        return Err(StorageError::NotAProject(path.to_owned()));
    }
    if version > supported {
        return Err(StorageError::NewerFormat {
            path: path.to_owned(),
            found: version,
            supported,
        });
    }
    let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if check != "ok" {
        return Err(StorageError::Corrupt {
            path: path.to_owned(),
            reason: check,
        });
    }
    Ok(version)
}

/// Takes the exclusive lock on an existing file before it is replaced.
/// `Locked` if another connection holds it; `None` if there is no file or
/// it is not a SQLite database (so nothing can hold a SQLite lock on it).
fn lock_existing(path: &Path) -> Result<Option<Connection>> {
    if !path.exists() {
        return Ok(None);
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags)?;
    let locked = conn.execute_batch(
        "PRAGMA locking_mode = EXCLUSIVE;
         BEGIN EXCLUSIVE; COMMIT;",
    );
    match locked {
        Ok(()) => Ok(Some(conn)),
        Err(e) if is_not_a_database(&e) => Ok(None),
        Err(e) => Err(classify(e, path)),
    }
}

/// Moves the finished `temp` file over `path` without a moment in which
/// another program could open the old file and then lose it to the rename.
///
/// On Unix a file can be renamed over while open, so the target's lock is
/// held until the new file is in place: anyone trying to open the old file
/// meanwhile gets `Locked`. On Windows a file open in another program cannot
/// be replaced, so the lock is released first; if something opens the
/// target in that instant, the rename fails and nothing is replaced.
fn replace(temp: &Path, path: &Path, lock: Option<Connection>) -> Result<()> {
    let release = |lock: Option<Connection>| -> Result<()> {
        if let Some(lock) = lock {
            lock.close().map_err(|(_, e)| StorageError::from(e))?;
        }
        Ok(())
    };
    if cfg!(windows) {
        release(lock)?;
        std::fs::rename(temp, path)?;
    } else {
        std::fs::rename(temp, path)?;
        // The lock now refers to the replaced file; closing it releases it.
        let _ = release(lock);
    }
    Ok(())
}

fn classify(err: rusqlite::Error, path: &Path) -> StorageError {
    if is_busy(&err) {
        StorageError::Locked(path.to_owned())
    } else if is_not_a_database(&err) {
        StorageError::NotAProject(path.to_owned())
    } else {
        err.into()
    }
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

fn write_all(tx: &Transaction<'_>, project: &Project) -> Result<()> {
    set_meta(tx, "record_format", &JSON_FORMAT_VERSION.to_string())?;
    set_meta(tx, "model_version", &novo_core::MODEL_VERSION.to_string())?;
    set_meta(tx, "info", &to_json(project.info()))?;
    set_meta(tx, "next_id", &project.next_id().to_string())?;
    let mut insert = tx.prepare_cached(
        "INSERT OR REPLACE INTO records (id, kind, page, data) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for record in project.records() {
        write_record(&mut insert, &record)?;
    }
    Ok(())
}

fn write_record(stmt: &mut rusqlite::CachedStatement<'_>, record: &Record) -> Result<()> {
    stmt.execute(params![
        raw(record.id()),
        kind_name(record.kind()),
        record.page().map(|p| p.raw() as i64),
        encode(record),
    ])?;
    Ok(())
}

/// The record in the model's own JSON form (tagged with `"record"`).
fn encode(record: &Record) -> String {
    to_json(record)
}

/// Parses a stored record and checks it matches its `kind` column.
fn decode(kind: &str, data: &str) -> std::result::Result<Record, String> {
    let record: Record = serde_json::from_str(data).map_err(|e| e.to_string())?;
    if kind_name(record.kind()) != kind {
        return Err(format!(
            "stored as {kind:?} but holds a {}",
            kind_name(record.kind())
        ));
    }
    Ok(record)
}

fn finish_save(tx: &Transaction<'_>) -> Result<()> {
    let seq: u64 = tx
        .query_row("SELECT value FROM meta WHERE key = 'save_seq'", [], |r| {
            r.get::<_, String>(0)
        })
        .optional()?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    set_meta(tx, "save_seq", &(seq + 1).to_string())?;
    set_meta(tx, "saved_at", &now_millis().to_string())?;
    set_meta(
        tx,
        "writer",
        concat!("KoperCAD ", env!("CARGO_PKG_VERSION")),
    )?;
    Ok(())
}

fn raw(id: AnyId) -> i64 {
    // Ids come from a counter starting at 1 and stay far below i64::MAX.
    id.raw() as i64
}

fn to_json<T: serde::Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).expect("model values serialize to JSON")
}

pub(crate) fn kind_name(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Page => "page",
        EntityKind::Device => "device",
        EntityKind::Symbol => "symbol",
        EntityKind::Junction => "junction",
        EntityKind::Wire => "wire",
        EntityKind::Potential => "potential",
        EntityKind::Terminal => "terminal",
        EntityKind::Cable => "cable",
        EntityKind::Part => "part",
        EntityKind::Text => "text",
        EntityKind::PotentialSystem => "potential_system",
        EntityKind::Structure => "structure",
        EntityKind::Dismissal => "dismissal",
        EntityKind::BlockDefinition => "block_definition",
        EntityKind::BlockInstance => "block_instance",
        EntityKind::Revision => "revision",
        EntityKind::Rail => "rail",
        EntityKind::CabinetPlacement => "cabinet_placement",
    }
}

fn new_file_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    // RandomState is seeded randomly per process; mixing in the time and a
    // counter keeps ids distinct within a process as well.
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    hasher.write_u32(std::process::id());
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    format!("{:016x}", hasher.finish())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `path` with `suffix` appended to its file name.
pub(crate) fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

pub(crate) fn remove_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The version after the newest, for steps made up by these tests.
    const NEXT: u32 = schema::SCHEMA_VERSION + 1;

    /// Every real step plus `step` as the next one.
    fn with(step: fn(&Transaction<'_>) -> rusqlite::Result<()>) -> Vec<Migration> {
        let mut chain = MIGRATIONS.to_vec();
        chain.push(Migration {
            to: NEXT,
            run: step,
        });
        chain
    }

    /// A future step: adds a column and fills it from the record JSON.
    fn future(tx: &Transaction<'_>) -> rusqlite::Result<()> {
        tx.execute_batch(
            "ALTER TABLE records ADD COLUMN name TEXT;
             UPDATE records SET name = json_extract(data, '$.name');",
        )
    }

    #[test]
    fn older_files_are_upgraded_with_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.novo");
        let mut project = Project::new("Old");
        project.transact("Page", |tx| tx.add_page("1")).unwrap();
        ProjectFile::create(&path, &project)
            .unwrap()
            .close()
            .unwrap();

        let chain = with(future);
        let file = ProjectFile::connect(&path, false, &chain).unwrap();
        assert_eq!(schema::user_version(&file.conn).unwrap(), NEXT);
        let name: String = file
            .conn
            .query_row("SELECT name FROM records WHERE kind = 'page'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "1");
        assert_eq!(file.load().unwrap().to_json(), project.to_json());
        file.close().unwrap();

        // The pre-upgrade copy is still a readable file of the old version.
        let backup = dir
            .path()
            .join(format!("old.novo.v{}.bak", schema::SCHEMA_VERSION));
        let old = ProjectFile::open_as(&backup, Role::Project).unwrap();
        assert_eq!(old.load().unwrap().to_json(), project.to_json());

        // Opened again, nothing is left to do.
        let file = ProjectFile::connect(&path, false, &chain).unwrap();
        assert_eq!(schema::user_version(&file.conn).unwrap(), NEXT);
    }

    #[test]
    fn version_1_files_gain_an_empty_snapshot_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.novo");
        let mut project = Project::new("Old");
        project.transact("Page", |tx| tx.add_page("1")).unwrap();
        // A version 1 file is a current one without the snapshot table.
        let file = ProjectFile::create(&path, &project).unwrap();
        file.conn
            .execute_batch("DROP TABLE snapshots; DROP TABLE sidecar; PRAGMA user_version = 1;")
            .unwrap();
        file.close().unwrap();

        assert_eq!(
            ProjectFile::read(&path).unwrap().to_json(),
            project.to_json()
        );
        let file = ProjectFile::open(&path).unwrap();
        assert_eq!(
            schema::user_version(&file.conn).unwrap(),
            schema::SCHEMA_VERSION
        );
        assert_eq!(file.load().unwrap().to_json(), project.to_json());
        assert!(file.snapshot_revisions().unwrap().is_empty());
        assert_eq!(file.sidecar("commissioning").unwrap(), None);
    }

    #[test]
    fn a_failed_upgrade_leaves_the_file_untouched() {
        fn broken(tx: &Transaction<'_>) -> rusqlite::Result<()> {
            tx.execute_batch("ALTER TABLE records ADD COLUMN x TEXT; SELECT * FROM missing;")
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.novo");
        let project = Project::new("Old");
        ProjectFile::create(&path, &project)
            .unwrap()
            .close()
            .unwrap();

        assert!(ProjectFile::connect(&path, false, &with(broken)).is_err());
        let file = ProjectFile::open(&path).unwrap();
        assert_eq!(
            schema::user_version(&file.conn).unwrap(),
            schema::SCHEMA_VERSION
        );
        assert_eq!(file.load().unwrap().to_json(), project.to_json());
    }
}
