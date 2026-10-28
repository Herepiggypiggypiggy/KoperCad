//! Library bundles: your parts, and optionally their CAD files, in one ZIP
//! file that moves to another computer or to a colleague.
//!
//! Two kinds, because the files of a maker are not ours to hand on
//! (`docs/PARTS-LIBRARY-DESIGN.md`, section 3a: no sync of maker files, nothing
//! imported goes into a shared export unless the user chooses to):
//!
//! - **Share**: only the parts you wrote yourself (`provenance.source` is
//!   `authored`). A part that came from a maker's catalog is not included;
//!   its manufacturer and order number go into a list of parts the receiver
//!   may want to import themselves, and nothing else of it (no description,
//!   no price). CAD files are left out unless you confirm that they are yours
//!   to share (you made them, or the maker allows it); the confirmation is
//!   written into the bundle.
//! - **Backup**: everything, including the maker files, for your own use on
//!   another computer. The bundle says so, and opening it asks the user to
//!   confirm that it is their own backup before the files are stored.
//!
//! A bundle is a ZIP with `manifest.json`, `parts.json` (the catalog
//! format), `sources.json` (the file records) and `files/00001.bin` and so on.
//! Files are stored under numbers, never under a path from the bundle, and
//! they go into the library through [`LocalLibrary::store`], which builds the
//! path itself, so a hand-made bundle cannot write anywhere else. Every file
//! is checked against its SHA-256 before it is stored.

use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::write::SimpleFileOptions;

use crate::catalog::{Catalog, CatalogPart, PartKey};
use crate::library::{LocalLibrary, NewSource, SourceRecord};

/// Format written; a bundle with a larger number was made by a newer KoperCAD.
pub const BUNDLE_FORMAT: u32 = 1;

const MANIFEST: &str = "manifest.json";
const PARTS: &str = "parts.json";
const SOURCES: &str = "sources.json";

/// Largest of the three JSON members that is read, in bytes.
const MAX_JSON_BYTES: u64 = 256 * 1024 * 1024;
/// Largest single file that is read from a bundle.
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
/// Most entries a bundle may hold.
const MAX_ENTRIES: usize = 100_000;

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("cannot access {}: {}", path.display(), crate::plain::plain_io_error(source))]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("this is not a KoperCAD library bundle ({0})")]
    NotBundle(String),
    #[error("the bundle is damaged: {0}")]
    Damaged(String),
    #[error("the bundle was made by a newer KoperCAD (format {0})")]
    TooNew(u32),
    #[error("{0}")]
    NeedsConfirmation(String),
    #[error("the parts library cannot be used: {0}")]
    Library(String),
}

/// Who a bundle is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BundleMode {
    /// Your own parts, for other people.
    Share,
    /// Everything, for yourself.
    Backup,
}

/// What to put in a bundle.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub mode: BundleMode,
    /// Put CAD files in the bundle. A backup holds them when this is set;
    /// a share needs `files_are_shareable` as well.
    pub include_files: bool,
    /// The user confirmed that the files are theirs to share. Only read for
    /// [`BundleMode::Share`].
    pub files_are_shareable: bool,
    /// Date written into the bundle, `2026-10-28`.
    pub today: String,
    /// KoperCAD version, for the record.
    pub app_version: String,
}

/// A part the bundle does not hold but its author used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Needed {
    pub manufacturer: String,
    pub order_number: String,
}

/// A file in the bundle: its number, checksum and size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Number of the member `files/<n>.bin`; the same index in `sources.json`.
    pub index: usize,
    pub sha256: String,
    pub bytes: u64,
}

/// What a bundle says about itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub mode: BundleMode,
    pub created: String,
    pub app: String,
    pub parts: usize,
    /// The user confirmed the files may be shared. Always true in a backup.
    #[serde(default)]
    pub files_shareable: bool,
    #[serde(default)]
    pub files: Vec<FileEntry>,
    /// Parts the sender used but did not include (share bundles).
    #[serde(default)]
    pub needs: Vec<Needed>,
    /// Text shown when the bundle is opened.
    #[serde(default)]
    pub notice: String,
}

/// What an export wrote.
#[derive(Debug, Clone, Default)]
pub struct ExportReport {
    pub parts: usize,
    pub files: usize,
    pub bytes: u64,
    /// Parts of a share bundle that are only listed by order number.
    pub listed_only: usize,
    /// Things the user should know: files left out and why.
    pub notes: Vec<String>,
}

fn notice_for(mode: BundleMode, files: bool) -> String {
    match (mode, files) {
        (BundleMode::Share, false) => {
            "Parts written by the sender. No CAD files are included; parts from a maker's \
             catalog are listed by order number only, to import from the maker."
                .into()
        }
        (BundleMode::Share, true) => {
            "Parts written by the sender, with CAD files the sender confirmed are theirs to \
             share. Parts from a maker's catalog are listed by order number only."
                .into()
        }
        (BundleMode::Backup, _) => {
            "A backup of the sender's own parts library, made for their own use on another \
             computer. It may hold files from makers' sites that were not meant to be passed \
             on. Open it only if it is your own backup."
                .into()
        }
    }
}

fn io_err(path: &Path) -> impl Fn(std::io::Error) -> BundleError + '_ {
    move |source| BundleError::Io {
        path: path.to_owned(),
        source,
    }
}

fn zip_err(e: impl std::fmt::Display) -> BundleError {
    BundleError::Damaged(e.to_string())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Writes a bundle of the user's parts to `out`.
///
/// `library` supplies the files; without it no file is included. The ZIP is
/// written beside `out` and renamed, so a failed export leaves nothing.
pub fn export_bundle(
    catalog: &Catalog,
    library: Option<&LocalLibrary>,
    opts: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, BundleError> {
    if opts.mode == BundleMode::Share && opts.include_files && !opts.files_are_shareable {
        return Err(BundleError::NeedsConfirmation(
            "CAD files can only go into a shared bundle once you confirm that they are yours \
             to share: you made them, or the maker allows passing them on."
                .into(),
        ));
    }
    let mut report = ExportReport::default();
    let mut parts: Vec<&CatalogPart> = Vec::new();
    let mut needs = Vec::new();
    for p in catalog.parts() {
        let own = p
            .provenance
            .as_ref()
            .is_some_and(crate::schema::Provenance::redistributable);
        if opts.mode == BundleMode::Backup || own {
            parts.push(p);
        } else {
            needs.push(Needed {
                manufacturer: p.manufacturer.clone(),
                order_number: p.order_number.clone(),
            });
        }
    }
    parts.sort_by(|a, b| {
        a.manufacturer
            .cmp(&b.manufacturer)
            .then_with(|| novo_core::natural_cmp(&a.order_number, &b.order_number))
    });
    needs.sort_by(|a, b| {
        a.manufacturer
            .cmp(&b.manufacturer)
            .then_with(|| novo_core::natural_cmp(&a.order_number, &b.order_number))
    });
    report.listed_only = needs.len();

    // The files of the parts that go in.
    let mut records: Vec<SourceRecord> = Vec::new();
    if opts.include_files {
        match library {
            None => report
                .notes
                .push("There is no parts library, so no files were included.".into()),
            Some(lib) => {
                let wanted: std::collections::HashSet<PartKey> =
                    parts.iter().map(|p| p.key()).collect();
                for r in lib
                    .records()
                    .map_err(|e| BundleError::Library(e.to_string()))?
                {
                    if wanted.contains(&r.key()) {
                        records.push(r);
                    }
                }
            }
        }
    }

    let tmp = out.with_extension("kcpl.tmp");
    let file = File::create(&tmp).map_err(io_err(&tmp))?;
    let mut zip = zip::ZipWriter::new(file);
    let json = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    // Files are mostly STEP text, which compresses well.
    let stored = json;

    let mut entries = Vec::new();
    let mut kept_records = Vec::new();
    let mut members: Vec<(usize, Vec<u8>)> = Vec::new();
    if let Some(lib) = library {
        for r in records {
            let path = lib.path_of(&r);
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let index = entries.len() + 1;
                    entries.push(FileEntry {
                        index,
                        sha256: hex(&Sha256::digest(&bytes)),
                        bytes: bytes.len() as u64,
                    });
                    report.bytes += bytes.len() as u64;
                    members.push((index, bytes));
                    kept_records.push(r);
                }
                Err(e) => report.notes.push(format!(
                    "{} of {} {} was left out: {}",
                    r.file_name,
                    r.manufacturer,
                    r.order_number,
                    crate::plain::plain_io_error(&e)
                )),
            }
        }
    }
    report.parts = parts.len();
    report.files = entries.len();

    let files_shareable = opts.mode == BundleMode::Backup || opts.files_are_shareable;
    let manifest = Manifest {
        format: BUNDLE_FORMAT,
        mode: opts.mode,
        created: opts.today.clone(),
        app: opts.app_version.clone(),
        parts: parts.len(),
        files_shareable,
        files: entries,
        needs,
        notice: notice_for(opts.mode, !kept_records.is_empty()),
    };
    let catalog_json = {
        let sub: Catalog = parts.iter().map(|p| (*p).clone()).collect();
        sub.to_json()
    };
    let write = |zip: &mut zip::ZipWriter<File>| -> Result<(), BundleError> {
        let mut put = |name: &str, bytes: &[u8], opts: SimpleFileOptions| {
            zip.start_file(name, opts).map_err(zip_err)?;
            zip.write_all(bytes).map_err(io_err(&tmp))
        };
        put(
            MANIFEST,
            serde_json::to_string_pretty(&manifest)
                .expect("manifest serializes")
                .as_bytes(),
            json,
        )?;
        put(PARTS, catalog_json.as_bytes(), json)?;
        put(
            SOURCES,
            serde_json::to_string_pretty(&kept_records)
                .expect("records serialize")
                .as_bytes(),
            json,
        )?;
        for (index, bytes) in &members {
            put(&format!("files/{index:05}.bin"), bytes, stored)?;
        }
        Ok(())
    };
    let written = write(&mut zip).and_then(|()| zip.finish().map(drop).map_err(zip_err));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, out) {
        let _ = std::fs::remove_file(&tmp);
        return Err(io_err(out)(e));
    }
    Ok(report)
}

/// A bundle opened for reading. Parts and records are in memory; files are
/// read from the ZIP one at a time when they are stored.
pub struct Bundle {
    pub manifest: Manifest,
    parts: Vec<CatalogPart>,
    records: Vec<SourceRecord>,
    archive: zip::ZipArchive<File>,
}

/// How the bundle's parts compare with the catalog it would go into.
#[derive(Debug, Clone, Default)]
pub struct Preview {
    pub new_parts: Vec<PartKey>,
    /// Parts already in the catalog with the same data.
    pub same: usize,
    /// Parts already in the catalog with different data.
    pub different: Vec<PartKey>,
    /// Files in the bundle, and how many of them the library does not have.
    pub files: usize,
    pub files_new: usize,
}

/// What the user decided about opening a bundle.
#[derive(Debug, Clone, Default)]
pub struct Choices {
    /// A part that exists with different data is replaced by the bundle's.
    /// Without this the existing part is kept.
    pub replace_different: bool,
    /// The user confirmed that a backup bundle is their own.
    pub confirmed_own_backup: bool,
}

/// What opening a bundle changed.
#[derive(Debug, Clone, Default)]
pub struct Applied {
    pub added: usize,
    pub replaced: usize,
    /// Existing parts with different data that were kept.
    pub kept: usize,
    pub same: usize,
    pub files_stored: usize,
    /// Files not stored, and why.
    pub notes: Vec<String>,
}

fn read_member<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, BundleError> {
    let file = archive
        .by_name(name)
        .map_err(|_| BundleError::NotBundle(format!("{name} is missing")))?;
    if file.size() > limit {
        return Err(BundleError::Damaged(format!(
            "{name} is {} bytes, more than the {limit} that are read",
            file.size()
        )));
    }
    let mut out = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut out)
        .map_err(|e| BundleError::Damaged(format!("{name}: {e}")))?;
    if out.len() as u64 > limit {
        return Err(BundleError::Damaged(format!(
            "{name} is larger than it says"
        )));
    }
    Ok(out)
}

impl Bundle {
    /// Opens a bundle and checks its structure; nothing is stored.
    pub fn open(path: &Path) -> Result<Bundle, BundleError> {
        let file = File::open(path).map_err(io_err(path))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| BundleError::NotBundle(format!("not a ZIP file: {e}")))?;
        if archive.len() > MAX_ENTRIES {
            return Err(BundleError::Damaged(format!(
                "the ZIP holds {} entries, more than {MAX_ENTRIES}",
                archive.len()
            )));
        }
        let manifest: Manifest =
            serde_json::from_slice(&read_member(&mut archive, MANIFEST, MAX_JSON_BYTES)?)
                .map_err(|e| BundleError::NotBundle(format!("manifest.json: {e}")))?;
        if manifest.format > BUNDLE_FORMAT {
            return Err(BundleError::TooNew(manifest.format));
        }
        let catalog = {
            let text = read_member(&mut archive, PARTS, MAX_JSON_BYTES)?;
            let text = String::from_utf8(text).map_err(|_| zip_err("parts.json is not text"))?;
            Catalog::parse(&text).map_err(|e| BundleError::Damaged(format!("parts.json: {e}")))?
        };
        let records: Vec<SourceRecord> =
            serde_json::from_slice(&read_member(&mut archive, SOURCES, MAX_JSON_BYTES)?)
                .map_err(|e| BundleError::Damaged(format!("sources.json: {e}")))?;
        if records.len() != manifest.files.len() {
            return Err(BundleError::Damaged(format!(
                "{} file records but {} files listed",
                records.len(),
                manifest.files.len()
            )));
        }
        Ok(Bundle {
            parts: catalog.parts().to_vec(),
            manifest,
            records,
            archive,
        })
    }

    pub fn parts(&self) -> &[CatalogPart] {
        &self.parts
    }

    pub fn file_count(&self) -> usize {
        self.records.len()
    }

    /// Whether opening it must be confirmed by the user: a backup holds
    /// files that were not meant to be passed on.
    pub fn needs_confirmation(&self) -> bool {
        self.manifest.mode == BundleMode::Backup && !self.records.is_empty()
    }

    /// How the bundle compares with `catalog` and `library`.
    pub fn preview(&self, catalog: &Catalog, library: Option<&LocalLibrary>) -> Preview {
        let mut p = Preview::default();
        for part in &self.parts {
            match catalog.get(&part.key()) {
                None => p.new_parts.push(part.key()),
                Some(have) if have == part => p.same += 1,
                Some(_) => p.different.push(part.key()),
            }
        }
        p.files = self.records.len();
        p.files_new = self
            .records
            .iter()
            .filter(|r| {
                let have = library.and_then(|l| l.records_for(&r.key()).ok());
                !have.is_some_and(|h| h.iter().any(|x| x.sha256 == r.sha256))
            })
            .count();
        p
    }

    /// Adds the bundle's parts to `catalog` and its files to `library`. The
    /// caller saves the catalog. Nothing is changed when a confirmation is
    /// missing.
    pub fn apply(
        &mut self,
        catalog: &mut Catalog,
        library: Option<&LocalLibrary>,
        choices: &Choices,
    ) -> Result<Applied, BundleError> {
        if self.needs_confirmation() && !choices.confirmed_own_backup {
            return Err(BundleError::NeedsConfirmation(
                "This is a backup that may hold files from makers' sites. Confirm that it is \
                 your own backup to open it."
                    .into(),
            ));
        }
        let mut done = Applied::default();
        for part in &self.parts {
            match catalog.get(&part.key()) {
                None => {
                    catalog.upsert(part.clone());
                    done.added += 1;
                }
                Some(have) if have == part => done.same += 1,
                Some(_) if choices.replace_different => {
                    catalog.upsert(part.clone());
                    done.replaced += 1;
                }
                Some(_) => done.kept += 1,
            }
        }
        if self.records.is_empty() {
            return Ok(done);
        }
        let Some(lib) = library else {
            done.notes
                .push("There is no parts library, so the files were not stored.".into());
            return Ok(done);
        };
        if !self.manifest.files_shareable && self.manifest.mode == BundleMode::Share {
            done.notes.push(
                "The bundle holds files but does not say they may be shared, so they were not \
                 stored."
                    .into(),
            );
            return Ok(done);
        }
        for (i, record) in self.records.clone().iter().enumerate() {
            let label = format!(
                "{} of {} {}",
                record.file_name, record.manufacturer, record.order_number
            );
            let Some(entry) = self.manifest.files.get(i) else {
                continue;
            };
            if catalog.get(&record.key()).is_none() {
                done.notes.push(format!(
                    "{label} was not stored: the bundle has no such part."
                ));
                continue;
            }
            if entry.bytes > MAX_FILE_BYTES {
                done.notes
                    .push(format!("{label} was not stored: it is too large."));
                continue;
            }
            let bytes = match read_member(
                &mut self.archive,
                &format!("files/{:05}.bin", entry.index),
                MAX_FILE_BYTES,
            ) {
                Ok(b) => b,
                Err(e) => {
                    done.notes.push(format!("{label} was not stored: {e}"));
                    continue;
                }
            };
            if bytes.len() as u64 != entry.bytes || hex(&Sha256::digest(&bytes)) != entry.sha256 {
                done.notes.push(format!(
                    "{label} was not stored: its checksum does not match, so it was changed or damaged."
                ));
                continue;
            }
            let stored = lib.store(
                &NewSource {
                    manufacturer: &record.manufacturer,
                    order_number: &record.order_number,
                    file_name: &record.file_name,
                    url: record.url.as_deref(),
                    flow: record.flow,
                    imported: &record.imported,
                    terms_url: record.terms_url.as_deref(),
                },
                &bytes,
            );
            match stored {
                Ok(_) => done.files_stored += 1,
                Err(e) => done.notes.push(format!("{label} was not stored: {e}")),
            }
        }
        Ok(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::ImportFlow;
    use crate::schema::Provenance;

    fn part(maker: &str, order: &str, source: Option<&str>) -> CatalogPart {
        let mut p = CatalogPart::new(maker, order);
        p.description = format!("{maker} {order} description");
        p.provenance = source.map(|s| Provenance {
            source: s.into(),
            ..Default::default()
        });
        p
    }

    fn source<'a>(p: &'a CatalogPart, name: &'a str) -> NewSource<'a> {
        NewSource {
            manufacturer: &p.manufacturer,
            order_number: &p.order_number,
            file_name: name,
            url: None,
            flow: ImportFlow::Manual,
            imported: "2026-10-28",
            terms_url: None,
        }
    }

    struct Bench {
        dir: tempfile::TempDir,
        catalog: Catalog,
        library: LocalLibrary,
    }

    fn bench() -> Bench {
        let dir = tempfile::tempdir().unwrap();
        let library = LocalLibrary::at(dir.path().join("lib"));
        let mut catalog = Catalog::new();
        let own = part("Workshop", "W-1", Some("authored"));
        let imported = part("Schneider Electric", "LC1D09BD", Some("import:bmecat"));
        let unknown = part("Old", "O-1", None);
        library
            .store(&source(&own, "w-1.stp"), b"STEP of my own part")
            .unwrap();
        library
            .store(&source(&imported, "lc1d09.stp"), b"STEP from the maker")
            .unwrap();
        for p in [own, imported, unknown] {
            catalog.upsert(p);
        }
        Bench {
            dir,
            catalog,
            library,
        }
    }

    fn options(mode: BundleMode, files: bool, shareable: bool) -> ExportOptions {
        ExportOptions {
            mode,
            include_files: files,
            files_are_shareable: shareable,
            today: "2026-10-28".into(),
            app_version: "test".into(),
        }
    }

    fn zip_text(path: &Path) -> String {
        let mut z = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut all = String::new();
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let mut bytes = Vec::new();
            f.read_to_end(&mut bytes).unwrap();
            all.push_str(&String::from_utf8_lossy(&bytes));
        }
        all
    }

    #[test]
    fn a_shared_bundle_holds_only_what_the_user_wrote() {
        let b = bench();
        let out = b.dir.path().join("share.kcpl");
        let r = export_bundle(
            &b.catalog,
            Some(&b.library),
            &options(BundleMode::Share, false, false),
            &out,
        )
        .unwrap();
        assert_eq!((r.parts, r.files, r.listed_only), (1, 0, 2));
        let all = zip_text(&out);
        assert!(all.contains("Workshop W-1 description"));
        // The maker's part and the part of unknown origin are listed by
        // order number only: nothing else of them is in the file.
        assert!(!all.contains("Schneider Electric LC1D09BD description"));
        assert!(!all.contains("Old O-1 description"));
        assert!(all.contains("LC1D09BD") && all.contains("O-1"));
        assert!(!all.contains("STEP from the maker"));
        assert!(!all.contains("STEP of my own part"));
        let bundle = Bundle::open(&out).unwrap();
        assert_eq!(bundle.manifest.mode, BundleMode::Share);
        assert_eq!(bundle.manifest.needs.len(), 2);
        assert!(!bundle.needs_confirmation());
    }

    #[test]
    fn files_go_into_a_shared_bundle_only_with_the_users_confirmation() {
        let b = bench();
        let out = b.dir.path().join("share.kcpl");
        let err = export_bundle(
            &b.catalog,
            Some(&b.library),
            &options(BundleMode::Share, true, false),
            &out,
        )
        .unwrap_err();
        assert!(matches!(err, BundleError::NeedsConfirmation(_)), "{err}");
        assert!(!out.exists(), "nothing was written");
        let r = export_bundle(
            &b.catalog,
            Some(&b.library),
            &options(BundleMode::Share, true, true),
            &out,
        )
        .unwrap();
        // Only the file of the part that is the user's own.
        assert_eq!(r.files, 1);
        let all = zip_text(&out);
        assert!(all.contains("STEP of my own part"));
        assert!(!all.contains("STEP from the maker"));
        let bundle = Bundle::open(&out).unwrap();
        assert!(bundle.manifest.files_shareable);
    }

    #[test]
    fn a_backup_holds_everything_and_asks_before_the_files_are_stored() {
        let b = bench();
        let out = b.dir.path().join("backup.kcpl");
        let r = export_bundle(
            &b.catalog,
            Some(&b.library),
            &options(BundleMode::Backup, true, false),
            &out,
        )
        .unwrap();
        assert_eq!((r.parts, r.files, r.listed_only), (3, 2, 0));

        let dir2 = tempfile::tempdir().unwrap();
        let lib2 = LocalLibrary::at(dir2.path().join("lib"));
        let mut cat2 = Catalog::new();
        let mut bundle = Bundle::open(&out).unwrap();
        assert!(bundle.needs_confirmation());
        let err = bundle
            .apply(&mut cat2, Some(&lib2), &Choices::default())
            .unwrap_err();
        assert!(matches!(err, BundleError::NeedsConfirmation(_)));
        assert!(cat2.is_empty(), "nothing changed without the confirmation");
        assert!(lib2.records().unwrap().is_empty());

        let done = bundle
            .apply(
                &mut cat2,
                Some(&lib2),
                &Choices {
                    confirmed_own_backup: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!((done.added, done.files_stored), (3, 2), "{:?}", done.notes);
        for p in b.catalog.parts() {
            assert_eq!(cat2.get(&p.key()), Some(p), "{}", p.order_number);
        }
        let key = PartKey::new("Schneider Electric", "LC1D09BD");
        let rec = lib2.records_for(&key).unwrap();
        assert_eq!(rec.len(), 1);
        assert_eq!(
            std::fs::read(lib2.path_of(&rec[0])).unwrap(),
            b"STEP from the maker"
        );
    }

    #[test]
    fn opening_a_bundle_twice_changes_nothing_the_second_time() {
        let b = bench();
        let out = b.dir.path().join("backup.kcpl");
        export_bundle(
            &b.catalog,
            Some(&b.library),
            &options(BundleMode::Backup, true, false),
            &out,
        )
        .unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        let lib2 = LocalLibrary::at(dir2.path().join("lib"));
        let mut cat2 = Catalog::new();
        let yes = Choices {
            confirmed_own_backup: true,
            ..Default::default()
        };
        Bundle::open(&out)
            .unwrap()
            .apply(&mut cat2, Some(&lib2), &yes)
            .unwrap();
        let mut again = Bundle::open(&out).unwrap();
        let p = again.preview(&cat2, Some(&lib2));
        assert_eq!(
            (p.new_parts.len(), p.same, p.different.len(), p.files_new),
            (0, 3, 0, 0)
        );
        let done = again.apply(&mut cat2, Some(&lib2), &yes).unwrap();
        assert_eq!(
            (done.added, done.replaced, done.kept, done.same),
            (0, 0, 0, 3)
        );
        assert_eq!(lib2.records().unwrap().len(), 2, "one record per file");
    }

    #[test]
    fn a_part_that_differs_is_kept_unless_the_user_says_to_replace_it() {
        let b = bench();
        let out = b.dir.path().join("share.kcpl");
        export_bundle(
            &b.catalog,
            None,
            &options(BundleMode::Share, false, false),
            &out,
        )
        .unwrap();
        let mut mine = Catalog::new();
        let mut edited = part("Workshop", "W-1", Some("authored"));
        edited.description = "my own wording".into();
        mine.upsert(edited);
        let mut bundle = Bundle::open(&out).unwrap();
        let p = bundle.preview(&mine, None);
        assert_eq!(p.different.len(), 1);
        let kept = bundle.apply(&mut mine, None, &Choices::default()).unwrap();
        assert_eq!((kept.kept, kept.replaced), (1, 0));
        assert_eq!(
            mine.find("Workshop", "W-1").unwrap().description,
            "my own wording"
        );
        let replaced = bundle
            .apply(
                &mut mine,
                None,
                &Choices {
                    replace_different: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(replaced.replaced, 1);
        assert_eq!(
            mine.find("Workshop", "W-1").unwrap().description,
            "Workshop W-1 description"
        );
    }

    fn rewrite(path: &Path, edit: impl Fn(&str, Vec<u8>) -> Vec<u8>) {
        let mut z = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut members = Vec::new();
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_owned();
            let mut bytes = Vec::new();
            f.read_to_end(&mut bytes).unwrap();
            members.push((name.clone(), edit(&name, bytes)));
        }
        let mut w = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, bytes) in members {
            w.start_file(name, SimpleFileOptions::default()).unwrap();
            w.write_all(&bytes).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn a_file_that_was_changed_in_the_bundle_is_not_stored() {
        let b = bench();
        let out = b.dir.path().join("backup.kcpl");
        export_bundle(
            &b.catalog,
            Some(&b.library),
            &options(BundleMode::Backup, true, false),
            &out,
        )
        .unwrap();
        rewrite(&out, |name, bytes| {
            if name == "files/00001.bin" {
                b"something else entirely".to_vec()
            } else {
                bytes
            }
        });
        let dir2 = tempfile::tempdir().unwrap();
        let lib2 = LocalLibrary::at(dir2.path().join("lib"));
        let mut cat2 = Catalog::new();
        let done = Bundle::open(&out)
            .unwrap()
            .apply(
                &mut cat2,
                Some(&lib2),
                &Choices {
                    confirmed_own_backup: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!((done.added, done.files_stored), (3, 1));
        assert!(
            done.notes.iter().any(|n| n.contains("checksum")),
            "{:?}",
            done.notes
        );
    }

    #[test]
    fn a_hand_made_bundle_cannot_write_outside_the_library() {
        let b = bench();
        let out = b.dir.path().join("backup.kcpl");
        export_bundle(
            &b.catalog,
            Some(&b.library),
            &options(BundleMode::Backup, true, false),
            &out,
        )
        .unwrap();
        // Records that name folders and parent directories.
        rewrite(&out, |name, bytes| {
            if name == "sources.json" {
                String::from_utf8(bytes)
                    .unwrap()
                    .replace("w-1.stp", "../../../evil.stp")
                    .replace("sources/", "../../escape/")
                    .into_bytes()
            } else {
                bytes
            }
        });
        let dir2 = tempfile::tempdir().unwrap();
        let root = dir2.path().join("lib");
        let lib2 = LocalLibrary::at(&root);
        let mut cat2 = Catalog::new();
        let done = Bundle::open(&out)
            .unwrap()
            .apply(
                &mut cat2,
                Some(&lib2),
                &Choices {
                    confirmed_own_backup: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(done.files_stored, 2, "{:?}", done.notes);
        let mut found = Vec::new();
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push(p);
                }
            }
        }
        walk(dir2.path(), &mut found);
        assert!(
            found.iter().all(|p| p.starts_with(&root)),
            "a file was written outside the library: {found:?}"
        );
        assert!(!dir2.path().join("evil.stp").exists());
    }

    #[test]
    fn files_that_are_not_bundles_or_are_too_new_or_damaged_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("x.kcpl");
        std::fs::write(&plain, b"this is text").unwrap();
        assert!(matches!(
            Bundle::open(&plain),
            Err(BundleError::NotBundle(_))
        ));
        assert!(matches!(
            Bundle::open(&dir.path().join("missing.kcpl")),
            Err(BundleError::Io { .. })
        ));

        let b = bench();
        let out = dir.path().join("b.kcpl");
        export_bundle(
            &b.catalog,
            Some(&b.library),
            &options(BundleMode::Backup, true, false),
            &out,
        )
        .unwrap();
        let newer = dir.path().join("newer.kcpl");
        std::fs::copy(&out, &newer).unwrap();
        rewrite(&newer, |name, bytes| {
            if name == MANIFEST {
                String::from_utf8(bytes)
                    .unwrap()
                    .replace("\"format\": 1", "\"format\": 99")
                    .into_bytes()
            } else {
                bytes
            }
        });
        assert!(matches!(Bundle::open(&newer), Err(BundleError::TooNew(99))));

        let fewer = dir.path().join("fewer.kcpl");
        std::fs::copy(&out, &fewer).unwrap();
        rewrite(&fewer, |name, bytes| {
            if name == SOURCES {
                b"[]".to_vec()
            } else {
                bytes
            }
        });
        assert!(matches!(Bundle::open(&fewer), Err(BundleError::Damaged(_))));

        let garbled = dir.path().join("garbled.kcpl");
        std::fs::copy(&out, &garbled).unwrap();
        rewrite(&garbled, |name, bytes| {
            if name == PARTS {
                b"{ not json".to_vec()
            } else {
                bytes
            }
        });
        assert!(matches!(
            Bundle::open(&garbled),
            Err(BundleError::Damaged(_))
        ));
    }

    #[test]
    fn the_qa_bundle_is_a_bundle() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../qa/fixtures/parts/library/example-share.kcpl");
        let bundle = Bundle::open(&path).unwrap();
        assert_eq!(bundle.manifest.mode, BundleMode::Share);
        assert_eq!(bundle.parts().len(), 2);
        assert_eq!(bundle.manifest.needs.len(), 1);
        assert!(!bundle.needs_confirmation());
        assert!(bundle.parts().iter().all(|p| {
            p.provenance
                .as_ref()
                .is_some_and(crate::schema::Provenance::redistributable)
        }));
    }

    #[test]
    fn a_failed_export_leaves_no_file() {
        let b = bench();
        let out = b.dir.path().join("no-such-folder").join("x.kcpl");
        let err = export_bundle(
            &b.catalog,
            None,
            &options(BundleMode::Backup, false, false),
            &out,
        );
        assert!(matches!(err, Err(BundleError::Io { .. })));
        assert!(!out.exists());
    }
}
