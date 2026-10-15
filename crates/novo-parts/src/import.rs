//! The Import button: one click on a part fetches its files into the user's
//! library, with a fallback when the maker's source cannot be fetched.
//!
//! The fetch itself sits behind [`Fetcher`]; this crate ships no network
//! code and no maker address (see `docs/PARTS-LIBRARY-DESIGN.md`, 3b). The
//! app supplies a fetcher for each maker whose source has been verified,
//! and [`ScriptedFetcher`] stands in for tests and UI work.
//!
//! Rules the state machine keeps:
//! - a job fetches one part, started by one click: no bulk, no background;
//! - the first Import for a maker needs the user to accept a notice;
//! - a maker that is not enabled, needs a login or blocks automated access
//!   ends in [`ImportState::Fallback`] (open the maker's page, then import
//!   the file that arrives in Downloads, see [`DownloadsWatcher`]);
//! - nothing leaves the user's machine, and everything is stored through
//!   [`LocalLibrary`].

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use thiserror::Error;

use crate::bmecat::import_bmecat;
use crate::catalog::{CatalogPart, Dimensions, PartKey};
use crate::library::{FileKind, ImportFlow, LocalLibrary, NewSource, SourceRecord};
use crate::step::step_box;

/// A file a maker offers for a part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    pub kind: FileKind,
    pub url: String,
    pub file_name: String,
}

/// Why a fetch did not give a file.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FetchError {
    #[error("the maker's source needs a login")]
    NeedsLogin,
    /// The server refused automated access (403, 429, a consent wall).
    #[error("the maker's server refused the download (status {0})")]
    Blocked(u16),
    #[error("the maker has no files for this part")]
    NotAvailable,
    #[error("the download failed: {0}")]
    Network(String),
}

/// A maker's public source. Implementations live in the app, one per
/// maker, and must follow the limits in the design doc: one request at a
/// time, a pause between requests, honest user agent, stop on 403/429.
pub trait Fetcher {
    /// The files the maker offers for the part.
    fn resolve(&self, key: &PartKey) -> Result<Vec<RemoteFile>, FetchError>;
    /// The content of one file.
    fn fetch(&self, file: &RemoteFile) -> Result<Vec<u8>, FetchError>;
}

/// What the app knows about a maker's source. Loaded from data, so a maker
/// can be switched off without a release. Empty by default: no maker is
/// enabled until its source has been verified.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MakerSource {
    pub manufacturer: String,
    /// Whether the app may fetch from this maker on the user's click.
    #[serde(default)]
    pub fetch_enabled: bool,
    /// The maker's page for a part; `{order}` stands for the order number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_url: Option<String>,
}

impl MakerSource {
    pub fn page_for(&self, order_number: &str) -> Option<String> {
        self.page_url
            .as_ref()
            .map(|t| t.replace("{order}", order_number))
    }
}

/// The makers the app knows.
#[derive(Debug, Clone, Default)]
pub struct SourceTable {
    makers: Vec<MakerSource>,
}

impl SourceTable {
    pub fn new(makers: Vec<MakerSource>) -> Self {
        SourceTable { makers }
    }

    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        Ok(SourceTable::new(serde_json::from_str(text)?))
    }

    pub fn get(&self, manufacturer: &str) -> Option<&MakerSource> {
        let m = crate::search::normalize_code(manufacturer);
        self.makers
            .iter()
            .find(|s| crate::search::normalize_code(&s.manufacturer) == m)
    }
}

/// Why an import fell back to the manual flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackReason {
    NotEnabled,
    NeedsLogin,
    Blocked(u16),
    NotAvailable,
}

/// What an import produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportOutcome {
    pub records: Vec<SourceRecord>,
    /// Parts read from a BMEcat file, for the caller to merge into the catalog.
    pub parts: Vec<CatalogPart>,
    /// Size estimated from a STEP file.
    pub dimensions: Option<Dimensions>,
    /// Files that were stored but could not be read for data, with why.
    pub notes: Vec<String>,
}

/// Where an import is. The UI shows this and nothing else.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportState {
    /// Waiting for the click.
    Idle,
    /// The user must accept the first-use notice for this maker.
    NeedsNotice {
        manufacturer: String,
    },
    Resolving,
    /// `done` of `total` files fetched.
    Fetching {
        done: usize,
        total: usize,
    },
    /// The fetch is not possible; offer "Open page" and the Downloads watch.
    Fallback {
        reason: FallbackReason,
        page_url: Option<String>,
    },
    Done(ImportOutcome),
    Failed(String),
}

impl ImportState {
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            ImportState::Done(_) | ImportState::Failed(_) | ImportState::Fallback { .. }
        )
    }
}

/// One import of one part.
pub struct ImportJob<'a> {
    library: &'a LocalLibrary,
    sources: &'a SourceTable,
    fetcher: &'a dyn Fetcher,
    manufacturer: String,
    order_number: String,
    today: String,
    state: ImportState,
    files: Vec<RemoteFile>,
    outcome: ImportOutcome,
}

impl<'a> ImportJob<'a> {
    /// A job for one part. `today` is the date stored in the records.
    pub fn new(
        library: &'a LocalLibrary,
        sources: &'a SourceTable,
        fetcher: &'a dyn Fetcher,
        manufacturer: &str,
        order_number: &str,
        today: &str,
    ) -> Self {
        ImportJob {
            library,
            sources,
            fetcher,
            manufacturer: manufacturer.to_owned(),
            order_number: order_number.to_owned(),
            today: today.to_owned(),
            state: ImportState::Idle,
            files: Vec::new(),
            outcome: ImportOutcome::default(),
        }
    }

    pub fn state(&self) -> &ImportState {
        &self.state
    }

    fn key(&self) -> PartKey {
        PartKey::new(&self.manufacturer, &self.order_number)
    }

    fn fallback(&mut self, reason: FallbackReason) {
        let page_url = self
            .sources
            .get(&self.manufacturer)
            .and_then(|s| s.page_for(&self.order_number));
        self.state = ImportState::Fallback { reason, page_url };
    }

    /// The user accepted the first-use notice.
    pub fn accept_notice(&mut self) -> Result<(), crate::library::LibraryError> {
        self.library.accept_notice(&self.manufacturer)?;
        if matches!(self.state, ImportState::NeedsNotice { .. }) {
            self.state = ImportState::Idle;
        }
        Ok(())
    }

    /// Moves one step on. Call it until [`ImportState::is_finished`] (or
    /// [`ImportState::NeedsNotice`], which waits for [`ImportJob::accept_notice`]).
    /// Each call may block on the network, so run it off the UI thread.
    pub fn step(&mut self) {
        match self.state.clone() {
            ImportState::Idle => {
                let enabled = self
                    .sources
                    .get(&self.manufacturer)
                    .is_some_and(|s| s.fetch_enabled);
                if !enabled {
                    return self.fallback(FallbackReason::NotEnabled);
                }
                if !self.library.notice_accepted(&self.manufacturer) {
                    self.state = ImportState::NeedsNotice {
                        manufacturer: self.manufacturer.clone(),
                    };
                    return;
                }
                self.state = ImportState::Resolving;
            }
            ImportState::Resolving => match self.fetcher.resolve(&self.key()) {
                Ok(files) if files.is_empty() => self.fallback(FallbackReason::NotAvailable),
                Ok(files) => {
                    self.state = ImportState::Fetching {
                        done: 0,
                        total: files.len(),
                    };
                    self.files = files;
                }
                Err(e) => self.on_fetch_error(e),
            },
            ImportState::Fetching { done, total } => {
                let file = self.files[done].clone();
                match self.fetcher.fetch(&file) {
                    Ok(bytes) => {
                        if let Err(e) = self.keep(
                            &file.file_name,
                            &bytes,
                            Some(&file.url),
                            ImportFlow::Fetched,
                        ) {
                            self.state = ImportState::Failed(e);
                            return;
                        }
                        self.state = if done + 1 == total {
                            ImportState::Done(std::mem::take(&mut self.outcome))
                        } else {
                            ImportState::Fetching {
                                done: done + 1,
                                total,
                            }
                        };
                    }
                    Err(e) => self.on_fetch_error(e),
                }
            }
            ImportState::NeedsNotice { .. }
            | ImportState::Fallback { .. }
            | ImportState::Done(_)
            | ImportState::Failed(_) => {}
        }
    }

    /// Runs steps until the job needs the user or has finished.
    pub fn run(&mut self) -> &ImportState {
        while !self.state.is_finished() && !matches!(self.state, ImportState::NeedsNotice { .. }) {
            self.step();
        }
        &self.state
    }

    fn on_fetch_error(&mut self, e: FetchError) {
        match e {
            FetchError::NeedsLogin => self.fallback(FallbackReason::NeedsLogin),
            FetchError::Blocked(code) => self.fallback(FallbackReason::Blocked(code)),
            FetchError::NotAvailable => self.fallback(FallbackReason::NotAvailable),
            FetchError::Network(m) => self.state = ImportState::Failed(m),
        }
    }

    /// The fallback flow ends here: the file the user downloaded (found by
    /// the [`DownloadsWatcher`] or dropped on the window) is stored and read.
    pub fn import_file(&mut self, path: &Path, flow: ImportFlow) -> &ImportState {
        let result = fs::read(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))
            .and_then(|bytes| {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.keep(&name, &bytes, None, flow)
            });
        self.state = match result {
            Ok(()) => ImportState::Done(std::mem::take(&mut self.outcome)),
            Err(e) => ImportState::Failed(e),
        };
        &self.state
    }

    /// Stores a file and reads what it holds.
    fn keep(
        &mut self,
        name: &str,
        bytes: &[u8],
        url: Option<&str>,
        flow: ImportFlow,
    ) -> Result<(), String> {
        let record = self
            .library
            .store(
                &NewSource {
                    manufacturer: &self.manufacturer,
                    order_number: &self.order_number,
                    file_name: name,
                    url,
                    flow,
                    imported: &self.today,
                },
                bytes,
            )
            .map_err(|e| e.to_string())?;
        match record.kind {
            FileKind::Step => match std::str::from_utf8(bytes)
                .map_err(|_| "not text".to_owned())
                .and_then(|t| step_box(t).map_err(|e| e.to_string()))
            {
                Ok(b) => self.outcome.dimensions = Some(b.as_dimensions()),
                Err(e) => self
                    .outcome
                    .notes
                    .push(format!("{name}: no size read ({e})")),
            },
            FileKind::Bmecat => match std::str::from_utf8(bytes)
                .map_err(|_| "not text".to_owned())
                .and_then(|t| import_bmecat(t, &self.manufacturer).map_err(|e| e.to_string()))
            {
                Ok(parts) => self.outcome.parts.extend(parts),
                Err(e) => self
                    .outcome
                    .notes
                    .push(format!("{name}: no parts read ({e})")),
            },
            FileKind::Dxf | FileKind::Other => {}
        }
        self.outcome.records.push(record);
        Ok(())
    }
}

/// What a [`ScriptedFetcher`] answers for a part: its files, or an error.
pub type ScriptedAnswer = Result<Vec<(RemoteFile, Vec<u8>)>, FetchError>;

/// A fetcher with canned answers, for tests and for building the UI before
/// a real source is verified. Never used by the app in a release.
#[derive(Default)]
pub struct ScriptedFetcher {
    pub files: HashMap<PartKey, ScriptedAnswer>,
}

impl Fetcher for ScriptedFetcher {
    fn resolve(&self, key: &PartKey) -> Result<Vec<RemoteFile>, FetchError> {
        match self.files.get(key) {
            None => Err(FetchError::NotAvailable),
            Some(Err(e)) => Err(e.clone()),
            Some(Ok(list)) => Ok(list.iter().map(|(f, _)| f.clone()).collect()),
        }
    }

    fn fetch(&self, file: &RemoteFile) -> Result<Vec<u8>, FetchError> {
        self.files
            .values()
            .filter_map(|v| v.as_ref().ok())
            .flatten()
            .find(|(f, _)| f == file)
            .map(|(_, b)| b.clone())
            .ok_or(FetchError::NotAvailable)
    }
}

/// The folder a browser saves downloads to: `Downloads` in the user's
/// profile. `None` if unknown.
pub fn default_downloads_dir() -> Option<PathBuf> {
    dirs::download_dir()
}

/// Watches the user's Downloads folder for the file they are downloading
/// from a maker's page. Poll it from the UI timer while the page is open;
/// it only looks at the one folder, only at files newer than the click, and
/// never reads a file until its size has stopped changing.
pub struct DownloadsWatcher {
    dir: PathBuf,
    since: SystemTime,
    order_code: String,
    last_sizes: HashMap<PathBuf, u64>,
}

impl DownloadsWatcher {
    /// Starts watching `dir` for files newer than `since` (the time of the
    /// click) that suit the part.
    pub fn new(dir: impl Into<PathBuf>, since: SystemTime, order_number: &str) -> Self {
        DownloadsWatcher {
            dir: dir.into(),
            since,
            order_code: crate::search::normalize_code(order_number),
            last_sizes: HashMap::new(),
        }
    }

    /// Files that have arrived and finished. A STEP or DXF file must carry
    /// the order number in its name (makers name files after the article);
    /// an XML file is accepted by type since catalog files are named by
    /// date or maker.
    pub fn poll(&mut self) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut ready = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            if [".crdownload", ".part", ".tmp", ".download"]
                .iter()
                .any(|s| lower.ends_with(s))
            {
                continue;
            }
            let kind = FileKind::of_file_name(&name);
            let suits = match kind {
                FileKind::Step | FileKind::Dxf => {
                    crate::search::normalize_code(&name).contains(&self.order_code)
                }
                FileKind::Bmecat => true,
                FileKind::Other => false,
            };
            let Ok(meta) = entry.metadata() else { continue };
            if !suits || !meta.is_file() || meta.modified().is_ok_and(|m| m < self.since) {
                continue;
            }
            let size = meta.len();
            if size > 0 && self.last_sizes.insert(path.clone(), size) == Some(size) {
                ready.push(path);
            }
        }
        ready.sort();
        ready
    }
}
