//! Salvage: reading what can be read from a damaged project file and
//! writing it as a recovered copy.
//!
//! A file cut short by a full disk or a bad copy, or with a few overwritten
//! pages, is refused by [`crate::ProjectFile::open`]: opening it for editing
//! would risk making it worse. Salvage never writes to the damaged file. It
//! reads it read-only, object by object, keeps every object that is
//! readable and consistent, and writes them as a new, clean project file
//! beside the original (`<name>.recovered.<ext>`, same extension). The user chooses whether
//! to continue in the copy.
//!
//! What it cannot do: bring back objects on pages the file no longer holds.
//! The report says how many objects were read back, how many were skipped
//! and whether the end of the file was lost, so nothing is claimed that is
//! not there.

use std::path::{Path, PathBuf};

use std::collections::BTreeMap;

use novo_core::{JSON_FORMAT_VERSION, Project, ProjectInfo, Record};
use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::error::{Result, StorageError};
use crate::file::{DecodeError, ProjectFile, decode, kind_name, sibling};
use crate::schema::{self, APPLICATION_ID};

/// What [`salvage`] read back from a damaged file.
#[derive(Debug)]
pub struct Salvage {
    /// The recovered project, as written to [`Salvage::copy`].
    pub project: Project,
    /// The new project file, beside the damaged one. A normal project file.
    pub copy: PathBuf,
    /// Objects read back and kept.
    pub recovered: usize,
    /// Objects found but not kept: unreadable data, an unknown kind, or a
    /// reference to an object that was lost.
    pub skipped: usize,
    /// The skipped objects by kind (`"symbol"`, `"wire"`, ...), as far as
    /// the file said what they were. Does not include objects lost behind
    /// a cut, which cannot be counted.
    pub lost: BTreeMap<String, usize>,
    /// Reading stopped early because part of the file is missing or
    /// unreadable, so more objects than `skipped` may be gone.
    pub cut_short: bool,
    /// The project information (title block texts, rules, standards) could
    /// not be read, so the copy has the defaults. The drawing is intact;
    /// the title block and project rules need to be filled in again.
    pub info_lost: bool,
}

impl Salvage {
    /// What the copy holds, by plain name ("1 page, 0 symbols, 0 wires").
    /// Pages, symbols and wires are always named, even at zero: an empty
    /// page is the thing to notice.
    pub fn kept_text(&self) -> String {
        let mut kept: BTreeMap<&'static str, usize> = BTreeMap::new();
        for record in self.project.records() {
            *kept.entry(kind_name(record.kind())).or_default() += 1;
        }
        let mut parts: Vec<String> = ["page", "symbol", "wire"]
            .into_iter()
            .map(|tag| plain_count(tag, kept.get(tag).copied().unwrap_or(0)))
            .collect();
        for (tag, n) in &kept {
            if !["page", "symbol", "wire"].contains(tag) {
                parts.push(plain_count(tag, *n));
            }
        }
        parts.join(", ")
    }

    /// The objects found but not kept, by plain name ("3 symbols, 1 wire"),
    /// or `None` when nothing that was found was dropped.
    pub fn lost_text(&self) -> Option<String> {
        (!self.lost.is_empty()).then(|| {
            self.lost
                .iter()
                .map(|(tag, n)| plain_count(tag, *n))
                .collect::<Vec<_>>()
                .join(", ")
        })
    }

    /// The full account for a person to check a recovered copy against:
    /// what is in it, what was found but dropped, and what cannot be
    /// counted. One line each, plain words.
    pub fn report_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("In the copy: {}.", self.kept_text())];
        if let Some(lost) = self.lost_text() {
            lines.push(format!("Found but unreadable, left out: {lost}."));
        } else if self.skipped > 0 {
            lines.push(format!(
                "{} objects were found but unreadable and left out.",
                self.skipped
            ));
        }
        if self.cut_short {
            lines.push(
                "The end of the file was lost: pages after that point, and everything drawn on them, may be missing and cannot be counted."
                    .to_owned(),
            );
        }
        if self.info_lost {
            lines.push(
                "The project information (title block, rules, standards) is back to the defaults."
                    .to_owned(),
            );
        }
        lines
    }

    /// One plain sentence for the status bar.
    pub fn summary(&self) -> String {
        let mut text = format!("Recovered {} objects", self.recovered);
        if self.skipped > 0 {
            text.push_str(&format!(", skipped {} unreadable", self.skipped));
        }
        if self.cut_short {
            text.push_str("; the end of the file was lost, so later pages may be missing");
        }
        if self.info_lost {
            text.push_str(
                "; the project information (title block, rules, standards) was unreadable and is back to the defaults, so check it",
            );
        }
        text
    }
}

/// The kind tags a project file stores, as [`kind_name`] writes them.
const KNOWN_TAGS: [&str; 18] = [
    "page",
    "device",
    "symbol",
    "junction",
    "wire",
    "potential",
    "terminal",
    "cable",
    "part",
    "text",
    "potential_system",
    "structure",
    "dismissal",
    "block_definition",
    "block_instance",
    "revision",
    "rail",
    "cabinet_placement",
];

/// "3 symbols", "1 wire" from a stored kind tag.
fn plain_count(tag: &str, n: usize) -> String {
    let word = match tag {
        "potential_system" => "supply system",
        "block_definition" => "block definition",
        "block_instance" => "block",
        "cabinet_placement" => "cabinet placement",
        other => other,
    };
    let plural = if n == 1 {
        word.to_owned()
    } else if let Some(stem) = word.strip_suffix('y') {
        format!("{stem}ies")
    } else {
        format!("{word}s")
    };
    format!("{n} {plural}")
}

/// Reads what can be read from the damaged project file at `path` and
/// writes it as a recovered copy next to it. The damaged file is only read.
///
/// A file that is not a KoperCAD project at all gives
/// [`StorageError::NotAProject`]; one from a newer version
/// [`StorageError::NewerFormat`] (it is not damaged, so it is not salvaged).
pub fn salvage(path: impl AsRef<Path>) -> Result<Salvage> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} does not exist", path.display()),
        )
        .into());
    }
    let not_a_project = || StorageError::NotAProject(path.to_owned());
    // Identify the file from its 100-byte header by hand: SQLite refuses
    // even the header pragmas of a file shorter than its header claims.
    let (app_id, version) = read_header(path)?.ok_or_else(not_a_project)?;
    if app_id != APPLICATION_ID || version == 0 {
        return Err(not_a_project());
    }
    if version > schema::SCHEMA_VERSION {
        return Err(StorageError::NewerFormat {
            path: path.to_owned(),
            found: version,
            supported: schema::SCHEMA_VERSION,
        });
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags).map_err(|_| not_a_project())?;
    // Lets SQLite read a file that is shorter than its header says.
    let _ = conn.execute_batch("PRAGMA writable_schema = ON");

    let mut cut_short = false;
    let mut meta = |key: &str| -> Option<String> {
        match conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()
        {
            Ok(value) => value,
            Err(_) => {
                cut_short = true;
                None
            }
        }
    };
    if let Some(format) = meta("record_format").and_then(|v| v.parse::<u32>().ok())
        && format > JSON_FORMAT_VERSION
    {
        return Err(StorageError::NewerFormat {
            path: path.to_owned(),
            found: format,
            supported: JSON_FORMAT_VERSION,
        });
    }
    let stored_info: Option<ProjectInfo> = meta("info").and_then(|v| serde_json::from_str(&v).ok());
    let info_lost = stored_info.is_none();
    let info = stored_info.unwrap_or_default();
    let next_id: u64 = meta("next_id").and_then(|v| v.parse().ok()).unwrap_or(1);

    let (records, mut skipped, mut lost, rows_cut) = read_records(&conn);
    cut_short |= rows_cut;

    let (project, dropped) = assemble(info, next_id, records);
    skipped += dropped.len();
    for record in &dropped {
        *lost.entry(kind_name(record.kind()).to_owned()).or_default() += 1;
    }
    let recovered = project.records().count();

    let copy = free_copy_name(path);
    let mut file = ProjectFile::create(&copy, &project)?;
    copy_side_tables(&conn, &mut file);
    file.close()?;

    Ok(Salvage {
        project,
        copy,
        recovered,
        skipped,
        lost,
        cut_short,
        info_lost,
    })
}

/// Every record that decodes, how many did not (in all, and by the kind the
/// file stored for them), and whether the scan hit unreadable pages and
/// stopped.
fn read_records(conn: &Connection) -> (Vec<Record>, usize, BTreeMap<String, usize>, bool) {
    let mut records = Vec::new();
    let mut skipped = 0;
    let mut lost = BTreeMap::new();
    let Ok(mut stmt) = conn.prepare("SELECT id, kind, data FROM records ORDER BY id") else {
        return (records, skipped, lost, true);
    };
    let Ok(rows) = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    }) else {
        return (records, skipped, lost, true);
    };
    for row in rows {
        let Ok((id, kind, data)) = row else {
            return (records, skipped, lost, true);
        };
        match decode(&kind, &data) {
            Ok(record) if record.id().raw() == id as u64 => records.push(record),
            Ok(_) | Err(DecodeError::UnknownKind | DecodeError::Damaged(_)) => {
                skipped += 1;
                // A kind tag that is itself damaged is not worth quoting.
                let tag = if KNOWN_TAGS.contains(&kind.as_str()) {
                    kind
                } else {
                    "unidentified object".to_owned()
                };
                *lost.entry(tag).or_default() += 1;
            }
        }
    }
    (records, skipped, lost, false)
}

/// Builds the largest consistent project from `records`. Objects are
/// inserted one at a time under the model's own rules; one that refers to
/// something not (yet) there waits for a later pass, and what is still
/// refused when a pass adds nothing is dropped. Returns the project and
/// the records that were dropped.
fn assemble(info: ProjectInfo, next_id: u64, records: Vec<Record>) -> (Project, Vec<Record>) {
    let mut project = Project::from_records(info.clone(), next_id, [])
        .or_else(|_| Project::from_records(ProjectInfo::default(), next_id, []))
        .expect("an empty project is always valid");
    let mut pending = records;
    loop {
        let before = pending.len();
        let mut again = Vec::new();
        for record in pending {
            if project
                .transact("Salvage", |tx| tx.insert_record(record.clone()))
                .is_err()
            {
                again.push(record);
            }
        }
        pending = again;
        if pending.is_empty() || pending.len() == before {
            break;
        }
    }
    (project, pending)
}

/// `<name>.recovered.<ext>`, or `<name>.recovered-2.<ext>` and so on if that
/// is taken, so an earlier recovered copy is never replaced. `<ext>` is the
/// damaged file's own extension (a project saved as `.kopercad` is recovered
/// as `.kopercad`), or the project extension when it has none.
fn free_copy_name(path: &Path) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| crate::PROJECT_EXTENSION.to_owned());
    let dir = path.parent().unwrap_or(Path::new(""));
    let first = dir.join(format!("{stem}.recovered.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem}.recovered-{n}.{ext}")))
        .find(|p| !p.exists() && !sibling(p, "-journal").exists())
        .expect("an unused name exists")
}

/// Copies the sidecar documents and revision snapshots that can still be
/// read. Best effort: a table that cannot be read is left empty.
fn copy_side_tables(conn: &Connection, file: &mut ProjectFile) {
    if let Ok(mut stmt) = conn.prepare("SELECT key, data FROM sidecar")
        && let Ok(rows) =
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
    {
        for (key, data) in rows.flatten() {
            let _ = file.put_sidecar(&key, &data);
        }
    }
    if let Ok(mut stmt) = conn.prepare("SELECT revision, format, data FROM snapshots")
        && let Ok(rows) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })
    {
        for (revision, format, data) in rows.flatten() {
            let _ = file.put_snapshot_raw(revision, format, &data);
        }
    }
}

/// The application id and schema version from the SQLite header, if the
/// file starts with one.
fn read_header(path: &Path) -> Result<Option<(i32, u32)>> {
    use std::io::Read as _;
    let mut head = [0u8; 100];
    let mut file = std::fs::File::open(path)?;
    let mut filled = 0;
    while filled < head.len() {
        match file.read(&mut head[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    if filled < head.len() || &head[..16] != b"SQLite format 3\0" {
        return Ok(None);
    }
    let be = |at: usize| u32::from_be_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
    Ok(Some((be(68) as i32, be(60))))
}
