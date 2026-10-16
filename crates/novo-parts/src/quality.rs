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
