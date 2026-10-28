//! The per-user install registry: where add-ons live on disk, which are enabled, and what to do
//! when one is broken.
//!
//! ```text
//! <root>/registry.toml              what is installed, enabled, and the fingerprint of each
//! <root>/trusted-keys.toml          publisher keys the person added (optional)
//! <root>/<id>/<version>/...         the files of one version
//! ```
//!
//! `<root>` is `%LOCALAPPDATA%\KoperCAD\addons` in the program. Nothing outside it is touched, no
//! registry keys are written, no network is used, no elevation is needed. The previous version
//! stays beside the new one, so rolling back is one step.
//!
//! Failure is contained: a damaged registry is set aside and the program starts with none; one
//! add-on whose files changed or whose manifest breaks a rule is shown as *Failed* with the
//! reason and does not load, and the others are unaffected.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AddonError, Result};
use crate::icons::IconDef;
use crate::manifest::{ICONS_FILE, Manifest, valid_id};
use crate::package::Package;
use crate::trust::{Trust, TrustStore, judge, sha256_hex};
use crate::version::Version;

const REGISTRY: &str = "registry.toml";
const TRUSTED_KEYS: &str = "trusted-keys.toml";

/// What the program is, for compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppInfo {
    pub version: Version,
    pub api_version: u32,
}

/// Whether a manifest fits this program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compat {
    Ok,
    /// The program is too old (or too new); the text says what is needed, in plain words.
    No(String),
}

/// Judges `manifest` against `app`.
pub fn check_compat(manifest: &Manifest, app: &AppInfo) -> Compat {
    if manifest.addon.api_version != app.api_version {
        return Compat::No(format!(
            "built for add-on API {}; this KoperCAD speaks API {}",
            manifest.addon.api_version, app.api_version
        ));
    }
    let want = manifest.requirement();
    if want.matches(app.version) {
        return Compat::Ok;
    }
    match want.lower_bound() {
        Some(low) if app.version < low => Compat::No(format!(
            "needs KoperCAD {low} or newer (this is {})",
            app.version
        )),
        _ => Compat::No(format!(
            "works with KoperCAD {want} (this is {})",
            app.version
        )),
    }
}

/// Where a package stands against what is installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Nothing of this id is installed.
    New,
    /// A newer version than the installed one.
    Update { from: Version },
    /// The same version and the same content.
    Same,
    /// The same version number with different content.
    SameVersionDifferent,
    /// Older than the installed version.
    Older { installed: Version },
    /// The installed copy is broken and this replaces it.
    Repair,
}

/// What the install prompt shows, computed without changing anything.
#[derive(Debug, Clone)]
pub struct Inspection {
    pub package: Package,
    pub compat: Compat,
    pub change: Change,
}

impl Inspection {
    /// `None` when installing is allowed; otherwise the reason it is not.
    pub fn blocked(&self) -> Option<String> {
        if let Compat::No(why) = &self.compat {
            return Some(format!("{} {why}", self.package.manifest.addon.name));
        }
        match &self.change {
            Change::Same => Some(format!(
                "{} {} is already installed",
                self.package.manifest.addon.name, self.package.manifest.addon.version
            )),
            Change::SameVersionDifferent => Some(format!(
                "{} {} is installed with different content; the file needs a new version number",
                self.package.manifest.addon.name, self.package.manifest.addon.version
            )),
            Change::Older { installed } => Some(format!(
                "{} {installed} is installed, which is newer than this file; roll back or remove it first",
                self.package.manifest.addon.name
            )),
            _ => None,
        }
    }

    /// Whether the person has to tick "I trust this file".
    pub fn needs_trust_tick(&self) -> bool {
        !self.package.trust.is_signed()
    }
}

/// What the person chose at the install prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallOptions {
    /// The tick "I trust this file", needed for a package that is not signed by a trusted key.
    pub accept_unsigned: bool,
    /// Turn it on straight away (a new install). An update keeps the state it had.
    pub enable: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        InstallOptions {
            accept_unsigned: false,
            enable: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    id: String,
    enabled: bool,
    /// The version in use.
    active: String,
    /// The version before it, still on disk, for Roll back.
    #[serde(default)]
    previous: Option<String>,
    /// SHA-256 of the active version's `manifest.toml` at install time.
    fingerprint: String,
    /// The person ticked "I trust this file" for the active version.
    #[serde(default)]
    accepted_unsigned: bool,
    /// The same for the previous version (it keeps its own answer through Roll back).
    #[serde(default)]
    previous_accepted: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Registry {
    #[serde(default, rename = "addon")]
    addons: Vec<Record>,
}

/// Why an installed add-on is not running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Enabled,
    Disabled,
    /// Does not fit this program version or API; the text says what is needed.
    Incompatible(String),
    /// Files missing or changed, or the manifest breaks a rule.
    Failed(String),
}

impl Status {
    pub fn is_enabled(&self) -> bool {
        matches!(self, Status::Enabled)
    }
}

/// One installed add-on as found at scan time.
#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub enabled_flag: bool,
    pub status: Status,
    pub active: Version,
    pub previous: Option<Version>,
    /// `None` only when the manifest could not be read (status is then `Failed`).
    pub manifest: Option<Manifest>,
    pub trust: Trust,
    pub accepted_unsigned: bool,
    pub fingerprint: String,
    pub dir: PathBuf,
    pub icons: Vec<IconDef>,
    /// Package path to "hash checked at this start".
    pub verified: BTreeMap<String, bool>,
}

impl Entry {
    /// The name to show, even for a failed one.
    pub fn display_name(&self) -> &str {
        self.manifest
            .as_ref()
            .map_or(self.id.as_str(), |m| m.addon.name.as_str())
    }
}

/// Everything installed, checked at this start. The program holds one of these.
#[derive(Debug, Clone, Default)]
pub struct AddonSet {
    pub entries: Vec<Entry>,
    /// Things the person should know (a damaged registry set aside, a bad trusted key).
    pub warnings: Vec<String>,
}

impl AddonSet {
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The gate the base program asks: is add-on `id` installed, compatible, intact and on?
    pub fn is_enabled(&self, id: &str) -> bool {
        self.get(id).is_some_and(|e| e.status.is_enabled())
    }

    /// The add-ons that are running.
    pub fn enabled(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.status.is_enabled())
    }

    /// How many need attention: failed or incompatible (for the badge on the menu item).
    pub fn attention(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e.status, Status::Failed(_) | Status::Incompatible(_)))
            .count()
    }

    /// The bytes of a data file of a running add-on. Only files the manifest lists can be
    /// read, and the hash is checked again, so a file changed after start is not served.
    pub fn read_file(&self, id: &str, path: &str) -> Result<Vec<u8>> {
        let entry = self
            .get(id)
            .filter(|e| e.status.is_enabled())
            .ok_or_else(|| AddonError::Unknown(id.to_owned()))?;
        let manifest = entry
            .manifest
            .as_ref()
            .ok_or_else(|| AddonError::Unknown(id.to_owned()))?;
        let want = manifest
            .files
            .get(path)
            .ok_or_else(|| AddonError::Integrity(format!("{path} is not a file of {id}")))?;
        let bytes = std::fs::read(entry.dir.join(path))?;
        if &sha256_hex(&bytes) != want {
            return Err(AddonError::Integrity(format!("{path} has been changed")));
        }
        Ok(bytes)
    }

    /// Data files of one `kind` from running add-ons, as `(add-on id, path)`.
    pub fn contributions(&self, kind: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for e in self.enabled() {
            if let Some(m) = &e.manifest {
                for c in m.contributions.iter().filter(|c| c.kind == kind) {
                    out.push((e.id.clone(), c.file.clone()));
                }
            }
        }
        out
    }
}

/// The install folder and the keys. Cheap to create; every operation reads the registry fresh,
/// so two program windows do not overwrite each other's changes.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
    trust: TrustStore,
    app: AppInfo,
}

impl Store {
    /// A store under `root`. The folder is created on the first install.
    pub fn new(root: impl Into<PathBuf>, app: AppInfo) -> Store {
        let root = root.into();
        let mut trust = TrustStore::builtin();
        if let Ok(text) = std::fs::read_to_string(root.join(TRUSTED_KEYS)) {
            // problems are reported by `scan`
            let _ = trust.extend_from_toml(&text);
        }
        Store { root, trust, app }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn app(&self) -> &AppInfo {
        &self.app
    }

    pub fn trust(&self) -> &TrustStore {
        &self.trust
    }

    fn load_registry(&self, warnings: &mut Vec<String>) -> Registry {
        let path = self.root.join(REGISTRY);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Registry::default(),
            Err(e) => {
                warnings.push(format!(
                    "The add-on list could not be read ({e}); no add-ons are loaded."
                ));
                return Registry::default();
            }
        };
        match toml::from_str::<Registry>(&text) {
            Ok(mut r) => {
                let before = r.addons.len();
                r.addons.retain(|rec| valid_id(&rec.id, 2, 48));
                if r.addons.len() != before {
                    warnings.push(
                        "The add-on list had entries with unusable names; they were ignored."
                            .into(),
                    );
                }
                r
            }
            Err(e) => {
                let aside = self.root.join("registry.toml.damaged");
                let _ = std::fs::rename(&path, &aside);
                warnings.push(format!(
                    "The add-on list was damaged ({e}); it was set aside as registry.toml.damaged and no add-ons are loaded. Install them again from their files."
                ));
                Registry::default()
            }
        }
    }

    fn save_registry(&self, registry: &Registry) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let text = toml::to_string_pretty(registry).map_err(|e| AddonError::Io(e.to_string()))?;
        let tmp = self
            .root
            .join(format!("registry.toml.new-{}", std::process::id()));
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, self.root.join(REGISTRY))?;
        Ok(())
    }

    /// Reads the registry and checks every installed add-on: manifest, fingerprint, the hash of
    /// every file, the signature against the current keys, and compatibility. Never fails; a
    /// problem becomes the add-on's status.
    pub fn scan(&self) -> AddonSet {
        let mut set = AddonSet::default();
        if let Ok(text) = std::fs::read_to_string(self.root.join(TRUSTED_KEYS)) {
            let mut scratch = TrustStore::default();
            for p in scratch.extend_from_toml(&text) {
                set.warnings.push(p);
            }
        }
        let registry = self.load_registry(&mut set.warnings);
        for rec in &registry.addons {
            set.entries.push(self.check_one(rec));
        }
        set
    }

    fn check_one(&self, rec: &Record) -> Entry {
        let active = Version::parse(&rec.active).unwrap_or_default();
        let dir = self.root.join(&rec.id).join(&rec.active);
        let mut entry = Entry {
            id: rec.id.clone(),
            enabled_flag: rec.enabled,
            status: Status::Disabled,
            active,
            previous: rec.previous.as_deref().and_then(Version::parse),
            manifest: None,
            trust: Trust::Unsigned {
                note: "not checked".into(),
            },
            accepted_unsigned: rec.accepted_unsigned,
            fingerprint: rec.fingerprint.clone(),
            dir: dir.clone(),
            icons: Vec::new(),
            verified: BTreeMap::new(),
        };
        let fail = |mut e: Entry, why: String| {
            e.status = Status::Failed(why);
            e
        };
        if Version::parse(&rec.active).is_none() {
            return fail(
                entry,
                format!(
                    "the installed version {:?} is not a version number",
                    rec.active
                ),
            );
        }
        let manifest_bytes = match std::fs::read(dir.join("manifest.toml")) {
            Ok(b) => b,
            Err(e) => {
                return fail(
                    entry,
                    format!(
                        "its files are missing or unreadable ({e}); install it again from its file"
                    ),
                );
            }
        };
        if sha256_hex(&manifest_bytes) != rec.fingerprint {
            return fail(
                entry,
                "its manifest changed after it was installed; install it again from its file"
                    .into(),
            );
        }
        let manifest = match Manifest::parse(&manifest_bytes) {
            Ok(m) => m,
            Err(e) => return fail(entry, e.to_string()),
        };
        if manifest.addon.id != rec.id || manifest.version() != active {
            return fail(
                entry,
                "its manifest names a different add-on or version than the one installed".into(),
            );
        }
        // Every listed file must exist with the right hash; nothing else may sit beside them.
        for (path, want) in &manifest.files {
            let ok = std::fs::read(dir.join(path))
                .map(|b| &sha256_hex(&b) == want)
                .unwrap_or(false);
            entry.verified.insert(path.clone(), ok);
        }
        if let Some((bad, _)) = entry.verified.iter().find(|(_, ok)| !**ok) {
            let why =
                format!("{bad} is missing or has been changed; install it again from its file");
            entry.manifest = Some(manifest);
            return fail(entry, why);
        }
        if let Some(extra) = unlisted_file(&dir, &manifest.files) {
            let why = format!(
                "{extra} sits in its folder but the manifest does not list it; install it again from its file"
            );
            entry.manifest = Some(manifest);
            return fail(entry, why);
        }
        let signature = std::fs::read(dir.join("signature")).ok();
        entry.trust = match judge(&manifest_bytes, signature.as_deref(), &self.trust) {
            Ok(t) => t,
            Err(e) => {
                entry.manifest = Some(manifest);
                return fail(entry, e.to_string());
            }
        };
        if !entry.trust.is_signed() && !rec.accepted_unsigned {
            entry.manifest = Some(manifest);
            return fail(
                entry,
                "it is not signed by a trusted publisher and was not accepted".into(),
            );
        }
        if manifest.files.contains_key(ICONS_FILE)
            && let Ok(bytes) = std::fs::read(dir.join(ICONS_FILE))
        {
            match crate::icons::parse(&rec.id, &bytes) {
                Ok(icons) => entry.icons = icons,
                Err(e) => {
                    entry.manifest = Some(manifest);
                    return fail(entry, e.to_string());
                }
            }
        }
        entry.status = match check_compat(&manifest, &self.app) {
            Compat::No(why) => Status::Incompatible(why),
            Compat::Ok if rec.enabled => Status::Enabled,
            Compat::Ok => Status::Disabled,
        };
        entry.manifest = Some(manifest);
        entry
    }

    /// Reads a package from memory and says what installing it would do. Changes nothing.
    pub fn inspect(&self, bytes: &[u8]) -> Result<Inspection> {
        let package = Package::read(bytes, &self.trust)?;
        let compat = check_compat(&package.manifest, &self.app);
        let mut warnings = Vec::new();
        let registry = self.load_registry(&mut warnings);
        let change = match registry
            .addons
            .iter()
            .find(|r| r.id == package.manifest.addon.id)
        {
            None => Change::New,
            Some(rec) => {
                let installed = Version::parse(&rec.active).unwrap_or_default();
                let new = package.manifest.version();
                let healthy = !matches!(self.check_one(rec).status, Status::Failed(_));
                if new > installed {
                    Change::Update { from: installed }
                } else if new < installed {
                    Change::Older { installed }
                } else if !healthy {
                    Change::Repair
                } else if rec.fingerprint == package.fingerprint {
                    Change::Same
                } else {
                    Change::SameVersionDifferent
                }
            }
        };
        Ok(Inspection {
            package,
            compat,
            change,
        })
    }

    /// Installs a package file's bytes.
    pub fn install_bytes(&self, bytes: &[u8], options: InstallOptions) -> Result<Inspection> {
        let inspection = self.inspect(bytes)?;
        if let Compat::No(why) = &inspection.compat {
            return Err(AddonError::Incompatible(format!(
                "{} {why}",
                inspection.package.manifest.addon.name
            )));
        }
        if let Some(why) = inspection.blocked() {
            return Err(AddonError::Version(why));
        }
        if inspection.needs_trust_tick() && !options.accept_unsigned {
            return Err(AddonError::NotAccepted);
        }
        let _lock = Lock::take(&self.root)?;
        let package = &inspection.package;
        let id = &package.manifest.addon.id;
        let version = package.manifest.addon.version.clone();
        let version = Version::parse(&version).unwrap_or_default().to_string();

        let addon_dir = self.root.join(id);
        let staging = addon_dir.join(format!(".staging-{}", std::process::id()));
        let target = addon_dir.join(&version);
        let result = (|| -> Result<()> {
            // Leftovers of an install that crashed (another process id, over an hour old).
            if let Ok(entries) = std::fs::read_dir(&addon_dir) {
                for e in entries.flatten() {
                    let stale = e.file_name().to_string_lossy().starts_with(".staging-")
                        && e.metadata()
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| t.elapsed().ok())
                            .is_some_and(|age| age > std::time::Duration::from_secs(3600));
                    if stale {
                        let _ = std::fs::remove_dir_all(e.path());
                    }
                }
            }
            if staging.exists() {
                std::fs::remove_dir_all(&staging)?;
            }
            std::fs::create_dir_all(&staging)?;
            std::fs::write(staging.join("manifest.toml"), &package.manifest_bytes)?;
            if let Some(sig) = &package.signature {
                std::fs::write(staging.join("signature"), sig)?;
            }
            for (path, data) in &package.files {
                let dest = staging.join(path);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&dest, data)?;
            }
            if target.exists() {
                std::fs::remove_dir_all(&target)?;
            }
            std::fs::rename(&staging, &target)?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }

        let mut warnings = Vec::new();
        let mut registry = self.load_registry(&mut warnings);
        let existing = registry.addons.iter().position(|r| &r.id == id);
        let mut drop_after_save: Option<PathBuf> = None;
        // Versions the registry already points at must survive a failed save.
        let referenced: Vec<String> = existing
            .map(|i| {
                let r = &registry.addons[i];
                std::iter::once(r.active.clone())
                    .chain(r.previous.clone())
                    .collect()
            })
            .unwrap_or_default();
        let record = match existing {
            Some(i) => {
                let old = registry.addons[i].clone();
                let upgrading = matches!(inspection.change, Change::Update { .. });
                // Keep one previous version for Roll back; remove the one before it.
                let previous = if upgrading {
                    Some(old.active.clone())
                } else {
                    old.previous.clone()
                };
                // The version before the previous one goes, unless it is the one just installed
                // or the one that stays; removed only once the registry is saved.
                if upgrading
                    && let Some(older) = &old.previous
                    && Some(older) != previous.as_ref()
                    && older != &version
                {
                    drop_after_save = Some(addon_dir.join(older));
                }
                Record {
                    id: id.clone(),
                    enabled: old.enabled,
                    active: version.clone(),
                    previous,
                    fingerprint: package.fingerprint.clone(),
                    accepted_unsigned: options.accept_unsigned && !package.trust.is_signed(),
                    previous_accepted: if upgrading {
                        old.accepted_unsigned
                    } else {
                        old.previous_accepted
                    },
                }
            }
            None => Record {
                id: id.clone(),
                enabled: options.enable,
                active: version.clone(),
                previous: None,
                fingerprint: package.fingerprint.clone(),
                accepted_unsigned: options.accept_unsigned && !package.trust.is_signed(),
                previous_accepted: false,
            },
        };
        match existing {
            Some(i) => registry.addons[i] = record,
            None => registry.addons.push(record),
        }
        registry.addons.sort_by(|a, b| a.id.cmp(&b.id));
        if let Err(e) = self.save_registry(&registry) {
            // Do not leave a version folder the registry does not know about.
            if existing.is_none() {
                let _ = std::fs::remove_dir_all(&addon_dir);
            } else if !referenced.contains(&version) {
                let _ = std::fs::remove_dir_all(&target);
            }
            return Err(e);
        }
        if let Some(dir) = drop_after_save {
            let _ = std::fs::remove_dir_all(dir);
        }
        Ok(inspection)
    }

    /// Installs a `.kcaddon` file.
    pub fn install_file(&self, path: &Path, options: InstallOptions) -> Result<Inspection> {
        let bytes = read_package_file(path)?;
        self.install_bytes(&bytes, options)
    }

    /// Reads a `.kcaddon` file and says what installing it would do.
    pub fn inspect_file(&self, path: &Path) -> Result<Inspection> {
        self.inspect(&read_package_file(path)?)
    }

    fn modify(&self, id: &str, f: impl FnOnce(&mut Registry, usize) -> Result<()>) -> Result<()> {
        let _lock = Lock::take(&self.root)?;
        let mut warnings = Vec::new();
        let mut registry = self.load_registry(&mut warnings);
        let i = registry
            .addons
            .iter()
            .position(|r| r.id == id)
            .ok_or_else(|| AddonError::Unknown(id.to_owned()))?;
        f(&mut registry, i)?;
        self.save_registry(&registry)
    }

    /// Turns an add-on on or off. Its files stay.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        self.modify(id, |r, i| {
            r.addons[i].enabled = enabled;
            Ok(())
        })
    }

    /// Removes an add-on and all its versions. Projects that used it keep their data for it.
    pub fn remove(&self, id: &str) -> Result<()> {
        if !valid_id(id, 2, 48) {
            return Err(AddonError::Unknown(id.to_owned()));
        }
        self.modify(id, |r, i| {
            r.addons.remove(i);
            Ok(())
        })?;
        let dir = self.root.join(id);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    /// Goes back to the previous version; the version left becomes the previous one, so it can
    /// be rolled forward again.
    pub fn roll_back(&self, id: &str) -> Result<Version> {
        let mut now = None;
        self.modify(id, |r, i| {
            let rec = &mut r.addons[i];
            let prev = rec
                .previous
                .clone()
                .ok_or_else(|| AddonError::NoPrevious(id.to_owned()))?;
            let dir = self.root.join(id).join(&prev);
            let manifest_bytes = std::fs::read(dir.join("manifest.toml"))
                .map_err(|e| AddonError::Io(format!("the earlier version is gone ({e})")))?;
            let manifest = Manifest::parse(&manifest_bytes)?;
            if manifest.addon.id != id {
                return Err(AddonError::Manifest(
                    "the earlier version belongs to another add-on".into(),
                ));
            }
            let sig = std::fs::read(dir.join("signature")).ok();
            let trust = judge(&manifest_bytes, sig.as_deref(), &self.trust)?;
            if !trust.is_signed() && !rec.previous_accepted {
                return Err(AddonError::NotAccepted);
            }
            let left = std::mem::replace(&mut rec.active, prev.clone());
            rec.previous = Some(left);
            std::mem::swap(&mut rec.accepted_unsigned, &mut rec.previous_accepted);
            rec.fingerprint = sha256_hex(&manifest_bytes);
            now = Version::parse(&prev);
            Ok(())
        })?;
        now.ok_or_else(|| AddonError::NoPrevious(id.to_owned()))
    }
}

/// A lock file beside the registry, so two program windows cannot rewrite it at the same time.
/// A lock older than a minute belongs to a crashed program and is broken.
struct Lock(PathBuf);

impl Lock {
    fn take(root: &Path) -> Result<Lock> {
        std::fs::create_dir_all(root)?;
        let path = root.join("registry.lock");
        for _ in 0..200 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Lock(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|age| age > std::time::Duration::from_secs(60));
                    if stale {
                        let _ = std::fs::remove_file(&path);
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(25));
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(AddonError::Io(
            "another KoperCAD window is changing the add-ons; try again in a moment".into(),
        ))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The first file under `dir` that the manifest does not list (or a symbolic link), as a
/// package path. `manifest.toml` and `signature` are not listed by design.
fn unlisted_file(dir: &Path, listed: &BTreeMap<String, String>) -> Option<String> {
    fn walk(
        base: &Path,
        dir: &Path,
        listed: &BTreeMap<String, String>,
        seen: &mut usize,
        depth: usize,
    ) -> Option<String> {
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            *seen += 1;
            if *seen > 5000 || depth > 8 {
                return Some("too many files".to_owned());
            }
            let path = entry.path();
            let rel = path
                .strip_prefix(base)
                .ok()?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let kind = entry.file_type().ok()?;
            if kind.is_symlink() {
                return Some(rel);
            }
            if kind.is_dir() {
                if let Some(found) = walk(base, &path, listed, seen, depth + 1) {
                    return Some(found);
                }
            } else if rel != "manifest.toml" && rel != "signature" && !listed.contains_key(&rel) {
                return Some(rel);
            }
        }
        None
    }
    let mut seen = 0;
    walk(dir, dir, listed, &mut seen, 0)
}

fn read_package_file(path: &Path) -> Result<Vec<u8>> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > crate::package::Limits::DEFAULT.max_package_bytes {
        return Err(AddonError::TooLarge(format!(
            "the file is {} MiB",
            meta.len() / (1024 * 1024)
        )));
    }
    Ok(std::fs::read(path)?)
}
