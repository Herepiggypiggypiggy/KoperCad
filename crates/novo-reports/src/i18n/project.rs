//! The translation table kept in the project properties.

use std::collections::BTreeMap;

use novo_core::{Project, Value};

use super::catalog::{Catalog, CatalogProblem, check_entry};

/// Project property: the language reports and PDFs print in (`en`, `nl`).
pub const LANGUAGE_KEY: &str = "i18n.language";
/// Project property: the project's translation table (JSON), entries the
/// user edited and whole languages the program does not ship.
pub const TABLE_KEY: &str = "i18n.table";

/// The project's own entries, per language code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectTable {
    languages: BTreeMap<String, Catalog>,
}

impl ProjectTable {
    pub fn language(&self, code: &str) -> Option<&Catalog> {
        self.languages.get(code)
    }

    pub fn codes(&self) -> impl Iterator<Item = &str> {
        self.languages.keys().map(String::as_str)
    }

    /// Sets one entry, creating the language if needed. A translation that
    /// is empty or changes the key's `{placeholders}` is refused with the
    /// reason, and nothing is stored.
    pub fn set(&mut self, code: &str, key: &str, text: &str) -> Result<(), String> {
        if let Some(why) = check_entry(key, text) {
            return Err(why);
        }
        self.languages
            .entry(code.to_owned())
            .or_insert_with(|| Catalog::new(code, ""))
            .set(key, text);
        Ok(())
    }

    /// Sets the decimal separator (`,` or `.`) the language prints measured
    /// values with, creating the language if needed.
    pub fn set_decimal(&mut self, code: &str, decimal: char) {
        self.languages
            .entry(code.to_owned())
            .or_insert_with(|| Catalog::new(code, ""))
            .decimal = Some(decimal.to_string());
    }

    /// Removes one entry (the shipped text, or English, shows again).
    pub fn remove(&mut self, code: &str, key: &str) {
        if let Some(c) = self.languages.get_mut(code) {
            c.entries.remove(key);
            if c.entries.is_empty() && c.name.is_empty() && c.decimal.is_none() {
                self.languages.remove(code);
            }
        }
    }

    /// Adds a language file without overwriting what the project already has
    /// for that code: the project's own entries win, the file fills in the
    /// rest. Entries that fail [`check_entry`] are dropped and returned.
    pub fn merge_language(&mut self, mut catalog: Catalog) -> Vec<CatalogProblem> {
        let bad = catalog.problems();
        for p in &bad {
            catalog.entries.remove(&p.key);
        }
        match self.languages.get_mut(&catalog.code) {
            Some(own) => {
                if own.name.is_empty() {
                    own.name = catalog.name;
                }
                if own.decimal.is_none() {
                    own.decimal = catalog.decimal;
                }
                for (key, text) in catalog.entries {
                    own.entries.entry(key).or_insert(text);
                }
            }
            None => {
                self.languages.insert(catalog.code.clone(), catalog);
            }
        }
        bad
    }

    /// Adds or replaces a whole language (a translation file loaded by the
    /// user). Entries that fail [`check_entry`] are dropped and returned
    /// with their reasons, so a bad file never prints wrong values.
    pub fn put_language(&mut self, mut catalog: Catalog) -> Vec<CatalogProblem> {
        let bad = catalog.problems();
        for p in &bad {
            catalog.entries.remove(&p.key);
        }
        self.languages.insert(catalog.code.clone(), catalog);
        bad
    }

    /// The value stored in [`TABLE_KEY`]; empty when there is nothing.
    pub fn to_property(&self) -> Option<String> {
        if self.languages.is_empty() {
            return None;
        }
        let list: Vec<&Catalog> = self.languages.values().collect();
        serde_json::to_string(&list).ok()
    }

    pub fn from_property(text: &str) -> Result<Self, serde_json::Error> {
        let list: Vec<Catalog> = serde_json::from_str(text)?;
        Ok(ProjectTable {
            languages: list.into_iter().map(|c| (c.code.clone(), c)).collect(),
        })
    }

    /// The table saved in a project; empty if none or unreadable (the
    /// reports then print the shipped text).
    pub fn of_project(project: &Project) -> Self {
        match project.info().properties.get(TABLE_KEY) {
            Some(Value::Text(t)) => ProjectTable::from_property(t).unwrap_or_default(),
            _ => ProjectTable::default(),
        }
    }
}

/// The language code saved in a project; `en` when none.
pub fn language_of(project: &Project) -> String {
    match project.info().properties.get(LANGUAGE_KEY) {
        Some(Value::Text(t)) if !t.trim().is_empty() => t.trim().to_owned(),
        _ => super::translator::ENGLISH.to_owned(),
    }
}
