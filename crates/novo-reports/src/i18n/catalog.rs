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
            entries: BTreeMap::new(),
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

/// The catalogs that ship with the program: Dutch. English is the source
/// text itself and needs no file.
pub fn builtin() -> Vec<Catalog> {
    const NL: &str = include_str!("nl.json");
    vec![serde_json::from_str(NL).expect("nl.json is valid")]
}
