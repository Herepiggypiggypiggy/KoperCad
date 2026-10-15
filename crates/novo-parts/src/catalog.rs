//! The part catalog: manufacturer articles the user can assign to devices.
//!
//! The catalog lives outside any project, in the user's profile
//! ([`user_catalog_path`]), so every project draws from the same parts. A
//! project keeps its own copy of each part it uses (`novo_core::Part`), so a
//! project file is complete without the catalog.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use novo_core::{Part, PartId, Properties, Value};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::collections::{BTreeMap, HashMap};

use crate::schema::{
    Asset, Classification, DataSource, LetThroughPoint, Lifecycle, PartRef, PartTerminal, Price,
    Provenance,
};
use crate::search::{Matcher, SearchDoc, normalize_code};

/// Version of the catalog file format written by [`Catalog::save`].
/// Version 2 adds classification, terminals, price, lifecycle, assets and
/// provenance; version 1 files still load.
pub const CATALOG_FORMAT_VERSION: u32 = 2;

/// Property keys this crate writes into `novo_core::Part::properties`. Other
/// workstreams read them (cabinet layout reads the dimensions, cables read
/// the cores), so they are stable.
pub mod keys {
    /// Width in mm (along the DIN rail).
    pub const WIDTH_MM: &str = "width_mm";
    /// Height in mm.
    pub const HEIGHT_MM: &str = "height_mm";
    /// Depth in mm (away from the mounting plate).
    pub const DEPTH_MM: &str = "depth_mm";
    /// Comma-separated core names of a cable part, in order: `BK,BN,GY,GNYE`.
    pub const CORES: &str = "cores";
    /// Cross-section of each core of a cable part, in mm².
    pub const CORE_CROSS_SECTION_MM2: &str = "core_cross_section_mm2";
    /// Comma-separated IEC 81346-2 letter codes the part fits: `K,Q`.
    pub const LETTER_CODES: &str = "letter_codes";
}

/// Outer size of a part in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Dimensions {
    pub width_mm: f64,
    pub height_mm: f64,
    pub depth_mm: f64,
}

/// One article in the catalog.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CatalogPart {
    pub manufacturer: String,
    /// Order (article) number, unique per manufacturer.
    pub order_number: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub type_number: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// IEC 81346-2 letter codes of devices this part fits, most typical
    /// first (`["K", "Q"]` for a contactor). Used to suggest parts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub letter_codes: Vec<String>,
    /// Extra search words: `contactor`, `coil 24V`, `mpcb`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<Dimensions>,
    /// Technical data with SI units in the key: `rated_current_a`,
    /// `coil_voltage_v`, `power_kw`, `poles`.
    #[serde(default, skip_serializing_if = "Properties::is_empty")]
    pub data: Properties,
    /// ETIM and ECLASS codes.
    #[serde(default, skip_serializing_if = "Classification::is_empty")]
    pub class: Classification,
    /// What the part does (`terminal`, `contactor`, `mcb`), the key to a symbol.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub function: String,
    /// Connection points of the part.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terminals: Vec<PartTerminal>,
    /// List price as imported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    #[serde(default, skip_serializing_if = "is_unknown_lifecycle")]
    pub lifecycle: Lifecycle,
    /// The part that replaces this one when it is phased out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<PartRef>,
    /// Parts that can stand in for this one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<PartRef>,
    /// Local files (STEP, DXF, mesh) in the user's library.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<Asset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    /// Source of individual `data` values, by key. A key with no entry has
    /// an unknown source.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<String, DataSource>,
    /// Let-through table of a protective device at 400 V, ascending in Ik.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub let_through: Vec<LetThroughPoint>,
}

fn is_unknown_lifecycle(value: &Lifecycle) -> bool {
    *value == Lifecycle::Unknown
}

/// Identity of a part: manufacturer and order number, compared without
/// case, spaces, dashes or dots (`3RT2016-1BB41` equals `3rt2016 1bb41`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PartKey {
    manufacturer: String,
    order_number: String,
}

impl PartKey {
    pub fn new(manufacturer: &str, order_number: &str) -> Self {
        PartKey {
            manufacturer: normalize_code(manufacturer),
            order_number: normalize_code(order_number),
        }
    }

    pub fn of_model(part: &Part) -> Self {
        PartKey::new(&part.manufacturer, &part.order_number)
    }
}

impl CatalogPart {
    pub fn new(manufacturer: impl Into<String>, order_number: impl Into<String>) -> Self {
        CatalogPart {
            manufacturer: manufacturer.into(),
            order_number: order_number.into(),
            ..Default::default()
        }
    }

    /// Where a `data` value came from; unknown when not recorded.
    pub fn source_of(&self, key: &str) -> DataSource {
        self.sources.get(key).copied().unwrap_or_default()
    }

    pub fn key(&self) -> PartKey {
        PartKey::new(&self.manufacturer, &self.order_number)
    }

    /// Whether this part suits a device with the given letter code. A part
    /// without letter codes suits anything.
    pub fn fits(&self, letter_code: &str) -> bool {
        self.letter_codes.is_empty()
            || self
                .letter_codes
                .iter()
                .any(|c| c.eq_ignore_ascii_case(letter_code))
    }

    /// Core names of a cable part (`BK,BN,GY,GNYE`), empty for other parts.
    pub fn cores(&self) -> Vec<String> {
        match self.data.get(keys::CORES) {
            Some(Value::Text(text)) => split_list(text),
            _ => Vec::new(),
        }
    }

    /// The project copy of this part, with dimensions and technical data in
    /// its properties.
    pub fn to_model(&self, id: PartId) -> Part {
        let mut properties = self.data.clone();
        if let Some(d) = self.dimensions {
            properties.insert(keys::WIDTH_MM.into(), Value::Float(d.width_mm));
            properties.insert(keys::HEIGHT_MM.into(), Value::Float(d.height_mm));
            properties.insert(keys::DEPTH_MM.into(), Value::Float(d.depth_mm));
        }
        if !self.letter_codes.is_empty() {
            properties.insert(
                keys::LETTER_CODES.into(),
                Value::Text(self.letter_codes.join(",")),
            );
        }
        Part {
            id,
            manufacturer: self.manufacturer.clone(),
            order_number: self.order_number.clone(),
            type_number: self.type_number.clone(),
            description: self.description.clone(),
            properties,
        }
    }

    /// The catalog form of a project part (inverse of [`CatalogPart::to_model`]).
    pub fn from_model(part: &Part) -> Self {
        let mut data = part.properties.clone();
        let num = |data: &mut Properties, key: &str| match data.remove(key) {
            Some(Value::Float(v)) => Some(v),
            Some(Value::Int(v)) => Some(v as f64),
            Some(other) => {
                data.insert(key.into(), other);
                None
            }
            None => None,
        };
        let width = num(&mut data, keys::WIDTH_MM);
        let height = num(&mut data, keys::HEIGHT_MM);
        let depth = num(&mut data, keys::DEPTH_MM);
        let dimensions = match (width, height, depth) {
            (Some(width_mm), Some(height_mm), Some(depth_mm)) => Some(Dimensions {
                width_mm,
                height_mm,
                depth_mm,
            }),
            _ => {
                // Partial dimensions stay as plain data.
                for (key, v) in [
                    (keys::WIDTH_MM, width),
                    (keys::HEIGHT_MM, height),
                    (keys::DEPTH_MM, depth),
                ] {
                    if let Some(v) = v {
                        data.insert(key.into(), Value::Float(v));
                    }
                }
                None
            }
        };
        let letter_codes = match data.remove(keys::LETTER_CODES) {
            Some(Value::Text(text)) => split_list(&text),
            Some(other) => {
                data.insert(keys::LETTER_CODES.into(), other);
                Vec::new()
            }
            None => Vec::new(),
        };
        CatalogPart {
            manufacturer: part.manufacturer.clone(),
            order_number: part.order_number.clone(),
            type_number: part.type_number.clone(),
            description: part.description.clone(),
            letter_codes,
            keywords: Vec::new(),
            dimensions,
            data,
            ..Default::default()
        }
    }

    /// Text the search matches against, besides the structured fields.
    pub(crate) fn data_text(&self) -> String {
        let mut text = String::new();
        for (key, value) in &self.data {
            text.push_str(key);
            text.push(' ');
            text.push_str(&crate::sheet::display_value(value));
            text.push(' ');
        }
        text
    }
}

impl fmt::Display for CatalogPart {
    /// `Siemens 3RT2016-1BB41`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.manufacturer, self.order_number)
    }
}

pub(crate) fn split_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Why a catalog file could not be read or written.
#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("cannot access the parts catalog {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("the parts catalog {path} is not valid: {source}")]
    Format {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("the parts catalog {path} was written by a newer KoperCAD (format {found})")]
    TooNew { path: PathBuf, found: u32 },
}

#[derive(Serialize, Deserialize)]
struct CatalogFile {
    format: u32,
    parts: Vec<CatalogPart>,
}

/// What importing parts into a catalog would change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergePreview {
    /// Parts the catalog does not have yet.
    pub added: Vec<PartKey>,
    /// Parts whose catalog entry would be replaced.
    pub changed: Vec<PartKey>,
    /// Parts already in the catalog exactly as imported.
    pub unchanged: usize,
}

/// A searchable set of parts, unique by [`PartKey`].
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    parts: Vec<CatalogPart>,
    /// Position of each part by key, so `get` and `upsert` stay constant
    /// time when a maker catalog brings 100,000 rows.
    index: HashMap<PartKey, usize>,
    /// Order number alone (normalised) to positions, ascending.
    by_order: HashMap<String, Vec<usize>>,
    /// Pre-lowered search text per part, same positions as `parts`.
    docs: Vec<SearchDoc>,
}

impl PartialEq for Catalog {
    fn eq(&self, other: &Self) -> bool {
        self.parts == other.parts
    }
}

impl Catalog {
    pub fn new() -> Self {
        Catalog::default()
    }

    /// The starter catalog shipped with the program: common IEC parts for
    /// motor starters and control circuits (contactors, breakers, overload
    /// relays, push buttons, terminals, cables). Example data to check
    /// against the manufacturer's current catalogue before ordering.
    pub fn starter() -> Self {
        let file: CatalogFile = serde_json::from_str(include_str!("../catalog/starter.json"))
            .expect("the built-in starter catalog is valid");
        let mut catalog = Catalog::new();
        for part in file.parts {
            catalog.upsert(part);
        }
        catalog
    }

    /// The catalog the program uses: the starter parts plus the user's own
    /// catalog file, whose entries win. A missing user file is not an error.
    pub fn load_default() -> Result<Self, CatalogError> {
        let mut catalog = Catalog::starter();
        if let Some(path) = user_catalog_path()
            && path.exists()
        {
            catalog.merge(Catalog::load(&path)?);
        }
        Ok(catalog)
    }

    pub fn len(&self) -> usize {
        self.parts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    pub fn parts(&self) -> &[CatalogPart] {
        &self.parts
    }

    pub fn get(&self, key: &PartKey) -> Option<&CatalogPart> {
        self.index.get(key).map(|&i| &self.parts[i])
    }

    pub fn find(&self, manufacturer: &str, order_number: &str) -> Option<&CatalogPart> {
        self.get(&PartKey::new(manufacturer, order_number))
    }

    /// Finds a part by order number alone (any manufacturer). Order numbers
    /// rarely collide between manufacturers; the first match wins.
    pub fn find_order_number(&self, order_number: &str) -> Option<&CatalogPart> {
        let wanted = normalize_code(order_number);
        let &first = self.by_order.get(&wanted)?.first()?;
        Some(&self.parts[first])
    }

    /// Adds a part, or replaces the one with the same key. Returns `true`
    /// when the part is new.
    pub fn upsert(&mut self, part: CatalogPart) -> bool {
        let key = part.key();
        match self.index.get(&key) {
            Some(&i) => {
                // The order number is part of the key, so `by_order` is unchanged.
                self.docs[i] = SearchDoc::new(&part);
                self.parts[i] = part;
                false
            }
            None => {
                let i = self.parts.len();
                self.by_order
                    .entry(normalize_code(&part.order_number))
                    .or_default()
                    .push(i);
                self.docs.push(SearchDoc::new(&part));
                self.parts.push(part);
                self.index.insert(key, i);
                true
            }
        }
    }

    /// Removes a part. Returns it if it was there.
    pub fn remove(&mut self, key: &PartKey) -> Option<CatalogPart> {
        let index = self.index.get(key).copied()?;
        let part = self.parts.remove(index);
        self.docs.remove(index);
        self.reindex();
        Some(part)
    }

    /// Keeps the parts for which `keep` is true and returns how many were
    /// removed. One pass and one re-index, so removing thousands of parts from
    /// a large catalog stays linear.
    pub fn retain(&mut self, mut keep: impl FnMut(&CatalogPart) -> bool) -> usize {
        let before = self.parts.len();
        let mut flags = Vec::with_capacity(before);
        self.parts.retain(|p| {
            let k = keep(p);
            flags.push(k);
            k
        });
        let mut flags = flags.into_iter();
        self.docs.retain(|_| flags.next().unwrap_or(true));
        self.reindex();
        before - self.parts.len()
    }

    fn reindex(&mut self) {
        self.index.clear();
        self.by_order.clear();
        for (i, part) in self.parts.iter().enumerate() {
            self.index.insert(part.key(), i);
            self.by_order
                .entry(normalize_code(&part.order_number))
                .or_default()
                .push(i);
        }
    }

    /// Adds every part of `other`; its entries replace equal keys here.
    /// Returns how many parts were new.
    pub fn merge(&mut self, other: Catalog) -> usize {
        other
            .parts
            .into_iter()
            .map(|p| self.upsert(p))
            .filter(|&new| new)
            .count()
    }

    /// What [`Catalog::merge`] would do with `parts`, without doing it: the
    /// import preview the user accepts before the catalog changes.
    pub fn preview_merge(&self, parts: &[CatalogPart]) -> MergePreview {
        let mut preview = MergePreview::default();
        for part in parts {
            match self.get(&part.key()) {
                None => preview.added.push(part.key()),
                Some(existing) if existing != part => preview.changed.push(part.key()),
                Some(_) => preview.unchanged += 1,
            }
        }
        preview
    }

    /// Parts matching every word of `query`, best match first. An empty
    /// query returns nothing.
    ///
    /// Exact order and type numbers rank first, then letter codes and
    /// keywords (`no`, `mpcb`, `K`), then words in the description,
    /// manufacturer or technical data. Order numbers match without spaces
    /// and dashes, so `3rt20161bb41` finds `3RT2016-1BB41`.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&CatalogPart> {
        let matcher = Matcher::new(query);
        if matcher.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<(u32, &CatalogPart)> = self
            .parts
            .iter()
            .zip(&self.docs)
            .filter_map(|(p, doc)| matcher.score_doc(doc).map(|s| (s, p)))
            .collect();
        hits.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.manufacturer.cmp(&b.1.manufacturer))
                .then_with(|| novo_core::natural_cmp(&a.1.order_number, &b.1.order_number))
        });
        hits.into_iter().take(limit).map(|(_, p)| p).collect()
    }

    /// Parts that fit a device with this letter code, optionally narrowed by
    /// a query, best first. With an empty query, the parts that list the
    /// letter code first come first.
    pub fn suggest(&self, letter_code: &str, query: &str, limit: usize) -> Vec<&CatalogPart> {
        if letter_code.is_empty() {
            return self.search(query, limit);
        }
        let matcher = Matcher::new(query);
        let mut hits: Vec<(u32, &CatalogPart)> = self
            .parts
            .iter()
            .zip(&self.docs)
            .filter(|(p, _)| !p.letter_codes.is_empty() && p.fits(letter_code))
            .filter_map(|(p, doc)| {
                let rank = p
                    .letter_codes
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case(letter_code))
                    .unwrap_or(0) as u32;
                let base = 100u32.saturating_sub(rank * 10);
                if matcher.is_empty() {
                    Some((base, p))
                } else {
                    matcher.score_doc(doc).map(|s| (s * 10 + base, p))
                }
            })
            .collect();
        hits.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.manufacturer.cmp(&b.1.manufacturer))
                .then_with(|| novo_core::natural_cmp(&a.1.order_number, &b.1.order_number))
        });
        hits.into_iter().take(limit).map(|(_, p)| p).collect()
    }

    /// Reads a catalog file.
    pub fn load(path: &Path) -> Result<Self, CatalogError> {
        let text = fs::read_to_string(path).map_err(|source| CatalogError::Io {
            path: path.into(),
            source,
        })?;
        Catalog::from_json(&text).map_err(|e| match e {
            FromJsonError::Format(source) => CatalogError::Format {
                path: path.into(),
                source,
            },
            FromJsonError::TooNew(found) => CatalogError::TooNew {
                path: path.into(),
                found,
            },
        })
    }

    /// Writes the catalog as JSON. The file is replaced only once the new
    /// one is complete, so a crash never leaves half a catalog. Parent
    /// folders are created (under the user's profile; no admin rights).
    pub fn save(&self, path: &Path) -> Result<(), CatalogError> {
        let io_err = |source| CatalogError::Io {
            path: path.into(),
            source,
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, self.to_json()).map_err(io_err)?;
        fs::rename(&tmp, path).map_err(io_err)
    }

    /// The catalog file contents.
    pub fn to_json(&self) -> String {
        let mut parts = self.parts.clone();
        parts.sort_by(|a, b| {
            a.manufacturer
                .cmp(&b.manufacturer)
                .then_with(|| novo_core::natural_cmp(&a.order_number, &b.order_number))
        });
        let file = CatalogFile {
            format: CATALOG_FORMAT_VERSION,
            parts,
        };
        serde_json::to_string_pretty(&file).expect("catalog serializes")
    }

    fn from_json(text: &str) -> Result<Self, FromJsonError> {
        let file: CatalogFile = serde_json::from_str(text).map_err(FromJsonError::Format)?;
        if file.format > CATALOG_FORMAT_VERSION {
            return Err(FromJsonError::TooNew(file.format));
        }
        let mut catalog = Catalog::new();
        for part in file.parts {
            catalog.upsert(part);
        }
        Ok(catalog)
    }
}

enum FromJsonError {
    Format(serde_json::Error),
    TooNew(u32),
}

impl FromIterator<CatalogPart> for Catalog {
    fn from_iter<I: IntoIterator<Item = CatalogPart>>(iter: I) -> Self {
        let mut catalog = Catalog::new();
        for part in iter {
            catalog.upsert(part);
        }
        catalog
    }
}

/// Where the user's own parts catalog lives: `%APPDATA%\KoperCAD\parts.json`
/// on Windows (the app moves it there from the old `NovoElectric` folder at
/// start; until then the old file is used), `$XDG_DATA_HOME/novoelectric/parts.json` (or
/// `~/.local/share/...`) elsewhere. Always inside the user's profile, so it
/// is writable without admin rights. `None` if no profile folder is known.
pub fn user_catalog_path() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    if cfg!(windows) {
        var("APPDATA").map(|dir| {
            let dir = PathBuf::from(dir);
            let new = dir.join("KoperCAD").join("parts.json");
            let old = dir.join("NovoElectric").join("parts.json");
            if !new.exists() && old.exists() {
                old
            } else {
                new
            }
        })
    } else {
        var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| var("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
            .map(|dir| dir.join("novoelectric").join("parts.json"))
    }
}
