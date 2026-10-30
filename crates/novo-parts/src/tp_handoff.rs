//! TraceParts hand-off: no key, no sign-in by KoperCAD.
//!
//! The user's own browser does the downloading. From a part, the app opens
//! TraceParts' search page for the manufacturer part number ([`search_url`]),
//! the user signs in and downloads the STEP model there, and
//! [`TracePartsWatcher`] sees the finished file land in their Downloads
//! folder. The caller judges the file against the part with `cadmatch`
//! (`CadIdentity::read` and `decide_drop`) and then files it with
//! [`file_into_cache`] under the per-user cache ([`cache_dir`]).
//!
//! TraceParts names its downloads after its own internal ids, so unlike the
//! maker watch this one does not require the part number in the file name:
//! any new STEP file, or ZIP holding one, counts as an arrival, and the
//! content check decides whether it is for this part. Nothing here talks to
//! the network, signs in, or reads a page; the search link is only text.
//!
//! The downloaded model stays in the user's own cache (never in the project
//! file or the repository), with a record of where it came from.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::import::{Arrival, MTIME_SLACK};
use crate::library::FileKind;
use crate::search::normalize_code;

/// Where TraceParts' own search is. The `KeyWords` query form is what the
/// site's search box produces; it has not been confirmed with a real
/// session, so the app also offers the plain [`HOME_URL`].
pub const SEARCH_URL: &str = "https://www.traceparts.com/en/search";
/// TraceParts' front page, the fallback when the search link does not work.
pub const HOME_URL: &str = "https://www.traceparts.com/en";

/// Most bytes of a STEP model taken into the cache (a guard against a
/// download that is not a model).
pub const MAX_MODEL_BYTES: u64 = 512 * 1024 * 1024;

/// The link that opens TraceParts' search for `part_number`, or `None` when
/// there is nothing to search for.
pub fn search_url(part_number: &str) -> Option<String> {
    let term = part_number.trim();
    if term.is_empty() {
        return None;
    }
    Some(format!(
        "{SEARCH_URL}?CatalogPath=&KeyWords={}",
        percent_encode(term)
    ))
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(b));
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The per-user folder for models downloaded from TraceParts:
/// `%LOCALAPPDATA%\KoperCAD\cache\traceparts` on Windows.
pub fn cache_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("KoperCAD").join("cache").join("traceparts"))
}

/// Watches the Downloads folder for a model the user saved from TraceParts.
/// Poll it from the UI timer while the search page is open.
pub struct TracePartsWatcher {
    dir: PathBuf,
    since: SystemTime,
    order_code: String,
    last_sizes: HashMap<PathBuf, u64>,
    /// Files already handed out, so each arrives once.
    given: Vec<PathBuf>,
}

impl TracePartsWatcher {
    /// Watches `dir` for files newer than `since` (the time the page was
    /// opened, with [`MTIME_SLACK`] allowed for coarse file clocks).
    pub fn new(dir: impl Into<PathBuf>, since: SystemTime, part_number: &str) -> Self {
        TracePartsWatcher {
            dir: dir.into(),
            since: since.checked_sub(MTIME_SLACK).unwrap_or(since),
            order_code: normalize_code(part_number),
            last_sizes: HashMap::new(),
            given: Vec::new(),
        }
    }

    /// Finished STEP files and ZIPs that arrived since the page was opened.
    /// A file counts once its size has stopped changing between two polls
    /// and it is not a browser's partial download. `names_the_part` tells
    /// whether the file name carries the part number; when it does not, the
    /// caller must check the content before taking the file.
    pub fn poll(&mut self) -> Vec<Arrival> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut ready = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if self.given.contains(&path) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            if [".crdownload", ".part", ".tmp", ".download", ".opdownload"]
                .iter()
                .any(|s| lower.ends_with(s))
            {
                continue;
            }
            if !matches!(
                FileKind::of_file_name(&name),
                FileKind::Step | FileKind::Zip
            ) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || meta.len() == 0 || meta.modified().is_ok_and(|m| m < self.since) {
                continue;
            }
            let size = meta.len();
            if self.last_sizes.insert(path.clone(), size) == Some(size) {
                let code = normalize_code(&name);
                ready.push(Arrival {
                    names_the_part: !self.order_code.is_empty() && code.contains(&self.order_code),
                    path: path.clone(),
                });
                self.given.push(path);
            }
        }
        ready.sort_by(|a, b| a.path.cmp(&b.path));
        ready
    }
}

/// Where a cached model came from; saved beside it as `<name>.source.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheRecord {
    /// The part number the user searched for.
    pub part_number: String,
    /// The STEP file's name in the cache.
    pub file: String,
    /// The name of the download it came from (a ZIP or STEP file).
    pub downloaded_as: String,
    /// SHA-256 of the STEP bytes.
    pub sha256: String,
    /// Always `traceparts.com`; the user's own download.
    pub source: String,
}

/// A model filed in the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filed {
    pub path: PathBuf,
    pub record: CacheRecord,
    /// The model was already in the cache (same bytes); nothing was written.
    pub already_there: bool,
}

/// Why a download could not be filed.
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("could not read {0}: {1}")]
    Read(String, io::Error),
    #[error("{0} is not a readable ZIP ({1})")]
    Zip(String, String),
    #[error("{0} holds no STEP model")]
    NoModel(String),
    #[error("{0} is larger than the {1} MB limit")]
    TooLarge(String, u64),
    #[error("could not write to the cache: {0}")]
    Write(io::Error),
}

/// Reads the STEP bytes of a download: the file itself, or the first STEP
/// member of a ZIP. Nothing is extracted to disk by path, so a member name
/// can never escape the cache.
pub fn read_model(download: &Path) -> Result<Vec<u8>, FileError> {
    let name = download
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let too_large = || FileError::TooLarge(name.clone(), MAX_MODEL_BYTES / (1024 * 1024));
    match FileKind::of_file_name(&name) {
        FileKind::Step => {
            let meta = fs::metadata(download).map_err(|e| FileError::Read(name.clone(), e))?;
            if meta.len() > MAX_MODEL_BYTES {
                return Err(too_large());
            }
            fs::read(download).map_err(|e| FileError::Read(name.clone(), e))
        }
        FileKind::Zip => {
            let f = fs::File::open(download).map_err(|e| FileError::Read(name.clone(), e))?;
            let mut zip = zip::ZipArchive::new(io::BufReader::new(f))
                .map_err(|e| FileError::Zip(name.clone(), e.to_string()))?;
            for i in 0..zip.len() {
                let Ok(member) = zip.by_index(i) else {
                    continue;
                };
                let member_name = String::from_utf8_lossy(member.name_raw()).into_owned();
                if !member.is_file() || FileKind::of_file_name(&member_name) != FileKind::Step {
                    continue;
                }
                if member.size() > MAX_MODEL_BYTES {
                    return Err(too_large());
                }
                let mut bytes = Vec::new();
                member
                    .take(MAX_MODEL_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| FileError::Read(name.clone(), e))?;
                if bytes.len() as u64 > MAX_MODEL_BYTES {
                    return Err(too_large());
                }
                return Ok(bytes);
            }
            Err(FileError::NoModel(name))
        }
        _ => Err(FileError::NoModel(name)),
    }
}

/// Files the STEP model of `download` in `cache_root` under the part number:
/// `<root>/<part number>/<part number>-<hash>.step` plus its source record.
/// The same model filed twice is kept once. The download itself is left in
/// Downloads; removing it is the user's call.
pub fn file_into_cache(
    cache_root: &Path,
    part_number: &str,
    download: &Path,
) -> Result<Filed, FileError> {
    let bytes = read_model(download)?;
    file_bytes_into_cache(
        cache_root,
        part_number,
        &download
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        &bytes,
    )
}

/// [`file_into_cache`] for model bytes already in memory.
pub fn file_bytes_into_cache(
    cache_root: &Path,
    part_number: &str,
    downloaded_as: &str,
    bytes: &[u8],
) -> Result<Filed, FileError> {
    let digest: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let folder = folder_name(part_number);
    let file = format!("{folder}-{}.step", &digest[..12]);
    let dir = cache_root.join(&folder);
    let path = dir.join(&file);
    let record = CacheRecord {
        part_number: part_number.trim().to_owned(),
        file,
        downloaded_as: downloaded_as.to_owned(),
        sha256: digest,
        source: "traceparts.com".into(),
    };
    if path.is_file() {
        return Ok(Filed {
            path,
            record,
            already_there: true,
        });
    }
    fs::create_dir_all(&dir).map_err(FileError::Write)?;
    // Write beside, then rename, so a crash never leaves half a model.
    let tmp = dir.join(format!("{}.partial", record.file));
    fs::write(&tmp, bytes).map_err(FileError::Write)?;
    fs::rename(&tmp, &path).map_err(FileError::Write)?;
    let json = serde_json::to_string_pretty(&record).map_err(|e| FileError::Write(e.into()))?;
    fs::write(path.with_extension("source.json"), json + "\n").map_err(FileError::Write)?;
    Ok(Filed {
        path,
        record,
        already_there: false,
    })
}

/// Models cached for a part number, newest name last.
pub fn cached_models(cache_root: &Path, part_number: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(cache_root.join(folder_name(part_number))) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "step"))
        .collect();
    out.sort();
    out
}

/// A folder name for a part number: letters and digits kept, anything else
/// (slashes, spaces, dots) becomes `_`, so a part number never names a path.
fn folder_name(part_number: &str) -> String {
    let name: String = part_number
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let name = name.trim_matches('_').to_owned();
    if name.is_empty() {
        "unnamed".into()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;

    const STEP: &[u8] = b"ISO-10303-21;\nHEADER;\nFILE_NAME('LC1D09P7.stp','',(''),(''),'','','');\nENDSEC;\nEND-ISO-10303-21;\n";

    fn zip_with(path: &Path, members: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (n, b) in members {
            z.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn search_link_encodes_the_part_number() {
        assert_eq!(
            search_url(" LC1D09P7 ").unwrap(),
            "https://www.traceparts.com/en/search?CatalogPath=&KeyWords=LC1D09P7"
        );
        assert!(
            search_url("2002-1201/ x&y")
                .unwrap()
                .ends_with("2002-1201%2F%20x%26y")
        );
        assert_eq!(search_url("  "), None);
    }

    #[test]
    fn watcher_waits_for_a_stable_file_and_hands_it_out_once() {
        let dl = tempfile::tempdir().unwrap();
        let mut w = TracePartsWatcher::new(dl.path(), SystemTime::now(), "LC1D09P7");
        assert!(w.poll().is_empty());
        fs::write(dl.path().join("model.step.crdownload"), b"x").unwrap();
        fs::write(dl.path().join("readme.pdf"), b"x").unwrap();
        zip_with(&dl.path().join("TP-8812334_STEP.zip"), &[("a/b.stp", STEP)]);
        assert!(w.poll().is_empty(), "first sight only records the size");
        let got = w.poll();
        assert_eq!(got.len(), 1);
        assert!(got[0].path.ends_with("TP-8812334_STEP.zip"));
        assert!(!got[0].names_the_part);
        assert!(w.poll().is_empty(), "each file arrives once");
    }

    #[test]
    fn watcher_flags_a_name_with_the_part_number() {
        let dl = tempfile::tempdir().unwrap();
        let mut w = TracePartsWatcher::new(dl.path(), SystemTime::now(), "LC1D09P7");
        fs::write(dl.path().join("LC1D09P7_3D.stp"), STEP).unwrap();
        w.poll();
        assert!(w.poll()[0].names_the_part);
    }

    #[test]
    fn watcher_ignores_files_older_than_the_click() {
        let dl = tempfile::tempdir().unwrap();
        fs::write(dl.path().join("old.stp"), STEP).unwrap();
        let later = SystemTime::now() + Duration::from_secs(3600);
        let mut w = TracePartsWatcher::new(dl.path(), later, "X");
        w.poll();
        assert!(w.poll().is_empty());
    }

    #[test]
    fn files_a_zip_model_once_with_its_source() {
        let dl = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let zip = dl.path().join("TP-1.zip");
        zip_with(&zip, &[("readme.txt", b"hi"), ("../evil/x.stp", STEP)]);
        let a = file_into_cache(cache.path(), "LC1D09P7", &zip).unwrap();
        assert!(!a.already_there);
        assert_eq!(fs::read(&a.path).unwrap(), STEP);
        assert!(a.path.starts_with(cache.path().join("LC1D09P7")));
        let rec: CacheRecord =
            serde_json::from_slice(&fs::read(a.path.with_extension("source.json")).unwrap())
                .unwrap();
        assert_eq!(rec, a.record);
        assert_eq!(rec.downloaded_as, "TP-1.zip");
        let b = file_into_cache(cache.path(), "LC1D09P7", &zip).unwrap();
        assert!(b.already_there);
        assert_eq!(cached_models(cache.path(), "LC1D09P7"), vec![a.path]);
    }

    #[test]
    fn part_numbers_never_name_a_path() {
        let cache = tempfile::tempdir().unwrap();
        let f = file_bytes_into_cache(cache.path(), "../../etc/x", "m.stp", STEP).unwrap();
        assert!(f.path.starts_with(cache.path()));
        assert_eq!(folder_name("..."), "unnamed");
    }

    #[test]
    fn refuses_a_zip_without_a_model_and_a_broken_zip() {
        let dl = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let none = dl.path().join("n.zip");
        zip_with(&none, &[("a.pdf", b"x")]);
        assert!(matches!(
            file_into_cache(cache.path(), "P", &none),
            Err(FileError::NoModel(_))
        ));
        let bad = dl.path().join("b.zip");
        fs::write(&bad, b"not a zip").unwrap();
        assert!(matches!(
            file_into_cache(cache.path(), "P", &bad),
            Err(FileError::Zip(..))
        ));
    }
}
