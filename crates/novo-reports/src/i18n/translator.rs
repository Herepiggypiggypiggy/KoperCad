//! Looking texts up: project table over the built-in file over English.

use std::borrow::Cow;

use super::catalog::{Catalog, builtin};
use super::project::ProjectTable;

/// The code of the source language.
pub const ENGLISH: &str = "en";

/// Translates English source texts into one language. A text with no entry
/// comes back as it is (English), so a missing key never blanks a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translator {
    code: String,
    /// Entries the project edited or added; they win.
    project: Catalog,
    /// The shipped file for the language, if there is one.
    shipped: Catalog,
}

impl Default for Translator {
    fn default() -> Self {
        Translator::english()
    }
}

impl Translator {
    /// No translation: every text is returned as written.
    pub fn english() -> Self {
        Translator {
            code: ENGLISH.to_owned(),
            project: Catalog::default(),
            shipped: Catalog::default(),
        }
    }

    /// The translator for `code`, with the project's own entries on top of
    /// the shipped file.
    pub fn new(code: &str, table: &ProjectTable) -> Self {
        if code == ENGLISH {
            return Translator::english();
        }
        let shipped = builtin()
            .into_iter()
            .find(|c| c.code == code)
            .unwrap_or_default();
        Translator {
            code: code.to_owned(),
            project: table.language(code).cloned().unwrap_or_default(),
            shipped,
        }
    }

    /// Everything this translator knows as one catalog (the shipped file
    /// with the project's entries on top): what File > Report languages
    /// exports as a language file.
    pub fn catalog(&self) -> Catalog {
        let mut c = Catalog::new(&self.code, &self.shipped.name);
        c.entries = self.shipped.entries.clone();
        c.entries.extend(self.project.entries.clone());
        if c.name.is_empty() {
            c.name = self.project.name.clone();
        }
        c.decimal = self
            .project
            .decimal_char()
            .or(self.shipped.decimal_char())
            .map(String::from);
        c
    }

    /// The decimal separator of measured values in this language.
    pub fn decimal(&self) -> char {
        self.project
            .decimal_char()
            .or(self.shipped.decimal_char())
            .unwrap_or('.')
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn is_english(&self) -> bool {
        self.code == ENGLISH
    }

    /// The translation of exactly `key`, if any.
    pub fn lookup(&self, key: &str) -> Option<&str> {
        self.project.get(key).or_else(|| self.shipped.get(key))
    }

    /// Whether `text` has a translation, exact or by pattern.
    pub fn knows(&self, text: &str) -> bool {
        self.is_english() || self.translate(text).is_some()
    }

    /// The translation of `text`: an exact entry, or an entry whose key has
    /// `{name}` placeholders that the text fills in (`Line 3` for
    /// `Line {n}`).
    pub fn translate(&self, text: &str) -> Option<String> {
        if let Some(t) = self.lookup(text) {
            return Some(t.to_owned());
        }
        self.project
            .entries
            .iter()
            .chain(self.shipped.entries.iter())
            .filter(|(k, _)| k.contains('{'))
            .find_map(|(key, translation)| {
                let mut captures = match_pattern(key, text)?;
                let decimal = self.decimal();
                if decimal != '.' {
                    for (_, value) in &mut captures {
                        if is_decimal_number(value) {
                            *value = value.replace('.', &decimal.to_string());
                        }
                    }
                }
                Some(fill(translation, &captures))
            })
    }

    /// `text` in the language, or as written. `key`, when given, is looked
    /// up first (a template's explicit translation key).
    pub fn tr<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.is_english() || text.is_empty() {
            return Cow::Borrowed(text);
        }
        match self.translate(text) {
            Some(t) => Cow::Owned(t),
            None => Cow::Borrowed(text),
        }
    }

    /// Like [`Translator::tr`], trying the explicit translation `key` first.
    pub fn tr_keyed(&self, key: Option<&str>, text: &str) -> String {
        if let Some(t) = key.and_then(|k| self.translate(k)) {
            return t;
        }
        self.tr(text).into_owned()
    }
}

/// A measured value written with a point (`1.5`, `-0.25`): the only kind of
/// captured text that follows the language's decimal sign.
fn is_decimal_number(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    match digits.split_once('.') {
        Some((a, b)) => {
            !a.is_empty()
                && !b.is_empty()
                && a.bytes().all(|c| c.is_ascii_digit())
                && b.bytes().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// Matches `text` against a key with `{name}` placeholders; returns the
/// captured values. Placeholders match as little as possible.
fn match_pattern(pattern: &str, text: &str) -> Option<Vec<(String, String)>> {
    let mut parts: Vec<(&str, Option<&str>)> = Vec::new(); // (literal, name after it)
    let mut rest = pattern;
    while let Some(i) = rest.find('{') {
        let end = rest[i..].find('}')? + i;
        parts.push((&rest[..i], Some(rest[i + 1..end].trim())));
        rest = &rest[end + 1..];
    }
    parts.push((rest, None));
    let mut out = Vec::new();
    let mut pos = 0;
    let (first, _) = parts[0];
    if !text.starts_with(first) {
        return None;
    }
    pos += first.len();
    for k in 0..parts.len() - 1 {
        let name = parts[k].1?;
        let literal = parts[k + 1].0;
        let value_end = if k + 2 == parts.len() {
            // Last placeholder: the literal must end the text.
            if !text.ends_with(literal) || text.len() < pos + literal.len() {
                return None;
            }
            text.len() - literal.len()
        } else if literal.is_empty() {
            return None; // two adjacent placeholders are ambiguous
        } else {
            pos + text[pos..].find(literal)?
        };
        if value_end == pos {
            return None;
        }
        out.push((name.to_owned(), text[pos..value_end].to_owned()));
        pos = value_end + literal.len();
    }
    Some(out)
}

fn fill(template: &str, captures: &[(String, String)]) -> String {
    // One pass, so a captured value that looks like `{other}` is kept as is.
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let named = tail.find('}').and_then(|end| {
            let name = tail[1..end].trim();
            captures
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| (v, end))
        });
        match named {
            Some((value, end)) => {
                out.push_str(value);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('{');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
