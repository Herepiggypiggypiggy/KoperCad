//! Parts quality: how complete the data of each catalog part is, and what
//! exactly is missing (lane PARTS-Q; the Quality tab of the Parts window
//! shows it).
//!
//! The score only counts what is recorded. Nothing here fills in a value:
//! a part without a power loss is `Missing` on that line, a value whose
//! source is not recorded is `Partial`, and the notes name the field so a
//! person knows what to look up.
//!
//! Six lines, with the weight each has in the percentage:
//!
//! | Line | Weight | Complete when |
//! | --- | --- | --- |
//! | Dimensions | 20 | width, height and depth are all positive numbers |
//! | Terminals | 20 | every connection point has a type and a wire range |
//! | Power loss | 15 | `power_loss_w` is a number from the maker or typed in |
//! | 3D model | 15 | a STEP file or mesh is stored |
//! | Price | 15 | a positive list price with currency and price list date |
//! | Origin | 15 | the source and the file it came from are recorded |
//!
//! `Partial` counts half.

use std::fmt::Write as _;

use novo_core::Value;

use crate::catalog::{Catalog, CatalogPart, PartKey, keys};
use crate::schema::{AssetKind, ConnectionKind, DataSource};

/// One thing a part's data is checked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Criterion {
    Dimensions,
    Terminals,
    PowerLoss,
    Geometry,
    Price,
    Provenance,
}

impl Criterion {
    pub const ALL: [Criterion; 6] = [
        Criterion::Dimensions,
        Criterion::Terminals,
        Criterion::PowerLoss,
        Criterion::Geometry,
        Criterion::Price,
        Criterion::Provenance,
    ];

    /// Short name for column heads and chips.
    pub fn label(self) -> &'static str {
        match self {
            Criterion::Dimensions => "Dimensions",
            Criterion::Terminals => "Terminals",
            Criterion::PowerLoss => "Power loss",
            Criterion::Geometry => "3D model",
            Criterion::Price => "Price",
            Criterion::Provenance => "Origin",
        }
    }

    /// Share of the percentage this line carries; the six add up to 100.
    pub fn weight(self) -> u32 {
        match self {
            Criterion::Dimensions | Criterion::Terminals => 20,
            _ => 15,
        }
    }
}

/// How far a part meets one criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Fill {
    Missing,
    Partial,
    Complete,
}

impl Fill {
    pub fn label(self) -> &'static str {
        match self {
            Fill::Missing => "missing",
            Fill::Partial => "partial",
            Fill::Complete => "complete",
        }
    }
}

/// The verdict on one criterion, with the reason in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub criterion: Criterion,
    pub fill: Fill,
    /// What is missing or why it counts as partial; empty when complete.
    pub note: String,
}

/// All six verdicts of one part and the percentage they add up to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completeness {
    /// In [`Criterion::ALL`] order.
    pub findings: Vec<Finding>,
    /// 0 to 100.
    pub percent: u8,
}

impl Completeness {
    pub fn finding(&self, criterion: Criterion) -> &Finding {
        let at = Criterion::ALL
            .iter()
            .position(|c| *c == criterion)
            .unwrap_or(0);
        &self.findings[at]
    }

    pub fn fill(&self, criterion: Criterion) -> Fill {
        self.finding(criterion).fill
    }

    /// The criteria that are not complete.
    pub fn gaps(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.fill != Fill::Complete)
    }

    pub fn is_complete(&self) -> bool {
        self.percent == 100
    }

    pub fn grade(&self) -> Grade {
        Grade::of(self.percent)
    }
}

/// Bands for colouring and filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Grade {
    /// Under 40 %.
    Poor,
    /// 40 to 79 %.
    Fair,
    /// 80 % and up.
    Good,
}

impl Grade {
    pub fn of(percent: u8) -> Grade {
        match percent {
            0..=39 => Grade::Poor,
            40..=79 => Grade::Fair,
            _ => Grade::Good,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Grade::Poor => "Poor",
            Grade::Fair => "Fair",
            Grade::Good => "Good",
        }
    }
}

/// Scores one part. Pure: the same part always gives the same answer.
pub fn assess(part: &CatalogPart) -> Completeness {
    let findings = vec![
        dimensions(part),
        terminals(part),
        power_loss(part),
        geometry(part),
        price(part),
        provenance(part),
    ];
    let points: f64 = findings
        .iter()
        .map(|f| {
            let factor = match f.fill {
                Fill::Missing => 0.0,
                Fill::Partial => 0.5,
                Fill::Complete => 1.0,
            };
            factor * f64::from(f.criterion.weight())
        })
        .sum();
    Completeness {
        findings,
        percent: points.round().clamp(0.0, 100.0) as u8,
    }
}

fn finding(criterion: Criterion, fill: Fill, note: impl Into<String>) -> Finding {
    Finding {
        criterion,
        fill,
        note: note.into(),
    }
}

fn usable(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Float(v) => Some(*v),
        Value::Int(v) => Some(*v as f64),
        _ => None,
    }
}

fn dimensions(part: &CatalogPart) -> Finding {
    let c = Criterion::Dimensions;
    if let Some(d) = part.dimensions {
        let bad: Vec<&str> = [
            ("width", d.width_mm),
            ("height", d.height_mm),
            ("depth", d.depth_mm),
        ]
        .into_iter()
        .filter(|(_, v)| !usable(*v))
        .map(|(n, _)| n)
        .collect();
        return if bad.is_empty() {
            finding(c, Fill::Complete, "")
        } else {
            finding(
                c,
                Fill::Partial,
                format!("{} is zero or not a number", bad.join(", ")),
            )
        };
    }
    // Loose values in the technical data count as partial: the size is
    // known on some sides but the part has no outline yet.
    let present: Vec<&str> = [
        ("width", keys::WIDTH_MM),
        ("height", keys::HEIGHT_MM),
        ("depth", keys::DEPTH_MM),
    ]
    .into_iter()
    .filter(|(_, k)| part.data.get(*k).and_then(number).is_some_and(usable))
    .map(|(n, _)| n)
    .collect();
    if present.is_empty() {
        finding(c, Fill::Missing, "no width, height or depth")
    } else {
        finding(
            c,
            Fill::Partial,
            format!("only {} given", present.join(" and ")),
        )
    }
}

fn terminals(part: &CatalogPart) -> Finding {
    let c = Criterion::Terminals;
    if part.terminals.is_empty() {
        return finding(c, Fill::Missing, "no connection points");
    }
    let total = part.terminals.len();
    let no_kind = part
        .terminals
        .iter()
        .filter(|t| t.kind == ConnectionKind::Unknown)
        .count();
    let no_range = part
        .terminals
        .iter()
        .filter(|t| !t.wire_mm2.is_some_and(|(lo, hi)| usable(lo) && usable(hi)))
        .count();
    if no_kind == 0 && no_range == 0 {
        return finding(c, Fill::Complete, "");
    }
    let mut note = String::new();
    if no_kind > 0 {
        let _ = write!(note, "{no_kind} of {total} without connection type");
    }
    if no_range > 0 {
        if !note.is_empty() {
            note.push_str(", ");
        }
        let _ = write!(note, "{no_range} of {total} without wire range");
    }
    finding(c, Fill::Partial, note)
}

fn power_loss(part: &CatalogPart) -> Finding {
    let c = Criterion::PowerLoss;
    let Some(value) = part.data.get(keys::POWER_LOSS_W) else {
        return finding(c, Fill::Missing, "no power_loss_w");
    };
    let Some(watts) = number(value) else {
        return finding(
            c,
            Fill::Partial,
            "power loss is stored as text, not a number",
        );
    };
    if !watts.is_finite() || watts < 0.0 {
        return finding(c, Fill::Partial, "power loss is not a usable number");
    }
    match part.source_of(keys::POWER_LOSS_W) {
        DataSource::MakerDatasheet | DataSource::Typed => finding(c, Fill::Complete, ""),
        DataSource::Estimated => finding(c, Fill::Partial, "power loss is an estimate"),
        DataSource::Unknown => finding(c, Fill::Partial, "source of the power loss not recorded"),
    }
}

fn geometry(part: &CatalogPart) -> Finding {
    let c = Criterion::Geometry;
    let has = |kind: AssetKind| part.assets.iter().any(|a| a.kind == kind);
    if has(AssetKind::Step) || has(AssetKind::Mesh) {
        finding(c, Fill::Complete, "")
    } else if has(AssetKind::Dxf2d) {
        finding(c, Fill::Partial, "2D outline only, no 3D model")
    } else {
        finding(c, Fill::Missing, "no STEP file or mesh")
    }
}

fn price(part: &CatalogPart) -> Finding {
    let c = Criterion::Price;
    let Some(price) = &part.price else {
        return finding(c, Fill::Missing, "no list price");
    };
    if !usable(price.amount) {
        return finding(c, Fill::Partial, "price is zero or not a number");
    }
    if price.currency.trim().is_empty() {
        return finding(c, Fill::Partial, "price has no currency");
    }
    if price.date.trim().is_empty() {
        return finding(c, Fill::Partial, "price list date not recorded");
    }
    finding(c, Fill::Complete, "")
}

fn provenance(part: &CatalogPart) -> Finding {
    let c = Criterion::Provenance;
    let Some(p) = &part.provenance else {
        return finding(c, Fill::Missing, "origin not recorded");
    };
    if p.source.trim().is_empty() {
        return finding(c, Fill::Missing, "origin not recorded");
    }
    if !p.redistributable() && p.file.trim().is_empty() {
        return finding(
            c,
            Fill::Partial,
            "imported, but the source file is not named",
        );
    }
    finding(c, Fill::Complete, "")
}

/// One part's score in a [`QualityReport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartQuality {
    pub key: PartKey,
    pub manufacturer: String,
    pub order_number: String,
    pub description: String,
    pub completeness: Completeness,
}

/// The scores of every part of a catalog, in catalog order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QualityReport {
    pub rows: Vec<PartQuality>,
}

/// Scores the whole catalog.
pub fn report(catalog: &Catalog) -> QualityReport {
    QualityReport {
        rows: catalog
            .parts()
            .iter()
            .map(|p| PartQuality {
                key: p.key(),
                manufacturer: p.manufacturer.clone(),
                order_number: p.order_number.clone(),
                description: p.description.clone(),
                completeness: assess(p),
            })
            .collect(),
    }
}

impl QualityReport {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Mean percentage over all parts; `None` for an empty catalog.
    pub fn average_percent(&self) -> Option<f64> {
        if self.rows.is_empty() {
            return None;
        }
        let sum: u64 = self
            .rows
            .iter()
            .map(|r| u64::from(r.completeness.percent))
            .sum();
        Some(sum as f64 / self.rows.len() as f64)
    }

    /// How many parts fall in a grade.
    pub fn count_grade(&self, grade: Grade) -> usize {
        self.rows
            .iter()
            .filter(|r| r.completeness.grade() == grade)
            .count()
    }

    /// How many parts are not complete on `criterion`.
    pub fn count_gaps(&self, criterion: Criterion) -> usize {
        self.rows
            .iter()
            .filter(|r| r.completeness.fill(criterion) != Fill::Complete)
            .count()
    }

    /// How many parts have `fill` on `criterion`.
    pub fn count_fill(&self, criterion: Criterion, fill: Fill) -> usize {
        self.rows
            .iter()
            .filter(|r| r.completeness.fill(criterion) == fill)
            .count()
    }

    /// Parts that are not complete on `criterion`, lowest score first,
    /// then by maker and order number so the list is stable.
    pub fn with_gap(&self, criterion: Criterion) -> Vec<&PartQuality> {
        let mut rows: Vec<&PartQuality> = self
            .rows
            .iter()
            .filter(|r| r.completeness.fill(criterion) != Fill::Complete)
            .collect();
        rows.sort_by(|a, b| Self::order(a, b));
        rows
    }

    /// Every part with at least one gap, lowest score first.
    pub fn incomplete(&self) -> Vec<&PartQuality> {
        let mut rows: Vec<&PartQuality> = self
            .rows
            .iter()
            .filter(|r| !r.completeness.is_complete())
            .collect();
        rows.sort_by(|a, b| Self::order(a, b));
        rows
    }

    fn order(a: &PartQuality, b: &PartQuality) -> std::cmp::Ordering {
        a.completeness
            .percent
            .cmp(&b.completeness.percent)
            .then_with(|| a.key.cmp(&b.key))
    }

    /// The missing-data report as CSV (UTF-8, comma separated, quoted where
    /// needed): one line per part that has a gap, the percentage, the
    /// verdict on each criterion and the notes. Opens in Excel.
    pub fn missing_csv(&self) -> String {
        let mut out = String::from("Manufacturer,Order number,Description,Complete %");
        for c in Criterion::ALL {
            out.push(',');
            out.push_str(c.label());
        }
        out.push_str(",Notes\n");
        for row in self.incomplete() {
            let done = &row.completeness;
            csv_field(&mut out, &row.manufacturer);
            out.push(',');
            csv_field(&mut out, &row.order_number);
            out.push(',');
            csv_field(&mut out, &row.description);
            let _ = write!(out, ",{}", done.percent);
            for c in Criterion::ALL {
                out.push(',');
                out.push_str(done.fill(c).label());
            }
            out.push(',');
            let notes: Vec<String> = done
                .gaps()
                .map(|f| format!("{}: {}", f.criterion.label(), f.note))
                .collect();
            csv_field(&mut out, &notes.join("; "));
            out.push('\n');
        }
        out
    }
}

fn csv_field(out: &mut String, text: &str) {
    // A cell that starts with a formula character would run in Excel.
    let formula = text.starts_with(['=', '+', '-', '@']);
    if formula || text.contains([',', '"', '\n', '\r']) {
        out.push('"');
        if formula {
            out.push('\'');
        }
        out.push_str(&text.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(text);
    }
}

// ---------------------------------------------------------------------
// Duplicates and conflicting values
// ---------------------------------------------------------------------

/// Why two or more catalog parts look like the same article.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DuplicateReason {
    /// The same order number under maker names that spell one maker two
    /// ways (`Siemens` and `Siemens AG`).
    SameMakerSpelling,
    /// The same order number under unrelated makers. Often a coincidence
    /// with a short code, so it is only a candidate.
    SameOrderNumber,
    /// The same type number under one maker with different order numbers.
    /// Variants (coil voltage, packaging) share a type number legitimately.
    SameTypeNumber,
}

impl DuplicateReason {
    pub fn label(self) -> &'static str {
        match self {
            DuplicateReason::SameMakerSpelling => "Same order number, maker spelled two ways",
            DuplicateReason::SameOrderNumber => "Same order number, different makers",
            DuplicateReason::SameTypeNumber => "Same type number, different order numbers",
        }
    }

    /// Whether this is very likely one article entered twice.
    pub fn likely(self) -> bool {
        self == DuplicateReason::SameMakerSpelling
    }
}

/// A field that two or more members of a group record with different values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// `width_mm`, `price`, `rated_current_a`, `lifecycle`, ...
    pub field: String,
    /// The value each member records, aligned with the members; `None` where
    /// that member does not record the field.
    pub values: Vec<Option<String>>,
}

/// Parts that look like one article, with the values they disagree on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub reason: DuplicateReason,
    /// Sorted by key.
    pub members: Vec<PartKey>,
    pub conflicts: Vec<Conflict>,
}

/// A value of one field, ready for comparison.
enum Datum {
    Num(f64, String),
    Text(String),
}

impl Datum {
    fn show(&self) -> &str {
        match self {
            Datum::Num(_, s) | Datum::Text(s) => s,
        }
    }

    fn same(&self, other: &Datum) -> bool {
        match (self, other) {
            (Datum::Num(a, _), Datum::Num(b, _)) => {
                (a - b).abs() <= 1e-9_f64.max(1e-6 * a.abs().max(b.abs()))
            }
            (Datum::Text(a), Datum::Text(b)) => a.trim().eq_ignore_ascii_case(b.trim()),
            _ => false,
        }
    }
}

fn num_datum(v: f64) -> Datum {
    Datum::Num(v, crate::sheet::display_value(&Value::Float(v)))
}

/// The recorded fields of a part that two sources can disagree about.
/// Fields the part does not record are left out: unknown is not a value.
fn recorded_fields(part: &CatalogPart) -> std::collections::BTreeMap<String, Datum> {
    let mut out = std::collections::BTreeMap::new();
    if let Some(d) = part.dimensions {
        for (k, v) in [
            (keys::WIDTH_MM, d.width_mm),
            (keys::HEIGHT_MM, d.height_mm),
            (keys::DEPTH_MM, d.depth_mm),
        ] {
            out.insert(k.to_owned(), num_datum(v));
        }
    }
    for (key, value) in &part.data {
        if crate::sheet::is_internal_key(key) || out.contains_key(key) {
            continue;
        }
        let datum = match value {
            Value::Float(v) => num_datum(*v),
            Value::Int(v) => num_datum(*v as f64),
            Value::Bool(b) => Datum::Text(b.to_string()),
            Value::Text(t) if t.trim().is_empty() => continue,
            Value::Text(t) => Datum::Text(t.clone()),
        };
        out.insert(key.clone(), datum);
    }
    if let Some(p) = part.price.as_ref().filter(|p| p.amount.is_finite()) {
        out.insert(
            "price".into(),
            Datum::Text(format!("{:.4} {}", p.each(), p.currency.trim())),
        );
    }
    if part.lifecycle != crate::schema::Lifecycle::Unknown {
        out.insert(
            "lifecycle".into(),
            Datum::Text(format!("{:?}", part.lifecycle)),
        );
    }
    if let Some(e) = &part.class.etim {
        out.insert("etim".into(), Datum::Text(e.clone()));
    }
    if let Some(e) = &part.class.eclass {
        out.insert("eclass".into(), Datum::Text(e.clone()));
    }
    if !part.terminals.is_empty() {
        out.insert(
            "terminal_count".into(),
            Datum::Num(
                part.terminals.len() as f64,
                part.terminals.len().to_string(),
            ),
        );
    }
    out
}

/// The fields at least two of `parts` record with different values.
pub fn conflicts_between(parts: &[&CatalogPart]) -> Vec<Conflict> {
    let fields: Vec<_> = parts.iter().map(|p| recorded_fields(p)).collect();
    let names: std::collections::BTreeSet<&String> = fields.iter().flat_map(|f| f.keys()).collect();
    let mut out = Vec::new();
    for name in names {
        let present: Vec<&Datum> = fields.iter().filter_map(|f| f.get(name)).collect();
        if present.len() < 2 || present.iter().all(|d| d.same(present[0])) {
            continue;
        }
        out.push(Conflict {
            field: name.clone(),
            values: fields
                .iter()
                .map(|f| f.get(name).map(|d| d.show().to_owned()))
                .collect(),
        });
    }
    out
}

/// Order numbers shorter than this are too generic to call a duplicate.
const MIN_ORDER_LEN: usize = 4;
/// Same for type numbers.
const MIN_TYPE_LEN: usize = 3;

/// Whether two normalised maker names spell the same maker
/// (`siemens` and `siemensag`).
fn same_maker(a: &str, b: &str) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() >= 3 && long.starts_with(short)
}

/// Finds parts that look like one article entered twice, with the values
/// they disagree on. Deterministic: likely duplicates first, then by key.
pub fn duplicate_groups(catalog: &Catalog) -> Vec<DuplicateGroup> {
    use crate::search::normalize_code;
    use std::collections::{BTreeMap, HashMap};

    let mut by_order: HashMap<String, Vec<&CatalogPart>> = HashMap::new();
    let mut by_type: HashMap<(String, String), Vec<&CatalogPart>> = HashMap::new();
    for part in catalog.parts() {
        let order = normalize_code(&part.order_number);
        if order.len() >= MIN_ORDER_LEN {
            by_order.entry(order).or_default().push(part);
        }
        let ty = normalize_code(&part.type_number);
        if ty.len() >= MIN_TYPE_LEN {
            by_type
                .entry((normalize_code(&part.manufacturer), ty))
                .or_default()
                .push(part);
        }
    }
    let mut groups: BTreeMap<(DuplicateReason, PartKey), DuplicateGroup> = BTreeMap::new();
    let mut add = |reason: DuplicateReason, mut parts: Vec<&CatalogPart>| {
        parts.sort_by_key(|p| p.key());
        let members: Vec<PartKey> = parts.iter().map(|p| p.key()).collect();
        let conflicts = conflicts_between(&parts);
        groups.insert(
            (reason, members[0].clone()),
            DuplicateGroup {
                reason,
                members,
                conflicts,
            },
        );
    };
    for parts in by_order.into_values().filter(|p| p.len() > 1) {
        let makers: Vec<String> = parts
            .iter()
            .map(|p| normalize_code(&p.manufacturer))
            .collect();
        let spelled_alike = makers
            .iter()
            .enumerate()
            .all(|(i, a)| makers.iter().skip(i + 1).all(|b| same_maker(a, b)));
        let reason = if spelled_alike {
            DuplicateReason::SameMakerSpelling
        } else {
            DuplicateReason::SameOrderNumber
        };
        add(reason, parts);
    }
    for parts in by_type.into_values().filter(|p| p.len() > 1) {
        add(DuplicateReason::SameTypeNumber, parts);
    }
    let mut out: Vec<DuplicateGroup> = groups.into_values().collect();
    out.sort_by(|a, b| {
        a.reason
            .cmp(&b.reason)
            .then_with(|| a.members.cmp(&b.members))
    });
    out
}

/// Parts of an incoming set (a maker file about to be merged) that exist in
/// the catalog under the same key but record different values. `values` of
/// each conflict are `[catalog, incoming]`.
pub fn incoming_conflicts(
    catalog: &Catalog,
    incoming: &[CatalogPart],
) -> Vec<(PartKey, Vec<Conflict>)> {
    incoming
        .iter()
        .filter_map(|new| {
            let old = catalog.get(&new.key())?;
            let conflicts = conflicts_between(&[old, new]);
            (!conflicts.is_empty()).then(|| (new.key(), conflicts))
        })
        .collect()
}

// ---------------------------------------------------------------------
// Review queue
// ---------------------------------------------------------------------

/// Version of the review file written by [`ReviewLedger::save`].
pub const REVIEW_FORMAT_VERSION: u32 = 1;

/// File name of the review ledger, next to the catalog file.
pub const REVIEW_FILE_NAME: &str = "parts-review.json";

/// The review ledger belonging to a catalog file: same folder.
pub fn review_path_for(catalog_path: &std::path::Path) -> std::path::PathBuf {
    catalog_path.with_file_name(REVIEW_FILE_NAME)
}

/// Whether a part's data can be relied on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trust {
    /// Written by a person in KoperCAD (`authored`).
    Authored,
    /// Imported, and a person looked at exactly this data.
    Reviewed { date: String },
    /// Imported, and a person said not to use it.
    Rejected { date: String, note: String },
    /// Imported and not looked at yet.
    Unreviewed,
    /// Reviewed once, but the data changed since.
    ChangedSinceReview,
    /// No origin recorded (the starter set, old files). Not queued: there is
    /// nothing to say who wrote it.
    Unrecorded,
}

impl Trust {
    pub fn label(&self) -> &'static str {
        match self {
            Trust::Authored => "Authored",
            Trust::Reviewed { .. } => "Reviewed",
            Trust::Rejected { .. } => "Rejected",
            Trust::Unreviewed => "Untrusted: not reviewed",
            Trust::ChangedSinceReview => "Untrusted: changed since review",
            Trust::Unrecorded => "Origin not recorded",
        }
    }

    /// Whether the part waits in the review queue.
    pub fn needs_review(&self) -> bool {
        matches!(self, Trust::Unreviewed | Trust::ChangedSinceReview)
    }
}

/// What a person decided about a part.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approved,
    Rejected,
}

/// One recorded decision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReviewEntry {
    pub decision: Decision,
    /// [`fingerprint`] of the data that was looked at.
    pub fingerprint: String,
    /// `2026-10-17`.
    pub date: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A fingerprint of what a maker's data says about a part. Local additions
/// are left out, so storing a STEP file or detecting connection points does
/// not undo a review: the 3D files and the terminal positions.
pub fn fingerprint(part: &CatalogPart) -> String {
    use sha2::{Digest, Sha256};
    let mut plain = part.clone();
    plain.assets.clear();
    for t in &mut plain.terminals {
        t.position_mm = None;
    }
    plain.sources.remove(crate::pins::POSITIONS_SOURCE_KEY);
    let json = serde_json::to_string(&plain).unwrap_or_default();
    Sha256::digest(json.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn ledger_key(part: &CatalogPart) -> String {
    use crate::search::normalize_code;
    format!(
        "{}|{}",
        normalize_code(&part.manufacturer),
        normalize_code(&part.order_number)
    )
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LedgerFile {
    format: u32,
    entries: std::collections::BTreeMap<String, ReviewEntry>,
}

/// Why the review file could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("cannot access the review file {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("the review file {path} is not valid: {source}")]
    Format {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },
    #[error("the review file {path} was written by a newer KoperCAD (format {found})")]
    TooNew {
        path: std::path::PathBuf,
        found: u32,
    },
}

/// A part waiting for review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub key: PartKey,
    pub manufacturer: String,
    pub order_number: String,
    pub description: String,
    pub trust: Trust,
    /// File the data was imported from; groups the queue.
    pub file: String,
    /// Completeness, so the thinnest imports can be looked at first.
    pub percent: u8,
}

/// Decisions on imported parts. Lives in a file next to the catalog, so the
/// catalog format stays as it is. A part is trusted only while its data is
/// exactly what was reviewed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewLedger {
    entries: std::collections::BTreeMap<String, ReviewEntry>,
}

impl ReviewLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Reads the ledger; a missing file is an empty ledger.
    pub fn load(path: &std::path::Path) -> Result<Self, ReviewError> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::new()),
            Err(source) => {
                return Err(ReviewError::Io {
                    path: path.into(),
                    source,
                });
            }
        };
        let file: LedgerFile =
            serde_json::from_str(&text).map_err(|source| ReviewError::Format {
                path: path.into(),
                source,
            })?;
        if file.format > REVIEW_FORMAT_VERSION {
            return Err(ReviewError::TooNew {
                path: path.into(),
                found: file.format,
            });
        }
        Ok(ReviewLedger {
            entries: file.entries,
        })
    }

    /// Writes the ledger through a temporary file, so a crash cannot leave
    /// half a file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), ReviewError> {
        let io = |source| ReviewError::Io {
            path: path.into(),
            source,
        };
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let text = serde_json::to_string_pretty(&LedgerFile {
            format: REVIEW_FORMAT_VERSION,
            entries: self.entries.clone(),
        })
        .unwrap_or_default();
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(io)?;
        std::fs::rename(&tmp, path).map_err(io)
    }

    /// How much to rely on `part`.
    pub fn trust_of(&self, part: &CatalogPart) -> Trust {
        let source = part.provenance.as_ref().map_or("", |p| p.source.trim());
        if source == "authored" {
            return Trust::Authored;
        }
        if !source.starts_with("import:") {
            return Trust::Unrecorded;
        }
        match self.entries.get(&ledger_key(part)) {
            None => Trust::Unreviewed,
            Some(e) if e.fingerprint != fingerprint(part) => Trust::ChangedSinceReview,
            Some(e) => match e.decision {
                Decision::Approved => Trust::Reviewed {
                    date: e.date.clone(),
                },
                Decision::Rejected => Trust::Rejected {
                    date: e.date.clone(),
                    note: e.note.clone(),
                },
            },
        }
    }

    /// Records that a person looked at exactly this data and accepts it.
    pub fn approve(&mut self, part: &CatalogPart, date: &str) {
        self.record(part, Decision::Approved, date, "");
    }

    /// Records that a person looked at this data and does not accept it.
    pub fn reject(&mut self, part: &CatalogPart, date: &str, note: &str) {
        self.record(part, Decision::Rejected, date, note);
    }

    fn record(&mut self, part: &CatalogPart, decision: Decision, date: &str, note: &str) {
        self.entries.insert(
            ledger_key(part),
            ReviewEntry {
                decision,
                fingerprint: fingerprint(part),
                date: date.to_owned(),
                note: note.to_owned(),
            },
        );
    }

    /// Approves every part of `keys` that is still in the queue; returns
    /// how many.
    pub fn approve_all(&mut self, catalog: &Catalog, keys: &[PartKey], date: &str) -> usize {
        let mut done = 0;
        for key in keys {
            if let Some(part) = catalog.get(key)
                && self.trust_of(part).needs_review()
            {
                self.approve(part, date);
                done += 1;
            }
        }
        done
    }

    /// Imported parts nobody has looked at (or whose data changed since),
    /// the thinnest data first, then by key.
    pub fn queue(&self, catalog: &Catalog) -> Vec<QueueItem> {
        let mut items: Vec<QueueItem> = catalog
            .parts()
            .iter()
            .filter_map(|p| {
                let trust = self.trust_of(p);
                trust.needs_review().then(|| QueueItem {
                    key: p.key(),
                    manufacturer: p.manufacturer.clone(),
                    order_number: p.order_number.clone(),
                    description: p.description.clone(),
                    trust,
                    file: p
                        .provenance
                        .as_ref()
                        .map_or_else(String::new, |x| x.file.clone()),
                    percent: assess(p).percent,
                })
            })
            .collect();
        items.sort_by(|a, b| a.percent.cmp(&b.percent).then_with(|| a.key.cmp(&b.key)));
        items
    }
}
