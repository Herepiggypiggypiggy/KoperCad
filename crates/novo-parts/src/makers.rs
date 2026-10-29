//! Maker aliases: the same maker is written many ways in a parts database
//! (`SE`, `SCHNEI`, `Schneider Electric SE`), and the catalog must hold one
//! name per maker so search, grouping and part matching work.
//!
//! The built-in table holds generic, public maker names and the codes
//! commonly used for them. The user's own codes (every shop invents some)
//! are kept in `maker-aliases.json` beside the user catalog.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::user_catalog_path;
use crate::search::normalize_code;

/// Maker display name and the spellings that mean it. Matching ignores case,
/// spaces and punctuation, so `Schneider-Electric` needs no entry of its own.
const BUILTIN: &[(&str, &[&str])] = &[
    (
        "Schneider Electric",
        &["SE", "SCHNEI", "SCHNEIDER", "Telemecanique", "Merlin Gerin"],
    ),
    ("Siemens", &["SIE", "SIEMEN"]),
    ("Rittal", &["RIT", "RITTA"]),
    ("ABB", &["ABB", "ASEA Brown Boveri"]),
    ("LAPP", &["LAPP", "LAP", "Lapp Kabel", "Lapp Group"]),
    ("WAGO", &["WAGO", "WAG"]),
    ("Phoenix Contact", &["PHOENIX", "PHX", "PXC"]),
    ("Lenze", &["LENZE", "LEN"]),
    ("Harting", &["HARTING", "HAR"]),
    ("Eaton", &["EATON", "EAT", "Moeller", "Eaton Moeller"]),
    ("Beckhoff", &["BECKHO", "BECK", "Beckhoff Automation"]),
    ("Murrelektronik", &["MURR", "MURREL", "Murr Elektronik"]),
    ("Bosch Rexroth", &["REXROTH", "REX", "Rexroth"]),
    ("Danfoss", &["DANFOS"]),
    ("Pilz", &["PILZ"]),
    ("Werma", &["WERMA"]),
    ("Lütze", &["LUTZE", "LUETZE", "LUT"]),
    ("Kopp", &["KOPP"]),
    ("ILME", &["ILME"]),
    ("Zeca", &["ZECA"]),
    ("Demag", &["DEMAG"]),
    ("Omron", &["OMR", "OMRON"]),
    (
        "Weidmüller",
        &["WEID", "WEIDMU", "Weidmuller", "Weidmueller"],
    ),
    ("Legrand", &["LEG", "LEGRAN"]),
    ("Hager", &["HAGER"]),
    ("Mitsubishi Electric", &["MITSU", "Mitsubishi"]),
    ("SEW-Eurodrive", &["SEW", "SEW Eurodrive"]),
    ("Sick", &["SICK"]),
    ("Pepperl+Fuchs", &["PEPPER", "Pepperl Fuchs", "P+F"]),
    ("Turck", &["TURCK"]),
    ("Festo", &["FESTO"]),
    ("Finder", &["FINDER"]),
    ("Mean Well", &["MEANWELL", "Mean Well"]),
];

/// Trailing legal-form words dropped when a name is looked up.
const LEGAL_FORMS: &[&str] = &[
    "gmbh", "ag", "kg", "co", "bv", "nv", "sa", "se", "inc", "ltd", "llc", "spa", "ab", "as", "oy",
];

/// Shortest code that may stand for a longer name (`SCHNEI`).
const MIN_PREFIX: usize = 3;

/// The file the user's own aliases are saved in.
pub const ALIAS_FILE_NAME: &str = "maker-aliases.json";

/// Maker spellings mapped to one display name each: the built-in table plus
/// the user's own codes (which win).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MakerAliases {
    /// Normalized spelling to display name, written by the user.
    #[serde(default)]
    user: BTreeMap<String, String>,
}

impl MakerAliases {
    /// Only the built-in table.
    pub fn builtin() -> Self {
        Self::default()
    }

    /// Where the user's aliases live, beside the user catalog.
    pub fn default_path() -> Option<PathBuf> {
        user_catalog_path().map(|p| p.with_file_name(ALIAS_FILE_NAME))
    }

    /// The user's aliases from `path`; the built-in table alone when the
    /// file is missing or unreadable (the first import then still works).
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Saves the user's aliases.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default())?;
        fs::rename(&tmp, path)
    }

    /// Makes `code` mean `name` (an empty name forgets the user's alias).
    pub fn set(&mut self, code: &str, name: &str) {
        let key = normalize_code(code);
        let name = name.trim();
        if key.is_empty() {
            return;
        }
        if name.is_empty() {
            self.user.remove(&key);
        } else {
            self.user.insert(key, name.to_owned());
        }
    }

    /// The user's own aliases as `(normalized code, name)`.
    pub fn user_entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.user.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// The display name a spelling stands for, `None` for an unknown maker.
    pub fn resolve(&self, text: &str) -> Option<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        // The whole text first, then without trailing legal-form words.
        let mut end = words.len();
        loop {
            let key = normalize_code(&words[..end].join(" "));
            if let Some(name) = self.exact(&key) {
                return Some(name);
            }
            let last = normalize_code(words[end - 1]);
            if end > 1 && (last.is_empty() || LEGAL_FORMS.contains(&last.as_str())) {
                end -= 1;
            } else {
                break;
            }
        }
        let key = normalize_code(text);
        self.by_prefix(&key)
    }

    /// The display name, or the text as typed (trimmed) for an unknown maker.
    pub fn display(&self, text: &str) -> String {
        self.resolve(text).unwrap_or_else(|| text.trim().to_owned())
    }

    fn exact(&self, key: &str) -> Option<String> {
        if key.is_empty() {
            return None;
        }
        if let Some(name) = self.user.get(key) {
            return Some(name.clone());
        }
        BUILTIN
            .iter()
            .find(|(name, aliases)| {
                normalize_code(name) == key || aliases.iter().any(|a| normalize_code(a) == key)
            })
            .map(|(name, _)| (*name).to_owned())
    }

    /// A code cut short (`SCHNEI`) when exactly one maker starts with it.
    fn by_prefix(&self, key: &str) -> Option<String> {
        if key.chars().count() < MIN_PREFIX {
            return None;
        }
        let mut found: Option<&str> = None;
        for (name, _) in BUILTIN {
            if normalize_code(name).starts_with(key) {
                if found.is_some_and(|f| f != *name) {
                    return None;
                }
                found = Some(name);
            }
        }
        found.map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shop_codes_for_one_maker_merge() {
        let a = MakerAliases::builtin();
        for code in [
            "SE",
            "SCHNEI",
            "Schneider Electric SE",
            "schneider-electric",
        ] {
            assert_eq!(a.display(code), "Schneider Electric", "{code}");
        }
        assert_eq!(a.display("SIE"), a.display("SIEMEN"));
        assert_eq!(a.display("RIT"), "Rittal");
        assert_eq!(a.display("RITTAL"), "Rittal");
        assert_eq!(a.display("Rittal GmbH & Co. KG"), "Rittal");
    }

    #[test]
    fn unknown_codes_stay_as_typed() {
        let a = MakerAliases::builtin();
        assert_eq!(a.resolve("XQZ"), None);
        assert_eq!(a.display("  XQZ "), "XQZ");
        // Two letters are too short to guess from.
        assert_eq!(a.resolve("SI"), None);
    }

    #[test]
    fn user_aliases_win_and_survive_a_save() {
        let mut a = MakerAliases::builtin();
        a.set("XQZ", "Example Works");
        a.set("SE", "Sonnenschein");
        assert_eq!(a.display("xqz"), "Example Works");
        assert_eq!(a.display("SE"), "Sonnenschein");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(ALIAS_FILE_NAME);
        a.save(&path).unwrap();
        assert_eq!(MakerAliases::load(&path), a);
        a.set("XQZ", "");
        assert_eq!(a.resolve("XQZ"), None);
    }
}
