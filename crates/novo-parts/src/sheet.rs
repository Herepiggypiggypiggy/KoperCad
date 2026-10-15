//! The property sheet behind the properties panel (WORKFLOW.md J12).
//!
//! A [`PropertySheet`] is built from the selection and lists what can be
//! read or changed about it as plain rows: the common fields for the kind of
//! object first (tag, description, part), then the part's data, then the
//! object's own properties, then the rest. Every row is reachable by typing
//! part of its name or value into the filter ([`PropertySheet::filter`]).
//!
//! The sheet is generic over the model: rows come from each record's JSON
//! form, so new model fields show up without code here, and edits go back
//! through the model's own validation as [`Command`]s. With several objects
//! selected, rows show the shared value (or that it is mixed), and an edit
//! applies to all of them as one command, so one undo step.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use novo_core::{
    AnyId, Command, DeviceTag, EntityKind, Part, PartId, Project, Record, SymbolRole, Value,
};
use serde_json::{Map, Number, Value as Json};
use thiserror::Error;

use crate::assign::count;
use crate::search::Matcher;

/// Which panel section a row belongs to, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    /// Tag, description, part, and the few fields each kind is about.
    Common,
    /// The assigned part: manufacturer, order number, type.
    Part,
    /// Technical data of the assigned part (read-only here; it belongs to
    /// the part).
    PartData,
    /// The object's own named properties.
    Properties,
    /// Everything else.
    More,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::Common => "Common",
            Group::Part => "Part",
            Group::PartData => "Part data",
            Group::Properties => "Properties",
            Group::More => "More",
        }
    }

    pub const ALL: [Group; 5] = [
        Group::Common,
        Group::Part,
        Group::PartData,
        Group::Properties,
        Group::More,
    ];
}

/// What a row edits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RowKey {
    /// A field of the object itself (`tag`, `description`, `cross_section_mm2`).
    Field(String),
    /// An entry of the object's `properties`.
    Property(String),
    /// The assigned part. Changed by picking from the catalog
    /// ([`crate::assign_part`]), not by typing a value.
    Part,
    /// A field of the assigned part: manufacturer, order number, type
    /// (read-only; pick another part to change them).
    PartField(String),
    /// A technical value of the assigned part (`rated_current_a`). Editing
    /// it overrides the value on the selected objects only, as their own
    /// property; [`PropertySheet::reset_to_part`] removes the override.
    PartData(String),
}

impl fmt::Display for RowKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowKey::Field(k) => f.write_str(k),
            RowKey::Property(k) => write!(f, "properties.{k}"),
            RowKey::Part => f.write_str("part"),
            RowKey::PartField(k) | RowKey::PartData(k) => write!(f, "part.{k}"),
        }
    }
}

/// The value a row shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowValue {
    /// Every selected object has this value.
    Same(String),
    /// The selected objects differ; the number of distinct values.
    Mixed(usize),
}

impl RowValue {
    /// The shared value, or `None` when mixed.
    pub fn same(&self) -> Option<&str> {
        match self {
            RowValue::Same(v) => Some(v),
            RowValue::Mixed(_) => None,
        }
    }
}

impl fmt::Display for RowValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowValue::Same(v) => f.write_str(v),
            RowValue::Mixed(n) => write!(f, "mixed ({n} values)"),
        }
    }
}

/// One line of the properties panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: RowKey,
    pub label: String,
    pub group: Group,
    pub value: RowValue,
    pub editable: bool,
    /// Unit shown after the value (`mm²`, `A`).
    pub unit: Option<&'static str>,
    /// Allowed values, for fields that are a fixed list (`kind`).
    pub choices: &'static [&'static str],
    /// Other words that find this row (`order` finds Part, `color` finds
    /// Colour).
    pub synonyms: &'static [&'static str],
    /// The object's own property differs from the value its part gives,
    /// so the part no longer decides it. Shown as an override, with
    /// [`PropertySheet::reset_to_part`] to undo it.
    pub overrides_part: bool,
    order: u32,
}

impl Row {
    /// Whether every word of the filter occurs in the row's label, key,
    /// group, synonyms, unit or value.
    pub fn matches(&self, matcher: &Matcher) -> bool {
        let key = self.key.to_string();
        let value = self.value.to_string();
        let mut fields: Vec<&str> = vec![&self.label, &key, self.group.label(), &value];
        fields.extend(self.synonyms.iter().copied());
        fields.extend(self.unit);
        fields.extend(self.choices.iter().copied());
        matcher.matches(fields)
    }
}

/// Why an edit was refused. The message is shown under the field, before
/// anything changes.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EditError {
    #[error("{0} is read-only here")]
    ReadOnly(String),
    #[error("pick a part from the catalog to change it")]
    UsePartSearch,
    #[error("select one device to change its tag; tags must be unique")]
    TagNeedsOne,
    #[error("{0}")]
    InvalidTag(String),
    #[error("{tag} is already used by another device")]
    TagInUse { tag: String },
    #[error("{label} must be a number")]
    NotANumber { label: String },
    #[error("{label} must be yes or no")]
    NotABool { label: String },
    #[error("{label} cannot be `{value}`{hint}")]
    Invalid {
        label: String,
        value: String,
        hint: String,
    },
    #[error("a property name needs at least one letter or digit")]
    EmptyName,
    #[error("nothing is selected")]
    NothingSelected,
}

/// Turns a canvas selection into the objects the panel edits: a symbol
/// drawn for a device edits the device, a terminal symbol its terminal.
/// Order is kept and duplicates removed.
pub fn resolve_selection(project: &Project, selection: &[AnyId]) -> Vec<AnyId> {
    let mut out: Vec<AnyId> = Vec::new();
    for &id in selection {
        let resolved = match id {
            AnyId::Symbol(s) => match project.symbol(s).map(|s| &s.role) {
                Some(SymbolRole::Device { device, .. }) => AnyId::from(*device),
                Some(SymbolRole::Terminal { terminals }) if terminals.len() == 1 => {
                    AnyId::from(terminals[0])
                }
                _ => id,
            },
            _ => id,
        };
        if project.contains(resolved) && !out.contains(&resolved) {
            out.push(resolved);
        }
    }
    out
}

/// Rows describing the selection; see the module docs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropertySheet {
    targets: Vec<AnyId>,
    title: String,
    rows: Vec<Row>,
}

struct FieldSpec {
    key: &'static str,
    label: &'static str,
    group: Group,
    order: u32,
    unit: Option<&'static str>,
    numeric: bool,
    editable: bool,
    synonyms: &'static [&'static str],
}

const fn spec(key: &'static str, label: &'static str, group: Group, order: u32) -> FieldSpec {
    FieldSpec {
        key,
        label,
        group,
        order,
        unit: None,
        numeric: false,
        editable: true,
        synonyms: &[],
    }
}

const FIELDS: &[FieldSpec] = &[
    FieldSpec {
        synonyms: &[
            "name",
            "designation",
            "reference designation",
            "bmk",
            "device",
        ],
        ..spec("tag", "Tag", Group::Common, 0)
    },
    spec("name", "Name", Group::Common, 1),
    FieldSpec {
        synonyms: &["page title", "heading"],
        ..spec("title", "Title", Group::Common, 2)
    },
    FieldSpec {
        synonyms: &["text", "function text", "comment", "beschrijving"],
        ..spec("description", "Description", Group::Common, 3)
    },
    FieldSpec {
        synonyms: &["maker", "brand", "hersteller", "fabrikant"],
        ..spec("manufacturer", "Manufacturer", Group::Common, 4)
    },
    FieldSpec {
        synonyms: &["article", "part number", "bestelnummer", "artikelnummer"],
        ..spec("order_number", "Order number", Group::Common, 5)
    },
    FieldSpec {
        synonyms: &["type", "model"],
        ..spec("type_number", "Type number", Group::Common, 6)
    },
    FieldSpec {
        synonyms: &["wire no", "label", "potential"],
        ..spec("number", "Wire number", Group::Common, 7)
    },
    FieldSpec {
        synonyms: &["color", "kleur", "farbe"],
        ..spec("color", "Colour", Group::Common, 8)
    },
    FieldSpec {
        unit: Some("mm²"),
        numeric: true,
        synonyms: &["section", "size", "area", "doorsnede", "querschnitt"],
        ..spec("cross_section_mm2", "Cross-section", Group::Common, 9)
    },
    FieldSpec {
        unit: Some("V"),
        numeric: true,
        synonyms: &["volt", "spanning", "potential"],
        ..spec("voltage", "Voltage", Group::Common, 10)
    },
    FieldSpec {
        unit: Some("m"),
        numeric: true,
        synonyms: &["lengte", "cable length"],
        ..spec("length_m", "Length", Group::Common, 11)
    },
    FieldSpec {
        numeric: true,
        synonyms: &["bridge", "jumper", "brug"],
        ..spec("jumper", "Jumper group", Group::Common, 12)
    },
    FieldSpec {
        numeric: true,
        synonyms: &["tier", "etage"],
        ..spec("level", "Level", Group::Common, 13)
    },
    FieldSpec {
        synonyms: &["type", "function"],
        ..spec("kind", "Kind", Group::More, 20)
    },
    FieldSpec {
        numeric: true,
        synonyms: &["sort", "position"],
        ..spec("order", "Sort position", Group::More, 21)
    },
    FieldSpec {
        editable: false,
        synonyms: &["symbol key", "library"],
        ..spec("symbol", "Symbol", Group::More, 22)
    },
    FieldSpec {
        editable: false,
        synonyms: &["sheet"],
        ..spec("page", "Page", Group::More, 23)
    },
    FieldSpec {
        editable: false,
        synonyms: &["terminal strip"],
        ..spec("strip", "Strip", Group::More, 24)
    },
    FieldSpec {
        editable: false,
        synonyms: &["conductors", "aders"],
        ..spec("cores", "Cores", Group::Common, 14)
    },
];

/// Fields that are internal, structural or drawn rather than typed.
const HIDDEN: &[&str] = &[
    "record",
    "id",
    "properties",
    "placement",
    "role",
    "pins",
    "bridges",
    "pass_through",
    "from",
    "to",
    "route",
    "position",
    "format",
];

/// Namespaces of properties that crates keep for their own bookkeeping:
/// declined suggestions (`terminal.suggest`, `cable.propose`), strip styles
/// (`terminal_part.*`, `terminal_symbol.*`), cable cores and shields, legacy
/// terminal rules. Each is edited through its own screen, so the property
/// sheet and the bulk table leave them out instead of showing raw ids.
pub const INTERNAL_KEY_PREFIXES: &[&str] = &[
    "connect.",
    "terminal.",
    "terminals.",
    "terminal_part.",
    "terminal_symbol.",
    "cable.",
];

/// Whether a property key belongs to an internal namespace
/// ([`INTERNAL_KEY_PREFIXES`]) and is not shown to the user as a property.
pub fn is_internal_key(key: &str) -> bool {
    INTERNAL_KEY_PREFIXES.iter().any(|p| key.starts_with(p))
}

const PART_SYNONYMS: &[&str] = &[
    "article",
    "order number",
    "catalog",
    "catalogue",
    "artikel",
    "part number",
    "manufacturer",
    "device data",
];

fn field_spec(key: &str) -> Option<&'static FieldSpec> {
    FIELDS.iter().find(|f| f.key == key)
}

fn choices(kind: EntityKind, key: &str) -> &'static [&'static str] {
    match (kind, key) {
        (EntityKind::Device, "kind") => &["general", "terminal_strip", "connector", "plc"],
        (EntityKind::Potential, "kind") => &[
            "line",
            "neutral",
            "protective_earth",
            "dc_positive",
            "dc_negative",
            "signal",
        ],
        (EntityKind::Terminal, "kind") => &[
            "feedthrough",
            "protective_earth",
            "neutral",
            "fuse",
            "disconnect",
        ],
        _ => &[],
    }
}

/// Unit suffixes of property keys (`rated_current_a`), longest first.
const UNIT_SUFFIXES: &[(&str, &str)] = &[
    ("_mm2", "mm²"),
    ("_mm", "mm"),
    ("_kw", "kW"),
    ("_ka", "kA"),
    ("_hz", "Hz"),
    ("_ma", "mA"),
    ("_va", "VA"),
    ("_kg", "kg"),
    ("_a", "A"),
    ("_v", "V"),
    ("_w", "W"),
    ("_s", "s"),
    ("_m", "m"),
];

/// `rated_current_a` → (`Rated current`, `A`).
pub fn property_label(key: &str) -> (String, Option<&'static str>) {
    // ASCII-only lowering keeps byte offsets equal to `key`'s, so slicing
    // `key` below stays on a char boundary (`to_lowercase` maps e.g. the
    // Kelvin sign U+212A, 3 bytes, to `k`, 1 byte).
    let lower = key.to_ascii_lowercase();
    let (stem, unit) = UNIT_SUFFIXES
        .iter()
        .find(|(suffix, _)| lower.ends_with(suffix) && lower.len() > suffix.len())
        .map(|(suffix, unit)| (&key[..key.len() - suffix.len()], Some(*unit)))
        .unwrap_or((key, None));
    (humanize(stem), unit)
}

fn humanize(key: &str) -> String {
    let words = key.replace(['_', '-'], " ");
    let mut chars = words.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A model value as the panel shows it: `9`, `2.2`, `yes`, `Siemens`.
pub fn display_value(value: &Value) -> String {
    match value {
        Value::Bool(b) => if *b { "yes" } else { "no" }.into(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Text(t) => t.clone(),
    }
}

fn display_json(value: &Json) -> String {
    match value {
        Json::Null => String::new(),
        Json::Bool(b) => if *b { "yes" } else { "no" }.into(),
        Json::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.to_string(),
            (None, Some(f)) => f.to_string(),
            _ => n.to_string(),
        },
        Json::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn parse_bool(text: &str) -> Option<bool> {
    match text.trim().to_lowercase().as_str() {
        "yes" | "y" | "true" | "on" | "1" | "ja" | "j" | "x" => Some(true),
        "no" | "n" | "false" | "off" | "0" | "nee" | "nein" | "" => Some(false),
        _ => None,
    }
}

/// Parses a number as people type it: `2.5`, `2,5` (decimal comma) or `1e3`.
pub fn parse_number(text: &str) -> Option<f64> {
    let t = text.trim();
    let v: f64 = t.parse().ok().or_else(|| {
        // One comma and no dot: a decimal comma.
        (t.matches(',').count() == 1 && !t.contains('.'))
            .then(|| t.replace(',', "."))
            .and_then(|t| t.parse().ok())
    })?;
    v.is_finite().then_some(v)
}

/// Turns typed text into a property value. With `like` (the value already
/// there) the type is kept: a number stays a number. A new value is a
/// whole number, a decimal number, yes/no (`true`/`false`) or text.
pub fn parse_value(text: &str, like: Option<&Value>) -> Option<Value> {
    let t = text.trim();
    match like {
        Some(Value::Text(_)) => Some(Value::Text(t.to_owned())),
        Some(Value::Int(_)) => match t.parse::<i64>() {
            Ok(i) => Some(Value::Int(i)),
            Err(_) => parse_number(t).map(Value::Float),
        },
        Some(Value::Float(_)) => parse_number(t).map(Value::Float),
        Some(Value::Bool(_)) => parse_bool(t).map(Value::Bool),
        None => Some(if let Ok(i) = t.parse::<i64>() {
            Value::Int(i)
        } else if let Some(f) = parse_number(t).filter(|_| t.chars().any(|c| c.is_ascii_digit())) {
            Value::Float(f)
        } else if t.eq_ignore_ascii_case("true") {
            Value::Bool(true)
        } else if t.eq_ignore_ascii_case("false") {
            Value::Bool(false)
        } else {
            Value::Text(t.to_owned())
        }),
    }
}

/// Normalizes a new property name: `Rated current A` → `rated_current_a`.
pub fn property_key(name: &str) -> Option<String> {
    let mut key = String::new();
    for word in name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        if !key.is_empty() {
            key.push('_');
        }
        key.extend(word.chars().flat_map(char::to_lowercase));
    }
    (!key.is_empty()).then_some(key)
}

fn noun(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Page => "page",
        EntityKind::Device => "device",
        EntityKind::Symbol => "symbol",
        EntityKind::Junction => "junction",
        EntityKind::Wire => "wire",
        EntityKind::Potential => "potential",
        EntityKind::Terminal => "terminal",
        EntityKind::Cable => "cable",
        EntityKind::Part => "part",
        EntityKind::Text => "text",
        EntityKind::PotentialSystem => "supply system",
        EntityKind::Structure => "structure",
        EntityKind::Dismissal => "dismissal",
        EntityKind::BlockDefinition => "block definition",
        EntityKind::BlockInstance => "block",
        EntityKind::Revision => "revision",
        EntityKind::Rail => "rail",
        EntityKind::CabinetPlacement => "cabinet placement",
    }
}

fn record_json(record: &Record) -> Map<String, Json> {
    match serde_json::to_value(record) {
        Ok(Json::Object(map)) => map,
        _ => Map::new(),
    }
}

/// The parts assigned to a record (a device's parts, a cable's part).
fn assigned_parts(record: &Record) -> Option<Vec<PartId>> {
    match record {
        Record::Device(d) => Some(d.parts.clone()),
        Record::Cable(c) => Some(c.part.into_iter().collect()),
        Record::Terminal(t) => Some(t.part.into_iter().collect()),
        // Pages, symbols, wires, junctions, potentials, parts and texts
        // carry no part.
        _ => None,
    }
}

fn part_label(project: &Project, parts: &[PartId]) -> String {
    let Some(first) = parts.first().and_then(|&p| project.part(p)) else {
        return String::new();
    };
    let mut label = format!("{} {}", first.manufacturer, first.order_number);
    if parts.len() > 1 {
        label.push_str(&format!(" (+{})", parts.len() - 1));
    }
    label
}

/// Collects the per-target values of one row into a [`RowValue`].
fn combine(values: impl IntoIterator<Item = String>) -> RowValue {
    let distinct: BTreeSet<String> = values.into_iter().collect();
    match distinct.len() {
        0 => RowValue::Same(String::new()),
        1 => RowValue::Same(distinct.into_iter().next().unwrap_or_default()),
        n => RowValue::Mixed(n),
    }
}

impl PropertySheet {
    /// Builds the sheet for a selection. Symbols are resolved to the device
    /// or terminal they show (see [`resolve_selection`]).
    pub fn build(project: &Project, selection: &[AnyId]) -> Self {
        let targets = resolve_selection(project, selection);
        let records: Vec<Record> = targets
            .iter()
            .filter_map(|&id| project.record(id))
            .collect();
        if records.is_empty() {
            return PropertySheet::default();
        }
        let title = Self::title_for(project, &records);
        let first_kind = records[0].kind();
        let one_kind = records.iter().all(|r| r.kind() == first_kind);
        let jsons: Vec<Map<String, Json>> = records.iter().map(record_json).collect();
        let mut rows = Vec::new();

        // Fields every selected object has.
        for key in jsons[0].keys() {
            if HIDDEN.contains(&key.as_str()) || !jsons.iter().all(|j| j.contains_key(key)) {
                continue;
            }
            if key == "parts" || key == "part" {
                continue; // the Part row below
            }
            let spec = field_spec(key);
            let display: Vec<String> = match key.as_str() {
                "page" => records
                    .iter()
                    .map(|r| {
                        r.page()
                            .and_then(|p| project.page(p))
                            .map(|p| p.name.clone())
                            .unwrap_or_default()
                    })
                    .collect(),
                "strip" => records
                    .iter()
                    .map(|r| match r {
                        Record::Terminal(t) => project
                            .device(t.strip)
                            .map(|d| d.tag.to_string())
                            .unwrap_or_default(),
                        _ => String::new(),
                    })
                    .collect(),
                "cores" => records
                    .iter()
                    .map(|r| match r {
                        Record::Cable(c) => {
                            let names: Vec<&str> =
                                c.cores.iter().map(|c| c.name.as_str()).collect();
                            format!("{} ({})", names.len(), names.join(", "))
                        }
                        _ => String::new(),
                    })
                    .collect(),
                _ => {
                    // Structured values (objects, lists) are drawn or edited
                    // elsewhere, not typed.
                    if jsons
                        .iter()
                        .any(|j| matches!(j[key], Json::Object(_) | Json::Array(_)))
                    {
                        continue;
                    }
                    jsons.iter().map(|j| display_json(&j[key])).collect()
                }
            };
            let structural = matches!(key.as_str(), "page" | "strip" | "cores");
            rows.push(Row {
                key: RowKey::Field(key.clone()),
                label: spec.map_or_else(|| humanize(key), |s| s.label.to_owned()),
                group: spec.map_or(Group::More, |s| s.group),
                value: combine(display),
                editable: !structural && spec.is_none_or(|s| s.editable),
                unit: spec.and_then(|s| s.unit),
                choices: if one_kind {
                    choices(first_kind, key)
                } else {
                    &[]
                },
                synonyms: spec.map_or(&[], |s| s.synonyms),
                order: spec.map_or(50, |s| s.order),
                overrides_part: false,
            });
        }

        // The part, when every selected object can carry one.
        let assigned: Option<Vec<Vec<PartId>>> = records.iter().map(assigned_parts).collect();
        if let Some(assigned) = assigned {
            rows.push(Row {
                key: RowKey::Part,
                label: "Part".into(),
                group: Group::Common,
                value: combine(assigned.iter().map(|p| part_label(project, p))),
                editable: true,
                unit: None,
                choices: &[],
                synonyms: PART_SYNONYMS,
                order: 4,
                overrides_part: false,
            });
            let firsts: BTreeSet<Option<PartId>> =
                assigned.iter().map(|p| p.first().copied()).collect();
            if let [Some(part)] = firsts.into_iter().collect::<Vec<_>>()[..]
                && let Some(part) = project.part(part)
            {
                rows.extend(Self::part_rows(part));
            }
        }
        let part_value = |record: &Record, key: &str| -> Option<Value> {
            assigned_parts(record)?
                .first()
                .and_then(|&p| project.part(p))
                .and_then(|p| p.properties.get(key).cloned())
        };

        // Named properties: union over the selection; a missing one counts
        // as empty, so it shows as mixed.
        let mut keys: BTreeMap<&str, ()> = BTreeMap::new();
        for record in &records {
            for key in record.properties().keys().filter(|k| !is_internal_key(k)) {
                keys.insert(key, ());
            }
        }
        let has_properties = records.iter().all(|r| !matches!(r, Record::Junction(_)));
        if has_properties {
            for key in keys.keys() {
                let (label, unit) = property_label(key);
                rows.push(Row {
                    key: RowKey::Property((*key).to_owned()),
                    label,
                    group: Group::Properties,
                    value: combine(records.iter().map(|r| {
                        r.properties()
                            .get(*key)
                            .map(display_value)
                            .unwrap_or_default()
                    })),
                    editable: true,
                    unit,
                    choices: &[],
                    synonyms: &[],
                    order: 100,
                    overrides_part: records.iter().any(|r| {
                        part_value(r, key).is_some_and(|v| r.properties().get(*key) != Some(&v))
                    }),
                });
            }
        }

        rows.sort_by(|a, b| {
            (a.group, a.order)
                .cmp(&(b.group, b.order))
                .then_with(|| novo_core::natural_cmp(&a.label, &b.label))
        });
        PropertySheet {
            targets,
            title,
            rows,
        }
    }

    fn part_rows(part: &Part) -> Vec<Row> {
        let field = |key: &str, label: &str, value: &str, order| Row {
            key: RowKey::PartField(key.into()),
            label: label.into(),
            group: Group::Part,
            value: RowValue::Same(value.into()),
            editable: false,
            unit: None,
            choices: &[],
            synonyms: field_spec(key).map_or(&[], |s| s.synonyms),
            order,
            overrides_part: false,
        };
        let mut rows = vec![
            field("manufacturer", "Manufacturer", &part.manufacturer, 0),
            field("order_number", "Order number", &part.order_number, 1),
        ];
        if !part.type_number.is_empty() {
            rows.push(field("type_number", "Type number", &part.type_number, 2));
        }
        if !part.description.is_empty() {
            rows.push(field(
                "description",
                "Part description",
                &part.description,
                3,
            ));
        }
        for (key, value) in &part.properties {
            let (label, unit) = property_label(key);
            rows.push(Row {
                key: RowKey::PartData(key.clone()),
                label,
                group: Group::PartData,
                value: RowValue::Same(display_value(value)),
                editable: true,
                unit,
                choices: &[],
                synonyms: &[],
                order: 100,
                overrides_part: false,
            });
        }
        rows
    }

    fn title_for(project: &Project, records: &[Record]) -> String {
        if let [record] = records {
            return match record {
                Record::Device(d) => d.tag.to_string(),
                Record::Cable(c) => c.tag.to_string(),
                Record::Page(p) => format!("Page {}", p.name),
                Record::Potential(p) => format!("Potential {}", p.name),
                Record::Terminal(t) => project
                    .terminal_designation(t.id)
                    .unwrap_or_else(|| format!("Terminal {}", t.name)),
                Record::Part(p) => format!("{} {}", p.manufacturer, p.order_number),
                Record::Wire(w) => match &w.number {
                    Some(n) => format!("Wire {n}"),
                    None => "Wire".into(),
                },
                Record::Symbol(s) => format!("Symbol {}", s.symbol.0),
                Record::Junction(_) => "Junction".into(),
                Record::Text(t) => match &t.content {
                    novo_core::TextContent::Literal { text } => format!("Text {text}"),
                    _ => "Text".into(),
                },
                Record::PotentialSystem(s) => format!("Supply system {}", s.name),
                Record::Structure(s) => format!("Structure {}", s.tag),
                Record::Dismissal(d) => format!("Dismissed {}", d.proposal),
                Record::BlockDefinition(d) => format!("Block definition {}", d.name),
                Record::BlockInstance(b) => format!("Block {}", b.id),
                Record::Revision(r) => format!("Revision {}", r.index),
                Record::Rail(_) => "Rail".into(),
                Record::CabinetPlacement(_) => "Cabinet placement".into(),
            };
        }
        let kind = records[0].kind();
        if records.iter().all(|r| r.kind() == kind) {
            count(records.len(), noun(kind))
        } else {
            count(records.len(), "object")
        }
    }

    /// The objects this sheet edits.
    pub fn targets(&self) -> &[AnyId] {
        &self.targets
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// `-K1`, `3 devices`, `Wire 12`.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// All rows, common first.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn row(&self, key: &RowKey) -> Option<&Row> {
        self.rows.iter().find(|r| &r.key == key)
    }

    /// Rows matching every word of `query` in their name, synonyms or
    /// value, in display order. An empty query returns every row.
    pub fn filter(&self, query: &str) -> Vec<&Row> {
        let matcher = Matcher::new(query);
        self.rows.iter().filter(|r| r.matches(&matcher)).collect()
    }

    /// The row Enter in the filter box jumps to: the first editable match.
    pub fn best_match(&self, query: &str) -> Option<&Row> {
        self.filter(query).into_iter().find(|r| r.editable)
    }

    /// Checks an edit without making it: the error to show under the field
    /// while typing, or `Ok`.
    pub fn validate(&self, project: &Project, key: &RowKey, input: &str) -> Result<(), EditError> {
        self.edit(project, key, input).map(|_| ())
    }

    /// The command that sets `key` to `input` on every target, as one undo
    /// step. `Ok(None)` when nothing would change.
    pub fn edit(
        &self,
        project: &Project,
        key: &RowKey,
        input: &str,
    ) -> Result<Option<Command>, EditError> {
        if self.targets.is_empty() {
            return Err(EditError::NothingSelected);
        }
        let row = self.row(key);
        let label = row.map_or_else(|| key.to_string(), |r| r.label.clone());
        if row.is_some_and(|r| !r.editable) {
            return Err(EditError::ReadOnly(label));
        }
        let commands = match key {
            RowKey::Part => return Err(EditError::UsePartSearch),
            RowKey::PartField(_) => return Err(EditError::ReadOnly(label)),
            RowKey::PartData(name) => self.edit_property(project, name, &label, input)?,
            RowKey::Field(field) if field == "tag" => self.edit_tag(project, input)?,
            RowKey::Field(field) => self.edit_field(project, field, &label, input)?,
            RowKey::Property(name) => self.edit_property(project, name, &label, input)?,
        };
        Ok(self.batch(format!("Set {}", label.to_lowercase()), commands))
    }

    /// Adds (or overwrites) a named property on every target.
    pub fn add_property(
        &self,
        project: &Project,
        name: &str,
        input: &str,
    ) -> Result<Option<Command>, EditError> {
        if self.targets.is_empty() {
            return Err(EditError::NothingSelected);
        }
        let key = property_key(name).ok_or(EditError::EmptyName)?;
        let (label, _) = property_label(&key);
        let commands = self.edit_property(project, &key, &label, input)?;
        Ok(self.batch(format!("Add property {label}"), commands))
    }

    /// Removes the selection's own value for `name` wherever its part gives
    /// one, so the part decides again (one undo step).
    pub fn reset_to_part(&self, project: &Project, name: &str) -> Option<Command> {
        let commands: Vec<Command> = self
            .targets
            .iter()
            .filter_map(|&id| project.record(id))
            .filter(|r| r.properties().contains_key(name))
            .filter(|r| {
                assigned_parts(r)
                    .and_then(|parts| parts.first().copied())
                    .and_then(|p| project.part(p))
                    .is_some_and(|p| p.properties.contains_key(name))
            })
            .map(|r| Command::SetProperty {
                target: r.id(),
                key: name.into(),
                value: None,
            })
            .collect();
        let (label, _) = property_label(name);
        self.batch(format!("Reset {} to part", label.to_lowercase()), commands)
    }

    /// Removes a named property from every target.
    pub fn remove_property(&self, project: &Project, name: &str) -> Option<Command> {
        let commands: Vec<Command> = self
            .targets
            .iter()
            .filter(|&&id| {
                project
                    .record(id)
                    .is_some_and(|r| r.properties().contains_key(name))
            })
            .map(|&id| Command::SetProperty {
                target: id,
                key: name.into(),
                value: None,
            })
            .collect();
        let (label, _) = property_label(name);
        self.batch(format!("Remove property {label}"), commands)
    }

    fn batch(&self, label: String, commands: Vec<Command>) -> Option<Command> {
        if commands.is_empty() {
            return None;
        }
        let label = if self.targets.len() > 1 {
            format!("{label} on {}", self.title)
        } else {
            label
        };
        Some(Command::Batch { label, commands })
    }

    fn edit_tag(&self, project: &Project, input: &str) -> Result<Vec<Command>, EditError> {
        let [target] = self.targets[..] else {
            return Err(EditError::TagNeedsOne);
        };
        let text = input.trim();
        let mut tag = DeviceTag::parse(text).map_err(|e| EditError::InvalidTag(e.to_string()))?;
        // Typing only `-K5` keeps the object's function and location (J4).
        let current = match project.record(target) {
            Some(Record::Device(d)) => d.tag.clone(),
            Some(Record::Cable(c)) => c.tag.clone(),
            _ => return Err(EditError::ReadOnly("Tag".into())),
        };
        for aspect in [novo_core::Aspect::Function, novo_core::Aspect::Location] {
            if !text.contains(aspect.prefix()) {
                *tag.levels_mut(aspect) = current.levels(aspect).to_vec();
            }
        }
        if tag == current {
            return Ok(Vec::new());
        }
        let taken = project
            .devices()
            .filter(|d| d.tag == tag)
            .map(|d| AnyId::from(d.id))
            .chain(
                project
                    .cables()
                    .filter(|c| c.tag == tag)
                    .map(|c| AnyId::from(c.id)),
            )
            .any(|id| id != target);
        if taken {
            return Err(EditError::TagInUse {
                tag: tag.to_string(),
            });
        }
        Ok(vec![match target {
            AnyId::Device(device) => Command::SetTag { device, tag },
            _ => {
                let Some(Record::Cable(mut cable)) = project.record(target) else {
                    return Err(EditError::ReadOnly("Tag".into()));
                };
                cable.tag = tag;
                Command::Replace {
                    record: Record::Cable(cable),
                }
            }
        }])
    }

    fn edit_field(
        &self,
        project: &Project,
        field: &str,
        label: &str,
        input: &str,
    ) -> Result<Vec<Command>, EditError> {
        let spec = field_spec(field);
        let text = input.trim();
        let mut commands = Vec::new();
        for &id in &self.targets {
            let Some(record) = project.record(id) else {
                continue;
            };
            let mut json = record_json(&record);
            let Some(old) = json.get(field).cloned() else {
                continue;
            };
            let candidates: Vec<Json> = match &old {
                Json::Bool(_) => vec![Json::Bool(parse_bool(text).ok_or_else(|| {
                    EditError::NotABool {
                        label: label.into(),
                    }
                })?)],
                Json::Number(n) => vec![number_json(text, n.is_f64(), label)?],
                Json::Null if text.is_empty() => vec![Json::Null],
                Json::Null if spec.is_some_and(|s| s.numeric) => {
                    vec![number_json(text, false, label)?]
                }
                Json::Null => vec![Json::String(text.into())],
                Json::String(_) if text.is_empty() => {
                    vec![Json::Null, Json::String(String::new())]
                }
                Json::String(_) => {
                    // Fixed lists accept the label words too: `terminal strip`.
                    let value = if spec.is_some_and(|s| s.key == "kind") {
                        text.to_lowercase().replace([' ', '-'], "_")
                    } else {
                        text.to_owned()
                    };
                    vec![Json::String(value)]
                }
                _ => return Err(EditError::ReadOnly(label.into())),
            };
            let mut edited = None;
            for candidate in candidates {
                json.insert(field.into(), candidate);
                if let Ok(rec) = serde_json::from_value::<Record>(Json::Object(json.clone())) {
                    edited = Some(rec);
                    break;
                }
            }
            let Some(edited) = edited else {
                let choices = choices(record.kind(), field);
                return Err(EditError::Invalid {
                    label: label.into(),
                    value: text.into(),
                    hint: if choices.is_empty() {
                        String::new()
                    } else {
                        format!("; choose {}", choices.join(", "))
                    },
                });
            };
            if edited.non_finite_field().is_some() {
                return Err(EditError::NotANumber {
                    label: label.into(),
                });
            }
            if edited != record {
                commands.push(Command::Replace { record: edited });
            }
        }
        Ok(commands)
    }

    fn edit_property(
        &self,
        project: &Project,
        name: &str,
        label: &str,
        input: &str,
    ) -> Result<Vec<Command>, EditError> {
        let mut commands = Vec::new();
        for &id in &self.targets {
            let Some(record) = project.record(id) else {
                continue;
            };
            if matches!(record, Record::Junction(_)) {
                continue;
            }
            let old = record.properties().get(name);
            // The type the selection already uses for this property.
            let like = old
                .cloned()
                .or_else(|| {
                    self.targets
                        .iter()
                        .filter_map(|&t| project.record(t))
                        .find_map(|r| r.properties().get(name).cloned())
                })
                .or_else(|| {
                    // An override of part data keeps the part's type.
                    assigned_parts(&record)?
                        .first()
                        .and_then(|&p| project.part(p))
                        .and_then(|p| p.properties.get(name).cloned())
                });
            let new = if input.trim().is_empty() {
                None
            } else {
                Some(parse_value(input, like.as_ref()).ok_or_else(|| match like {
                    Some(Value::Bool(_)) => EditError::NotABool {
                        label: label.into(),
                    },
                    _ => EditError::NotANumber {
                        label: label.into(),
                    },
                })?)
            };
            if new.as_ref() != old {
                commands.push(Command::SetProperty {
                    target: id,
                    key: name.into(),
                    value: new,
                });
            }
        }
        Ok(commands)
    }
}

fn number_json(text: &str, float: bool, label: &str) -> Result<Json, EditError> {
    let err = || EditError::NotANumber {
        label: label.into(),
    };
    let v = parse_number(text).ok_or_else(err)?;
    if !float && v.fract() == 0.0 && v.abs() < 9.0e15 {
        Ok(Json::Number(Number::from(v as i64)))
    } else {
        Number::from_f64(v).map(Json::Number).ok_or_else(err)
    }
}
