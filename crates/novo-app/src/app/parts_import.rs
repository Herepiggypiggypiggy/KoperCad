//! The hand-off import behind the Parts window's Import button: open the
//! maker's page, watch the Downloads folder for the file, or take a file
//! the user picked or dropped, store it in the local library and read what
//! it holds. No network code: every file is the user's own download.
//! Design: `docs/PARTS-LIBRARY-DESIGN.md` sections 3a to 3d.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::SystemTime;

use novo_parts::{
    Arrival, Catalog, CatalogPart, DownloadsWatcher, FetchError, Fetcher, ImportFlow, ImportJob,
    ImportState, LocalLibrary, PartKey, RemoteFile, SourceTable, recipe_for,
};

/// Seconds between looks at the Downloads folder.
pub(crate) const POLL_SECONDS: f64 = 1.0;

/// How long the hand-off waits for a download before it says nothing came.
/// A person signing in on a maker's site and finding the file needs minutes,
/// not an hour; past this the strip stops spinning and offers Try again.
pub(crate) const WAIT_LIMIT_SECONDS: f64 = 600.0;

/// How long reading one file (copy, unpack, parse) may take before the
/// window stops waiting for it. The work runs on its own thread, so the
/// window stays usable meanwhile.
pub(crate) const READ_LIMIT_SECONDS: f64 = 120.0;

/// No maker has a verified fetch source yet, so nothing is ever fetched
/// from here; every click falls back to the hand-off flow. A maker's
/// adapter replaces this when its source is verified.
pub(crate) struct NoFetcher;

impl Fetcher for NoFetcher {
    fn resolve(&self, _: &PartKey) -> Result<Vec<RemoteFile>, FetchError> {
        Err(FetchError::NotAvailable)
    }
    fn fetch(&self, _: &RemoteFile) -> Result<Vec<u8>, FetchError> {
        Err(FetchError::NotAvailable)
    }
}

/// Where one import is.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Phase {
    /// The page is open; waiting for the user's download or a dropped file.
    Waiting,
    /// A finished file arrived that does not name the part (catalog XML or
    /// EDZ, matched by type alone): ask before importing it.
    Confirm(PathBuf),
    /// A file is being stored and read on a worker thread.
    Reading,
    Done(Summary),
    Failed(String),
}

/// What an import found, in lines the window shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Summary {
    pub file: String,
    pub lines: Vec<String>,
}

/// One import of one part, from the click to the result.
pub(crate) struct Import {
    pub key: PartKey,
    pub label: String,
    pub page_url: Option<String>,
    pub page_verified: bool,
    /// What the user does on the maker's page, from the maker's recipe.
    pub steps: Vec<String>,
    /// Set when the order number does not look like the maker's.
    pub order_warning: Option<String>,
    pub phase: Phase,
    watcher: Option<DownloadsWatcher>,
    /// Files the user said no to, so the watch does not ask again.
    declined: Vec<PathBuf>,
    pub last_poll: f64,
    /// When the wait began, in the window's clock; set by the first frame.
    waiting_since: Option<f64>,
    /// The window's clock at the last frame.
    now: f64,
    /// The read in progress, while the phase is [`Phase::Reading`].
    read: Option<ReadJob>,
}

/// The result a read thread sends back.
type ReadResult = Result<(Summary, Vec<CatalogPart>), String>;

/// One file being read on its own thread.
struct ReadJob {
    rx: Receiver<ReadResult>,
    file: String,
    /// When the read began, in the window's clock; set by the first frame.
    since: Option<f64>,
}

/// What a finished read changed, for the window to merge into its catalog.
pub(crate) struct ReadDone {
    pub changed: Vec<CatalogPart>,
}

impl Import {
    /// Starts an import of `part`: asks the engine (which says the fetch is
    /// not enabled) and begins watching `downloads`.
    pub(crate) fn start(
        part: &CatalogPart,
        library: &LocalLibrary,
        sources: &SourceTable,
        downloads: Option<&Path>,
        today: &str,
    ) -> Import {
        let mut job = ImportJob::new(
            library,
            sources,
            &NoFetcher,
            &part.manufacturer,
            &part.order_number,
            today,
        );
        let (phase, page_url) = match job.run().clone() {
            ImportState::Fallback { page_url, .. } => (Phase::Waiting, page_url),
            ImportState::NeedsNotice { manufacturer } => (
                Phase::Failed(format!(
                    "Downloading from {manufacturer} inside KoperCAD is not available yet."
                )),
                None,
            ),
            ImportState::Failed(e) => (Phase::Failed(e), None),
            // Idle, Resolving, Fetching and Done cannot be the end state of `run`
            // with a fetcher that has no files.
            _ => (Phase::Waiting, None),
        };
        let recipe = recipe_for(&part.manufacturer);
        // A maker without a confirmed part page: open its home page, whose
        // steps say to search for the number.
        let (page_url, page_verified) = match (page_url, recipe) {
            (None, Some(r)) if phase == Phase::Waiting => {
                (Some(r.page_for(&part.order_number)), true)
            }
            (url, _) => (
                url,
                sources
                    .get(&part.manufacturer)
                    .is_some_and(|s| s.page_verified),
            ),
        };
        Import {
            key: part.key(),
            label: part.to_string(),
            page_url,
            page_verified,
            steps: recipe.map_or_else(Vec::new, |r| r.steps_for(&part.order_number)),
            order_warning: recipe.and_then(|r| r.check_order(&part.order_number)),
            phase,
            declined: Vec::new(),
            watcher: downloads.map(|d| {
                DownloadsWatcher::for_maker(
                    d,
                    SystemTime::now(),
                    &part.manufacturer,
                    &part.order_number,
                )
            }),
            last_poll: f64::NEG_INFINITY,
            waiting_since: None,
            now: 0.0,
            read: None,
        }
    }

    /// Starts reading `path` for `part` on a worker thread; the window
    /// calls [`Import::tick`] each frame to collect the result. `wake`
    /// repaints the window when the thread is done.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn begin_read(
        &mut self,
        part: CatalogPart,
        path: &Path,
        flow: ImportFlow,
        library: LocalLibrary,
        sources: SourceTable,
        catalog_path: Option<PathBuf>,
        today: String,
        wake: impl Fn() + Send + 'static,
    ) {
        let (tx, rx) = channel();
        let file = path.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name("parts-import".into())
            .spawn(move || {
                // A panic in a reader must end as an error on screen, not
                // as a spinner that never stops.
                let result = catch_unwind(AssertUnwindSafe(|| {
                    import_path(
                        &part,
                        &file,
                        flow,
                        &library,
                        &sources,
                        catalog_path.as_deref(),
                        &today,
                    )
                }))
                .unwrap_or_else(|_| Err("the file reader stopped unexpectedly".into()));
                let _ = tx.send(result);
                wake();
            });
        self.read = None;
        match spawned {
            Ok(_) => {
                self.read = Some(ReadJob {
                    rx,
                    file: path
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                    since: None,
                });
                self.phase = Phase::Reading;
            }
            Err(e) => self.phase = Phase::Failed(format!("could not start reading: {e}")),
        }
    }

    /// Advances the clocks: collects a finished read, ends a read or a wait
    /// that ran too long. Returns what a finished read changed.
    pub(crate) fn tick(&mut self, now: f64) -> Option<ReadDone> {
        self.now = now;
        match self.phase {
            Phase::Waiting => {
                let since = *self.waiting_since.get_or_insert(now);
                if now - since > WAIT_LIMIT_SECONDS {
                    self.phase = Phase::Failed(format!(
                        "No file arrived within {} minutes. Nothing was imported.",
                        (WAIT_LIMIT_SECONDS / 60.0).round()
                    ));
                }
                None
            }
            Phase::Reading => {
                let job = self.read.as_mut()?;
                let since = *job.since.get_or_insert(now);
                match job.rx.try_recv() {
                    Ok(Ok((summary, changed))) => {
                        self.read = None;
                        self.phase = Phase::Done(summary);
                        Some(ReadDone { changed })
                    }
                    Ok(Err(e)) => {
                        self.read = None;
                        self.phase = Phase::Failed(e);
                        None
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.read = None;
                        self.phase = Phase::Failed("the file reader stopped unexpectedly".into());
                        None
                    }
                    Err(TryRecvError::Empty) if now - since > READ_LIMIT_SECONDS => {
                        let file = std::mem::take(&mut job.file);
                        self.read = None;
                        self.phase = Phase::Failed(format!(
                            "Reading {file} took longer than {} seconds, so KoperCAD stopped \
waiting. Try a smaller file, or the STEP file alone.",
                            READ_LIMIT_SECONDS.round()
                        ));
                        None
                    }
                    Err(TryRecvError::Empty) => None,
                }
            }
            _ => None,
        }
    }

    /// Puts the import into a read whose result comes over `rx`.
    #[cfg(test)]
    pub(crate) fn install_read_for_test(&mut self, rx: Receiver<ReadResult>, file: &str) {
        self.read = Some(ReadJob {
            rx,
            file: file.into(),
            since: None,
        });
        self.phase = Phase::Reading;
    }

    /// Blocks until a started read has finished (tests, and shutdown paths
    /// that must not leave a half-written library).
    #[cfg(test)]
    pub(crate) fn wait_read(&mut self, limit: std::time::Duration) -> Option<ReadDone> {
        let job = self.read.as_ref()?;
        let outcome = match job.rx.recv_timeout(limit) {
            Ok(r) => r,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err("read timed out".into()),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err("the file reader stopped".into())
            }
        };
        self.read = None;
        match outcome {
            Ok((summary, changed)) => {
                self.phase = Phase::Done(summary);
                Some(ReadDone { changed })
            }
            Err(e) => {
                self.phase = Phase::Failed(e);
                None
            }
        }
    }

    /// How long the hand-off has been waiting, as `m:ss`.
    pub(crate) fn waited_text(&self) -> String {
        let secs = self.waiting_since.map_or(0.0, |s| (self.now - s).max(0.0)) as u64;
        format!("{}:{:02}", secs / 60, secs % 60)
    }

    /// The user said no to this file.
    pub(crate) fn decline(&mut self, path: PathBuf) {
        self.declined.push(path);
        self.phase = Phase::Waiting;
    }

    /// The first finished file the watcher found since the last call.
    pub(crate) fn poll(&mut self, now: f64) -> Option<Arrival> {
        if self.phase != Phase::Waiting || now - self.last_poll < POLL_SECONDS {
            return None;
        }
        self.last_poll = now;
        let declined = &self.declined;
        // A file that names the part wins over one that merely looks like a
        // download: the first is imported at once, the second is asked about.
        let mut fresh: Vec<Arrival> = self
            .watcher
            .as_mut()?
            .poll()
            .into_iter()
            .filter(|a| !declined.contains(&a.path))
            .collect();
        fresh.sort_by_key(|a| !a.names_the_part);
        fresh.into_iter().next()
    }

    /// Names of new downloads the watch passed over because they do not
    /// mention the part, at most three.
    pub(crate) fn passed_over(&self) -> Vec<String> {
        self.watcher.as_ref().map_or_else(Vec::new, |w| {
            w.skipped()
                .iter()
                .take(3)
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect()
        })
    }

    pub(crate) fn watching(&self) -> bool {
        self.watcher.is_some()
    }
}

/// Stores the file in the library and reads it. What the file adds to the
/// part (parts from a BMEcat file, a size read from STEP) goes into the
/// user's catalog at `catalog_path`; the changed parts come back so the
/// open window can show them.
pub(crate) fn import_path(
    part: &CatalogPart,
    path: &Path,
    flow: ImportFlow,
    library: &LocalLibrary,
    sources: &SourceTable,
    catalog_path: Option<&Path>,
    today: &str,
) -> Result<(Summary, Vec<CatalogPart>), String> {
    let mut job = ImportJob::new(
        library,
        sources,
        &NoFetcher,
        &part.manufacturer,
        &part.order_number,
        today,
    );
    let outcome = match job.import_file(path, flow).clone() {
        ImportState::Done(outcome) => outcome,
        ImportState::Failed(e) => return Err(e),
        other => return Err(format!("the import stopped early ({other:?})")),
    };
    let mut changed: Vec<CatalogPart> = outcome.parts.clone();
    // EPLAN XML found inside an EDZ or ZIP: read it as parts too.
    let mut eplan_lines = Vec::new();
    for (name, text) in &outcome.xml_members {
        match super::eplan_parts::read_eplan_parts(text, &part.manufacturer) {
            Ok(r) => {
                eplan_lines.push(format!(
                    "{name}: {} EPLAN part(s) read, {} row(s) rejected.",
                    r.parts.len(),
                    r.skipped.len()
                ));
                changed.extend(r.parts);
            }
            Err(e) => eplan_lines.push(format!("{name}: no EPLAN parts ({e}).")),
        }
    }
    if let Some(d) = outcome.dimensions
        && part.dimensions.is_none()
    {
        changed.push(CatalogPart {
            dimensions: Some(d),
            ..part.clone()
        });
    }
    if !changed.is_empty() {
        let Some(catalog_path) = catalog_path else {
            return Err("no profile folder to save the catalog in".into());
        };
        // An abandoned slow read can still finish while a retry runs: one
        // load-modify-save at a time, and a rename so a half-written file
        // never replaces the catalog.
        static CATALOG_SAVE: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = CATALOG_SAVE.lock().unwrap_or_else(|e| e.into_inner());
        let mut user = if catalog_path.exists() {
            Catalog::load(catalog_path).map_err(|e| e.to_string())?
        } else {
            Catalog::new()
        };
        for p in &changed {
            user.upsert(p.clone());
        }
        let tmp = catalog_path.with_extension("json.tmp");
        user.save(&tmp).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, catalog_path).map_err(|e| e.to_string())?;
    }

    let mut summary = Summary {
        file: outcome
            .records
            .first()
            .map(|r| r.file_name.clone())
            .unwrap_or_default(),
        lines: Vec::new(),
    };
    for r in &outcome.records {
        summary.lines.push(format!(
            "Saved {} ({} bytes) in your parts library.",
            r.file_name, r.bytes
        ));
    }
    if let Some(d) = outcome.dimensions {
        summary.lines.push(format!(
            "Size read from the STEP file: {} \u{d7} {} \u{d7} {} mm{}.",
            d.width_mm,
            d.height_mm,
            d.depth_mm,
            if part.dimensions.is_some() {
                " (the part already has dimensions, left as they are)"
            } else {
                ""
            }
        ));
    }
    if !outcome.parts.is_empty() {
        summary.lines.push(format!(
            "{} part(s) added to your catalog.",
            outcome.parts.len()
        ));
        summary.lines.extend(outcome.recipe.summary_lines());
    }
    summary.lines.extend(eplan_lines);
    if !outcome.dxf_members.is_empty() {
        summary.lines.push(format!(
            "{} DXF file(s) inside are stored; reading them into the part comes later.",
            outcome.dxf_members.len()
        ));
    }
    summary.lines.extend(outcome.notes);
    Ok((summary, changed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn part() -> CatalogPart {
        CatalogPart {
            manufacturer: "WAGO".into(),
            order_number: "2002-1201".into(),
            ..Default::default()
        }
    }

    const STEP: &str = "ISO-10303-21;\nDATA;\n#1=CARTESIAN_POINT('',(0.,0.,0.));\n#2=CARTESIAN_POINT('',(5.2,0.,0.));\n#3=CARTESIAN_POINT('',(5.2,70.,0.));\n#4=CARTESIAN_POINT('',(0.,70.,32.));\nENDSEC;\nEND-ISO-10303-21;\n";

    #[test]
    fn a_click_starts_the_hand_off_with_the_makers_page() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::at(dir.path().join("lib"));
        let imp = Import::start(
            &part(),
            &lib,
            &SourceTable::builtin(),
            Some(dir.path()),
            "2026-10-15",
        );
        assert_eq!(imp.phase, Phase::Waiting);
        assert!(imp.watching());
        assert_eq!(
            imp.page_url.as_deref(),
            Some("https://wago.partcommunity.com/3d-cad-models/?catalog=wago&part=2002-1201")
        );
    }

    #[test]
    fn a_maker_without_a_confirmed_part_page_opens_its_home_page_with_steps() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::at(dir.path().join("lib"));
        let weidmueller = CatalogPart {
            manufacturer: "Weidmüller".into(),
            order_number: "WDU 2.5".into(),
            ..Default::default()
        };
        let imp = Import::start(
            &weidmueller,
            &lib,
            &SourceTable::builtin(),
            None,
            "2026-10-15",
        );
        assert_eq!(imp.page_url.as_deref(), Some("https://www.weidmueller.com"));
        assert!(
            imp.steps.iter().any(|s| s.contains("WDU 2.5")),
            "{:?}",
            imp.steps
        );
        assert!(
            imp.order_warning.is_some(),
            "a type name is not an order number"
        );
        assert!(!imp.watching(), "no Downloads folder known");
        // A maker with no recipe at all keeps the old behaviour: no page, no steps.
        let other = CatalogPart {
            manufacturer: "Nobody".into(),
            order_number: "1".into(),
            ..Default::default()
        };
        let imp = Import::start(&other, &lib, &SourceTable::builtin(), None, "2026-10-15");
        assert!(imp.page_url.is_none() && imp.steps.is_empty());
    }

    #[test]
    fn a_downloaded_bmecat_file_adds_structured_parts_and_says_what_it_read() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::at(dir.path().join("lib"));
        let file = dir.path().join("lapp_bmecat.xml");
        fs::write(
            &file,
            include_str!("../../../novo-parts/tests/golden/lapp/sample.xml"),
        )
        .unwrap();
        let cat = dir.path().join("parts.json");
        let lapp = CatalogPart {
            manufacturer: "Lapp".into(),
            order_number: "1119303".into(),
            ..Default::default()
        };
        let (summary, changed) = import_path(
            &lapp,
            &file,
            ImportFlow::Manual,
            &lib,
            &SourceTable::builtin(),
            Some(&cat),
            "2026-10-15",
        )
        .unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].function, "cable");
        assert!(
            summary
                .lines
                .iter()
                .any(|l| l.contains("structured part data")),
            "{:?}",
            summary.lines
        );
    }

    #[test]
    fn a_downloaded_step_file_is_stored_and_its_size_saved_to_the_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::at(dir.path().join("lib"));
        let file = dir.path().join("2002-1201_HI.stp");
        fs::write(&file, STEP).unwrap();
        let cat = dir.path().join("parts.json");
        let (summary, changed) = import_path(
            &part(),
            &file,
            ImportFlow::Downloads,
            &lib,
            &SourceTable::builtin(),
            Some(&cat),
            "2026-10-15",
        )
        .unwrap();
        assert!(lib.has(&part().key()), "the file is in the library");
        assert_eq!(changed.len(), 1);
        assert!(changed[0].dimensions.is_some());
        assert!(summary.lines.iter().any(|l| l.contains("Saved")));
        let saved = Catalog::load(&cat).unwrap();
        assert!(saved.get(&part().key()).unwrap().dimensions.is_some());
        let rec = &lib.records_for(&part().key()).unwrap()[0];
        assert_eq!(rec.flow, ImportFlow::Downloads);
        assert_eq!(rec.imported, "2026-10-15");
    }

    #[test]
    fn an_unreadable_file_fails_with_a_reason_and_stores_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::at(dir.path().join("lib"));
        let err = import_path(
            &part(),
            &dir.path().join("missing.stp"),
            ImportFlow::Manual,
            &lib,
            &SourceTable::builtin(),
            None,
            "2026-10-15",
        )
        .unwrap_err();
        assert!(err.contains("cannot read"), "{err}");
        assert!(!lib.has(&part().key()));
    }

    #[test]
    fn the_watcher_picks_up_a_file_named_after_the_article() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::at(dir.path().join("lib"));
        let mut imp = Import::start(
            &part(),
            &lib,
            &SourceTable::builtin(),
            Some(dir.path()),
            "2026-10-15",
        );
        assert_eq!(imp.poll(0.0), None, "nothing downloaded yet");
        fs::write(dir.path().join("WAGO_20021201HI.zip"), b"zipdata").unwrap();
        assert_eq!(imp.poll(0.5), None, "polls are a second apart");
        // The first sighting only records the size; the second, unchanged, releases it.
        assert_eq!(imp.poll(1.5), None);
        let got = imp.poll(3.0).expect("finished file");
        assert!(got.path.ends_with("WAGO_20021201HI.zip"));
        assert!(got.names_the_part);
        imp.decline(got.path);
        assert_eq!(imp.poll(5.0), None, "a declined file is not offered again");
    }
}
