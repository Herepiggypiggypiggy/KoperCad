//! The preview and validation of a table import: what every row of the file
//! becomes, row by row, before the catalog is touched.
//!
//! [`build_preview`] reads each row through the [`Mapping`], converts values
//! to the units KoperCAD stores, checks them and reports what it did in
//! words. Nothing is guessed: a value that cannot be read or is implausible
//! is left out of the part (and said so), and a field the file has no column
//! for stays unknown. [`ImportPreview::coverage`] lists how many parts have
//! each field, so the person sees what the file does not tell.

use std::collections::{BTreeSet, HashMap};

use novo_core::Value;

use crate::catalog::{Catalog, CatalogPart, Dimensions, MergePreview, PartKey, keys, split_list};
use crate::csv::cell_value;
use crate::recipes::{
    connection_kind, currency_of, lifecycle_of, local_keys as lk, parse_quantity,
};
use crate::schema::{
    ConnectionKind, DataSource, Lifecycle, PartTerminal, Price, Provenance, Side, eng,
};
use crate::tablefile::Sheet;
use crate::tablemap::{Mapping, Target, is_eclass_code, is_etim_class, stored_unit};

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Information: an assumption or a value left unknown.
    Note,
    /// A value was left out or looks wrong; the row is still imported.
    Warning,
    /// The row is not imported.
    Error,
}

/// One finding about a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    /// Header of the column, empty for a finding about the whole row.
    pub column: String,
    pub message: String,
}

/// What the import does with a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStatus {
    /// A part the catalog does not have.
    New,
    /// A part the catalog has, with different data: the entry is replaced.
    Changed,
    /// Already in the catalog exactly like this.
    Unchanged,
    /// Not imported (see the issues).
    Skipped,
}

/// One row of the file as the import sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewRow {
    /// Row number in the sheet, 1-based, as the spreadsheet shows it.
    pub line: usize,
    pub status: RowStatus,
    pub manufacturer: String,
    pub order_number: String,
    pub description: String,
    pub issues: Vec<Issue>,
    /// Position in [`ImportPreview::parts`] when the row is imported.
    pub part: Option<usize>,
}

impl PreviewRow {
    pub fn worst(&self) -> Option<Severity> {
        self.issues.iter().map(|i| i.severity).max()
    }
}

/// How many of the imported parts have one field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    pub label: &'static str,
    pub have: usize,
    pub total: usize,
}

/// Counts for the summary line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub new: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub skipped: usize,
    /// Rows imported with at least one warning.
    pub warned: usize,
}

/// Everything the dialog shows before Import.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportPreview {
    /// Mapping problems that stop the import. When not empty there are no
    /// rows and no parts.
    pub problems: Vec<String>,
    pub rows: Vec<PreviewRow>,
    /// The parts that would be added or replaced, one per maker and order
    /// number.
    pub parts: Vec<CatalogPart>,
    /// What merging the parts into the catalog would do.
    pub merge: MergePreview,
    pub coverage: Vec<Coverage>,
    /// Units that were assumed because neither header nor cell had one.
    pub assumptions: Vec<String>,
}

impl ImportPreview {
    pub fn counts(&self) -> Counts {
        let mut c = Counts::default();
        for row in &self.rows {
            match row.status {
                RowStatus::New => c.new += 1,
                RowStatus::Changed => c.changed += 1,
                RowStatus::Unchanged => c.unchanged += 1,
                RowStatus::Skipped => c.skipped += 1,
            }
            if row.status != RowStatus::Skipped && row.worst() >= Some(Severity::Warning) {
                c.warned += 1;
            }
        }
        c
    }

    /// Whether Import would change the catalog.
    pub fn can_import(&self) -> bool {
        let c = self.counts();
        self.problems.is_empty() && c.new + c.changed > 0
    }

    /// Adds the parts to `catalog`. Returns how many were new.
    pub fn apply(&self, catalog: &mut Catalog) -> usize {
        self.parts
            .iter()
            .filter(|p| catalog.upsert((*p).clone()))
            .count()
    }

    /// The one-line summary: `12 new, 3 changed, 1 unchanged, 2 skipped`.
    pub fn summary(&self) -> String {
        let c = self.counts();
        let mut parts = vec![format!("{} new", c.new), format!("{} changed", c.changed)];
        if c.unchanged > 0 {
            parts.push(format!("{} already in the catalog", c.unchanged));
        }
        if c.skipped > 0 {
            parts.push(format!("{} not imported", c.skipped));
        }
        parts.join(", ")
    }

    /// A plain-text report of the preview, for saving next to the file.
    pub fn render(&self, file: &str) -> String {
        use std::fmt::Write as _;
        let mut out = format!("Parts table import: {file}\n{}\n", self.summary());
        for problem in &self.problems {
            let _ = writeln!(out, "PROBLEM  {problem}");
        }
        for a in &self.assumptions {
            let _ = writeln!(out, "ASSUMED  {a}");
        }
        for c in &self.coverage {
            let _ = writeln!(out, "FIELD    {}: {} of {} parts", c.label, c.have, c.total);
        }
        for row in &self.rows {
            for i in &row.issues {
                let level = match i.severity {
                    Severity::Error => "ERROR  ",
                    Severity::Warning => "WARNING",
                    Severity::Note => "NOTE   ",
                };
                let column = if i.column.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", i.column)
                };
                let _ = writeln!(
                    out,
                    "{level} row {} {}{column}: {}",
                    row.line, row.order_number, i.message
                );
            }
        }
        out
    }
}

/// Reads every row below the header through `mapping`. `file` and `today`
/// go into each part's provenance. `catalog` decides new, changed or
/// unchanged and is not modified.
pub fn build_preview(
    sheet: &Sheet,
    mapping: &Mapping,
    file: &str,
    today: &str,
    catalog: &Catalog,
) -> ImportPreview {
    let problems = mapping.problems();
    if !problems.is_empty() {
        return ImportPreview {
            problems,
            ..Default::default()
        };
    }
    let mut rows: Vec<PreviewRow> = Vec::new();
    let mut built: Vec<Option<CatalogPart>> = Vec::new();
    let mut by_key: HashMap<PartKey, usize> = HashMap::new();
    let mut assumed: BTreeSet<usize> = BTreeSet::new();
    let body = sheet.rows.get(mapping.header_row + 1..).unwrap_or_default();
    for (i, cells) in body.iter().enumerate() {
        if cells.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let line = sheet.line_of(mapping.header_row + 1 + i);
        let mut row = PreviewRow {
            line,
            status: RowStatus::Skipped,
            manufacturer: String::new(),
            order_number: String::new(),
            description: String::new(),
            issues: Vec::new(),
            part: None,
        };
        match read_row(cells, mapping, file, today, &mut row, &mut assumed) {
            Some(part) => {
                row.manufacturer = part.manufacturer.clone();
                row.order_number = part.order_number.clone();
                row.description = part.description.clone();
                let key = part.key();
                let index = built.len();
                built.push(Some(part));
                if let Some(&earlier) = by_key.get(&key) {
                    built[earlier] = None;
                    if let Some(old) = rows.iter_mut().find(|r| r.part == Some(earlier)) {
                        old.part = None;
                        old.status = RowStatus::Skipped;
                        old.issues.push(Issue {
                            severity: Severity::Warning,
                            column: String::new(),
                            message: format!(
                                "the same part is in row {line}, which is used instead"
                            ),
                        });
                    }
                }
                by_key.insert(key, index);
                row.part = Some(index);
            }
            None => {
                row.manufacturer = text_at(cells, mapping.column_of(&Target::Manufacturer));
                row.order_number = text_at(cells, mapping.column_of(&Target::OrderNumber));
                row.description = text_at(cells, mapping.column_of(&Target::Description));
            }
        }
        rows.push(row);
    }
    // Compact the parts and point the rows at the new positions.
    let mut remap = vec![None; built.len()];
    let mut parts = Vec::new();
    for (old, part) in built.into_iter().enumerate() {
        if let Some(part) = part {
            remap[old] = Some(parts.len());
            parts.push(part);
        }
    }
    for row in &mut rows {
        row.part = row.part.and_then(|p| remap[p]);
        if let Some(p) = row.part {
            row.status = match catalog.get(&parts[p].key()) {
                None => RowStatus::New,
                Some(existing) if *existing != parts[p] => RowStatus::Changed,
                Some(_) => RowStatus::Unchanged,
            };
        }
    }
    let merge = catalog.preview_merge(&parts);
    let coverage = coverage_of(&parts);
    let assumptions = assumed
        .into_iter()
        .filter_map(|c| {
            let column = &mapping.columns[c];
            let unit = stored_unit(column.target.dim()?);
            (!unit.is_empty()).then(|| {
                format!(
                    "Column \"{}\" has no unit in its header or in some cells: those values are taken as {unit}.",
                    column.header
                )
            })
        })
        .collect();
    ImportPreview {
        problems,
        rows,
        parts,
        merge,
        coverage,
        assumptions,
    }
}

fn text_at(cells: &[String], column: Option<usize>) -> String {
    column
        .and_then(|c| cells.get(c))
        .map(|t| t.trim().to_owned())
        .unwrap_or_default()
}

/// Everything one row says, before it becomes a part.
#[derive(Default)]
struct Draft {
    manufacturer: String,
    order_number: String,
    type_number: String,
    description: String,
    letter_codes: Vec<String>,
    keywords: Vec<String>,
    function: String,
    size: [Option<f64>; 3],
    measures: Vec<(&'static str, f64)>,
    etim: Option<String>,
    eclass: Option<String>,
    price: Option<(f64, String)>,
    lifecycle: Lifecycle,
    connection: Option<ConnectionKind>,
    connection_count: Option<usize>,
    wire_min: Option<f64>,
    wire_max: Option<f64>,
    custom: Vec<(String, Value)>,
}

fn read_row(
    cells: &[String],
    mapping: &Mapping,
    file: &str,
    today: &str,
    row: &mut PreviewRow,
    assumed: &mut BTreeSet<usize>,
) -> Option<CatalogPart> {
    let mut d = Draft::default();
    for (index, column) in mapping.columns.iter().enumerate() {
        let raw = cells.get(index).map(|c| c.trim()).unwrap_or_default();
        if raw.is_empty() || column.target == Target::Ignore {
            continue;
        }
        let mut warn = |severity: Severity, message: String| {
            row.issues.push(Issue {
                severity,
                column: column.header.clone(),
                message,
            });
        };
        match &column.target {
            Target::Ignore => {}
            Target::Manufacturer => d.manufacturer = raw.to_owned(),
            Target::OrderNumber => {
                d.order_number = raw.to_owned();
                if looks_like_excel_exponent(raw) {
                    warn(
                        Severity::Warning,
                        format!(
                            "\"{raw}\" looks like Excel turned a long number into a power of ten; the digits are lost. Format the column as text in the source and export again"
                        ),
                    );
                }
                if raw.chars().count() > 40 {
                    warn(
                        Severity::Warning,
                        "an order number this long is unusual; check the column".into(),
                    );
                }
            }
            Target::TypeNumber => d.type_number = raw.to_owned(),
            Target::Description => d.description = raw.to_owned(),
            Target::LetterCodes => {
                d.letter_codes = split_list(&raw.replace([';', ' '], ","))
                    .into_iter()
                    .map(|c| c.trim_start_matches('-').to_uppercase())
                    .collect();
            }
            Target::Keywords => d.keywords = split_list(&raw.replace(';', ",")),
            Target::Function => d.function = raw.to_lowercase(),
            Target::Width | Target::Height | Target::Depth => {
                let slot = match column.target {
                    Target::Width => 0,
                    Target::Height => 1,
                    _ => 2,
                };
                if let Some(v) = number(raw, column, &mut warn, assumed, index)
                    && plausible(v, 3000.0, "size in mm", &mut warn, false, false)
                {
                    d.size[slot] = Some(v);
                }
            }
            Target::Measure(key) => {
                if let Some(v) = number(raw, column, &mut warn, assumed, index) {
                    let bounds = match *key {
                        eng::POWER_LOSS_W => Some((5000.0, "power loss in W")),
                        eng::RATED_CURRENT_A | eng::AC3_CURRENT_A | eng::AC1_CURRENT_A => {
                            Some((10_000.0, "current in A"))
                        }
                        "rated_voltage_v" | eng::COIL_VOLTAGE_V => Some((36_000.0, "voltage in V")),
                        eng::POLES => Some((8.0, "pole count")),
                        _ => None,
                    };
                    let ok = bounds.is_none_or(|(hi, what)| {
                        // Too high is a warning and the value is kept;
                        // zero (except a loss) or negative is dropped.
                        plausible(v, hi, what, &mut warn, true, *key == eng::POWER_LOSS_W)
                    });
                    let poles_whole = *key != eng::POLES || (v.fract() == 0.0 && v >= 1.0);
                    if !poles_whole {
                        warn(
                            Severity::Warning,
                            format!("{raw} is not a pole count; the value is left unknown"),
                        );
                    } else if ok {
                        d.measures.push((key, v));
                    }
                }
            }
            Target::Etim => {
                let code = raw.trim().to_uppercase();
                if is_etim_class(&code) {
                    d.etim = Some(code);
                } else {
                    warn(
                        Severity::Warning,
                        format!(
                            "\"{raw}\" is not an ETIM class (EC and six digits); the class is left unknown"
                        ),
                    );
                }
            }
            Target::Eclass => {
                if is_eclass_code(raw) {
                    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
                    d.eclass = Some(format!(
                        "{}-{}-{}-{}",
                        &digits[0..2],
                        &digits[2..4],
                        &digits[4..6],
                        &digits[6..8]
                    ));
                } else {
                    warn(
                        Severity::Warning,
                        format!(
                            "\"{raw}\" is not an ECLASS code (eight digits); the code is left unknown"
                        ),
                    );
                }
            }
            Target::Price => {
                let currency = currency_of(raw).unwrap_or_else(|| mapping.currency.clone());
                if let Some(v) = price(raw, &mut warn) {
                    d.price = Some((v, currency));
                }
            }
            Target::Lifecycle => {
                d.lifecycle = lifecycle_of(raw);
                if d.lifecycle == Lifecycle::Unknown {
                    warn(
                        Severity::Note,
                        format!("status \"{raw}\" is not recognised; left unknown"),
                    );
                }
            }
            Target::Connection => {
                let kind = connection_kind(raw);
                if kind == ConnectionKind::Unknown {
                    warn(
                        Severity::Note,
                        format!("connection \"{raw}\" is not recognised; left unknown"),
                    );
                } else {
                    d.connection = Some(kind);
                }
            }
            Target::ConnectionCount => {
                if let Some(v) = number(raw, column, &mut warn, assumed, index)
                    && v.fract() == 0.0
                    && v >= 1.0
                {
                    d.connection_count = Some(v as usize);
                } else {
                    warn(
                        Severity::Warning,
                        format!("\"{raw}\" is not a number of connections"),
                    );
                }
            }
            Target::WireMin | Target::WireMax => {
                if let Some(v) = number(raw, column, &mut warn, assumed, index)
                    && plausible(v, 1000.0, "conductor size in mm²", &mut warn, false, false)
                {
                    if column.target == Target::WireMin {
                        d.wire_min = Some(v);
                    } else {
                        d.wire_max = Some(v);
                    }
                }
            }
            Target::WireRange => match parse_quantity(raw) {
                Some(q) => {
                    let unit = if q.unit.is_empty() {
                        column.unit.to_lowercase().replace('²', "2")
                    } else {
                        q.unit.clone()
                    };
                    match crate::recipes::Dim::Area.factor(&unit) {
                        Some(f) => {
                            if q.unit.is_empty() && column.unit.is_empty() {
                                assumed.insert(index);
                            }
                            d.wire_min = q.range.then_some(q.low * f);
                            d.wire_max = Some(q.high * f);
                            if !q.range {
                                warn(
                                    Severity::Note,
                                    "one value, taken as the largest conductor; the smallest is unknown".into(),
                                );
                            }
                        }
                        None => warn(
                            Severity::Warning,
                            format!("the unit {unit} is not a conductor size; left unknown"),
                        ),
                    }
                }
                None => warn(
                    Severity::Warning,
                    format!("\"{raw}\" holds no conductor size; left unknown"),
                ),
            },
            Target::Custom(key) => d.custom.push((key.clone(), cell_value(raw))),
        }
    }

    // Row-level checks.
    if d.manufacturer.is_empty() {
        d.manufacturer = mapping.default_manufacturer.trim().to_owned();
    }
    if d.order_number.is_empty() {
        row.issues.push(Issue {
            severity: Severity::Error,
            column: String::new(),
            message: "no order number; the row is not imported".into(),
        });
        return None;
    }
    if d.manufacturer.is_empty() {
        row.issues.push(Issue {
            severity: Severity::Error,
            column: String::new(),
            message: "no manufacturer in the row and no default maker set; the row is not imported"
                .into(),
        });
        return None;
    }
    if let (Some(lo), Some(hi)) = (d.wire_min, d.wire_max)
        && lo > hi
    {
        row.issues.push(Issue {
            severity: Severity::Warning,
            column: String::new(),
            message: format!("smallest conductor {lo} mm² is larger than the largest {hi} mm²; both are left unknown"),
        });
        d.wire_min = None;
        d.wire_max = None;
    }
    Some(make_part(d, mapping, file, today))
}

fn make_part(d: Draft, mapping: &Mapping, file: &str, today: &str) -> CatalogPart {
    let mut part = CatalogPart::new(d.manufacturer, d.order_number);
    part.type_number = d.type_number;
    part.description = d.description;
    part.letter_codes = d.letter_codes;
    part.keywords = d.keywords;
    part.function = d.function;
    let mut source_keys: Vec<String> = Vec::new();
    match d.size {
        [Some(width_mm), Some(height_mm), Some(depth_mm)] => {
            part.dimensions = Some(Dimensions {
                width_mm,
                height_mm,
                depth_mm,
            });
        }
        sizes => {
            for (key, v) in [keys::WIDTH_MM, keys::HEIGHT_MM, keys::DEPTH_MM]
                .into_iter()
                .zip(sizes)
            {
                if let Some(v) = v {
                    part.data.insert(key.into(), Value::Float(round(v)));
                    source_keys.push(key.into());
                }
            }
        }
    }
    for (key, v) in d.measures {
        let value = if key == eng::POLES {
            Value::Int(v as i64)
        } else {
            Value::Float(round(v))
        };
        part.data.insert(key.into(), value);
        source_keys.push(key.into());
    }
    for (key, v) in d.custom {
        part.data.entry(key).or_insert(v);
    }
    for (key, v) in [
        (lk::WIRE_MIN_MM2, d.wire_min),
        (lk::WIRE_MAX_MM2, d.wire_max),
    ] {
        if let Some(v) = v {
            part.data.insert(key.into(), Value::Float(round(v)));
            source_keys.push(key.into());
        }
    }
    if let Some(n) = d.connection_count {
        part.data
            .insert(lk::CONNECTION_COUNT.into(), Value::Int(n as i64));
    }
    if part.function == "terminal"
        && let Some(n) = d.connection_count.filter(|n| (1..=8).contains(n))
    {
        let wire = d
            .wire_min
            .zip(d.wire_max)
            .map(|(a, b)| (round(a), round(b)));
        part.terminals = (1..=n)
            .map(|i| PartTerminal {
                name: i.to_string(),
                kind: d.connection.unwrap_or_default(),
                wire_mm2: wire,
                side: Side::Unknown,
                position_mm: None,
            })
            .collect();
    }
    part.class.etim = d.etim;
    part.class.eclass = d.eclass;
    part.lifecycle = d.lifecycle;
    if let Some((amount, currency)) = d.price {
        part.price = Some(Price {
            amount: round(amount),
            currency,
            per: 1,
            date: today.to_owned(),
        });
    }
    // The person says what the file is; not saying it leaves the source of
    // every value unstated.
    if mapping.origin != DataSource::Unknown {
        for key in source_keys {
            part.sources.insert(key, mapping.origin);
        }
    }
    part.provenance = Some(Provenance {
        source: "import:table".into(),
        file: file.to_owned(),
        imported: today.to_owned(),
        licence_note: format!("user-supplied table {file}; not redistributable"),
    });
    crate::panel::migrate_properties(&mut part.data);
    part
}

/// Reads a numeric cell in the column's unit. Pushes the reason and returns
/// `None` when the cell is not one number in a unit that fits.
fn number(
    raw: &str,
    column: &crate::tablemap::ColumnMap,
    warn: &mut impl FnMut(Severity, String),
    assumed: &mut BTreeSet<usize>,
    index: usize,
) -> Option<f64> {
    let dim = column.target.dim()?;
    let (text, ambiguous) = normalise_number_text(raw);
    if let Some(note) = ambiguous {
        warn(Severity::Warning, note);
    }
    let Some(q) = parse_quantity(&text) else {
        warn(
            Severity::Warning,
            format!("\"{raw}\" is not a number; the value is left unknown"),
        );
        return None;
    };
    if q.range {
        warn(
            Severity::Warning,
            format!("\"{raw}\" is a range, not one value; the value is left unknown"),
        );
        return None;
    }
    let unit = if q.unit.is_empty() {
        column.unit.to_lowercase().replace('²', "2")
    } else {
        q.unit.clone()
    };
    let Some(factor) = dim.factor(&unit) else {
        warn(
            Severity::Warning,
            format!(
                "the unit {} in \"{raw}\" does not fit {}; the value is left unknown",
                q.unit,
                column.target.label()
            ),
        );
        return None;
    };
    if q.unit.is_empty() && column.unit.is_empty() {
        assumed.insert(index);
    }
    Some(q.low * factor)
}

/// Bounds check after conversion. Zero, negative or beyond `hi` for sizes
/// drops the value; for ratings (`keep_high`) a high value is kept with a
/// warning.
fn plausible(
    v: f64,
    hi: f64,
    what: &str,
    warn: &mut impl FnMut(Severity, String),
    keep_high: bool,
    zero_ok: bool,
) -> bool {
    if !v.is_finite() || v < 0.0 || (v == 0.0 && !zero_ok) {
        warn(
            Severity::Warning,
            format!("{v} is not a possible {what}; the value is left unknown"),
        );
        return false;
    }
    if v > hi {
        if keep_high {
            warn(
                Severity::Warning,
                format!("{v} is unusually high for a {what}; imported, please check the unit"),
            );
            return true;
        }
        warn(
            Severity::Warning,
            format!("{v} is not a possible {what}; the value is left unknown"),
        );
        return false;
    }
    true
}

fn price(raw: &str, warn: &mut impl FnMut(Severity, String)) -> Option<f64> {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | ' ' | '\u{a0}'))
        .collect();
    let (text, ambiguous) = normalise_number_text(cleaned.trim());
    if let Some(note) = ambiguous {
        warn(Severity::Warning, note);
    }
    match parse_quantity(&text) {
        Some(q) if q.low.is_finite() && q.low > 0.0 && !q.range => Some(q.low),
        _ => {
            warn(
                Severity::Warning,
                format!("\"{raw}\" is not a price; the price is left unknown"),
            );
            None
        }
    }
}

/// Makes the leading number of a cell readable by the quantity parser:
/// `1.234,56` and `1,234.56` lose their thousands separator. A single
/// separator followed by three digits (`1,500`) is ambiguous: it is read as
/// a decimal separator and a note says so.
pub(crate) fn normalise_number_text(raw: &str) -> (String, Option<String>) {
    let t = raw.replace(['\u{a0}', '\u{202f}'], " ");
    let t = t.trim();
    let end = t
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | ',' | '+' | '-')))
        .unwrap_or(t.len());
    let (num, rest) = t.split_at(end);
    let dots = num.matches('.').count();
    let commas = num.matches(',').count();
    if dots > 0 && commas > 0 {
        let decimal = if num.rfind('.') > num.rfind(',') {
            '.'
        } else {
            ','
        };
        let thousands = if decimal == '.' { ',' } else { '.' };
        let fixed: String = num
            .chars()
            .filter(|c| *c != thousands)
            .map(|c| if c == decimal { '.' } else { c })
            .collect();
        return (format!("{fixed}{rest}"), None);
    }
    for sep in ['.', ','] {
        let count = num.matches(sep).count();
        if count > 1 {
            // `1.234.567`: thousands separators only.
            let fixed: String = num.chars().filter(|c| *c != sep).collect();
            return (format!("{fixed}{rest}"), None);
        }
        if count == 1
            && let Some((int, frac)) = num.split_once(sep)
            && frac.len() == 3
            && (1..=3).contains(&int.trim_start_matches(['+', '-']).len())
            && int.trim_start_matches(['+', '-']) != "0"
        {
            let value = format!("{int}.{frac}");
            return (
                format!("{value}{rest}"),
                Some(format!(
                    "\"{num}\" is read as {value} (a decimal separator); if it means {int}{frac}, remove the thousands separator in the file"
                )),
            );
        }
    }
    (t.to_owned(), None)
}

/// `1.23457E+11`: what Excel shows for a long number stored as a number.
fn looks_like_excel_exponent(text: &str) -> bool {
    let t = text.trim();
    let Some((mantissa, exp)) = t.split_once(['E', 'e']) else {
        return false;
    };
    let exp = exp.trim_start_matches(['+', '-']);
    !mantissa.is_empty()
        && mantissa
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
        && mantissa.chars().any(|c| c.is_ascii_digit())
        && !exp.is_empty()
        && exp.chars().all(|c| c.is_ascii_digit())
}

fn round(v: f64) -> f64 {
    (v * 1e9).round() / 1e9
}

fn coverage_of(parts: &[CatalogPart]) -> Vec<Coverage> {
    let total = parts.len();
    let count = |f: &dyn Fn(&CatalogPart) -> bool| parts.iter().filter(|p| f(p)).count();
    let has_data = |p: &CatalogPart, key: &str| p.data.contains_key(key);
    let rows: [(&'static str, usize); 9] = [
        ("Description", count(&|p| !p.description.is_empty())),
        (
            "Dimensions (width, height, depth)",
            count(&|p| p.dimensions.is_some()),
        ),
        ("Power loss", count(&|p| has_data(p, eng::POWER_LOSS_W))),
        (
            "Rated current",
            count(&|p| has_data(p, eng::RATED_CURRENT_A)),
        ),
        (
            "Conductor range",
            count(&|p| has_data(p, lk::WIRE_MIN_MM2) && has_data(p, lk::WIRE_MAX_MM2)),
        ),
        ("Terminals", count(&|p| !p.terminals.is_empty())),
        ("ETIM or ECLASS class", count(&|p| !p.class.is_empty())),
        ("Price", count(&|p| p.price.is_some())),
        (
            "Lifecycle status",
            count(&|p| p.lifecycle != Lifecycle::Unknown),
        ),
    ];
    rows.into_iter()
        .map(|(label, have)| Coverage { label, have, total })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_and_decimals_are_told_apart_or_flagged() {
        assert_eq!(normalise_number_text("1.234,56").0, "1234.56");
        assert_eq!(normalise_number_text("1,234.56").0, "1234.56");
        assert_eq!(normalise_number_text("1.234.567").0, "1234567");
        assert_eq!(normalise_number_text("12,5 mm").0, "12,5 mm");
        let (text, note) = normalise_number_text("1,500 W");
        assert_eq!(text, "1.500 W");
        assert!(note.unwrap().contains("thousands"));
        // 0,500 is half, no doubt.
        assert!(normalise_number_text("0,500").1.is_none());
    }

    #[test]
    fn excel_powers_of_ten_are_spotted() {
        assert!(looks_like_excel_exponent("1.23457E+11"));
        assert!(looks_like_excel_exponent("4,0E+12"));
        assert!(!looks_like_excel_exponent("3RT2016-1BB41"));
        assert!(!looks_like_excel_exponent("E+11"));
    }
}
