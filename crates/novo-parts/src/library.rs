//! The per-user local library: files a user imported from a maker (STEP,
//! DXF, BMEcat) and a source record for each.
//!
//! Everything lives in the user's profile ([`default_library_root`]), so it
//! needs no admin rights and works offline. The library is never shared,
//! synced or bundled: it holds what this user downloaded for their own use.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::catalog::PartKey;
use crate::search::normalize_code;

/// Version of `records.json`.
pub const LIBRARY_FORMAT_VERSION: u32 = 1;

/// What a stored file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Step,
    Dxf,
    Bmecat,
    /// A price list or product table saved as CSV (a spreadsheet must be saved
    /// as CSV first).
    Csv,
    /// A ZIP holding a STEP or DXF file and a readme, as maker portals serve them.
    Zip,
    /// An EPLAN data package (`.edz`, a ZIP of part data and macros).
    Edz,
    Other,
}

impl FileKind {
    /// The kind a file name suggests, by extension.
    pub fn of_file_name(name: &str) -> FileKind {
        match name
            .rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("stp" | "step" | "p21") => FileKind::Step,
            Some("dxf") => FileKind::Dxf,
            Some("xml" | "bmecat") => FileKind::Bmecat,
            Some("csv") => FileKind::Csv,
            Some("zip") => FileKind::Zip,
            Some("edz") => FileKind::Edz,
            _ => FileKind::Other,
        }
    }
}

/// How a file reached the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportFlow {
    /// The app fetched it after the user clicked Import.
    Fetched,
    /// The user downloaded it in their browser and it arrived in Downloads.
    Downloads,
    /// The user picked or dropped the file.
    Manual,
}

/// Where a stored file came from. One per file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub manufacturer: String,
    pub order_number: String,
    pub kind: FileKind,
    /// The file name as downloaded.
    pub file_name: String,
    /// Path of the stored copy below the library root, with `/` separators.
    pub stored_path: String,
    /// The address it was fetched from, when the app fetched it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The terms the file came with (a link in its readme), kept so the
    /// user can see under what terms they hold it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms_url: Option<String>,
    pub sha256: String,
    pub bytes: u64,
    /// Date of the import as the caller writes it (`2026-10-15`).
    pub imported: String,
    pub flow: ImportFlow,
}

impl SourceRecord {
    pub fn key(&self) -> PartKey {
        PartKey::new(&self.manufacturer, &self.order_number)
    }
}

/// Why the library could not be read or written.
#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("cannot access the parts library {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("the parts library records {path} are not valid: {source}")]
    Format {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("the parts library {path} was written by a newer KoperCAD (format {found})")]
    TooNew { path: PathBuf, found: u32 },
}

#[derive(Serialize, Deserialize, Default)]
struct RecordsFile {
    format: u32,
    #[serde(default)]
    records: Vec<SourceRecord>,
    /// Manufacturers whose first-use notice the user accepted.
    #[serde(default)]
    notices: Vec<String>,
}

/// What the library needs to know to store one file.
#[derive(Debug, Clone, Copy)]
pub struct NewSource<'a> {
    pub manufacturer: &'a str,
    pub order_number: &'a str,
    pub file_name: &'a str,
    pub url: Option<&'a str>,
    pub flow: ImportFlow,
    /// Date of the import as the caller writes it (`2026-10-15`).
    pub imported: &'a str,
    /// Link to the terms that came with the file, if it named any.
    pub terms_url: Option<&'a str>,
}

/// The files and records of one user. Cheap to create; it reads the
/// records file on each call, so two windows never see stale state.
#[derive(Debug, Clone)]
pub struct LocalLibrary {
    root: PathBuf,
}

impl LocalLibrary {
    /// A library below `root`. Nothing is created until something is stored.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        LocalLibrary { root: root.into() }
    }

    /// The library in the user's profile, `None` if no profile folder is known.
    pub fn open_default() -> Option<Self> {
        default_library_root().map(LocalLibrary::at)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn records_path(&self) -> PathBuf {
        self.root.join("records.json")
    }

    fn read(&self) -> Result<RecordsFile, LibraryError> {
        let path = self.records_path();
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(RecordsFile::default()),
            Err(source) => return Err(LibraryError::Io { path, source }),
        };
        let file: RecordsFile =
            serde_json::from_str(&text).map_err(|source| LibraryError::Format {
                path: path.clone(),
                source,
            })?;
        if file.format > LIBRARY_FORMAT_VERSION {
            return Err(LibraryError::TooNew {
                path,
                found: file.format,
            });
        }
        Ok(file)
    }

    fn write(&self, mut file: RecordsFile) -> Result<(), LibraryError> {
        file.format = LIBRARY_FORMAT_VERSION;
        let path = self.records_path();
        let io_err = |source| LibraryError::Io {
            path: path.clone(),
            source,
        };
        fs::create_dir_all(&self.root).map_err(io_err)?;
        let text = serde_json::to_string_pretty(&file).expect("records serialize");
        // Write beside the target and rename, so a crash never leaves half a file.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, text).map_err(io_err)?;
        fs::rename(&tmp, &path).map_err(io_err)
    }

    /// All records, in the order they were stored.
    pub fn records(&self) -> Result<Vec<SourceRecord>, LibraryError> {
        Ok(self.read()?.records)
    }

    /// The records of one part.
    pub fn records_for(&self, key: &PartKey) -> Result<Vec<SourceRecord>, LibraryError> {
        Ok(self
            .read()?
            .records
            .into_iter()
            .filter(|r| &r.key() == key)
            .collect())
    }

    /// Whether any file of the part is stored.
    pub fn has(&self, key: &PartKey) -> bool {
        self.records_for(key).is_ok_and(|r| !r.is_empty())
    }

    /// The absolute path of a stored file.
    pub fn path_of(&self, record: &SourceRecord) -> PathBuf {
        record
            .stored_path
            .split('/')
            .fold(self.root.clone(), |p, part| p.join(part))
    }

    /// Stores a copy of `bytes` and its record. A file of the same part
    /// and name replaces the earlier one.
    pub fn store(&self, source: &NewSource, bytes: &[u8]) -> Result<SourceRecord, LibraryError> {
        let NewSource {
            manufacturer,
            order_number,
            file_name,
            url,
            flow,
            imported,
            terms_url: _,
        } = *source;
        let name = safe_file_name(file_name);
        let stored_path = format!(
            "sources/{}/{}/{}",
            dir_name(&normalize_code(manufacturer)),
            dir_name(&normalize_code(order_number)),
            name
        );
        let record = SourceRecord {
            manufacturer: manufacturer.to_owned(),
            order_number: order_number.to_owned(),
            kind: FileKind::of_file_name(&name),
            file_name: name,
            stored_path,
            url: url.map(str::to_owned),
            terms_url: source.terms_url.map(str::to_owned),
            sha256: hex(&Sha256::digest(bytes)),
            bytes: bytes.len() as u64,
            imported: imported.to_owned(),
            flow,
        };
        // Read the records first: a newer or damaged records file must not
        // leave a stored file nobody knows about.
        let mut file = self.read()?;
        let target = self.path_of(&record);
        let io_err = |source| LibraryError::Io {
            path: target.clone(),
            source,
        };
        fs::create_dir_all(target.parent().expect("stored path has a parent")).map_err(io_err)?;
        fs::write(&target, bytes).map_err(io_err)?;
        file.records.retain(|r| r.stored_path != record.stored_path);
        file.records.push(record.clone());
        self.write(file)?;
        Ok(record)
    }

    /// Removes a part's files and records (the user's "Remove from library").
    pub fn remove(&self, key: &PartKey) -> Result<usize, LibraryError> {
        let mut file = self.read()?;
        let (gone, keep): (Vec<_>, Vec<_>) =
            file.records.into_iter().partition(|r| &r.key() == key);
        for r in &gone {
            let _ = fs::remove_file(self.path_of(r));
        }
        file.records = keep;
        let n = gone.len();
        self.write(file)?;
        Ok(n)
    }

    /// Whether the user accepted the first-use notice for a manufacturer.
    pub fn notice_accepted(&self, manufacturer: &str) -> bool {
        let m = normalize_code(manufacturer);
        self.read()
            .is_ok_and(|f| f.notices.iter().any(|n| normalize_code(n) == m))
    }

    /// Remembers that the user accepted the notice for a manufacturer.
    pub fn accept_notice(&self, manufacturer: &str) -> Result<(), LibraryError> {
        let mut file = self.read()?;
        if !self.notice_accepted(manufacturer) {
            file.notices.push(manufacturer.to_owned());
            self.write(file)?;
        }
        Ok(())
    }
}

/// A folder name from a normalized code: ASCII letters and digits only.
fn dir_name(code: &str) -> String {
    let s: String = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if s.is_empty() { "_".into() } else { s }
}

/// A file name with no folders, no reserved characters and no leading dot.
fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned
        .trim_matches(|c: char| c == '.' || c == ' ')
        .to_owned();
    if cleaned.is_empty() {
        return "file".into();
    }
    // Windows device names (`CON`, `NUL`, `COM1`, also as `nul.step`) cannot be files.
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_uppercase();
    let numbered = stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && matches!(stem.as_bytes()[3], b'1'..=b'9');
    let reserved = numbered || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL");
    if reserved {
        format!("_{cleaned}")
    } else {
        cleaned
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Where the library lives: next to the user's catalog, in
/// `%APPDATA%\KoperCAD\library` on Windows. Inside the profile, so no admin
/// rights. `None` if no profile folder is known.
pub fn default_library_root() -> Option<PathBuf> {
    crate::catalog::user_catalog_path().and_then(|p| p.parent().map(|d| d.join("library")))
}
