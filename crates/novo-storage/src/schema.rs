//! The SQLite schema and its migrations.
//!
//! The schema version lives in SQLite's `user_version` header field and the
//! file is marked as ours through `application_id`. A new file is an empty
//! database migrated from version 0, so creating a file and upgrading an old
//! one run the same steps.
//!
//! # Adding a migration
//!
//! Append a [`Migration`] to [`MIGRATIONS`] with `to` one higher than the
//! last, and bump nothing else: [`SCHEMA_VERSION`] follows the list. A step
//! gets the open transaction and may run SQL or rewrite rows in Rust (for
//! example to reshape record JSON after a model change). All pending steps of
//! one open run in a single transaction, so a failed upgrade leaves the file
//! untouched, and the original is copied to a backup first.

use std::path::Path;

use rusqlite::{Connection, Transaction};

use crate::error::{Result, StorageError};

/// Marks a SQLite file as a NovoElectric project ("NVOE").
pub const APPLICATION_ID: i32 = 0x4E56_4F45;

/// The schema version this build writes, and the newest it can read.
pub const SCHEMA_VERSION: u32 = latest(MIGRATIONS);

/// One upgrade step, from `to - 1` to `to`.
#[derive(Clone, Copy)]
pub struct Migration {
    pub to: u32,
    pub run: fn(&Transaction<'_>) -> rusqlite::Result<()>,
}

/// Every schema step, oldest first.
pub const MIGRATIONS: &[Migration] = &[Migration { to: 1, run: v1 }];

const fn latest(migrations: &[Migration]) -> u32 {
    match migrations.last() {
        Some(m) => m.to,
        None => 0,
    }
}

/// Version 1: key/value metadata and one row per model record.
///
/// `records.data` is the object as JSON (the serde form of `Page`, `Wire`,
/// ...) and `kind` says which. `page` is a copy kept for indexed queries,
/// such as loading a single page later on.
fn v1(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "CREATE TABLE meta (
             key   TEXT PRIMARY KEY,
             value TEXT NOT NULL
         ) WITHOUT ROWID;
         CREATE TABLE records (
             id   INTEGER PRIMARY KEY,
             kind TEXT NOT NULL,
             page INTEGER,
             data TEXT NOT NULL
         );
         CREATE INDEX records_kind ON records(kind);
         CREATE INDEX records_page ON records(page) WHERE page IS NOT NULL;",
    )
}

pub(crate) fn user_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

pub(crate) fn application_id(conn: &Connection) -> rusqlite::Result<i32> {
    conn.query_row("PRAGMA application_id", [], |r| r.get(0))
}

/// Brings the file up to the newest version in `migrations`. Before
/// upgrading an existing file (version above 0), a copy is written next to
/// it as `<file>.v<old>.bak`.
pub(crate) fn migrate(conn: &mut Connection, path: &Path, migrations: &[Migration]) -> Result<()> {
    let target = latest(migrations);
    let current = user_version(conn)?;
    if current > target {
        return Err(StorageError::NewerFormat {
            path: path.to_owned(),
            found: current,
            supported: target,
        });
    }
    if current == target {
        return Ok(());
    }
    if current > 0 {
        let mut backup = path.as_os_str().to_owned();
        backup.push(format!(".v{current}.bak"));
        let backup = std::path::PathBuf::from(backup);
        if backup.exists() {
            std::fs::remove_file(&backup)?;
        }
        conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])?;
    }
    let tx = conn.transaction()?;
    for step in migrations.iter().filter(|m| m.to > current) {
        (step.run)(&tx)?;
        tx.pragma_update(None, "user_version", step.to)?;
    }
    tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    tx.commit()?;
    Ok(())
}
