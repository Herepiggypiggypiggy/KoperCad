//! The user's own symbol table: EPLAN symbol numbers to KoperCAD symbols.
//!
//! EPLAN identifies a drawn symbol by library set, number and variant. Those
//! numbers belong to the user's EPLAN installation, so KoperCAD ships none of
//! them: the table starts empty and is filled by [`UserTable::learn`] from
//! the user's own project or library export, then edited and saved as TOML.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// EPLAN's reference to a drawn symbol: library set (`ESymLib` id), symbol
/// number and variant, as `A1381`, `A1382` and `A1383` carry them in a PXF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SymbolKey {
    pub set: u32,
    pub number: u32,
    #[serde(default)]
    pub variant: u32,
}

impl std::fmt::Display for SymbolKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.set, self.number, self.variant)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Row {
    #[serde(flatten)]
    key: SymbolKey,
    /// Pack-less symbol path (`contacts/no`) or a full id.
    kopercad: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    symbol: Vec<Row>,
}

/// Symbol keys the user has mapped, kept as pack-less paths so the table
/// works for either pack.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserTable {
    rows: BTreeMap<SymbolKey, String>,
}

impl UserTable {
    pub fn new() -> UserTable {
        UserTable::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Maps `key` to `symbol` (a pack-less path or a full id).
    pub fn insert(&mut self, key: SymbolKey, symbol: &str) {
        self.rows
            .insert(key, novo_symbols::Pack::path_of(symbol).to_string());
    }

    /// The pack-less path `key` maps to.
    pub fn get(&self, key: SymbolKey) -> Option<&str> {
        self.rows.get(&key).map(String::as_str)
    }

    /// The key mapped to `symbol_path`, when the user has one (the lowest if
    /// several).
    pub fn key_for(&self, symbol_path: &str) -> Option<SymbolKey> {
        self.rows
            .iter()
            .find(|(_, s)| s.as_str() == symbol_path)
            .map(|(k, _)| *k)
    }

    pub fn iter(&self) -> impl Iterator<Item = (SymbolKey, &str)> {
        self.rows.iter().map(|(k, v)| (*k, v.as_str()))
    }

    pub fn from_toml(text: &str) -> Result<UserTable, toml::de::Error> {
        let file: File = toml::from_str(text)?;
        let mut table = UserTable::new();
        for row in file.symbol {
            table.insert(row.key, &row.kopercad);
        }
        Ok(table)
    }

    pub fn to_toml(&self) -> String {
        let file = File {
            symbol: self
                .rows
                .iter()
                .map(|(k, v)| Row {
                    key: *k,
                    kopercad: v.clone(),
                })
                .collect(),
        };
        toml::to_string(&file).expect("rows serialise")
    }
}
