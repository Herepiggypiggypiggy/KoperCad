//! The per-user download cache: `%LOCALAPPDATA%\KoperCAD\cache\traceparts`.
//!
//! Downloaded models are personal files of the user. They are never put in
//! a project file, a library bundle or the repository. The cache holds an
//! `index.json` plus one file per model; all writes are atomic (temp file,
//! then rename), a damaged index is rebuilt empty, and the total size is
//! capped by dropping the least recently used files.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::TpError;
use crate::search::normalize_code;

/// How long "TraceParts has no model" is remembered.
pub const MISS_TTL_SECS: u64 = 7 * 24 * 3600;
pub const DEFAULT_CAP_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    pub fetched: u64,
    pub used: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    /// key: normalized part number + `|` + format id.
    #[serde(default)]
    files: BTreeMap<String, Entry>,
    /// key: normalized part number -> when "no model" was learnt.
    #[serde(default)]
    misses: BTreeMap<String, u64>,
}

/// Serialises index read-modify-write within this process.
static INDEX_LOCK: Mutex<()> = Mutex::new(());

pub struct DownloadCache {
    root: PathBuf,
    pub cap_bytes: u64,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn key(part: &str, format: &str) -> String {
    format!("{}|{}", normalize_code(part), format.to_ascii_lowercase())
}

/// STEP text, or a ZIP (TraceParts often zips its output).
pub fn looks_like_cad(bytes: &[u8]) -> Result<(), TpError> {
    if bytes.starts_with(b"PK\x03\x04") {
        return Ok(());
    }
    let head = &bytes[..bytes.len().min(512)];
    if String::from_utf8_lossy(head).contains("ISO-10303-21") {
        return Ok(());
    }
    let what = if bytes.is_empty() {
        "the file is empty"
    } else if head.first() == Some(&b'<') || head.first() == Some(&b'{') {
        "it looks like a web page or an error message"
    } else {
        "it is neither a STEP file nor a ZIP"
    };
    Err(TpError::NotCad(what.into()))
}

impl DownloadCache {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        DownloadCache {
            root: root.into(),
            cap_bytes: DEFAULT_CAP_BYTES,
        }
    }

    pub fn default_cache() -> Option<Self> {
        let dir = novo_storage::profile::data_local_dir()?;
        Some(Self::at(
            dir.join("KoperCAD").join("cache").join("traceparts"),
        ))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    fn load(&self) -> Index {
        fs::read_to_string(self.index_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save(&self, idx: &Index) -> Result<(), TpError> {
        let text = serde_json::to_string_pretty(idx).map_err(|e| TpError::Io(e.to_string()))?;
        write_atomic(&self.index_path(), text.as_bytes())
    }

    /// The cached file for this part and format, if it is still intact.
    /// Marks it as used.
    pub fn lookup(&self, part: &str, format: &str) -> Option<PathBuf> {
        let _lock = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut idx = self.load();
        let k = key(part, format);
        let entry = idx.files.get_mut(&k)?;
        let path = self.root.join(&entry.file);
        match fs::metadata(&path) {
            Ok(m) if m.len() == entry.bytes => {
                entry.used = now();
                let _ = self.save(&idx);
                Some(path)
            }
            _ => {
                idx.files.remove(&k);
                let _ = self.save(&idx);
                None
            }
        }
    }

    /// Stores a download after checking that it is CAD data.
    pub fn store(
        &self,
        part: &str,
        format: &str,
        ext: &str,
        bytes: &[u8],
    ) -> Result<PathBuf, TpError> {
        looks_like_cad(bytes)?;
        let sha: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let ext: String = ext
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(8)
            .collect();
        let name = format!(
            "{}.{}",
            &sha[..24],
            if ext.is_empty() { "bin" } else { &ext }
        );
        let path = self.root.join(&name);
        let _lock = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        write_atomic(&path, bytes)?;
        let mut idx = self.load();
        let t = now();
        idx.misses.remove(&normalize_code(part));
        let k = key(part, format);
        if let Some(old) = idx.files.insert(
            k,
            Entry {
                file: name.clone(),
                bytes: bytes.len() as u64,
                sha256: sha,
                fetched: t,
                used: t,
            },
        ) && old.file != name
            && !idx.files.values().any(|e| e.file == old.file)
        {
            let _ = fs::remove_file(self.root.join(&old.file));
        }
        self.evict(&mut idx, &name);
        self.save(&idx)?;
        Ok(path)
    }

    fn evict(&self, idx: &mut Index, keep: &str) {
        let mut total: u64 = idx.files.values().map(|e| e.bytes).sum();
        while total > self.cap_bytes {
            let Some((k, e)) = idx
                .files
                .iter()
                .filter(|(_, e)| e.file != keep)
                .min_by_key(|(_, e)| e.used)
                .map(|(k, e)| (k.clone(), e.clone()))
            else {
                break;
            };
            idx.files.remove(&k);
            if !idx.files.values().any(|o| o.file == e.file) {
                let _ = fs::remove_file(self.root.join(&e.file));
            }
            total -= e.bytes;
        }
    }

    /// Remembers that TraceParts has no model for this part.
    pub fn note_miss(&self, part: &str) -> Result<(), TpError> {
        let _lock = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut idx = self.load();
        idx.misses.insert(normalize_code(part), now());
        self.save(&idx)
    }

    pub fn recent_miss(&self, part: &str) -> bool {
        self.load()
            .misses
            .get(&normalize_code(part))
            .is_some_and(|t| now().saturating_sub(*t) < MISS_TTL_SECS)
    }

    /// Deletes files the index does not know (left by a crash between
    /// writing a file and saving the index) and stray temp files.
    pub fn sweep(&self) {
        let _lock = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let idx = self.load();
        let Ok(dir) = fs::read_dir(&self.root) else {
            return;
        };
        for e in dir.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "index.json" || idx.files.values().any(|f| f.file == name) {
                continue;
            }
            if e.path().is_file() {
                let _ = fs::remove_file(e.path());
            }
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.load().files.values().map(|e| e.bytes).sum()
    }

    pub fn len(&self) -> usize {
        self.load().files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Deletes every cached model and the index.
    pub fn clear(&self) -> Result<(), TpError> {
        match fs::remove_dir_all(&self.root) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(TpError::Io(e.to_string())),
        }
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), TpError> {
    let io = |e: std::io::Error| TpError::Io(e.to_string());
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(io)?;
    fs::rename(&tmp, path).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: &[u8] = b"ISO-10303-21;\nHEADER;\nENDSEC;\nEND-ISO-10303-21;\n";

    #[test]
    fn stores_and_finds_by_normalized_part_number() {
        let dir = tempfile::tempdir().unwrap();
        let c = DownloadCache::at(dir.path());
        let p = c.store("LC1D09BD", "step", "stp", STEP).unwrap();
        assert_eq!(fs::read(&p).unwrap(), STEP);
        assert_eq!(c.lookup("lc1d09 bd", "STEP"), Some(p));
        assert!(c.lookup("LC1D12BD", "step").is_none());
    }

    #[test]
    fn refuses_a_login_page() {
        let dir = tempfile::tempdir().unwrap();
        let c = DownloadCache::at(dir.path());
        let e = c
            .store("X", "step", "stp", b"<html>sign in</html>")
            .unwrap_err();
        assert!(matches!(e, TpError::NotCad(_)));
        assert!(c.is_empty());
    }

    #[test]
    fn a_truncated_or_deleted_file_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let c = DownloadCache::at(dir.path());
        let p = c.store("A1", "step", "stp", STEP).unwrap();
        fs::write(&p, b"ISO").unwrap();
        assert!(c.lookup("A1", "step").is_none());
        assert!(c.is_empty());
    }

    #[test]
    fn a_damaged_index_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let c = DownloadCache::at(dir.path());
        fs::write(dir.path().join("index.json"), "garbage").unwrap();
        assert!(c.is_empty());
        c.store("A1", "step", "stp", STEP).unwrap();
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn the_cap_drops_the_least_recently_used() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = DownloadCache::at(dir.path());
        let big = |n: u8| {
            let mut v = STEP.to_vec();
            v.extend(vec![n; 1000]);
            v
        };
        c.cap_bytes = 2200;
        c.store("A", "step", "stp", &big(1)).unwrap();
        // Make A older than B.
        let mut idx = c.load();
        idx.files.values_mut().for_each(|e| e.used = 1);
        c.save(&idx).unwrap();
        c.store("B", "step", "stp", &big(2)).unwrap();
        c.store("C", "step", "stp", &big(3)).unwrap();
        assert!(c.lookup("A", "step").is_none());
        assert!(c.lookup("C", "step").is_some());
        assert!(c.total_bytes() <= 2200);
    }

    #[test]
    fn misses_expire_and_a_store_clears_them() {
        let dir = tempfile::tempdir().unwrap();
        let c = DownloadCache::at(dir.path());
        c.note_miss("Z9").unwrap();
        assert!(c.recent_miss("z-9"));
        let mut idx = c.load();
        idx.misses
            .values_mut()
            .for_each(|t| *t = now() - MISS_TTL_SECS - 1);
        c.save(&idx).unwrap();
        assert!(!c.recent_miss("Z9"));
        c.note_miss("Z9").unwrap();
        c.store("Z9", "step", "stp", STEP).unwrap();
        assert!(!c.recent_miss("Z9"));
    }

    #[test]
    fn sweep_removes_unindexed_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let c = DownloadCache::at(dir.path());
        let kept = c.store("A", "step", "stp", STEP).unwrap();
        let stray = dir.path().join("orphan.stp");
        fs::write(&stray, STEP).unwrap();
        c.sweep();
        assert!(kept.exists());
        assert!(!stray.exists());
        assert!(c.lookup("A", "step").is_some());
    }

    #[test]
    fn clear_removes_everything() {
        let dir = tempfile::tempdir().unwrap();
        let c = DownloadCache::at(dir.path().join("tp"));
        c.store("A", "step", "stp", STEP).unwrap();
        c.clear().unwrap();
        assert!(c.is_empty());
        c.clear().unwrap();
    }
}
