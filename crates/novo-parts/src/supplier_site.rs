//! Supplier sites that cannot be imported directly (Rittal PARTcommunity,
//! TraceParts, WAGO, Siemens, ...): the list, the private folders, and the
//! rules for what the in-app browser may keep. Pure: no window, no network.
//!
//! The browser window itself lives in `novo-app`. The rules it follows:
//!
//! * The user drives the browser and signs in themselves. KoperCAD never
//!   types, clicks, reads a page, or asks a supplier anything on their
//!   behalf (some, like CADENAS portals, forbid automated queries).
//! * No password is read or stored by KoperCAD. A supplier's own cookies stay
//!   in that supplier's private profile folder under the user profile, and
//!   [`forget_profile`] removes them.
//! * A download lands in a private folder per supplier under
//!   `%LOCALAPPDATA%\KoperCAD\browser\landing\<id>\`, outside the library and
//!   outside every shared bundle, until the user takes it into Parts.
//! * Only the file types the supplier list names are kept.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::import::{Arrival, MTIME_SLACK};
use crate::library::FileKind;
use crate::search::normalize_code;

/// One supplier site, as data. The built-in list can be changed without a
/// release: `suppliers.json` in the browser folder replaces an entry with the
/// same `id`, adds new ones, and `"hidden": true` removes one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Supplier {
    /// Folder-safe name: lower case letters, digits and `-`.
    pub id: String,
    pub name: String,
    /// Where the browser opens.
    pub start_url: String,
    /// The supplier's search for a part number; `{order}` stands for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_url: Option<String>,
    /// File kinds that are kept. Anything else a page offers is refused.
    pub accepts: Vec<FileKind>,
    /// Plain notes shown behind the "?" (sign-in, which button to press).
    #[serde(default)]
    pub notes: String,
    /// Whether the supplier asks the user to sign in.
    #[serde(default)]
    pub needs_login: bool,
    /// Whether the addresses were confirmed with a real download.
    #[serde(default)]
    pub verified: bool,
    /// Maker names (as in the catalog) this supplier serves.
    #[serde(default)]
    pub makers: Vec<String>,
    /// User list only: remove the built-in entry with this id.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// Why something was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The file kind is not one the supplier list names.
    KindNotAccepted(String),
    /// The suggested file name is empty or has no usable characters.
    BadName,
    /// The address is not a plain http or https page.
    BadAddress,
    /// The supplier id would not be a safe folder name.
    BadId,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::KindNotAccepted(n) => write!(f, "{n} is not a file type Parts can take"),
            Refusal::BadName => write!(f, "the file has no usable name"),
            Refusal::BadAddress => write!(f, "only http and https pages open here"),
            Refusal::BadId => write!(f, "the supplier id is not a safe folder name"),
        }
    }
}

impl Supplier {
    /// Whether `id` is safe to use as a folder name.
    pub fn id_is_safe(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 40
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !id.starts_with('-')
    }

    /// The page to open: the search for `order` when the supplier has one and
    /// a part number is known, else the start page.
    pub fn open_url(&self, order: Option<&str>) -> String {
        match (&self.search_url, order.map(str::trim)) {
            (Some(t), Some(o)) if !o.is_empty() => t.replace("{order}", &url_encode(o)),
            _ => self.start_url.clone(),
        }
    }

    /// The host of the start page, shown in the window title so the user sees
    /// where they are.
    pub fn host(&self) -> Option<String> {
        host_of(&self.start_url)
    }

    /// Whether this supplier's site is the one `url` points to (same host or a
    /// sub-host of the start page's registrable name).
    pub fn serves_url(&self, url: &str) -> bool {
        let (Some(mine), Some(theirs)) = (self.host(), host_of(url)) else {
            return false;
        };
        let root = |h: &str| {
            let parts: Vec<&str> = h.split('.').collect();
            parts[parts.len().saturating_sub(2)..].join(".")
        };
        mine == theirs || root(&mine) == root(&theirs)
    }

    /// Whether a file with this suggested name may be kept.
    pub fn check_file(&self, suggested: &str) -> Result<String, Refusal> {
        let name = safe_file_name(suggested).ok_or(Refusal::BadName)?;
        let kind = FileKind::of_file_name(&name);
        if kind == FileKind::Other || !self.accepts.contains(&kind) {
            return Err(Refusal::KindNotAccepted(name));
        }
        Ok(name)
    }
}

/// The built-in suppliers. Addresses are the suppliers' public entry pages;
/// `verified` is true only where a real download was made (WAGO, from the
/// owner's capture). Nothing is fetched from them by KoperCAD.
pub fn builtin() -> Vec<Supplier> {
    use FileKind::{Dxf, Edz, Step, Zip};
    let s = |id: &str,
             name: &str,
             start: &str,
             search: Option<&str>,
             accepts: &[FileKind],
             notes: &str,
             login: bool,
             verified: bool,
             makers: &[&str]| Supplier {
        id: id.into(),
        name: name.into(),
        start_url: start.into(),
        search_url: search.map(str::to_owned),
        accepts: accepts.to_vec(),
        notes: notes.into(),
        needs_login: login,
        verified,
        makers: makers.iter().map(|m| (*m).to_owned()).collect(),
        hidden: false,
    };
    vec![
        s(
            "rittal",
            "Rittal PARTcommunity",
            "https://rittal.partcommunity.com/3d-cad-models/",
            None,
            &[Step, Dxf, Zip],
            "Rittal's CAD portal (CADENAS). Find the article, press the download button, choose STEP or DXF. The portal asks you to sign in; do that in this window.",
            true,
            false,
            &["Rittal"],
        ),
        s(
            "traceparts",
            "TraceParts",
            "https://www.traceparts.com/",
            Some("https://www.traceparts.com/en/search?KeyWords={order}"),
            &[Step, Zip],
            "Open the part, press Download CAD model, choose STEP. TraceParts asks you to sign in.",
            true,
            false,
            &[],
        ),
        s(
            "wago",
            "WAGO CAD models",
            "https://wago.partcommunity.com/3d-cad-models/",
            Some("https://wago.partcommunity.com/3d-cad-models/?catalog=wago&part={order}"),
            &[Step, Dxf, Zip, Edz],
            "WAGO's CAD portal (CADENAS). Sign in in this window, press the CAD button of the article, choose STEP.",
            true,
            true,
            &["WAGO"],
        ),
        s(
            "siemens",
            "Siemens Industry Mall",
            "https://mall.industry.siemens.com/",
            None,
            &[Step, Dxf, Zip, Edz],
            "Find the article, open Product data (CAx download manager) and choose STEP or EPLAN. Siemens asks you to sign in.",
            true,
            false,
            &["Siemens"],
        ),
        s(
            "phoenix",
            "Phoenix Contact",
            "https://www.phoenixcontact.com/",
            None,
            &[Step, Dxf, Zip, Edz],
            "Open the article, choose CAD data or EPLAN data. Some downloads need a Phoenix Contact or EPLAN account.",
            true,
            false,
            &["Phoenix Contact"],
        ),
        s(
            "schneider",
            "Schneider Electric",
            "https://www.se.com/",
            Some("https://www.se.com/ww/en/product/{order}/"),
            &[Step, Dxf, Zip, Edz],
            "Open the product page, choose Downloads, then CAD (STEP) or EPLAN data.",
            false,
            false,
            &["Schneider Electric"],
        ),
        s(
            "abb",
            "ABB",
            "https://new.abb.com/",
            Some("https://new.abb.com/products/{order}"),
            &[Step, Dxf, Zip, Edz],
            "Open the product page, choose Documents, then CAD or EPLAN data.",
            false,
            false,
            &["ABB"],
        ),
        s(
            "eplan-dataportal",
            "EPLAN Data Portal",
            "https://dataportal.eplan.com/",
            Some("https://dataportal.eplan.com/part-details/{order}"),
            &[Step, Dxf, Zip, Edz, FileKind::Bmecat],
            "Needs an EPLAN ID. Download the part data; KoperCAD takes the part, its STEP and its properties.",
            true,
            false,
            &[],
        ),
    ]
}

/// The built-in list with the user's `suppliers.json` applied. A broken user
/// file or entry is skipped (the second value says so), never fatal.
pub fn load(user_file: Option<&Path>) -> (Vec<Supplier>, Option<String>) {
    let mut list = builtin();
    let Some(path) = user_file else {
        return (list, None);
    };
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return (list, None),
        Err(e) => return (list, Some(format!("{}: {e}", path.display()))),
    };
    let entries: Vec<serde_json::Value> = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return (list, Some(format!("{}: {e}", path.display()))),
    };
    let mut problem = None;
    for value in entries {
        match serde_json::from_value::<Supplier>(value) {
            Ok(s) if !Supplier::id_is_safe(&s.id) => {
                problem = Some(format!("supplier id \"{}\" is not a safe name", s.id));
            }
            Ok(s) if host_of(&s.start_url).is_none() => {
                problem = Some(format!("{}: only http and https pages open", s.name));
            }
            Ok(s) => {
                list.retain(|b| b.id != s.id);
                if !s.hidden {
                    list.push(s);
                }
            }
            Err(e) => problem = Some(e.to_string()),
        }
    }
    (list, problem)
}

/// The supplier a page address belongs to, for buttons that used to open the
/// system browser.
pub fn supplier_for_url<'a>(list: &'a [Supplier], url: &str) -> Option<&'a Supplier> {
    // The exact host first: Rittal and WAGO both sit under partcommunity.com.
    let host = host_of(url)?;
    list.iter()
        .find(|s| s.host().is_some_and(|h| h == host))
        .or_else(|| list.iter().find(|s| s.serves_url(url)))
}

/// The supplier that serves a maker, by catalog name.
pub fn supplier_for_maker<'a>(list: &'a [Supplier], maker: &str) -> Option<&'a Supplier> {
    let m = normalize_code(maker);
    list.iter()
        .find(|s| s.makers.iter().any(|x| normalize_code(x) == m))
}

/// `%LOCALAPPDATA%\KoperCAD\browser`: everything the in-app browser keeps.
/// Not the library folder, so library bundles never include it.
pub fn browser_root() -> Option<PathBuf> {
    novo_storage::profile::data_local_dir().map(|d| d.join("KoperCAD").join("browser"))
}

/// The user's supplier list file.
pub fn user_list_path(root: &Path) -> PathBuf {
    root.join("suppliers.json")
}

/// The supplier's private browser profile (cookies and the site's storage).
pub fn profile_dir(root: &Path, id: &str) -> Result<PathBuf, Refusal> {
    Supplier::id_is_safe(id)
        .then(|| root.join("profile").join(id))
        .ok_or(Refusal::BadId)
}

/// Where the supplier's downloads land until Parts takes them.
pub fn landing_dir(root: &Path, id: &str) -> Result<PathBuf, Refusal> {
    Supplier::id_is_safe(id)
        .then(|| root.join("landing").join(id))
        .ok_or(Refusal::BadId)
}

/// Removes the supplier's saved sign-in (its profile folder). Landed files
/// stay. A missing profile is fine.
pub fn forget_profile(root: &Path, id: &str) -> io::Result<()> {
    let dir = profile_dir(root, id)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
    match fs::remove_dir_all(dir) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The supplier a file in a landing folder came from, as the address kept
/// in its source record: `<...>/browser/landing/<id>/<file>` gives that
/// supplier's start page. `None` for any other file. Every window download
/// is recorded this way, so the personal-model rule (`is_personal_model`)
/// keeps it out of Share bundles whatever the file says about itself.
pub fn landed_source_url(path: &Path) -> Option<String> {
    let id_dir = path.parent()?;
    let id = id_dir.file_name()?.to_str()?;
    let landing = id_dir.parent()?;
    let browser = landing.parent()?;
    if landing.file_name()? != "landing" || browser.file_name()? != "browser" {
        return None;
    }
    let (list, _) = load(Some(&user_list_path(browser)));
    list.into_iter().find(|s| s.id == id).map(|s| s.start_url)
}

/// The suffix a download is written under until it is complete; the watcher
/// never takes a file with it.
pub const PARTIAL_SUFFIX: &str = ".part";

/// A file name taken from a page's suggestion: the last path component with
/// control and reserved characters replaced, no leading dots, at most 120
/// characters. `None` if nothing usable is left.
pub fn safe_file_name(suggested: &str) -> Option<String> {
    let last = suggested.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = last
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' ']);
    if cleaned.is_empty() || !cleaned.contains('.') {
        return None;
    }
    let mut out: String = cleaned.chars().take(120).collect();
    // Windows device names are not file names.
    let stem = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")))
    {
        out.insert(0, '_');
    }
    Some(out)
}

/// Where a download is written while it runs: `<landing>/<stamp>-<name>.part`.
/// The browser renames it to [`final_path`] when it is complete.
pub fn partial_path(landing: &Path, name: &str, stamp: u64) -> PathBuf {
    landing.join(format!("{stamp}-{name}{PARTIAL_SUFFIX}"))
}

/// The finished file for a partial path, or `None` if it is not one.
pub fn final_path(partial: &Path) -> Option<PathBuf> {
    let name = partial.file_name()?.to_str()?;
    let stem = name.strip_suffix(PARTIAL_SUFFIX)?;
    Some(partial.with_file_name(stem))
}

/// Deletes landed files older than `max_age` (downloads are personal and
/// are not kept for long once Parts has had its chance). Returns how many.
pub fn purge_landing(dir: &Path, max_age: std::time::Duration) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .ok()
            .filter(|m| m.is_file())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > max_age);
        if old && fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Only plain web pages open: http or https, with no user name or password in
/// the address and a host.
pub fn url_allowed(url: &str) -> bool {
    host_of(url).is_some()
}

fn host_of(url: &str) -> Option<String> {
    let lower = url
        .get(..8)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let rest = if lower == "https://" {
        &url[8..]
    } else if lower.starts_with("http://") {
        &url[7..]
    } else {
        return None;
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.contains('@') || authority.is_empty() {
        return None;
    }
    let host = authority.split(':').next()?.to_ascii_lowercase();
    (!host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'))
    .then_some(host)
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Watches one supplier's landing folder. Everything in it was put there by
/// the in-app browser, so the file name need not carry the part number; the
/// caller still judges the content (`CadIdentity`, `decide_drop`) before
/// taking a file. Poll it from the UI timer.
pub struct LandingWatcher {
    dir: PathBuf,
    since: SystemTime,
    order_code: String,
    last_sizes: HashMap<PathBuf, u64>,
    given: Vec<PathBuf>,
    /// When set, only these kinds arrive (the Downloads fallback uses it,
    /// since that folder holds the user's other files too).
    kinds: Option<Vec<FileKind>>,
}

impl LandingWatcher {
    /// Files older than `since` (less [`MTIME_SLACK`]) are not new.
    pub fn new(dir: impl Into<PathBuf>, since: SystemTime, order_number: &str) -> Self {
        LandingWatcher {
            dir: dir.into(),
            since: since.checked_sub(MTIME_SLACK).unwrap_or(since),
            order_code: normalize_code(order_number),
            last_sizes: HashMap::new(),
            given: Vec::new(),
            kinds: None,
        }
    }

    /// Only files of these kinds arrive.
    pub fn with_kinds(mut self, kinds: &[FileKind]) -> Self {
        self.kinds = Some(kinds.to_vec());
        self
    }

    /// Finished files whose size did not change between two polls.
    pub fn poll(&mut self) -> Vec<Arrival> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut ready = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if self.given.contains(&path)
                || name.to_ascii_lowercase().ends_with(PARTIAL_SUFFIX)
                || FileKind::of_file_name(&name) == FileKind::Other
                || self
                    .kinds
                    .as_ref()
                    .is_some_and(|k| !k.contains(&FileKind::of_file_name(&name)))
            {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || meta.len() == 0 || meta.modified().is_ok_and(|m| m < self.since) {
                continue;
            }
            let size = meta.len();
            if self.last_sizes.insert(path.clone(), size) == Some(size) {
                ready.push(Arrival {
                    names_the_part: !self.order_code.is_empty()
                        && normalize_code(&name).contains(&self.order_code),
                    path: path.clone(),
                });
                self.given.push(path);
            }
        }
        ready.sort_by(|a, b| a.path.cmp(&b.path));
        ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rittal() -> Supplier {
        builtin().into_iter().find(|s| s.id == "rittal").unwrap()
    }

    #[test]
    fn builtin_ids_are_safe_urls_open_and_every_supplier_takes_some_file() {
        let list = builtin();
        let mut ids: Vec<_> = list.iter().map(|s| s.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), list.len(), "ids are unique");
        for s in &list {
            assert!(Supplier::id_is_safe(&s.id), "{}", s.id);
            assert!(url_allowed(&s.start_url), "{}", s.id);
            assert!(!s.accepts.is_empty(), "{}", s.id);
            assert!(!s.accepts.contains(&FileKind::Other), "{}", s.id);
            if let Some(t) = &s.search_url {
                assert!(t.contains("{order}") && url_allowed(t), "{}", s.id);
            }
        }
    }

    #[test]
    fn open_url_uses_the_search_only_with_a_part_number() {
        let wago = builtin().into_iter().find(|s| s.id == "wago").unwrap();
        assert!(wago.open_url(Some("2002-1201")).ends_with("part=2002-1201"));
        assert_eq!(wago.open_url(None), wago.start_url);
        assert_eq!(wago.open_url(Some("  ")), wago.start_url);
        // No search address: always the start page.
        assert_eq!(rittal().open_url(Some("3209500")), rittal().start_url);
        // The part number is encoded into the address.
        let tp = builtin()
            .into_iter()
            .find(|s| s.id == "traceparts")
            .unwrap();
        assert!(tp.open_url(Some("A&B 1")).ends_with("KeyWords=A%26B%201"));
    }

    #[test]
    fn files_are_checked_by_kind_and_cleaned() {
        let r = rittal();
        assert_eq!(
            r.check_file("C:\\x\\..\\3209500.STEP").unwrap(),
            "3209500.STEP"
        );
        assert_eq!(r.check_file("../../etc/a.zip").unwrap(), "a.zip");
        assert!(matches!(
            r.check_file("setup.exe"),
            Err(Refusal::KindNotAccepted(_))
        ));
        assert!(matches!(
            r.check_file("notes.txt"),
            Err(Refusal::KindNotAccepted(_))
        ));
        // Rittal's entry takes no BMEcat/EDZ, the EPLAN portal does.
        assert!(r.check_file("c.edz").is_err());
        assert_eq!(r.check_file(""), Err(Refusal::BadName));
        assert_eq!(r.check_file("noextension"), Err(Refusal::BadName));
        assert_eq!(
            safe_file_name("..hidden.step").as_deref(),
            Some("hidden.step")
        );
        assert_eq!(safe_file_name("a:b?.step").as_deref(), Some("a_b_.step"));
        assert_eq!(safe_file_name("CON.step").as_deref(), Some("_CON.step"));
        assert!(
            safe_file_name(&format!("{}.step", "x".repeat(300)))
                .unwrap()
                .len()
                <= 120
        );
    }

    #[test]
    fn only_plain_web_pages_open() {
        assert!(url_allowed("https://wago.partcommunity.com/3d-cad-models/"));
        assert!(url_allowed("http://example.com:8080/a?b#c"));
        for bad in [
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
            "ftp://x.com/",
            "https://user:pw@example.com/",
            "https://",
            "https:///x",
            "https://exa mple.com/",
            "data:text/html,hi",
        ] {
            assert!(!url_allowed(bad), "{bad}");
        }
    }

    #[test]
    fn a_supplier_serves_its_own_sub_hosts_only() {
        let r = rittal();
        assert!(r.serves_url("https://rittal.partcommunity.com/x"));
        assert!(r.serves_url("https://login.partcommunity.com/x"));
        assert!(!r.serves_url("https://evilpartcommunity.com/x"));
        assert!(!r.serves_url("file:///x"));
    }

    #[test]
    fn folders_are_per_supplier_and_outside_the_library() {
        let root = Path::new("/u/KoperCAD/browser");
        let p = profile_dir(root, "rittal").unwrap();
        let l = landing_dir(root, "rittal").unwrap();
        assert_ne!(p, l);
        assert_ne!(l, landing_dir(root, "wago").unwrap());
        assert_eq!(profile_dir(root, "../x"), Err(Refusal::BadId));
        assert_eq!(landing_dir(root, "A"), Err(Refusal::BadId));
        assert_eq!(landing_dir(root, ""), Err(Refusal::BadId));
        // Never under the library root, so bundles cannot pick them up.
        if let Some(lib) = crate::default_library_root() {
            assert!(!l.starts_with(lib));
        }
        assert!(
            browser_root()
                .is_none_or(|b| b.ends_with("KoperCAD/browser") || b.ends_with("KoperCAD\\browser"))
        );
    }

    #[test]
    fn forget_profile_removes_only_that_profile() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for id in ["rittal", "wago"] {
            let p = profile_dir(root, id).unwrap();
            fs::create_dir_all(p.join("Cookies")).unwrap();
            fs::write(p.join("Cookies").join("c"), b"x").unwrap();
        }
        let landed = landing_dir(root, "rittal").unwrap();
        fs::create_dir_all(&landed).unwrap();
        fs::write(landed.join("a.step"), b"x").unwrap();
        forget_profile(root, "rittal").unwrap();
        assert!(!profile_dir(root, "rittal").unwrap().exists());
        assert!(profile_dir(root, "wago").unwrap().exists());
        assert!(landed.join("a.step").exists());
        forget_profile(root, "rittal").unwrap();
        assert!(forget_profile(root, "../x").is_err());
    }

    #[test]
    fn a_landed_file_names_its_supplier() {
        let p = Path::new("/u/KoperCAD/browser/landing/siemens/1-a.step");
        assert_eq!(
            landed_source_url(p).as_deref(),
            Some("https://mall.industry.siemens.com/")
        );
        assert_eq!(landed_source_url(Path::new("/u/Downloads/a.step")), None);
        assert_eq!(
            landed_source_url(Path::new("/u/browser/landing/nobody/a.step")),
            None
        );
        assert_eq!(
            landed_source_url(Path::new("/u/other/landing/siemens/a.step")),
            None
        );
    }

    #[test]
    fn partial_and_final_names_pair_up() {
        let p = partial_path(Path::new("/l"), "a.step", 42);
        assert_eq!(p, Path::new("/l/42-a.step.part"));
        assert_eq!(final_path(&p).unwrap(), Path::new("/l/42-a.step"));
        assert_eq!(final_path(Path::new("/l/a.step")), None);
    }

    #[test]
    fn the_user_list_adds_replaces_hides_and_survives_bad_entries() {
        let dir = tempfile::tempdir().unwrap();
        let f = user_list_path(dir.path());
        assert_eq!(load(Some(&f)).1, None, "no file is fine");
        fs::write(
            &f,
            r#"[
              {"id":"lapp","name":"LAPP","start_url":"https://lapp.example/","accepts":["step"]},
              {"id":"wago","name":"WAGO mine","start_url":"https://wago.example/","accepts":["zip"]},
              {"id":"rittal","name":"x","start_url":"https://x/","accepts":[],"hidden":true},
              {"id":"../bad","name":"b","start_url":"https://b/","accepts":["step"]},
              {"id":"ftp","name":"f","start_url":"ftp://f/","accepts":["step"]},
              {"nonsense":1}
            ]"#,
        )
        .unwrap();
        let (list, problem) = load(Some(&f));
        assert!(problem.is_some());
        assert!(list.iter().any(|s| s.id == "lapp"));
        assert_eq!(
            list.iter().find(|s| s.id == "wago").unwrap().name,
            "WAGO mine"
        );
        assert!(!list.iter().any(|s| s.id == "rittal"));
        assert!(!list.iter().any(|s| s.id == "../bad" || s.id == "ftp"));
        fs::write(&f, "not json").unwrap();
        let (list, problem) = load(Some(&f));
        assert!(problem.is_some());
        assert_eq!(list, builtin());
    }

    #[test]
    fn finds_the_supplier_for_a_page_or_a_maker() {
        let list = builtin();
        assert_eq!(
            supplier_for_url(
                &list,
                "https://wago.partcommunity.com/3d-cad-models/?catalog=wago&part=1"
            )
            .unwrap()
            .id,
            "wago"
        );
        assert_eq!(supplier_for_maker(&list, "Rittal").unwrap().id, "rittal");
        assert_eq!(
            supplier_for_maker(&list, "phoenix-contact").unwrap().id,
            "phoenix"
        );
        assert!(supplier_for_maker(&list, "Unknown").is_none());
        assert!(supplier_for_url(&list, "https://example.org/").is_none());
    }

    #[test]
    fn the_landing_watcher_takes_a_finished_file_once_and_skips_partials() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = LandingWatcher::new(dir.path(), SystemTime::now(), "3209500");
        fs::write(dir.path().join("1-x.step.part"), b"half").unwrap();
        fs::write(dir.path().join("2-readme.txt"), b"x").unwrap();
        fs::write(dir.path().join("3-3209500.step"), b"ISO-10303").unwrap();
        fs::write(dir.path().join("4-other.zip"), b"PK").unwrap();
        assert!(w.poll().is_empty(), "size must hold for two polls");
        let got = w.poll();
        let names: Vec<_> = got
            .iter()
            .map(|a| {
                (
                    a.path.file_name().unwrap().to_string_lossy().into_owned(),
                    a.names_the_part,
                )
            })
            .collect();
        assert_eq!(
            names,
            vec![
                ("3-3209500.step".to_owned(), true),
                ("4-other.zip".to_owned(), false)
            ]
        );
        assert!(w.poll().is_empty(), "each file arrives once");
    }

    #[test]
    fn the_kind_filter_and_the_purge() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.step"), b"x").unwrap();
        fs::write(dir.path().join("b.zip"), b"x").unwrap();
        let mut w =
            LandingWatcher::new(dir.path(), SystemTime::now(), "").with_kinds(&[FileKind::Step]);
        w.poll();
        let got = w.poll();
        assert_eq!(got.len(), 1);
        assert!(got[0].path.ends_with("a.step"));
        assert_eq!(
            purge_landing(dir.path(), std::time::Duration::from_secs(3600)),
            0
        );
        assert_eq!(purge_landing(dir.path(), std::time::Duration::ZERO), 2);
        assert_eq!(
            purge_landing(&dir.path().join("none"), std::time::Duration::ZERO),
            0
        );
    }

    #[test]
    fn a_file_older_than_the_click_is_not_new() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("old.step"), b"x").unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(60);
        let mut w = LandingWatcher::new(dir.path(), later, "1");
        assert!(w.poll().is_empty() && w.poll().is_empty());
    }
}
