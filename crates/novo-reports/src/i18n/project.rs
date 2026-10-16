//! The translation table kept in the project properties.

use std::collections::BTreeMap;

use novo_core::{Project, Value};

use super::catalog::Catalog;

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

    /// Sets one entry, creating the language if needed.
    pub fn set(&mut self, code: &str, key: &str, text: &str) {
        self.languages
            .entry(code.to_owned())
            .or_insert_with(|| Catalog::new(code, ""))
            .set(key, text);
    }

    /// Removes one entry (the shipped text, or English, shows again).
    pub fn remove(&mut self, code: &str, key: &str) {
        if let Some(c) = self.languages.get_mut(code) {
            c.entries.remove(key);
            if c.entries.is_empty() && c.name.is_empty() {
                self.languages.remove(code);
            }
        }
    }

    /// Adds or replaces a whole language (a translation file loaded by the
    /// user).
    pub fn put_language(&mut self, catalog: Catalog) {
        self.languages.insert(catalog.code.clone(), catalog);
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
