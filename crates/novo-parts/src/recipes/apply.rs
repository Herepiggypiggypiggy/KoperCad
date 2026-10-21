//! The reading half of a recipe: feature names and values from a BMEcat or
//! CSV file become structured catalog parts.
//!
//! Rules match on the words of the feature name (`Rated current (IN)` is
//! `rated current in`), in English, German and Dutch. A value is converted to
//! the stored unit only when its unit is known and belongs to what the rule
//! measures; anything else is left where it was and listed in the report,
//! never guessed.

use std::collections::BTreeMap;

use novo_core::Value;

use super::quantity::{Dim, value_quantity};
use super::{FunctionHint, MakerRecipe};
use crate::bmecat::{ECLASS_CLASS_KEY, ETIM_CLASS_KEY};
use crate::catalog::{CatalogPart, Dimensions, keys};
use crate::schema::{
    ConnectionKind, DataSource, Lifecycle, PartRef, PartTerminal, Price, Provenance, Side, eng,
};

/// `data` keys this reader writes beyond [`eng`] and [`keys`].
pub mod local_keys {
    pub const RATED_VOLTAGE_V: &str = "rated_voltage_v";
    pub const OUTPUT_CURRENT_A: &str = "output_current_a";
    pub const OUTPUT_VOLTAGE_V: &str = "output_voltage_v";
    /// Smallest and largest conductor a terminal takes, in mm².
    pub const WIRE_MIN_MM2: &str = "wire_min_mm2";
    pub const WIRE_MAX_MM2: &str = "wire_max_mm2";
    pub const CONNECTION_COUNT: &str = "connection_count";
}
use local_keys as lk;

/// Where the file came from, for the part's provenance.
#[derive(Debug, Clone, Copy)]
pub struct ApplyContext<'a> {
    /// `import:bmecat` or `import:csv`.
    pub source: &'a str,
    pub file: &'a str,
    /// Import date, `2026-10-15`.
    pub imported: &'a str,
}

/// One value the reader mapped.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapped {
    pub order_number: String,
    /// The feature as the file named it (the `data` key).
    pub feature: String,
    /// What it became, such as `rated_current_a = 24`.
    pub became: String,
}

/// One value the reader recognised but did not map, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Skipped {
    pub order_number: String,
    pub feature: String,
    pub reason: String,
}

/// What the reader did with a file's parts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RecipeReport {
    pub parts: usize,
    pub mapped: Vec<Mapped>,
    pub skipped: Vec<Skipped>,
    /// Features no rule knows, with the number of parts that had them. They
    /// stay in the part's data unchanged.
    pub unmapped: BTreeMap<String, usize>,
    /// Things for the user to check (order numbers that look wrong, terminal
    /// names that were numbered).
    pub warnings: Vec<String>,
}

impl RecipeReport {
    /// Adds another file's report to this one.
    pub fn absorb(&mut self, other: RecipeReport) {
        self.parts += other.parts;
        self.mapped.extend(other.mapped);
        self.skipped.extend(other.skipped);
        for (k, n) in other.unmapped {
            *self.unmapped.entry(k).or_default() += n;
        }
        for w in other.warnings {
            if !self.warnings.contains(&w) {
                self.warnings.push(w);
            }
        }
    }

    /// Lines for the import summary.
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.parts == 0 {
            return lines;
        }
        lines.push(format!(
            "Read {} value(s) into structured part data for {} part(s), marked as from the maker's data.",
            self.mapped.len(),
            self.parts
        ));
        if !self.skipped.is_empty() {
            let mut why: BTreeMap<&str, usize> = BTreeMap::new();
            for s in &self.skipped {
                *why.entry(s.reason.as_str()).or_default() += 1;
            }
            let list: Vec<String> = why.iter().map(|(r, n)| format!("{n} {r}")).collect();
            lines.push(format!("Left as written: {}.", list.join("; ")));
        }
        if !self.unmapped.is_empty() {
            lines.push(format!(
                "{} other feature(s) kept as properties without special meaning.",
                self.unmapped.len()
            ));
        }
        lines.extend(self.warnings.iter().cloned());
        lines
    }
}

#[derive(Clone, Copy)]
enum Target {
    /// A number in the stored unit of `dim`, under `key`.
    Num(&'static str, Dim),
    /// A range whose low and high end go to two keys.
    Range(&'static str, &'static str, Dim),
    /// Text kept under `key` as the maker wrote it.
    Text(&'static str),
    /// Conductor range of a terminal (a cable's section for cable makers).
    WireRange,
    WireMin,
    WireMax,
    Size(usize),
    Connection,
    ConnectionCount,
    Lifecycle,
    Successor,
    Price,
    Currency,
    PriceUnit,
    PriceDate,
    /// Recognised, but not stored, for the reason given.
    Skip(&'static str),
    /// Cable data: only read for cables.
    CableSection,
    CableCores,
    CableOuterDiameter,
    CableMaxTemp,
    CableResistance,
}

struct Rule {
    target: Target,
    any: &'static [&'static str],
    also: &'static [&'static str],
    not: &'static [&'static str],
}

const fn rule(
    target: Target,
    any: &'static [&'static str],
    also: &'static [&'static str],
    not: &'static [&'static str],
) -> Rule {
    Rule {
        target,
        any,
        also,
        not,
    }
}

/// First match wins, so specific rules come first.
const RULES: &[Rule] = &[
    rule(
        Target::Skip("power loss given per pole or conductor, not for the whole part"),
        &[
            "power loss",
            "power dissipation",
            "dissipated power",
            "verlustleistung",
            "dissipation",
        ],
        &[
            "per conductor",
            "per pole",
            "per phase",
            "per terminal",
            "per contact",
            "pro leiter",
            "pro pol",
        ],
        &[],
    ),
    rule(
        Target::Num(eng::POWER_LOSS_W, Dim::PowerW),
        &[
            "power loss",
            "power dissipation",
            "dissipated power",
            "verlustleistung",
            "dissipation",
        ],
        &[],
        &[],
    ),
    rule(
        Target::Num(eng::RATED_POWER_KW, Dim::PowerKw),
        &["ac 3", "ac3"],
        &["power", "kw", "leistung", "vermögen", "vermoegen"],
        &["loss", "verlust", "dissipation"],
    ),
    rule(
        Target::Num(eng::AC3_CURRENT_A, Dim::Current),
        &["ac 3", "ac3"],
        &["current", "ie", "strom", "stroom"],
        &["power", "kw", "leistung", "loss", "verlust", "dissipation"],
    ),
    rule(
        Target::Num(eng::AC1_CURRENT_A, Dim::Current),
        &["ac 1", "ac1"],
        &["current", "ie", "strom", "stroom"],
        &["power", "kw", "leistung", "loss", "verlust", "dissipation"],
    ),
    rule(
        Target::Num(eng::COIL_VOLTAGE_V, Dim::Voltage),
        &[
            "coil voltage",
            "control voltage",
            "control circuit voltage",
            "control supply voltage",
            "actuating voltage",
            "spulenspannung",
            "steuerspannung",
            "stuurspanning",
        ],
        &[],
        &["insulation"],
    ),
    rule(
        Target::Num(eng::ICU_KA, Dim::CurrentKa),
        &[
            "icu",
            "ultimate breaking capacity",
            "ultimate short circuit breaking capacity",
        ],
        &[],
        &[],
    ),
    rule(
        Target::Num(eng::ICS_KA, Dim::CurrentKa),
        &[
            "ics",
            "service breaking capacity",
            "service short circuit breaking capacity",
        ],
        &[],
        &[],
    ),
    rule(
        Target::Num(eng::SCCR_KA, Dim::CurrentKa),
        &["sccr", "short circuit current rating"],
        &[],
        &[],
    ),
    rule(
        Target::Range(eng::SETTING_MIN_A, eng::SETTING_MAX_A, Dim::Current),
        &[
            "setting range",
            "adjustment range",
            "adjustable response",
            "response value",
            "einstellbereich",
            "instelbereik",
        ],
        &["current", "strom", "stroom"],
        &[],
    ),
    rule(
        Target::Text(eng::TRIP_CLASS),
        &[
            "trip class",
            "tripping class",
            "auslöseklasse",
            "ausloeseklasse",
        ],
        &[],
        &[],
    ),
    rule(
        Target::Text(eng::TRIP_CURVE),
        &[
            "tripping characteristic",
            "trip characteristic",
            "trip curve",
            "tripping curve",
            "auslösecharakteristik",
            "ausloesecharakteristik",
        ],
        &[],
        &[],
    ),
    rule(
        Target::Num(eng::POLES, Dim::Count),
        &[
            "number of poles",
            "poles",
            "polzahl",
            "anzahl der pole",
            "aantal polen",
            "pole number",
        ],
        &[],
        &["pitch"],
    ),
    rule(
        Target::Num(lk::OUTPUT_CURRENT_A, Dim::Current),
        &["output current", "ausgangsstrom", "uitgangsstroom"],
        &[],
        &[],
    ),
    rule(
        Target::Num(lk::OUTPUT_VOLTAGE_V, Dim::Voltage),
        &["output voltage", "ausgangsspannung", "uitgangsspanning"],
        &[],
        &[],
    ),
    rule(
        Target::Num(eng::RATED_CURRENT_A, Dim::Current),
        &[
            "rated current",
            "nominal current",
            "nennstrom",
            "bemessungsstrom",
            "current rating",
            "nominale stroom",
            "rated operational current",
            "rated operating current",
            "operating current",
        ],
        &[],
        &[
            "ac 3",
            "ac3",
            "ac 1",
            "ac1",
            "residual",
            "leakage",
            "short",
            "breaking",
            "trip",
            "setting",
            "conditional",
            "fault",
            "differential",
            "output",
            "kurzschluss",
        ],
    ),
    rule(
        Target::Num(lk::RATED_VOLTAGE_V, Dim::Voltage),
        &[
            "rated voltage",
            "nominal voltage",
            "nennspannung",
            "bemessungsspannung",
            "operating voltage",
            "nominale spanning",
            "rated operational voltage",
        ],
        &[],
        &[
            "coil",
            "control",
            "insulation",
            "impulse",
            "supply",
            "output",
            "input",
            "spulen",
            "steuer",
            "isolation",
        ],
    ),
    rule(
        Target::WireMin,
        &CROSS_SECTION,
        &["min", "minimum"],
        &["max", "maximum", "awg", "kcmil", "outer"],
    ),
    rule(
        Target::WireMax,
        &CROSS_SECTION,
        &["max", "maximum"],
        &["min", "minimum", "awg", "kcmil", "outer"],
    ),
    rule(
        Target::WireRange,
        &CROSS_SECTION,
        &[],
        &["awg", "kcmil", "outer", "min", "max", "minimum", "maximum"],
    ),
    rule(
        Target::Connection,
        &[
            "connection method",
            "connection type",
            "connection technology",
            "type of connection",
            "anschlussart",
            "anschlusstechnik",
            "anschlusstechnologie",
            "aansluitwijze",
            "aansluittype",
        ],
        &[],
        &[],
    ),
    rule(
        Target::ConnectionCount,
        &[
            "number of connections",
            "number of conductor connections",
            "number of terminal points",
            "number of connection points",
            "anzahl der anschlüsse",
            "anzahl anschlüsse",
            "anzahl der anschluesse",
            "aantal aansluitingen",
        ],
        &[],
        &[],
    ),
    rule(
        Target::Lifecycle,
        &[
            "product status",
            "lifecycle",
            "life cycle",
            "produktstatus",
            "lebenszyklus",
            "article status",
            "artikelstatus",
            "commercial status",
        ],
        &[],
        &[],
    ),
    rule(
        Target::Successor,
        &[
            "successor",
            "replacement product",
            "replaced by",
            "nachfolger",
            "vervanger",
        ],
        &[],
        &[],
    ),
    rule(
        Target::PriceUnit,
        &[
            "price unit",
            "price per",
            "preiseinheit",
            "price quantity",
            "prijseenheid",
            "price basis",
        ],
        &[],
        &[],
    ),
    rule(
        Target::PriceDate,
        &["price date", "price valid from", "preisdatum", "prijsdatum"],
        &[],
        &[],
    ),
    rule(
        Target::Currency,
        &["currency", "währung", "waehrung", "valuta"],
        &[],
        &[],
    ),
    rule(
        Target::Price,
        &[
            "list price",
            "price",
            "listenpreis",
            "bruttopreis",
            "preis",
            "prijs",
            "catalog price",
            "catalogue price",
        ],
        &[],
        &["unit", "per", "quantity", "basis", "date", "group", "code"],
    ),
    rule(
        Target::Size(0),
        &["width", "breite", "breedte"],
        &[],
        &[
            "wire",
            "conductor",
            "cable",
            "pitch",
            "gap",
            "slot",
            "strip",
        ],
    ),
    rule(
        Target::Size(1),
        &["height", "höhe", "hoehe", "hoogte"],
        &[],
        &[
            "wire",
            "conductor",
            "cable",
            "pitch",
            "gap",
            "slot",
            "strip",
        ],
    ),
    rule(
        Target::Size(2),
        &["depth", "tiefe", "diepte"],
        &[],
        &[
            "wire",
            "conductor",
            "cable",
            "pitch",
            "gap",
            "slot",
            "strip",
        ],
    ),
    rule(
        Target::CableOuterDiameter,
        &[
            "outer diameter",
            "outside diameter",
            "overall diameter",
            "außendurchmesser",
            "aussendurchmesser",
            "buitendiameter",
        ],
        &[],
        &[],
    ),
    rule(
        Target::CableCores,
        &[
            "number of cores",
            "number of conductors",
            "core count",
            "adernzahl",
            "anzahl der adern",
            "aantal aders",
        ],
        &[],
        &[],
    ),
    rule(
        Target::CableMaxTemp,
        &[
            "max operating temperature",
            "maximum operating temperature",
            "max conductor temperature",
            "maximum conductor temperature",
            "temperature range",
        ],
        &[],
        &["storage", "ambient", "transport"],
    ),
    rule(
        Target::CableResistance,
        &[
            "conductor resistance",
            "dc resistance",
            "resistance at 20",
            "leiterwiderstand",
        ],
        &[],
        &[],
    ),
    rule(Target::CableSection, &CROSS_SECTION, &[], &["awg", "kcmil"]),
];

const CROSS_SECTION: [&str; 11] = [
    "conductor cross section",
    "wire cross section",
    "cross section",
    "cross sectional area",
    "connectable conductor cross section",
    "clamping range",
    "wire size",
    "conductor size",
    "anschlussquerschnitt",
    "leiterquerschnitt",
    "dwarsdoorsnede",
];

/// Words that, last in a feature name, are its unit (`Width (mm)`).
const NAME_UNITS: [&str; 14] = [
    "mm", "cm", "m", "a", "ma", "ka", "v", "kv", "w", "kw", "mm2", "°c", "c", "ω",
];

fn sentence(key: &str) -> (String, Option<&'static str>) {
    let words: Vec<String> = key
        .split(|c: char| !c.is_alphanumeric() && c != '°')
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let unit = words
        .last()
        .and_then(|w| NAME_UNITS.iter().find(|u| **u == w.as_str()).copied());
    (format!(" {} ", words.join(" ")), unit)
}

fn has(sentence: &str, phrase: &str) -> bool {
    sentence.contains(&format!(" {phrase} "))
}

fn find_rule(sentence: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| {
        r.any.iter().any(|p| has(sentence, p))
            && (r.also.is_empty() || r.also.iter().any(|p| has(sentence, p)))
            && !r.not.iter().any(|p| has(sentence, p))
    })
}

#[derive(Default)]
struct Draft {
    wire_min: Option<f64>,
    wire_max: Option<f64>,
    size: [Option<f64>; 3],
    connection: Option<ConnectionKind>,
    connection_count: Option<usize>,
    price: Option<f64>,
    currency: Option<String>,
    per: Option<u32>,
    price_date: Option<String>,
}

pub(crate) fn connection_kind(text: &str) -> ConnectionKind {
    let t = text.to_lowercase();
    let any = |ws: &[&str]| ws.iter().any(|w| t.contains(w));
    if any(&["push-in", "push in", "pushin", "push‑in"]) {
        ConnectionKind::PushIn
    } else if any(&["screwless", "schraublos"]) {
        ConnectionKind::Screwless
    } else if any(&[
        "spring",
        "feder",
        "cage clamp",
        "tension clamp",
        "veer",
        "tension-clamp",
    ]) {
        ConnectionKind::Spring
    } else if any(&["faston", "flat plug", "flachsteck"]) {
        ConnectionKind::Faston
    } else if any(&["solder", "löt", "loet"]) {
        ConnectionKind::SolderPin
    } else if any(&["ring lug", "ring terminal", "fork", "kabelschuh", "lug"]) {
        ConnectionKind::Lug
    } else if any(&["screw", "schraub", "schroef"]) {
        ConnectionKind::Screw
    } else if any(&["plug", "pin", "stecker"]) {
        ConnectionKind::Pin
    } else {
        ConnectionKind::Unknown
    }
}

pub(crate) fn lifecycle_of(text: &str) -> Lifecycle {
    let t = text.trim().to_lowercase();
    let any = |ws: &[&str]| ws.iter().any(|w| t.contains(w));
    if any(&[
        "end of commercialisation",
        "end of commercialization",
        "phase out",
        "phase-out",
        "phasing out",
        "auslauf",
        "uitlopend",
        "end of life",
        "last time buy",
    ]) {
        Lifecycle::PhasingOut
    } else if any(&[
        "discontinued",
        "abgekündigt",
        "abgekuendigt",
        "obsolete",
        "withdrawn",
        "vervallen",
        "not available",
        "nicht mehr",
        "end of service",
    ]) {
        Lifecycle::Discontinued
    } else if any(&[
        "active",
        "aktiv",
        "in production",
        "available",
        "commercialised",
        "commercialized",
        "current",
        "actief",
        "serienprodukt",
    ]) || t == "a"
    {
        Lifecycle::Active
    } else {
        Lifecycle::Unknown
    }
}

pub(crate) fn currency_of(text: &str) -> Option<String> {
    const CODES: [&str; 12] = [
        "EUR", "USD", "GBP", "CHF", "SEK", "DKK", "NOK", "PLN", "CZK", "JPY", "CNY", "HUF",
    ];
    let upper = text.to_uppercase();
    for c in CODES {
        if upper.split(|ch: char| !ch.is_alphabetic()).any(|w| w == c) {
            return Some(c.to_owned());
        }
    }
    if text.contains('€') {
        Some("EUR".into())
    } else if text.contains('£') {
        Some("GBP".into())
    } else if text.contains('$') {
        Some("USD".into())
    } else {
        None
    }
}

fn text_of(v: &Value) -> String {
    match v {
        Value::Text(t) => t.trim().to_owned(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
    }
}

fn round(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

fn num_value(v: f64, dim: Dim) -> Value {
    if dim == Dim::Count && v.fract() == 0.0 {
        Value::Int(v as i64)
    } else {
        Value::Float(round(v))
    }
}

fn eclass_code(text: &str) -> Option<String> {
    let digits: String = text.chars().filter(char::is_ascii_digit).collect();
    (digits.len() == 8).then(|| {
        format!(
            "{}-{}-{}-{}",
            &digits[0..2],
            &digits[2..4],
            &digits[4..6],
            &digits[6..8]
        )
    })
}

fn etim_code(text: &str) -> Option<String> {
    let t = text.trim().to_uppercase();
    (t.len() == 8 && t.starts_with("EC") && t[2..].chars().all(|c| c.is_ascii_digit())).then_some(t)
}

impl MakerRecipe {
    /// Reads the structured data out of freshly imported parts. The raw
    /// features stay in `data`; the structured fields and the engineering
    /// keys are added, marked [`DataSource::MakerDatasheet`].
    pub fn apply(&self, parts: &mut [CatalogPart], ctx: &ApplyContext) -> RecipeReport {
        let mut report = RecipeReport {
            parts: parts.len(),
            ..Default::default()
        };
        let mut bad_orders = Vec::new();
        for part in parts.iter_mut() {
            self.canonical_manufacturer(part);
            if let Some(w) = self.check_order(&part.order_number)
                && !self.manufacturer.is_empty()
            {
                bad_orders.push((part.order_number.clone(), w));
            }
            self.apply_one(part, ctx, &mut report);
        }
        for (order, w) in bad_orders.iter().take(5) {
            report.warnings.push(format!("{order}: {w}"));
        }
        if bad_orders.len() > 5 {
            report.warnings.push(format!(
                "...and {} more order numbers to check.",
                bad_orders.len() - 5
            ));
        }
        report.warnings.dedup();
        report
    }

    fn canonical_manufacturer(&self, part: &mut CatalogPart) {
        if self.manufacturer.is_empty() {
            return;
        }
        let m = crate::search::normalize_code(&part.manufacturer);
        let names = std::iter::once(self.manufacturer).chain(self.aliases.iter().copied());
        if names
            .map(crate::search::normalize_code)
            .any(|n| !n.is_empty() && m.starts_with(&n))
        {
            part.manufacturer = self.manufacturer.to_owned();
        }
    }

    fn function_of(&self, part: &CatalogPart) -> Option<&'static FunctionHint> {
        let text = format!(
            "{} {} {}",
            part.description,
            part.keywords.join(" "),
            part.type_number
        )
        .to_lowercase();
        self.functions
            .iter()
            .find(|f| f.phrases.iter().any(|p| text.contains(p)))
    }

    fn apply_one(&self, part: &mut CatalogPart, ctx: &ApplyContext, report: &mut RecipeReport) {
        let order = part.order_number.clone();
        if part.function.is_empty()
            && let Some(hint) = self.function_of(part)
        {
            part.function = hint.function.to_owned();
            if part.letter_codes.is_empty() {
                part.letter_codes = hint.letters.iter().map(|s| (*s).to_owned()).collect();
            }
        }
        let is_cable =
            self.cable_maker || matches!(part.function.as_str(), "cable" | "cable_assembly");

        // Classification codes move out of the data bag into the structure.
        for (key, is_etim) in [(ETIM_CLASS_KEY, true), (ECLASS_CLASS_KEY, false)] {
            let Some(v) = part.data.get(key) else {
                continue;
            };
            let raw = text_of(v);
            let code = if is_etim {
                etim_code(&raw)
            } else {
                eclass_code(&raw)
            };
            match code {
                Some(c) => {
                    if is_etim {
                        part.class.etim = Some(c);
                    } else {
                        part.class.eclass = Some(c);
                    }
                    part.data.remove(key);
                }
                None => report.skipped.push(Skipped {
                    order_number: order.clone(),
                    feature: key.to_owned(),
                    reason: "classification code that is not in the usual form".into(),
                }),
            }
        }

        let mut draft = Draft::default();
        let features: Vec<(String, Value)> = part
            .data
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (feature, value) in features {
            let (words, name_unit) = sentence(&feature);
            let Some(rule) = find_rule(&words) else {
                if !is_known_key(&feature) {
                    *report.unmapped.entry(feature).or_default() += 1;
                }
                continue;
            };
            if matches!(
                rule.target,
                Target::CableSection
                    | Target::CableCores
                    | Target::CableOuterDiameter
                    | Target::CableMaxTemp
                    | Target::CableResistance
            ) && !is_cable
            {
                *report.unmapped.entry(feature).or_default() += 1;
                continue;
            }
            let mut q = value_quantity(&value);
            if let Some(q) = q.as_mut()
                && q.unit.is_empty()
                && let Some(u) = name_unit
            {
                q.unit = u.to_owned();
            }
            macro_rules! note {
                ($became:expr) => {
                    report.mapped.push(Mapped {
                        order_number: order.clone(),
                        feature: feature.clone(),
                        became: $became,
                    })
                };
            }
            macro_rules! skip {
                ($reason:expr) => {
                    report.skipped.push(Skipped {
                        order_number: order.clone(),
                        feature: feature.clone(),
                        reason: $reason.to_owned(),
                    })
                };
            }
            match rule.target {
                Target::Num(key, dim) => {
                    let Some(q) = q else {
                        skip!("value that is not a number");
                        continue;
                    };
                    let Some(f) = dim.factor(&q.unit) else {
                        skip!("value with a unit that does not fit");
                        continue;
                    };
                    if q.range {
                        skip!("range where one value was expected");
                        continue;
                    }
                    let v = num_value(q.low * f, dim);
                    part.data.entry(key.to_owned()).or_insert_with(|| v.clone());
                    part.sources
                        .insert(key.to_owned(), DataSource::MakerDatasheet);
                    note!(format!("{key} = {}", text_of(&v)));
                }
                Target::Range(lo_key, hi_key, dim) => {
                    let Some(q) = q else {
                        skip!("value that is not a number");
                        continue;
                    };
                    let Some(f) = dim.factor(&q.unit) else {
                        skip!("value with a unit that does not fit");
                        continue;
                    };
                    for (key, v) in [(lo_key, q.low * f), (hi_key, q.high * f)] {
                        part.data
                            .entry(key.to_owned())
                            .or_insert_with(|| num_value(v, dim));
                        part.sources
                            .insert(key.to_owned(), DataSource::MakerDatasheet);
                    }
                    note!(format!("{lo_key} and {hi_key}"));
                }
                Target::Text(key) => {
                    let t = text_of(&value);
                    if t.is_empty() {
                        continue;
                    }
                    part.data
                        .entry(key.to_owned())
                        .or_insert_with(|| Value::Text(t.clone()));
                    part.sources
                        .insert(key.to_owned(), DataSource::MakerDatasheet);
                    note!(format!("{key} = {t}"));
                }
                Target::WireRange | Target::WireMin | Target::WireMax | Target::CableSection => {
                    let Some(q) = q else {
                        skip!("value that is not a number");
                        continue;
                    };
                    let Some(f) = Dim::Area.factor(&q.unit) else {
                        skip!("value with a unit that does not fit");
                        continue;
                    };
                    let (lo, hi) = (q.low * f, q.high * f);
                    if is_cable {
                        for key in [eng::SECTION_MM2, keys::CORE_CROSS_SECTION_MM2] {
                            part.data
                                .entry(key.to_owned())
                                .or_insert_with(|| Value::Float(round(hi)));
                            part.sources
                                .insert(key.to_owned(), DataSource::MakerDatasheet);
                        }
                        note!(format!("{} = {}", eng::SECTION_MM2, round(hi)));
                        continue;
                    }
                    let lo_hi = match rule.target {
                        Target::WireMin => (Some(lo), None),
                        Target::WireMax => (None, Some(hi)),
                        _ => (Some(lo), Some(hi)),
                    };
                    if let Some(lo) = lo_hi.0 {
                        draft.wire_min = Some(draft.wire_min.map_or(lo, |m| m.min(lo)));
                    }
                    if let Some(hi) = lo_hi.1 {
                        draft.wire_max = Some(draft.wire_max.map_or(hi, |m| m.max(hi)));
                    }
                    note!("conductor range".into());
                }
                Target::Size(axis) => {
                    let Some(q) = q else {
                        skip!("value that is not a number");
                        continue;
                    };
                    let Some(f) = Dim::Length.factor(&q.unit) else {
                        skip!("value with a unit that does not fit");
                        continue;
                    };
                    draft.size[axis] = Some(round(q.high * f));
                    note!(format!(
                        "size {} = {} mm",
                        ["width", "height", "depth"][axis],
                        round(q.high * f)
                    ));
                }
                Target::Skip(reason) => skip!(reason),
                Target::Connection => {
                    let k = connection_kind(&text_of(&value));
                    if k == ConnectionKind::Unknown {
                        skip!("connection wording that is not recognised");
                    } else {
                        draft.connection = Some(k);
                        note!(format!("connection {k:?}"));
                    }
                }
                Target::ConnectionCount => match q {
                    Some(q) if !q.range && q.low >= 1.0 && q.low.fract() == 0.0 => {
                        draft.connection_count = Some(q.low as usize);
                        part.data
                            .entry(lk::CONNECTION_COUNT.into())
                            .or_insert(Value::Int(q.low as i64));
                        part.sources
                            .insert(lk::CONNECTION_COUNT.into(), DataSource::MakerDatasheet);
                        note!(format!("{} = {}", lk::CONNECTION_COUNT, q.low));
                    }
                    _ => skip!("value that is not a count"),
                },
                Target::Lifecycle => {
                    let l = lifecycle_of(&text_of(&value));
                    if l == Lifecycle::Unknown {
                        skip!("status wording that is not recognised");
                    } else {
                        part.lifecycle = l;
                        note!(format!("lifecycle {l:?}"));
                    }
                }
                Target::Successor => {
                    let t = text_of(&value);
                    if !t.is_empty() && t != order {
                        part.replaced_by = Some(PartRef {
                            manufacturer: part.manufacturer.clone(),
                            order_number: t.clone(),
                        });
                        note!(format!("replaced by {t}"));
                    }
                }
                Target::Price => {
                    let text = text_of(&value);
                    match value_quantity(&value) {
                        Some(q) if !q.range => {
                            draft.price = Some(q.low);
                            if draft.currency.is_none() {
                                draft.currency = currency_of(&text);
                            }
                        }
                        _ => skip!("price that is not a number"),
                    }
                }
                Target::Currency => draft.currency = currency_of(&text_of(&value)),
                Target::PriceUnit => {
                    if let Some(q) = q
                        && q.low >= 1.0
                        && q.low.fract() == 0.0
                    {
                        draft.per = Some(q.low as u32);
                    }
                }
                Target::PriceDate => draft.price_date = Some(text_of(&value)),
                Target::CableCores => match q {
                    Some(q) if !q.range && q.low >= 1.0 => {
                        let v = Value::Int(q.low as i64);
                        part.data.entry(eng::CORE_COUNT.into()).or_insert(v);
                        part.sources
                            .insert(eng::CORE_COUNT.into(), DataSource::MakerDatasheet);
                        note!(format!("{} = {}", eng::CORE_COUNT, q.low));
                    }
                    _ => skip!("value that is not a count"),
                },
                Target::CableOuterDiameter => {
                    match q.and_then(|q| Some((Dim::Length.factor(&q.unit)?, q))) {
                        Some((f, q)) => {
                            let v = Value::Float(round(q.high * f));
                            part.data.entry(eng::OUTER_DIAMETER_MM.into()).or_insert(v);
                            part.sources
                                .insert(eng::OUTER_DIAMETER_MM.into(), DataSource::MakerDatasheet);
                            note!(format!(
                                "{} = {}",
                                eng::OUTER_DIAMETER_MM,
                                round(q.high * f)
                            ));
                        }
                        None => skip!("value with a unit that does not fit"),
                    }
                }
                Target::CableMaxTemp => {
                    match q.and_then(|q| Some((Dim::Temperature.factor(&q.unit)?, q))) {
                        Some((f, q)) => {
                            let v = Value::Float(round(q.high * f));
                            part.data.entry(eng::MAX_TEMP_C.into()).or_insert(v);
                            part.sources
                                .insert(eng::MAX_TEMP_C.into(), DataSource::MakerDatasheet);
                            note!(format!("{} = {}", eng::MAX_TEMP_C, round(q.high * f)));
                        }
                        None => skip!("value with a unit that does not fit"),
                    }
                }
                Target::CableResistance => {
                    match q.and_then(|q| Some((Dim::ResistancePerKm.factor(&q.unit)?, q))) {
                        Some((f, q)) if !q.range => {
                            let v = Value::Float(round(q.low * f));
                            part.data.entry(eng::R20_OHM_PER_KM.into()).or_insert(v);
                            part.sources
                                .insert(eng::R20_OHM_PER_KM.into(), DataSource::MakerDatasheet);
                            note!(format!("{} = {}", eng::R20_OHM_PER_KM, round(q.low * f)));
                        }
                        _ => skip!("resistance whose unit is not ohm per km"),
                    }
                }
            }
        }

        // Dimensions: all three, or the known ones as data.
        match draft.size {
            [Some(width_mm), Some(height_mm), Some(depth_mm)] => {
                if part.dimensions.is_none() {
                    part.dimensions = Some(Dimensions {
                        width_mm,
                        height_mm,
                        depth_mm,
                    });
                }
            }
            sizes => {
                for (key, v) in [keys::WIDTH_MM, keys::HEIGHT_MM, keys::DEPTH_MM]
                    .into_iter()
                    .zip(sizes)
                {
                    if let Some(v) = v {
                        part.data.entry(key.into()).or_insert(Value::Float(v));
                    }
                }
            }
        }

        if let (Some(lo), Some(hi)) = (draft.wire_min, draft.wire_max) {
            for (key, v) in [(lk::WIRE_MIN_MM2, lo), (lk::WIRE_MAX_MM2, hi)] {
                part.data
                    .entry(key.into())
                    .or_insert(Value::Float(round(v)));
                part.sources.insert(key.into(), DataSource::MakerDatasheet);
            }
        }

        // Terminals only for a terminal block whose file states how many
        // connections it has; the names are the usual 1 to N numbering.
        if part.terminals.is_empty()
            && part.function == "terminal"
            && let Some(n) = draft.connection_count.filter(|n| (1..=8).contains(n))
        {
            let wire = draft
                .wire_min
                .zip(draft.wire_max)
                .map(|(a, b)| (round(a), round(b)));
            part.terminals = (1..=n)
                .map(|i| PartTerminal {
                    name: i.to_string(),
                    kind: draft.connection.unwrap_or_default(),
                    wire_mm2: wire,
                    side: Side::Unknown,
                    position_mm: None,
                })
                .collect();
            let w = "Terminal names are numbered 1 to N where the file gives only the count; check them against the part.";
            if !report.warnings.iter().any(|x| x == w) {
                report.warnings.push(w.into());
            }
        }

        // A price needs a currency; without one it is left out.
        if let Some(amount) = draft.price {
            match draft.currency {
                Some(currency) => {
                    report.mapped.push(Mapped {
                        order_number: order.clone(),
                        feature: "price".into(),
                        became: format!("price {amount} {currency}"),
                    });
                    part.price = Some(Price {
                        amount,
                        currency,
                        per: draft.per.unwrap_or(1).max(1),
                        date: draft.price_date.unwrap_or_default(),
                    });
                }
                None => report.skipped.push(Skipped {
                    order_number: order.clone(),
                    feature: "price".into(),
                    reason: "price without a currency".into(),
                }),
            }
        }

        part.provenance = Some(Provenance {
            source: ctx.source.to_owned(),
            file: ctx.file.to_owned(),
            imported: ctx.imported.to_owned(),
            licence_note: format!(
                "user-supplied from {}; not redistributable",
                if self.manufacturer.is_empty() {
                    part.manufacturer.as_str()
                } else {
                    self.manufacturer
                }
            ),
        });
    }
}

/// Keys the importers or this reader write themselves, not features.
fn is_known_key(key: &str) -> bool {
    matches!(
        key,
        keys::WIDTH_MM
            | keys::HEIGHT_MM
            | keys::DEPTH_MM
            | keys::CORES
            | keys::CORE_CROSS_SECTION_MM2
            | keys::LETTER_CODES
    )
}
