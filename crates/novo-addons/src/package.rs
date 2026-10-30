//! Reading a `.kcaddon` file: a zip with `manifest.toml`, optional `signature`, an optional
//! `engine.wasm` and data files. Reading is where hostile files are stopped: nothing is written
//! to disk here, every entry name is checked, expansion is counted as it happens (a header that
//! lies about its size gets nowhere), and every file must match the SHA-256 in the manifest.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};

use crate::error::{AddonError, Result};
use crate::icons::{self, IconDef};
use crate::layout::PanelLayout;
use crate::manifest::{ICONS_FILE, MAX_MANIFEST_BYTES, Manifest, check_path};
use crate::trust::{Trust, TrustStore, judge, sha256_hex};

/// File extension of an add-on package.
pub const EXTENSION: &str = "kcaddon";

/// Safety limits for reading a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The `.kcaddon` file itself.
    pub max_package_bytes: u64,
    pub max_entries: usize,
    /// One file after expansion.
    pub max_file_bytes: u64,
    /// All files after expansion.
    pub max_total_bytes: u64,
    /// Expansion factor beyond which a file larger than 1 MiB is refused.
    pub max_ratio: u64,
}

impl Limits {
    pub const DEFAULT: Limits = Limits {
        max_package_bytes: 64 * 1024 * 1024,
        max_entries: 1000,
        max_file_bytes: 32 * 1024 * 1024,
        max_total_bytes: 128 * 1024 * 1024,
        max_ratio: 200,
    };
}

impl Default for Limits {
    fn default() -> Self {
        Limits::DEFAULT
    }
}

/// A package that was read and verified. Nothing of it is on disk yet.
#[derive(Debug, Clone)]
pub struct Package {
    pub manifest: Manifest,
    pub manifest_bytes: Vec<u8>,
    /// Every file listed in the manifest, by package path.
    pub files: BTreeMap<String, Vec<u8>>,
    pub signature: Option<Vec<u8>>,
    pub trust: Trust,
    /// SHA-256 of `manifest.toml`; it names every other file's hash, so it names the content.
    pub fingerprint: String,
    /// SHA-256 of the whole `.kcaddon` file.
    pub file_sha256: String,
    /// The add-on's own icons (`data/icons.toml`), validated.
    pub icons: Vec<IconDef>,
}

impl Package {
    /// Reads and verifies `bytes` with the default limits.
    pub fn read(bytes: &[u8], trust: &TrustStore) -> Result<Package> {
        Package::read_with(bytes, trust, Limits::DEFAULT)
    }

    pub fn read_with(bytes: &[u8], trust: &TrustStore, limits: Limits) -> Result<Package> {
        if bytes.len() as u64 > limits.max_package_bytes {
            return Err(AddonError::TooLarge(format!(
                "the file is {} MiB; the limit is {} MiB",
                bytes.len() / (1024 * 1024),
                limits.max_package_bytes / (1024 * 1024)
            )));
        }
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| AddonError::NotAPackage(e.to_string()))?;
        if archive.len() > limits.max_entries {
            return Err(AddonError::TooLarge(format!(
                "{} entries; the limit is {}",
                archive.len(),
                limits.max_entries
            )));
        }

        let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut lowered = BTreeSet::new();
        let mut total: u64 = 0;
        for i in 0..archive.len() {
            let file = archive
                .by_index(i)
                .map_err(|e| AddonError::NotAPackage(e.to_string()))?;
            let name = file
                .name()
                .map_err(|e| AddonError::NotAPackage(e.to_string()))?
                .into_owned();
            let is_dir = file.is_dir();
            check_path(name.trim_end_matches('/'))?;
            if file.encrypted() {
                return Err(AddonError::NotAPackage(format!("{name} is encrypted")));
            }
            // A symbolic link in the zip would point outside the add-on folder once unpacked
            // by another tool; refuse the file rather than guess what it means.
            if file.unix_mode().is_some_and(|m| m & 0o170_000 == 0o120_000) {
                return Err(AddonError::UnsafePath(format!("{name:?}: a symbolic link")));
            }
            if !lowered.insert(name.trim_end_matches('/').to_lowercase()) {
                return Err(AddonError::UnsafePath(format!(
                    "{name:?}: the name appears twice (names are compared without case)"
                )));
            }
            if is_dir {
                continue;
            }
            let cap = if name == "manifest.toml" {
                MAX_MANIFEST_BYTES as u64
            } else {
                limits.max_file_bytes
            };
            if file.size() > cap {
                return Err(AddonError::TooLarge(format!(
                    "{name} claims {} MiB; the limit is {} MiB",
                    file.size() / (1024 * 1024),
                    cap / (1024 * 1024)
                )));
            }
            let compressed = file.compressed_size().max(1);
            let mut data = Vec::new();
            file.take(cap + 1)
                .read_to_end(&mut data)
                .map_err(|e| AddonError::NotAPackage(format!("{name}: {e}")))?;
            let actual = data.len() as u64;
            if actual > cap {
                return Err(AddonError::TooLarge(format!(
                    "{name} expands beyond {} MiB",
                    cap / (1024 * 1024)
                )));
            }
            if actual > 1024 * 1024 && actual / compressed > limits.max_ratio {
                return Err(AddonError::TooLarge(format!(
                    "{name} expands {}-fold, which no real add-on file does",
                    actual / compressed
                )));
            }
            total = total.saturating_add(actual);
            if total > limits.max_total_bytes {
                return Err(AddonError::TooLarge(format!(
                    "the files add up to more than {} MiB",
                    limits.max_total_bytes / (1024 * 1024)
                )));
            }
            entries.insert(name, data);
        }

        let manifest_bytes = entries
            .remove("manifest.toml")
            .ok_or_else(|| AddonError::Manifest("the package has no manifest.toml".into()))?;
        let signature = entries.remove("signature");
        let manifest = Manifest::parse(&manifest_bytes)?;
        if let Some(engine) = &manifest.engine
            && entries
                .get(&engine.file)
                .is_some_and(|wasm| wasm.len() > crate::host::MAX_ENGINE_BYTES)
        {
            return Err(AddonError::TooLarge(format!(
                "{} is larger than {} MiB",
                engine.file,
                crate::host::MAX_ENGINE_BYTES / (1024 * 1024)
            )));
        }

        // Every file is listed and matches; every listed file is there.
        for name in entries.keys() {
            if !manifest.files.contains_key(name) {
                return Err(AddonError::Integrity(format!(
                    "{name} is in the package but not listed in the manifest"
                )));
            }
        }
        for (name, want) in &manifest.files {
            let Some(data) = entries.get(name) else {
                return Err(AddonError::Integrity(format!(
                    "{name} is listed in the manifest but missing from the package"
                )));
            };
            if &sha256_hex(data) != want {
                return Err(AddonError::Integrity(format!("{name} has been changed")));
            }
        }

        let trust = judge(&manifest_bytes, signature.as_deref(), trust)?;
        // A revoked key still gives a readable package (the install prompt names it and refuses);
        // a signature that does not fit is a damaged or doctored file.
        if let Trust::Tampered { .. } = &trust {
            return Err(AddonError::Tampered(trust.refusal().unwrap_or_default()));
        }
        let icons = match entries.get(ICONS_FILE) {
            Some(data) => icons::parse(&manifest.addon.id, data)?,
            None => Vec::new(),
        };
        for panel in &manifest.panels {
            if let Some(data) = entries.get(&panel.file) {
                PanelLayout::parse_for_api(data, manifest.addon.api_version)
                    .map_err(|e| AddonError::Manifest(format!("{}: {e}", panel.file)))?;
            }
        }
        Ok(Package {
            fingerprint: sha256_hex(&manifest_bytes),
            file_sha256: sha256_hex(bytes),
            manifest,
            manifest_bytes,
            files: entries,
            signature,
            trust,
            icons,
        })
    }
}
