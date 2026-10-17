//! A translation file: English source text to the text of one language.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Entries of one language. Keys are the English texts the reports and
/// templates print; a key may hold `{name}` placeholders, which a
/// translation must keep (see [`Catalog::problems`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Catalog {
    /// Language code (`nl`).
    pub code: String,
    /// The language's own name, shown in the picker (`Nederlands`).
    #[serde(default)]
    pub name: String,
    /// The decimal separator of measured values on pages and in CSV: `,` or
    /// `.`. Left out, the language uses `.`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimal: Option<String>,
    #[serde(default)]
    pub entries: BTreeMap<String, String>,
}

/// What is wrong with an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogProblem {
    pub key: String,
    pub message: String,
}

/// The `{names}` in a text, in order, without braces. `{{` and `}}` are
/// literal braces and are skipped.
pub fn placeholders(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find('{') {
        let tail = &rest[i..];
        if let Some(after) = tail.strip_prefix("{{") {
            rest = after;
            continue;
        }
        match tail.find('}') {
            Some(end) => {
                out.push(tail[1..end].trim().to_owned());
                rest = &tail[end + 1..];
            }
            None => break,
        }
    }
    out
}

/// Why `text` cannot be the translation of `key`, if it cannot: it is empty
/// or it keeps different `{placeholders}` (order does not matter).
pub fn check_entry(key: &str, text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return Some("the translation is empty".into());
    }
    let (mut a, mut b) = (placeholders(key), placeholders(text));
    a.sort();
    b.sort();
    (a != b).then(|| {
        if a.is_empty() {
            "the translation must not contain {placeholders}".to_owned()
        } else {
            format!("the translation must keep {{{}}}", a.join("}, {"))
        }
    })
}

impl Catalog {
    pub fn new(code: &str, name: &str) -> Self {
        Catalog {
            code: code.to_owned(),
            name: name.to_owned(),
            decimal: None,
            entries: BTreeMap::new(),
        }
    }

    /// The decimal separator, if the file names a usable one (`,` or `.`).
    pub fn decimal_char(&self) -> Option<char> {
        match self.decimal.as_deref().map(str::trim) {
            Some(",") => Some(','),
            Some(".") => Some('.'),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    pub fn set(&mut self, key: &str, text: &str) {
        self.entries.insert(key.to_owned(), text.to_owned());
    }

    /// Entries whose translation changes the placeholders of the key, or is
    /// empty: such a line would print wrong or blank values.
    pub fn problems(&self) -> Vec<CatalogProblem> {
        self.entries
            .iter()
            .filter_map(|(key, text)| {
                check_entry(key, text).map(|message| CatalogProblem {
                    key: key.clone(),
                    message,
                })
            })
            .collect()
    }
}

/// File name suffix of a language file.
pub const EXTENSION: &str = ".kopercad-lang.json";

/// Why a language file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum LanguageFileError {
    #[error("not a language file: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("the file has no language code")]
    NoCode,
}

impl Catalog {
    /// The language as pretty JSON for a file: the code, the name and the
    /// entries, sorted so files diff well.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Reads a language file. The code is lower-cased; entries that change
    /// placeholders are returned with their reasons by the caller through
    /// [`Catalog::problems`] (or dropped by `ProjectTable::put_language`).
    pub fn from_json(text: &str) -> Result<Catalog, LanguageFileError> {
        let mut c: Catalog = serde_json::from_str(text)?;
        c.code = c.code.trim().to_lowercase();
        if c.code.is_empty() {
            return Err(LanguageFileError::NoCode);
        }
        Ok(c)
    }
}

/// The catalogs that ship with the program: Dutch. English is the source
/// text itself and needs no file.
pub fn builtin() -> Vec<Catalog> {
    const NL: &str = include_str!("nl.json");
    vec![serde_json::from_str(NL).expect("nl.json is valid")]
}
