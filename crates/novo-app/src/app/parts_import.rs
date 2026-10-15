//! The hand-off import behind the Parts window's Import button: open the
//! maker's page, watch the Downloads folder for the file, or take a file
//! the user picked or dropped, store it in the local library and read what
//! it holds. No network code: every file is the user's own download.
//! Design: `docs/PARTS-LIBRARY-DESIGN.md` sections 3a to 3d.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use novo_parts::{
    Catalog, CatalogPart, DownloadsWatcher, FetchError, Fetcher, ImportFlow, ImportJob,
    ImportState, LocalLibrary, PartKey, RemoteFile, SourceTable,
};

/// Seconds between looks at the Downloads folder.
pub(crate) const POLL_SECONDS: f64 = 1.0;

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
    pub phase: Phase,
    watcher: Option<DownloadsWatcher>,
    pub last_poll: f64,
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
        let page_verified = sources
            .get(&part.manufacturer)
            .is_some_and(|s| s.page_verified);
        Import {
            key: part.key(),
            label: part.to_string(),
            page_url,
            page_verified,
            phase,
            watcher: downloads
                .map(|d| DownloadsWatcher::new(d, SystemTime::now(), &part.order_number)),
            last_poll: f64::NEG_INFINITY,
        }
    }

    /// The first finished file the watcher found since the last call.
    pub(crate) fn poll(&mut self, now: f64) -> Option<PathBuf> {
        if self.phase != Phase::Waiting || now - self.last_poll < POLL_SECONDS {
            return None;
        }
        self.last_poll = now;
        self.watcher.as_mut()?.poll().into_iter().next()
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
        let mut user = if catalog_path.exists() {
            Catalog::load(catalog_path).map_err(|e| e.to_string())?
        } else {
            Catalog::new()
        };
        for p in &changed {
            user.upsert(p.clone());
        }
        user.save(catalog_path).map_err(|e| e.to_string())?;
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
    }
    if !outcome.xml_members.is_empty() || !outcome.dxf_members.is_empty() {
        summary.lines.push(format!(
            "{} EPLAN XML and {} DXF file(s) inside are stored; reading them into the part comes later.",
            outcome.xml_members.len(),
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
        assert!(got.ends_with("WAGO_20021201HI.zip"));
    }
}
