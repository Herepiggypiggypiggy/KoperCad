//! The mapping step of the table import: which column of the file is which
//! field of a part, and how a column's unit is read.
//!
//! [`Mapping::suggest`] looks at the header row (English, German and Dutch
//! words, units written in the header) and at the values, and proposes a
//! mapping with a short reason per column. The person corrects it in the
//! import dialog; [`SavedMappings`] remembers the corrected mapping for the
//! next file with the same columns.
//!
//! Nothing is invented: a column that cannot be recognised is offered as
//! technical data under its own name (or ignored when it has no header),
//! and a field no column provides stays unknown on the part.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::csv::{Column, column_for};
use crate::recipes::Dim;
use crate::schema::DataSource;
use crate::sheet::property_key;
use crate::tablefile::Sheet;

/// A known technical value: its stored key (SI unit in the name), what it
/// measures (decides which units convert) and the header words that name it.
pub struct Measure {
    pub key: &'static str,
    pub label: &'static str,
    pub dim: Dim,
    /// Header words, lower case with spaces and punctuation removed.
    pub words: &'static [&'static str],
}

/// Values the importer converts to the units the rest of KoperCAD reads.
pub const MEASURES: &[Measure] = &[
    Measure {
        key: "rated_current_a",
        label: "Rated current",
        dim: Dim::Current,
        words: &[
            "ratedcurrent",
            "nennstrom",
            "nominalcurrent",
            "currentrating",
            "ratedoperationalcurrent",
            "bemessungsstrom",
            "nominalestroom",
            "nominaalstroom",
            "stroom",
            "ie",
            "in",
            "current",
            "ratedcurrentie",
            "ratedoperatingcurrent",
            "operatingcurrent",
            "continuouscurrent",
            "bemessungsbetriebsstrom",
            "betriebsstrom",
            "nennstromie",
            "werkstroom",
            "bedrijfsstroom",
            "stroomsterkte",
            "ampere",
            "amps",
            "nominalestroomie",
        ],
    },
    Measure {
        key: "ac3_current_a",
        label: "AC-3 current",
        dim: Dim::Current,
        words: &[
            "ac3current",
            "ac3",
            "ieac3",
            "ratedcurrentac3",
            "nennstromac3",
        ],
    },
    Measure {
        key: "ac1_current_a",
        label: "AC-1 current",
        dim: Dim::Current,
        words: &[
            "ac1current",
            "ac1",
            "ieac1",
            "ratedcurrentac1",
            "nennstromac1",
        ],
    },
    Measure {
        key: "rated_voltage_v",
        label: "Rated voltage",
        dim: Dim::Voltage,
        words: &[
            "ratedvoltage",
            "nennspannung",
            "ratedoperationalvoltage",
            "bemessungsspannung",
            "nominalvoltage",
            "nominalespanning",
            "nominaalspanning",
            "spanning",
            "voltage",
            "ue",
            "operatingvoltage",
            "ratedoperatingvoltageue",
            "voltagerating",
            "betriebsspannung",
            "bemessungsbetriebsspannung",
            "nennspannungue",
            "bedrijfsspanning",
            "nominalespanningue",
        ],
    },
    Measure {
        key: "coil_voltage_v",
        label: "Coil voltage",
        dim: Dim::Voltage,
        words: &[
            "coilvoltage",
            "spulenspannung",
            "controlvoltage",
            "steuerspannung",
            "spoelspanning",
            "stuurspanning",
            "ucoil",
            "uc",
            "coil",
            "spule",
            "spoel",
            "controlcircuitvoltage",
            "controlsupplyvoltage",
            "steuerkreisspannung",
            "stuurstroomspanning",
            "magnetspoelspanning",
            "coilvoltageuc",
        ],
    },
    Measure {
        key: "power_loss_w",
        label: "Power loss",
        dim: Dim::PowerW,
        words: &[
            "powerloss",
            "powerdissipation",
            "heatdissipation",
            "dissipation",
            "verlustleistung",
            "verlustleistungw",
            "vermogensverlies",
            "vermogensverlies",
            "verlies",
            "losses",
            "pv",
            "totalpowerloss",
            "dissipatedpower",
            "heatloss",
            "thermalloss",
            "warmeverlust",
            "warmeabgabe",
            "warmteverlies",
            "warmteafgifte",
            "dissipatie",
            "verliesvermogen",
        ],
    },
    Measure {
        key: "rated_power_kw",
        label: "Rated power",
        dim: Dim::PowerKw,
        words: &[
            "ratedpower",
            "nennleistung",
            "motorpower",
            "motorleistung",
            "nominaalvermogen",
            "vermogen",
            "leistung",
            "power",
            "ratedmotorpower",
            "motorrating",
            "outputpower",
            "ratedoutput",
            "shaftpower",
            "abgabeleistung",
            "nennleistungmotor",
            "motorvermogen",
            "asvermogen",
            "nominaalmotorvermogen",
        ],
    },
    Measure {
        key: "breaking_capacity_icu_ka",
        label: "Breaking capacity Icu",
        dim: Dim::CurrentKa,
        words: &[
            "icu",
            "breakingcapacity",
            "ratedbreakingcapacity",
            "schaltvermogen",
            "ausschaltvermogen",
            "uitschakelvermogen",
            "icn",
            "shortcircuitbreakingcapacity",
            "ratedshortcircuitbreakingcapacity",
            "schaltvermogenicu",
            "kortsluitvermogen",
            "uitschakelvermogenicu",
        ],
    },
    Measure {
        key: "setting_min_a",
        label: "Setting range, from",
        dim: Dim::Current,
        words: &[
            "settingmin",
            "settingrangemin",
            "einstellbereichmin",
            "irmin",
        ],
    },
    Measure {
        key: "setting_max_a",
        label: "Setting range, to",
        dim: Dim::Current,
        words: &[
            "settingmax",
            "settingrangemax",
            "einstellbereichmax",
            "irmax",
        ],
    },
    Measure {
        key: "poles",
        label: "Poles",
        dim: Dim::Count,
        words: &[
            "poles",
            "numberofpoles",
            "polzahl",
            "polanzahl",
            "polen",
            "aantalpolen",
            "pole",
            "pol",
            "numberofpole",
            "polesnumber",
            "polenanzahl",
            "polenaantal",
            "aantalpool",
            "pools",
        ],
    },
    Measure {
        key: "section_mm2",
        label: "Conductor cross-section",
        dim: Dim::Area,
        words: &[
            "crosssection",
            "conductorcrosssection",
            "querschnitt",
            "leiterquerschnitt",
            "doorsnede",
            "section",
            "nominalcrosssection",
            "nennquerschnitt",
            "wirecrosssection",
            "cablecrosssection",
            "kabelquerschnitt",
            "aderquerschnitt",
            "kabeldoorsnede",
            "aderdoorsnede",
            "nominaledoorsnede",
        ],
    },
    Measure {
        key: "max_ambient_c",
        label: "Highest ambient temperature",
        dim: Dim::Temperature,
        words: &[
            "maxambient",
            "maxambienttemperature",
            "maximumambienttemperature",
            "ambienttemperaturemax",
            "umgebungstemperaturmax",
            "maxumgebungstemperatur",
            "maxoperatingtemperature",
            "maximumoperatingtemperature",
            "maxbetriebstemperatur",
            "maxomgevingstemperatuur",
            "omgevingstemperatuurmax",
            "maximaleomgevingstemperatuur",
            "upperambienttemperature",
        ],
    },
];

/// The measure stored under `key`.
pub fn measure(key: &str) -> Option<&'static Measure> {
    MEASURES.iter().find(|m| m.key == key)
}

/// What a file column becomes on the part.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    /// Left out of the import.
    Ignore,
    Manufacturer,
    OrderNumber,
    TypeNumber,
    Description,
    LetterCodes,
    Keywords,
    /// What the part does: `terminal`, `contactor`, `mcb` ...
    Function,
    Width,
    Height,
    Depth,
    /// A known value in the key's SI unit (`rated_current_a`).
    Measure(&'static str),
    Etim,
    Eclass,
    /// List price; the currency is the mapping's currency unless the cell
    /// names one.
    Price,
    /// Date of the price list (`2026-10-15`, `15.10.2026`, an Excel date).
    PriceDate,
    /// Pieces the price is for (`100`: the price is per 100 pieces).
    PricePer,
    Lifecycle,
    /// Connection method: screw, push-in, spring ...
    Connection,
    ConnectionCount,
    WireMin,
    WireMax,
    /// One cell holding both ends, `0.2 ... 6 mm²`.
    WireRange,
    /// Technical data kept under this key exactly as written in the file.
    Custom(String),
}

impl Target {
    /// Stable text for saved mappings.
    pub fn id(&self) -> String {
        match self {
            Target::Ignore => "ignore".into(),
            Target::Manufacturer => "manufacturer".into(),
            Target::OrderNumber => "order_number".into(),
            Target::TypeNumber => "type_number".into(),
            Target::Description => "description".into(),
            Target::LetterCodes => "letter_codes".into(),
            Target::Keywords => "keywords".into(),
            Target::Function => "function".into(),
            Target::Width => "width".into(),
            Target::Height => "height".into(),
            Target::Depth => "depth".into(),
            Target::Measure(key) => format!("measure:{key}"),
            Target::Etim => "etim".into(),
            Target::Eclass => "eclass".into(),
            Target::Price => "price".into(),
            Target::PriceDate => "price_date".into(),
            Target::PricePer => "price_per".into(),
            Target::Lifecycle => "lifecycle".into(),
            Target::Connection => "connection".into(),
            Target::ConnectionCount => "connection_count".into(),
            Target::WireMin => "wire_min".into(),
            Target::WireMax => "wire_max".into(),
            Target::WireRange => "wire_range".into(),
            Target::Custom(key) => format!("custom:{key}"),
        }
    }

    /// Reads [`Target::id`]; `None` for text this version does not know.
    pub fn from_id(id: &str) -> Option<Target> {
        Some(match id {
            "ignore" => Target::Ignore,
            "manufacturer" => Target::Manufacturer,
            "order_number" => Target::OrderNumber,
            "type_number" => Target::TypeNumber,
            "description" => Target::Description,
            "letter_codes" => Target::LetterCodes,
            "keywords" => Target::Keywords,
            "function" => Target::Function,
            "width" => Target::Width,
            "height" => Target::Height,
            "depth" => Target::Depth,
            "etim" => Target::Etim,
            "eclass" => Target::Eclass,
            "price" => Target::Price,
            "price_date" => Target::PriceDate,
            "price_per" => Target::PricePer,
            "lifecycle" => Target::Lifecycle,
            "connection" => Target::Connection,
            "connection_count" => Target::ConnectionCount,
            "wire_min" => Target::WireMin,
            "wire_max" => Target::WireMax,
            "wire_range" => Target::WireRange,
            other => {
                if let Some(key) = other.strip_prefix("measure:") {
                    Target::Measure(measure(key)?.key)
                } else {
                    Target::Custom(other.strip_prefix("custom:")?.to_owned())
                }
            }
        })
    }

    /// The name shown in the mapping dropdown.
    pub fn label(&self) -> String {
        match self {
            Target::Ignore => "Ignore this column".into(),
            Target::Manufacturer => "Manufacturer".into(),
            Target::OrderNumber => "Order number".into(),
            Target::TypeNumber => "Type number".into(),
            Target::Description => "Description".into(),
            Target::LetterCodes => "Letter codes (K, Q ...)".into(),
            Target::Keywords => "Search keywords".into(),
            Target::Function => "Function (contactor, terminal ...)".into(),
            Target::Width => "Width (mm)".into(),
            Target::Height => "Height (mm)".into(),
            Target::Depth => "Depth (mm)".into(),
            Target::Measure(key) => {
                let m = measure(key);
                format!(
                    "{} ({})",
                    m.map_or(*key, |m| m.label),
                    stored_unit(m.map_or(Dim::Count, |m| m.dim))
                )
                .replace(" ()", "")
            }
            Target::Etim => "ETIM class".into(),
            Target::Eclass => "ECLASS code".into(),
            Target::Price => "List price".into(),
            Target::PriceDate => "Price list date".into(),
            Target::PricePer => "Price is per (pieces)".into(),
            Target::Lifecycle => "Lifecycle status".into(),
            Target::Connection => "Connection method".into(),
            Target::ConnectionCount => "Number of connections".into(),
            Target::WireMin => "Smallest conductor (mm²)".into(),
            Target::WireMax => "Largest conductor (mm²)".into(),
            Target::WireRange => "Conductor range (mm²)".into(),
            Target::Custom(key) => format!("Other data: {key}"),
        }
    }

    /// The dimension a numeric column is converted in, if it is numeric.
    pub fn dim(&self) -> Option<Dim> {
        match self {
            Target::Width | Target::Height | Target::Depth => Some(Dim::Length),
            Target::Measure(key) => measure(key).map(|m| m.dim),
            Target::WireMin | Target::WireMax | Target::WireRange => Some(Dim::Area),
            Target::ConnectionCount => Some(Dim::Count),
            _ => None,
        }
    }

    /// Fields that may be filled by one column only.
    fn single(&self) -> bool {
        !matches!(self, Target::Ignore | Target::Custom(_))
    }

    /// The choices of the dropdown, in the order they are listed. `Custom`
    /// is offered separately with a name box.
    pub fn choices() -> Vec<Target> {
        let mut all = vec![
            Target::Ignore,
            Target::Manufacturer,
            Target::OrderNumber,
            Target::TypeNumber,
            Target::Description,
            Target::LetterCodes,
            Target::Keywords,
            Target::Function,
            Target::Width,
            Target::Height,
            Target::Depth,
        ];
        all.extend(MEASURES.iter().map(|m| Target::Measure(m.key)));
        all.extend([
            Target::Etim,
            Target::Eclass,
            Target::Price,
            Target::PriceDate,
            Target::PricePer,
            Target::Lifecycle,
            Target::Connection,
            Target::ConnectionCount,
            Target::WireMin,
            Target::WireMax,
            Target::WireRange,
        ]);
        all
    }
}

impl Serialize for Target {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.id())
    }
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let id = String::deserialize(d)?;
        Ok(Target::from_id(&id).unwrap_or(Target::Ignore))
    }
}

/// The unit a dimension is stored in, for labels.
pub fn stored_unit(dim: Dim) -> &'static str {
    match dim {
        Dim::Current => "A",
        Dim::CurrentKa => "kA",
        Dim::Voltage => "V",
        Dim::PowerW => "W",
        Dim::PowerKw => "kW",
        Dim::Length => "mm",
        Dim::Area => "mm²",
        Dim::Temperature => "°C",
        Dim::ResistancePerKm => "Ω/km",
        Dim::Count => "",
    }
}

/// One column of the file and what it becomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnMap {
    /// The header as written in the file.
    pub header: String,
    pub target: Target,
    /// The unit the cells are in when a cell names none (`mA`, `kW`).
    /// Empty: the target's stored unit is assumed, and the preview says so.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
    /// Why the suggestion was made. Not saved.
    #[serde(skip)]
    pub why: String,
}

/// A complete mapping for one sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    /// Index of the header row in the sheet (0-based).
    pub header_row: usize,
    pub columns: Vec<ColumnMap>,
    /// Maker for rows with no manufacturer column or an empty cell.
    #[serde(default)]
    pub default_manufacturer: String,
    /// Currency of a price cell that names none.
    #[serde(default = "default_currency")]
    pub currency: String,
    /// What the file is, recorded on every imported value. `Unknown` means
    /// the person did not say, and values stay "source not stated".
    #[serde(default)]
    pub origin: DataSource,
    /// What happens when two rows have the same maker and order number.
    #[serde(default)]
    pub duplicates: DuplicateRule,
}

/// What to do with rows that have the same maker and order number. The
/// catalog holds one entry per maker and order number, so two entries cannot
/// both be kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateRule {
    /// The row lowest in the file is used (the order of an update list).
    #[default]
    KeepLast,
    /// The row highest in the file is used.
    KeepFirst,
    /// The first row is used and later rows fill the fields it leaves empty.
    Merge,
}

impl DuplicateRule {
    pub const ALL: [DuplicateRule; 3] = [
        DuplicateRule::KeepLast,
        DuplicateRule::KeepFirst,
        DuplicateRule::Merge,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DuplicateRule::KeepLast => "Keep the last row",
            DuplicateRule::KeepFirst => "Keep the first row",
            DuplicateRule::Merge => "Keep the first row and fill its gaps from the others",
        }
    }
}

fn default_currency() -> String {
    "EUR".into()
}

impl Mapping {
    /// The column mapped to `target`, if any.
    pub fn column_of(&self, target: &Target) -> Option<usize> {
        self.columns.iter().position(|c| &c.target == target)
    }

    /// Things that stop the import, in words. Empty when it can run.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.column_of(&Target::OrderNumber).is_none() {
            out.push("No column is the order number. Pick the column that holds the maker's article number.".into());
        }
        for (i, column) in self.columns.iter().enumerate() {
            if !column.target.single() {
                continue;
            }
            if let Some(j) = self.columns[..i]
                .iter()
                .position(|c| c.target == column.target)
            {
                out.push(format!(
                    "Columns \"{}\" and \"{}\" are both mapped to {}. Ignore one of them.",
                    self.columns[j].header,
                    column.header,
                    column.target.label()
                ));
            }
        }
        for column in &self.columns {
            if let Some(dim) = column.target.dim()
                && !column.unit.is_empty()
                && dim
                    .factor(&column.unit.to_lowercase().replace('²', "2"))
                    .is_none()
            {
                out.push(format!(
                    "Column \"{}\": the unit {} does not fit {}.",
                    column.header,
                    column.unit,
                    column.target.label()
                ));
            }
        }
        out
    }

    /// Whether the maker is known for every row: a column, or a default.
    pub fn maker_known(&self) -> bool {
        self.column_of(&Target::Manufacturer).is_some()
            || !self.default_manufacturer.trim().is_empty()
    }

    /// Proposes a mapping for `sheet`, reading `header_row` as the headers.
    pub fn suggest(sheet: &Sheet, header_row: usize) -> Mapping {
        let headers = sheet.rows.get(header_row).cloned().unwrap_or_default();
        let body = sheet.rows.get(header_row + 1..).unwrap_or_default();
        let width = headers
            .len()
            .max(body.iter().take(200).map(Vec::len).max().unwrap_or(0));
        let mut columns: Vec<ColumnMap> = (0..width)
            .map(|i| {
                let header = headers.get(i).cloned().unwrap_or_default();
                let samples: Vec<&str> = body
                    .iter()
                    .take(200)
                    .filter_map(|r| r.get(i))
                    .map(String::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                suggest_column(&header, &samples)
            })
            .collect();
        // Two columns that want the same field: the first keeps it, the
        // later one is offered as plain data so nothing silently overwrites.
        for i in 0..columns.len() {
            if !columns[i].target.single() {
                continue;
            }
            if columns[..i].iter().any(|c| c.target == columns[i].target) {
                let key = property_key(&columns[i].header).unwrap_or_else(|| "column".into());
                columns[i].why = format!(
                    "{} already has a column, so this one is kept as data",
                    columns[i].target.label()
                );
                columns[i].target = Target::Custom(key);
            }
        }
        Mapping {
            header_row,
            columns,
            default_manufacturer: String::new(),
            currency: default_currency(),
            origin: DataSource::Unknown,
            duplicates: DuplicateRule::default(),
        }
    }

    /// A fingerprint of the header row, so a saved mapping is offered again
    /// for a file with the same columns.
    pub fn signature(headers: &[String]) -> String {
        let mut hasher = Sha256::new();
        for h in headers {
            hasher.update(h.trim().to_lowercase().as_bytes());
            hasher.update([0]);
        }
        hasher
            .finalize()
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// The first of the first 30 rows that looks like a header: nearly as wide
/// as the widest row, with almost no plain numbers in it. A title line above
/// the table is narrower (even when a `;` in the title splits it in two) and
/// is skipped.
pub fn guess_header_row(sheet: &Sheet) -> usize {
    let filled = |row: &Vec<String>| row.iter().filter(|c| !c.trim().is_empty()).count();
    let widest = sheet.rows.iter().take(30).map(filled).max().unwrap_or(0);
    let wanted = (widest * 3).div_ceil(5).max(2);
    for (i, row) in sheet.rows.iter().take(30).enumerate() {
        let n = filled(row);
        let numeric = row
            .iter()
            .filter(|c| !c.trim().is_empty())
            .filter(|c| c.trim().replace(',', ".").parse::<f64>().is_ok())
            .count();
        if n >= wanted && numeric * 4 <= n {
            return i;
        }
    }
    0
}

fn suggest_column(header: &str, samples: &[&str]) -> ColumnMap {
    let (name, unit) = split_header(header);
    let squashed = squash(&name);
    let make = |target: Target, why: String, unit: String| ColumnMap {
        header: header.to_owned(),
        target,
        unit,
        why,
    };
    let unit_for = |t: &Target| -> (String, String) {
        match (t.dim(), unit.as_str()) {
            (Some(dim), u) if !u.is_empty() && dim.factor(u).is_some() => (
                display_unit(&unit),
                format!(", unit {} from the header", display_unit(&unit)),
            ),
            (Some(dim), u) if !u.is_empty() => (
                String::new(),
                format!(
                    ", the header unit {u} does not fit, {} assumed",
                    stored_unit(dim)
                ),
            ),
            (Some(dim), _) => {
                let u = stored_unit(dim);
                if u.is_empty() {
                    (String::new(), String::new())
                } else {
                    (
                        String::new(),
                        format!(
                            ", no unit in the header, {u} assumed unless a cell says otherwise"
                        ),
                    )
                }
            }
            _ => (String::new(), String::new()),
        }
    };

    if squashed.is_empty() {
        if samples.is_empty() {
            return make(Target::Ignore, "empty column".into(), String::new());
        }
        return make(Target::Ignore, "no header".into(), String::new());
    }
    if matches!(
        squashed.as_str(),
        "nr" | "no"
            | "pos"
            | "position"
            | "lfdnr"
            | "lfd"
            | "idx"
            | "index"
            | "row"
            | "rownumber"
            | "nummer"
    ) {
        return make(
            Target::Ignore,
            "looks like a row number".into(),
            String::new(),
        );
    }
    // Exact measure words first: `Power loss` must not become `Power`.
    for m in MEASURES {
        if m.words.contains(&squashed.as_str()) {
            let t = Target::Measure(m.key);
            let (u, note) = unit_for(&t);
            return make(
                t,
                format!(
                    "header \"{}\" names {}{note}",
                    name.trim(),
                    m.label.to_lowercase()
                ),
                u,
            );
        }
    }
    let by_words: &[(&[&str], Target)] = &[
        (
            &[
                "etim",
                "etimclass",
                "etimklasse",
                "etimclassid",
                "etimcode",
                "etimgroup",
            ],
            Target::Etim,
        ),
        (
            &[
                "eclass",
                "eclasscode",
                "eclassclass",
                "ecl",
                "eclass11",
                "eclass12",
                "eclass13",
            ],
            Target::Eclass,
        ),
        (
            &[
                "price",
                "listprice",
                "preis",
                "listenpreis",
                "prijs",
                "brutoprijs",
                "bruttopreis",
                "netprice",
                "unitprice",
                "eur",
                "cost",
                "grossprice",
                "catalogprice",
                "catalogueprice",
                "retailprice",
                "msrp",
                "katalogpreis",
                "verkaufspreis",
                "einzelpreis",
                "listprijs",
                "adviesprijs",
                "catalogusprijs",
                "stuksprijs",
                "prijsperstuk",
                "prijsexclbtw",
            ],
            Target::Price,
        ),
        (
            &[
                "pricedate",
                "pricelistdate",
                "datepricelist",
                "priceeffectivedate",
                "pricevalidfrom",
                "validfrom",
                "validfromdate",
                "validsince",
                "effectivedate",
                "effectivefrom",
                "preisdatum",
                "preisstand",
                "preislistendatum",
                "preislistenstand",
                "gultigab",
                "gultigvon",
                "gultigkeitsdatum",
                "gultigabdatum",
                "prijsdatum",
                "prijsstand",
                "prijslijstdatum",
                "geldigvanaf",
                "geldigvan",
                "ingangsdatum",
            ],
            Target::PriceDate,
        ),
        (
            &[
                "per",
                "priceper",
                "pricefor",
                "priceunit",
                "pricebasis",
                "pricequantity",
                "priceqty",
                "perquantity",
                "preiseinheit",
                "preisbasis",
                "preismenge",
                "preisproe",
                "bezugsmenge",
                "pe",
                "prijseenheid",
                "prijsper",
                "prijsbasis",
                "prijshoeveelheid",
            ],
            Target::PricePer,
        ),
        (
            &[
                "lifecycle",
                "status",
                "lifecyclestatus",
                "productstatus",
                "lebenszyklus",
                "artikelstatus",
                "levenscyclus",
                "lifecyclephase",
                "lifecyclestage",
                "productlifecycle",
                "produktstatus",
                "produktlebenszyklus",
                "availability",
                "verfugbarkeit",
                "beschikbaarheid",
                "leverstatus",
                "levensfase",
                "plmstatus",
            ],
            Target::Lifecycle,
        ),
        (
            &[
                "connection",
                "connectiontype",
                "connectionmethod",
                "connectiontechnology",
                "anschluss",
                "anschlussart",
                "anschlusstechnik",
                "aansluiting",
                "aansluittype",
                "connectionsystem",
                "connectingmethod",
                "terminaltype",
                "clampingtype",
                "clampingsystem",
                "anschlusssystem",
                "klemmenart",
                "klemmentechnik",
                "aansluitmethode",
                "aansluitwijze",
                "aansluitsysteem",
                "klemtechniek",
            ],
            Target::Connection,
        ),
        (
            &[
                "connections",
                "numberofconnections",
                "connectioncount",
                "anschlusse",
                "anschlusse",
                "anzahlanschlusse",
                "aantalaansluitingen",
                "terminalpoints",
                "numberofterminals",
                "terminalcount",
                "numberofterminalpoints",
                "anzahlklemmen",
                "klemmenanzahl",
                "aantalklemmen",
            ],
            Target::ConnectionCount,
        ),
        (
            &[
                "wiremin",
                "conductormin",
                "minconductor",
                "minwire",
                "minimumconductorsize",
                "leitermin",
                "minimaleleiter",
            ],
            Target::WireMin,
        ),
        (
            &[
                "wiremax",
                "conductormax",
                "maxconductor",
                "maxwire",
                "maximumconductorsize",
                "leitermax",
                "maximaleleiter",
            ],
            Target::WireMax,
        ),
        (
            &[
                "wirerange",
                "conductorrange",
                "conductorsize",
                "wiresize",
                "leiterquerschnittsbereich",
                "anschlussquerschnitt",
                "clampingrange",
                "klemmbereich",
                "aansluitbereik",
            ],
            Target::WireRange,
        ),
        (
            &[
                "function",
                "funktion",
                "functie",
                "category",
                "kategorie",
                "categorie",
                "devicetype",
                "gerateart",
                "productgroup",
                "productfamily",
                "produktgruppe",
                "produktfamilie",
                "warengruppe",
                "artikelgruppe",
                "productgroep",
                "productfamilie",
                "artikelgroep",
                "apparaatsoort",
            ],
            Target::Function,
        ),
    ];
    // `Price EUR`, `Preis USD`: a price word with the currency after it.
    let price_stem = [
        "euro", "eur", "usd", "gbp", "chf", "sek", "dkk", "nok", "pln",
    ]
    .iter()
    .find_map(|c| squashed.strip_suffix(c))
    .filter(|stem| !stem.is_empty());
    for (words, target) in by_words {
        if words.contains(&squashed.as_str())
            || (*target == Target::Price && price_stem.is_some_and(|stem| words.contains(&stem)))
        {
            let (u, note) = unit_for(target);
            return make(
                target.clone(),
                format!(
                    "header \"{}\" names {}{note}",
                    name.trim(),
                    target.label().to_lowercase()
                ),
                u,
            );
        }
    }
    // The shared header words of the catalog CSV importer.
    let base = match column_for(&name) {
        Column::Manufacturer => Some(Target::Manufacturer),
        Column::OrderNumber => Some(Target::OrderNumber),
        Column::TypeNumber => Some(Target::TypeNumber),
        Column::Description => Some(Target::Description),
        Column::LetterCodes => Some(Target::LetterCodes),
        Column::Keywords => Some(Target::Keywords),
        Column::Width => Some(Target::Width),
        Column::Height => Some(Target::Height),
        Column::Depth => Some(Target::Depth),
        _ => None,
    };
    if let Some(target) = base {
        let (u, note) = unit_for(&target);
        return make(
            target.clone(),
            format!(
                "header \"{}\" names {}{note}",
                name.trim(),
                target.label().to_lowercase()
            ),
            u,
        );
    }
    // Content: ETIM and ECLASS codes are recognisable by their shape.
    if !samples.is_empty() {
        let share =
            |f: fn(&str) -> bool| samples.iter().filter(|s| f(s)).count() * 5 >= samples.len() * 4;
        if share(is_etim_class) {
            return make(
                Target::Etim,
                "the values look like ETIM classes (EC000000)".into(),
                String::new(),
            );
        }
        if share(is_eclass_code) {
            return make(
                Target::Eclass,
                "the values look like ECLASS codes".into(),
                String::new(),
            );
        }
    }
    let key = property_key(header).unwrap_or_else(|| "column".into());
    make(
        Target::Custom(key.clone()),
        format!("not recognised, kept as data under \"{key}\""),
        String::new(),
    )
}

/// `EC000897`.
pub(crate) fn is_etim_class(text: &str) -> bool {
    let t = text.trim();
    t.len() == 8 && t[..2].eq_ignore_ascii_case("EC") && t[2..].bytes().all(|b| b.is_ascii_digit())
}

/// `27-14-11-01`, `27141101`.
pub(crate) fn is_eclass_code(text: &str) -> bool {
    let t = text.trim();
    let digits: String = t.chars().filter(|c| *c != '-' && *c != ' ').collect();
    digits.len() == 8
        && digits.bytes().all(|b| b.is_ascii_digit())
        && t.chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == ' ')
}

fn squash(text: &str) -> String {
    crate::csv::squash_header(text)
}

/// A unit as people write it: `ma` is `mA`, `mm2` is `mm²`.
fn display_unit(unit: &str) -> String {
    match unit {
        "a" => "A",
        "ma" => "mA",
        "ka" => "kA",
        "v" => "V",
        "mv" => "mV",
        "kv" => "kV",
        "w" => "W",
        "kw" => "kW",
        "mw" => "mW",
        "ps" => "PS",
        "pk" => "pk",
        "btu/h" => "BTU/h",
        "mm2" => "mm²",
        "cm2" => "cm²",
        "c" | "°c" => "°C",
        other => other,
    }
    .to_owned()
}

/// Units a header may carry in brackets or after the name.
const BRACKET_UNITS: &[&str] = &[
    "a", "ma", "ka", "v", "mv", "kv", "w", "kw", "mw", "hp", "ps", "pk", "btu/h", "mm", "cm", "m",
    "in", "ft", "mm2", "mm²", "cm2", "°c", "c", "ω/km", "ohm/km",
];

/// Units recognised after the name without brackets (`Power loss W`). Short
/// ambiguous ones (`m`, `c`, `in`) are left out: `Type m` is not a unit.
const TAIL_UNITS: &[&str] = &[
    "a", "ma", "ka", "v", "kv", "w", "kw", "mw", "ps", "mm", "cm", "mm2", "mm²",
];

/// Splits `Rated current [A]`, `Nennstrom (A)`, `Verlustleistung in W` and
/// `Power loss W` into the name and the unit (lower case, `²` kept as `2`).
/// The unit is empty when the header names none.
pub fn split_header(header: &str) -> (String, String) {
    let h = header.trim();
    for (open, close) in [('[', ']'), ('(', ')')] {
        if h.ends_with(close)
            && let Some(at) = h.rfind(open)
        {
            let inner = h[at + 1..h.len() - 1]
                .trim()
                .to_lowercase()
                .replace('²', "2");
            if BRACKET_UNITS.contains(&inner.as_str()) {
                return (h[..at].trim().to_owned(), inner);
            }
        }
    }
    let tokens: Vec<&str> = h.split_whitespace().collect();
    if tokens.len() >= 2 {
        let last = tokens[tokens.len() - 1].to_lowercase().replace('²', "2");
        if TAIL_UNITS.contains(&last.as_str()) {
            let mut name: Vec<&str> = tokens[..tokens.len() - 1].to_vec();
            if matches!(
                name.last().map(|w| w.to_lowercase()).as_deref(),
                Some("in" | "/" | "-" | "[" | "in:" | "(")
            ) {
                name.pop();
            }
            if !name.is_empty() {
                return (name.join(" "), last);
            }
        }
    }
    (h.to_owned(), String::new())
}

/// Mappings the person has corrected, by header signature, kept in the
/// user's profile so the next file with the same columns needs no work.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SavedMappings {
    #[serde(default)]
    by_signature: BTreeMap<String, Mapping>,
}

impl SavedMappings {
    /// Reads the file; a missing or unreadable file is an empty store (the
    /// saved mappings are a convenience, never a reason to fail an import).
    pub fn load(path: &Path) -> SavedMappings {
        fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        fs::write(path, text)
    }

    /// The saved mapping for a file with these headers.
    pub fn find(&self, headers: &[String]) -> Option<Mapping> {
        let mut mapping = self.by_signature.get(&Mapping::signature(headers))?.clone();
        // The explanations are not saved: say where the mapping came from.
        for column in &mut mapping.columns {
            column.why = "from your saved mapping for this layout".into();
        }
        Some(mapping)
    }

    pub fn remember(&mut self, headers: &[String], mapping: &Mapping) {
        self.by_signature
            .insert(Mapping::signature(headers), mapping.clone());
    }
}

/// Where the saved mappings live, next to the user's catalog.
pub fn saved_mappings_path() -> Option<std::path::PathBuf> {
    crate::catalog::user_catalog_path().map(|p| p.with_file_name("parts-table-mappings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_in_brackets_and_after_the_name_are_split_off() {
        assert_eq!(
            split_header("Rated current [A]"),
            ("Rated current".into(), "a".into())
        );
        assert_eq!(
            split_header("Nennstrom (mA)"),
            ("Nennstrom".into(), "ma".into())
        );
        assert_eq!(
            split_header("Verlustleistung in W"),
            ("Verlustleistung".into(), "w".into())
        );
        assert_eq!(
            split_header("Power loss W"),
            ("Power loss".into(), "w".into())
        );
        assert_eq!(split_header("Width (mm)"), ("Width".into(), "mm".into()));
        assert_eq!(
            split_header("Cross-section [mm²]"),
            ("Cross-section".into(), "mm2".into())
        );
        // Not units.
        assert_eq!(split_header("Type m"), ("Type m".into(), String::new()));
        assert_eq!(
            split_header("Colour (grey)"),
            ("Colour (grey)".into(), String::new())
        );
    }

    #[test]
    fn target_ids_round_trip() {
        for t in Target::choices()
            .into_iter()
            .chain([Target::Custom("weight_kg".into())])
        {
            assert_eq!(Target::from_id(&t.id()), Some(t.clone()), "{t:?}");
        }
        assert_eq!(Target::from_id("nonsense"), None);
    }

    #[test]
    fn etim_and_eclass_codes_are_recognised_by_shape() {
        assert!(is_etim_class("EC000897"));
        assert!(!is_etim_class("EC00089"));
        assert!(is_eclass_code("27-14-11-01"));
        assert!(is_eclass_code("27141101"));
        assert!(!is_eclass_code("12345"));
    }
}
