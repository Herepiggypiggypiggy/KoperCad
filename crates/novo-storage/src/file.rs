//! One project file: a single SQLite database holding a whole project.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use novo_core::{AnyId, ChangeSet, EntityKind, JSON_FORMAT_VERSION, Project, ProjectInfo, Record};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};

use crate::error::{Result, StorageError, is_busy, is_not_a_database};
use crate::schema::{self, APPLICATION_ID, MIGRATIONS, Migration};

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
    /// overwritten.
    pub fn create(path: impl AsRef<Path>, project: &Project) -> Result<ProjectFile> {
        Self::create_as(path.as_ref(), project, Role::Project, |_| Ok(()))
    }

    /// Opens an existing project file, upgrading its schema if it was written
    /// by an older version.
    pub fn open(path: impl AsRef<Path>) -> Result<ProjectFile> {
        Self::open_as(path.as_ref(), Role::Project)
    }

    /// Opens `path`, reads the project and closes the file again. For tools
    /// that only read, such as the command-line exporter.
    pub fn read(path: impl AsRef<Path>) -> Result<Project> {
        Self::open(path)?.load()
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
        let temp = sibling(path, ".saving");
        remove_if_exists(&temp)?;
        let written = (|| {
            let mut file = Self::connect(&temp, true, MIGRATIONS)?;
            let tx = file.conn.transaction()?;
            set_meta(&tx, "role", role.as_str())?;
            set_meta(&tx, "created_at", &now_millis().to_string())?;
            write_all(&tx, project)?;
            finish_save(&tx)?;
            extra(&tx)?;
            tx.commit()?;
            // Close before moving: Windows cannot rename an open file.
            file.close()
        })();
        if let Err(err) = written {
            let _ = std::fs::remove_file(&temp);
            return Err(err);
        }
        std::fs::rename(&temp, path)?;
        Self::open_as(path, role)
    }

    pub(crate) fn open_as(path: &Path, role: Role) -> Result<ProjectFile> {
        if !path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} does not exist", path.display()),
            )
            .into());
        }
        let file = Self::connect(path, false, MIGRATIONS)?;
        let stored = file.meta("role")?;
        if stored.as_deref() != Some(role.as_str()) {
            return Err(StorageError::NotAProject(path.to_owned()));
        }
        Ok(file)
    }

    /// Opens (and with `create`, creates) a database, takes the exclusive
    /// lock, checks it is ours and runs pending migrations.
    fn connect(path: &Path, create: bool, migrations: &[Migration]) -> Result<ProjectFile> {
        let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        if create {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        let mut conn = Connection::open_with_flags(path, flags)?;
        let not_ours = || StorageError::NotAProject(path.to_owned());
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

        let app_id = schema::application_id(&conn).map_err(|e| classify(e, path))?;
        let version = schema::user_version(&conn)?;
        if create {
            if version != 0 || app_id != 0 {
                return Err(not_ours());
            }
        } else if app_id != APPLICATION_ID || version == 0 {
            return Err(not_ours());
        }
        if !create {
            let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
            if check != "ok" {
                return Err(StorageError::Corrupt {
                    path: path.to_owned(),
                    reason: check,
                });
            }
        }
        schema::migrate(&mut conn, path, migrations)?;
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
        if format != JSON_FORMAT_VERSION {
            return Err(corrupt(format!(
                "records use format {format}, this version reads {JSON_FORMAT_VERSION}"
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

    /// When the file was last written.
    pub fn saved_at(&self) -> Result<Option<SystemTime>> {
        Ok(self
            .meta("saved_at")?
            .and_then(|v| v.parse::<u64>().ok())
            .map(|ms| UNIX_EPOCH + Duration::from_millis(ms)))
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

/// The record's object as JSON, without the record wrapper: the `kind`
/// column already says what it is, so the row holds the plain `Page`,
/// `Wire`, ... and the type is stored only once.
fn encode(record: &Record) -> String {
    match record {
        Record::Page(r) => to_json(r),
        Record::Device(r) => to_json(r),
        Record::Symbol(r) => to_json(r),
        Record::Junction(r) => to_json(r),
        Record::Wire(r) => to_json(r),
        Record::Potential(r) => to_json(r),
        Record::Terminal(r) => to_json(r),
        Record::Cable(r) => to_json(r),
        Record::Part(r) => to_json(r),
    }
}

fn decode(kind: &str, data: &str) -> std::result::Result<Record, String> {
    fn parse<T: serde::de::DeserializeOwned>(data: &str) -> std::result::Result<T, String> {
        serde_json::from_str(data).map_err(|e| e.to_string())
    }
    Ok(match kind {
        "page" => Record::Page(parse(data)?),
        "device" => Record::Device(parse(data)?),
        "symbol" => Record::Symbol(parse(data)?),
        "junction" => Record::Junction(parse(data)?),
        "wire" => Record::Wire(parse(data)?),
        "potential" => Record::Potential(parse(data)?),
        "terminal" => Record::Terminal(parse(data)?),
        "cable" => Record::Cable(parse(data)?),
        "part" => Record::Part(parse(data)?),
        other => return Err(format!("unknown kind {other:?}")),
    })
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
        concat!("NovoElectric ", env!("CARGO_PKG_VERSION")),
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
    }
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

    /// A future step: adds a column and fills it from the record JSON.
    fn v2(tx: &Transaction<'_>) -> rusqlite::Result<()> {
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

        let chain = [MIGRATIONS[0], Migration { to: 2, run: v2 }];
        let file = ProjectFile::connect(&path, false, &chain).unwrap();
        assert_eq!(schema::user_version(&file.conn).unwrap(), 2);
        let name: String = file
            .conn
            .query_row("SELECT name FROM records WHERE kind = 'page'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "1");
        assert_eq!(file.load().unwrap().to_json(), project.to_json());
        file.close().unwrap();

        // The pre-upgrade copy is still a readable version 1 file.
        let backup = dir.path().join("old.novo.v1.bak");
        let old = ProjectFile::open_as(&backup, Role::Project).unwrap();
        assert_eq!(old.load().unwrap().to_json(), project.to_json());

        // Opened again, nothing is left to do.
        let file = ProjectFile::connect(&path, false, &chain).unwrap();
        assert_eq!(schema::user_version(&file.conn).unwrap(), 2);
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

        let chain = [MIGRATIONS[0], Migration { to: 2, run: broken }];
        assert!(ProjectFile::connect(&path, false, &chain).is_err());
        let file = ProjectFile::open(&path).unwrap();
        assert_eq!(schema::user_version(&file.conn).unwrap(), 1);
        assert_eq!(file.load().unwrap().to_json(), project.to_json());
    }
}
