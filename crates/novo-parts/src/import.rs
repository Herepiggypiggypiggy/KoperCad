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
use crate::csv::import_catalog;
use crate::library::{FileKind, ImportFlow, LocalLibrary, NewSource, SourceRecord};
use crate::recipes::{ApplyContext, GENERIC, MakerRecipe, RecipeReport, recipe_for};
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
    /// Terms the maker's files come under, kept in the source record when a
    /// file names none itself. Filled from data; never guessed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms_url: Option<String>,
    /// Whether `page_url` has been confirmed with a real download. Until
    /// it has, the app says the page may not open the right part.
    #[serde(default)]
    pub page_verified: bool,
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

    /// The makers KoperCAD knows pages for. Every entry has fetching off:
    /// only the user's own download from the maker's page is supported.
    /// `page_verified` marks the pages confirmed by a real download.
    pub fn builtin() -> Self {
        let m = |name: &str, page: Option<&str>, terms: Option<&str>, verified: bool| MakerSource {
            manufacturer: name.into(),
            fetch_enabled: false,
            page_url: page.map(str::to_owned),
            terms_url: terms.map(str::to_owned),
            page_verified: verified,
        };
        SourceTable::new(vec![
            // Pattern seen in search results (`/<country>/<language>/product/<ref>/<slug>/`);
            // the slug and the `ww` site are not confirmed. Confirm with one real download.
            m(
                "Schneider Electric",
                Some("https://www.se.com/ww/en/product/{order}/"),
                Some("https://www.se.com/my/en/about-us/legal/terms-of-use/"),
                false,
            ),
            // Owner's capture of 2002-1201: the CAD button leads here (login required).
            m(
                "WAGO",
                Some("https://wago.partcommunity.com/3d-cad-models/?catalog=wago&part={order}"),
                Some("https://www.cadenas.de/terms-of-use-3d-cad-models"),
                true,
            ),
            // Owner's capture of 3044076: Phoenix's own pages were down; the
            // Eplan Data Portal part page opens, its download needs an Eplan ID.
            m(
                "Phoenix Contact",
                Some("https://dataportal.eplan.com/part-details/PXC.{order}"),
                None,
                true,
            ),
            // No Siemens page address found yet.
            m("Siemens", None, None, false),
        ])
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
    /// XML files found inside a ZIP or EDZ, as (name, text), for the app to
    /// pass to the EPLAN parts reader in `novo-interop`. This crate does not
    /// read them: that reader depends on this one.
    pub xml_members: Vec<(String, String)>,
    /// DXF files, direct or inside a ZIP or EDZ, as (name, text), for the app
    /// to pass to the DXF reader in `novo-interop` to make the part's 2D
    /// outline or symbol candidate. Same reason as `xml_members`.
    pub dxf_members: Vec<(String, String)>,
    /// Files that were stored but could not be read for data, with why.
    pub notes: Vec<String>,
    /// What the maker's recipe read into the parts above.
    pub recipe: Box<RecipeReport>,
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

    /// What has been stored so far. After [`ImportState::Failed`] this still
    /// lists the files that were stored before the failure, so the app can
    /// show them (or remove them) instead of losing track of them.
    pub fn stored_so_far(&self) -> &ImportOutcome {
        &self.outcome
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
        // A ZIP is read once here: its STEP file for the size, its readme for
        // the terms link. The ZIP itself is what gets stored.
        let is_archive = matches!(FileKind::of_file_name(name), FileKind::Zip | FileKind::Edz);
        let (inner_step, terms) = if is_archive {
            match read_zip(bytes) {
                Ok(z) => {
                    // Catalog packages carry BMEcat files; EPLAN data carries other XML.
                    for (member, text) in z.xml {
                        if looks_like_bmecat(&text) {
                            self.read_bmecat(&member, &text);
                        } else {
                            self.outcome.xml_members.push((member, text));
                        }
                    }
                    self.outcome.dxf_members.extend(z.dxf);
                    (z.step, z.terms_url)
                }
                Err(e) => {
                    self.outcome.notes.push(format!("{name}: {e}"));
                    (None, None)
                }
            }
        } else {
            (None, None)
        };
        let terms = terms.or_else(|| {
            self.sources
                .get(&self.manufacturer)
                .and_then(|s| s.terms_url.clone())
        });
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
                    terms_url: terms.as_deref(),
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
            FileKind::Bmecat => match std::str::from_utf8(bytes) {
                Ok(t) => self.read_bmecat(name, t),
                Err(_) => self
                    .outcome
                    .notes
                    .push(format!("{name}: no parts read (not text)")),
            },
            FileKind::Csv => self.read_csv(name, bytes),
            FileKind::Zip | FileKind::Edz => {
                if let Some((inner, text)) = inner_step {
                    match step_box(&text) {
                        Ok(b) => self.outcome.dimensions = Some(b.as_dimensions()),
                        Err(e) => self
                            .outcome
                            .notes
                            .push(format!("{name}: {inner}: no size read ({e})")),
                    }
                } else if record.kind == FileKind::Zip {
                    self.outcome
                        .notes
                        .push(format!("{name}: no STEP file inside"));
                }
            }
            FileKind::Dxf => self
                .outcome
                .dxf_members
                .push((name.to_owned(), String::from_utf8_lossy(bytes).into_owned())),
            FileKind::Other => {}
        }
        self.outcome.records.push(record);
        Ok(())
    }
}

impl ImportJob<'_> {
    fn recipe(&self) -> &'static MakerRecipe {
        recipe_for(&self.manufacturer).unwrap_or(&GENERIC)
    }

    /// Reads BMEcat text into parts and runs the maker's recipe over them.
    fn read_bmecat(&mut self, name: &str, text: &str) {
        match import_bmecat(text, &self.manufacturer) {
            Ok(parts) => self.take_parts(parts, name, "import:bmecat"),
            Err(e) => self
                .outcome
                .notes
                .push(format!("{name}: no parts read ({e})")),
        }
    }

    /// Reads a CSV price list or product table into parts.
    fn read_csv(&mut self, name: &str, bytes: &[u8]) {
        let text = decode_text(bytes);
        match import_catalog(&text, &self.manufacturer) {
            Ok(report) => {
                for (line, why) in &report.skipped {
                    self.outcome
                        .notes
                        .push(format!("{name}: line {line} skipped ({why})"));
                }
                self.take_parts(report.parts, name, "import:csv");
            }
            Err(e) => self
                .outcome
                .notes
                .push(format!("{name}: no parts read ({e})")),
        }
    }

    fn take_parts(&mut self, mut parts: Vec<CatalogPart>, file: &str, source: &str) {
        let ctx = ApplyContext {
            source,
            file,
            imported: &self.today,
        };
        let report = self.recipe().apply(&mut parts, &ctx);
        self.outcome.recipe.absorb(report);
        self.outcome.parts.extend(parts);
    }
}

fn looks_like_bmecat(text: &str) -> bool {
    let head: String = text.chars().take(4096).collect();
    head.to_ascii_uppercase().contains("<BMECAT")
}

/// Text of a downloaded table: UTF-8 (with or without BOM), else the
/// Windows-1252 that German spreadsheet exports use.
fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(t) => t.to_owned(),
        Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
    }
}

fn cp1252(b: u8) -> char {
    match b {
        0x80 => '\u{20ac}',
        0x8a => '\u{160}',
        0x8e => '\u{17d}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x9a => '\u{161}',
        0x9e => '\u{17e}',
        _ => char::from(b),
    }
}

/// What a [`ScriptedFetcher`] answers for a part: its files, or an error.
pub type ScriptedAnswer = Result<Vec<(RemoteFile, Vec<u8>)>, FetchError>;

/// What a maker's ZIP holds.
struct ZipContent {
    /// Name and text of the first STEP file.
    step: Option<(String, String)>,
    /// The first link to terms of use found in a readme.
    terms_url: Option<String>,
    /// XML members (name, text), at most [`ZIP_MAX_XML`].
    xml: Vec<(String, String)>,
    /// DXF members (name, text), at most [`ZIP_MAX_DXF`].
    dxf: Vec<(String, String)>,
}

const ZIP_MAX_DXF: usize = 8;
const ZIP_MAX_XML: usize = 16;
const ZIP_MAX_ENTRIES: usize = 64;
const ZIP_MAX_MEMBER_BYTES: u64 = 200 * 1024 * 1024;
/// All members read from one archive together.
const ZIP_MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

/// Reads a ZIP in memory (nothing is written to disk, so member names can
/// never escape anywhere). Only the first STEP file and text files are read.
fn read_zip(bytes: &[u8]) -> Result<ZipContent, String> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("not a readable ZIP ({e})"))?;
    if archive.len() > ZIP_MAX_ENTRIES {
        return Err(format!(
            "ZIP has {} files, more than {ZIP_MAX_ENTRIES}",
            archive.len()
        ));
    }
    let mut out = ZipContent {
        step: None,
        terms_url: None,
        xml: Vec::new(),
        dxf: Vec::new(),
    };
    let mut total_read = 0u64;
    for i in 0..archive.len() {
        let Ok(file) = archive.by_index(i) else {
            continue;
        };
        if !file.is_file() {
            continue;
        }
        let name = file.name().to_owned();
        let lower = name.to_ascii_lowercase();
        let is_step = FileKind::of_file_name(&name) == FileKind::Step;
        let is_text = lower.ends_with(".txt");
        let is_xml = lower.ends_with(".xml") && out.xml.len() < ZIP_MAX_XML;
        let is_dxf = FileKind::of_file_name(&name) == FileKind::Dxf && out.dxf.len() < ZIP_MAX_DXF;
        if !(is_step && out.step.is_none()
            || is_text && out.terms_url.is_none()
            || is_xml
            || is_dxf)
        {
            continue;
        }
        // Check the declared size first, and keep a budget for the whole archive.
        let room = ZIP_MAX_TOTAL_BYTES - total_read;
        if file.size() > ZIP_MAX_MEMBER_BYTES.min(room) {
            return Err(format!(
                "{name} in the ZIP is too large ({} bytes) to read",
                file.size()
            ));
        }
        let mut raw = Vec::new();
        file.take(ZIP_MAX_MEMBER_BYTES.min(room))
            .read_to_end(&mut raw)
            .map_err(|e| format!("cannot read {name} in the ZIP ({e})"))?;
        total_read += raw.len() as u64;
        let text = String::from_utf8_lossy(&raw).into_owned();
        if is_step {
            out.step = Some((name, text));
        } else if is_xml {
            out.xml.push((name, text));
        } else if is_dxf {
            out.dxf.push((name, text));
        } else {
            out.terms_url = find_terms_url(&text);
        }
    }
    Ok(out)
}

/// The first `http(s)` link in a readme that mentions terms, or failing
/// that nothing: we do not guess which link is the terms.
fn find_terms_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| "()<>[],;\"'".contains(c))
                .trim_end_matches('.')
        })
        .find(|w| {
            let l = w.to_ascii_lowercase();
            (l.starts_with("http://") || l.starts_with("https://")) && l.contains("terms")
        })
        .map(str::to_owned)
}

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

/// How far before the click a file's time may be and still count as new.
/// Covers the coarse file-time clock (a few ms on Linux) and FAT/exFAT's
/// 2 s rounding on a Downloads folder on a removable or network disk.
pub const MTIME_SLACK: std::time::Duration = std::time::Duration::from_secs(2);

/// A finished file found in Downloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrival {
    pub path: PathBuf,
    /// Whether the file name carries the order number. When false (catalog
    /// XML and EDZ files are matched by type alone) the app should ask the
    /// user before importing it.
    pub names_the_part: bool,
}

/// Watches the user's Downloads folder for the file they are downloading
/// from a maker's page. Poll it from the UI timer while the page is open;
/// it only looks at the one folder, only at files newer than the click, and
/// never reads a file until its size has stopped changing.
pub struct DownloadsWatcher {
    dir: PathBuf,
    since: SystemTime,
    /// Codes any of which, in a file name, ties it to the part.
    order_codes: Vec<String>,
    recipe: Option<&'static MakerRecipe>,
    last_sizes: HashMap<PathBuf, u64>,
}

impl DownloadsWatcher {
    /// Starts watching `dir` for files newer than `since` (the time of the
    /// click, give or take [`MTIME_SLACK`]: file times are rounded by the
    /// system clock and by the file system, so a file saved right after the
    /// click can carry a time a little before it. Without the slack such a
    /// file was sometimes missed).
    /// click) that suit the part.
    pub fn new(dir: impl Into<PathBuf>, since: SystemTime, order_number: &str) -> Self {
        DownloadsWatcher {
            dir: dir.into(),
            since: since.checked_sub(MTIME_SLACK).unwrap_or(since),
            order_codes: vec![crate::search::normalize_code(order_number)],
            recipe: None,
            last_sizes: HashMap::new(),
        }
    }

    /// Like [`DownloadsWatcher::new`], with the maker's recipe: the number
    /// variants its files use, the catalog packages it publishes, and the
    /// file types its downloads come as.
    pub fn for_maker(
        dir: impl Into<PathBuf>,
        since: SystemTime,
        manufacturer: &str,
        order_number: &str,
    ) -> Self {
        let mut w = Self::new(dir, since, order_number);
        if let Some(recipe) = recipe_for(manufacturer) {
            w.order_codes = recipe.file_codes(order_number);
            w.recipe = Some(recipe);
        }
        w
    }

    /// Files that have arrived and finished. A STEP or DXF file must carry
    /// the order number in its name (makers name files after the article);
    /// an XML file is accepted by type since catalog files are named by
    /// date or maker.
    pub fn poll(&mut self) -> Vec<Arrival> {
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
            let code = crate::search::normalize_code(&name);
            let names_the_part = self
                .order_codes
                .iter()
                .any(|c| !c.is_empty() && code.contains(c));
            let package = self.recipe.is_some_and(|r| r.recognises_package(&name));
            let kind_ok = self.recipe.is_none_or(|r| r.accepts_kind(kind));
            let suits = kind_ok
                && match kind {
                    FileKind::Step | FileKind::Dxf => names_the_part,
                    FileKind::Zip | FileKind::Csv => names_the_part || package,
                    FileKind::Bmecat | FileKind::Edz => true,
                    FileKind::Other => false,
                };
            let Ok(meta) = entry.metadata() else { continue };
            if !suits || !meta.is_file() || meta.modified().is_ok_and(|m| m < self.since) {
                continue;
            }
            let size = meta.len();
            if size > 0 && self.last_sizes.insert(path.clone(), size) == Some(size) {
                ready.push(Arrival {
                    path,
                    names_the_part,
                });
            }
        }
        ready.sort_by(|a, b| a.path.cmp(&b.path));
        ready
    }
}
