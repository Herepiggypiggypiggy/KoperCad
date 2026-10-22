//! CSV import for the parts catalog, and the delimited-text parser the bulk
//! editor shares for Excel paste.
//!
//! Catalog exports come from many places (manufacturer price lists, EPLAN
//! part exports via Excel, a colleague's spreadsheet), so the importer is
//! forgiving: it detects the delimiter (`;` from European Excel, `,`, or
//! tab), reads quoted fields, accepts decimal commas and maps headers in
//! English, German and Dutch. Columns it does not know become technical
//! data under their own name, so nothing in the file is lost.

use novo_core::Value;

use crate::catalog::{CatalogPart, Dimensions, split_list};
use crate::sheet::{parse_number, property_key};

/// One row of delimited text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelimitedRow {
    /// The 1-based line in the text where the row starts. A quoted cell
    /// with line breaks makes a row span several lines, and blank lines are
    /// skipped, so this is not the row's index.
    pub line: usize,
    pub fields: Vec<String>,
}

/// Why delimited text could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("the quote that opens a cell on line {line} is never closed")]
    UnterminatedQuote { line: usize },
}

/// Splits delimited text into rows of fields. The delimiter is detected
/// from the first line (tab, `;` or `,`, whichever occurs most outside
/// quotes). Handles quoted fields with doubled quotes and line breaks, a
/// leading byte order mark, and CRLF line ends. Blank lines are skipped. A
/// quote opens a quoted cell only at the start of a cell (`5"` is text). A
/// quoted cell that is never closed is an error rather than a cell holding
/// the rest of the file.
///
/// The text must already be decoded: a file saved by Excel as "CSV" is
/// often Windows-1252, not UTF-8, and the caller converts it first.
pub fn parse_delimited(text: &str) -> Result<Vec<DelimitedRow>, ParseError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let delimiter = detect_delimiter(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut field_start = true;
    let mut line = 1;
    let mut row_line = 1;
    let mut quote_line = 1;
    let mut chars = text.chars().peekable();
    let mut finish_row = |row: &mut Vec<String>, row_line: usize| {
        if row.iter().any(|f| !f.trim().is_empty()) {
            rows.push(DelimitedRow {
                line: row_line,
                fields: std::mem::take(row),
            });
        } else {
            row.clear();
        }
    };
    while let Some(c) = chars.next() {
        let newline = c == '\n' || (c == '\r' && chars.peek() != Some(&'\n'));
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field_start {
            quoted = true;
            quote_line = line;
            field_start = false;
        } else if c == delimiter {
            row.push(std::mem::take(&mut field));
            field_start = true;
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            finish_row(&mut row, row_line);
            field_start = true;
            line += 1;
            row_line = line;
            continue;
        } else {
            field.push(c);
            field_start = false;
        }
        if newline {
            line += 1;
        }
    }
    if quoted {
        return Err(ParseError::UnterminatedQuote { line: quote_line });
    }
    row.push(field);
    finish_row(&mut row, row_line);
    Ok(rows)
}

/// The delimiter the lines of the text agree on. A title line above the
/// table (`Price list 2026, example`) must not decide it, so the first
/// lines vote: each candidate scores the lines that hold its most common
/// non-zero count outside quotes, weighted by that count. Ties go to tab
/// (Excel paste), then `;`.
fn detect_delimiter(text: &str) -> char {
    const CANDIDATES: [char; 3] = ['\t', ';', ','];
    let mut per_line: Vec<[usize; 3]> = Vec::new();
    let mut counts = [0usize; 3];
    let mut quoted = false;
    let mut field_start = true;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                } else {
                    quoted = false;
                }
            }
            continue;
        }
        match c {
            '"' if field_start => {
                quoted = true;
                field_start = false;
            }
            '\n' | '\r' => {
                if counts.iter().any(|n| *n > 0) {
                    per_line.push(counts);
                    if per_line.len() >= 30 {
                        break;
                    }
                }
                counts = [0; 3];
                field_start = true;
            }
            _ => {
                if let Some(i) = CANDIDATES.iter().position(|d| *d == c) {
                    counts[i] += 1;
                    field_start = true;
                } else {
                    field_start = false;
                }
            }
        }
    }
    if counts.iter().any(|n| *n > 0) && per_line.len() < 30 {
        per_line.push(counts);
    }
    let mut best = (',', 0usize);
    for (i, d) in CANDIDATES.iter().enumerate() {
        let mut votes: std::collections::BTreeMap<usize, usize> = Default::default();
        for line in &per_line {
            if line[i] > 0 {
                *votes.entry(line[i]).or_default() += 1;
            }
        }
        // The most common count wins; on a tie the larger count (a wider
        // table beats a one-off title).
        let Some((&count, &lines)) = votes.iter().max_by_key(|(c, n)| (**n, **c)) else {
            continue;
        };
        let score = lines * count;
        if score > best.1 {
            best = (*d, score);
        }
    }
    best.0
}

/// What a catalog column holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Column {
    Manufacturer,
    OrderNumber,
    TypeNumber,
    Description,
    LetterCodes,
    Keywords,
    Width,
    Height,
    Depth,
    /// Technical data stored under this key.
    Data(String),
    /// An empty header; the column is ignored.
    Ignored,
}

/// Header words per column, compared lower case with spaces and
/// punctuation removed.
const HEADERS: &[(&[&str], Column)] = &[
    (
        &[
            "manufacturer",
            "mfr",
            "maker",
            "brand",
            "vendor",
            "hersteller",
            "fabrikant",
            "producent",
            "merk",
        ],
        Column::Manufacturer,
    ),
    (
        &[
            "ordernumber",
            "orderno",
            "ordernr",
            "order",
            "articlenumber",
            "articleno",
            "article",
            "item",
            "itemno",
            "itemnumber",
            "artikelnummer",
            "artikelnr",
            "artikel",
            "bestellnummer",
            "bestellnr",
            "bestelnummer",
            "bestelnr",
            "partnumber",
            "partno",
            "catalognumber",
            "catalogno",
            "catno",
            "mlfb",
            "sku",
        ],
        Column::OrderNumber,
    ),
    (
        &[
            "typenumber",
            "type",
            "typnummer",
            "typenummer",
            "typ",
            "model",
            "typebezeichnung",
        ],
        Column::TypeNumber,
    ),
    (
        &[
            "description",
            "desc",
            "beschreibung",
            "bezeichnung",
            "omschrijving",
            "beschrijving",
            "text",
            "name",
        ],
        Column::Description,
    ),
    (
        &[
            "lettercode",
            "lettercodes",
            "kennbuchstabe",
            "kennbuchstaben",
            "lettercodering",
            "iec81346",
        ],
        Column::LetterCodes,
    ),
    (
        &["keywords", "keyword", "tags", "schlagworte", "trefwoorden"],
        Column::Keywords,
    ),
    (
        &[
            "width",
            "widthmm",
            "breite",
            "breitemm",
            "breedte",
            "breedtemm",
            "b",
        ],
        Column::Width,
    ),
    (
        &[
            "height", "heightmm", "hohe", "hoehe", "höhe", "hohemm", "hoogte", "hoogtemm", "h",
        ],
        Column::Height,
    ),
    (
        &[
            "depth", "depthmm", "tiefe", "tiefemm", "diepte", "dieptemm", "t", "d",
        ],
        Column::Depth,
    ),
];

/// A header as the word tables compare it: lower case, accents folded
/// (`Höhe` is `hohe`, `Anschlüsse` is `anschlusse`, `ß` is `ss`), spaces and
/// punctuation removed.
pub(crate) fn squash_header(header: &str) -> String {
    let mut out = String::new();
    for c in header.chars().flat_map(char::to_lowercase) {
        match c {
            'ä' | 'à' | 'á' | 'â' | 'å' => out.push('a'),
            'ö' | 'ò' | 'ó' | 'ô' => out.push('o'),
            'ü' | 'ù' | 'ú' | 'û' => out.push('u'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'ï' | 'í' | 'ì' | 'î' => out.push('i'),
            'ç' => out.push('c'),
            'ñ' => out.push('n'),
            'ß' => out.push_str("ss"),
            c if c.is_alphanumeric() => out.push(c),
            _ => {}
        }
    }
    out
}

/// Maps one header to its column.
pub fn column_for(header: &str) -> Column {
    let squashed = squash_header(header);
    if squashed.is_empty() {
        return Column::Ignored;
    }
    for (words, column) in HEADERS {
        if words.contains(&squashed.as_str()) {
            return column.clone();
        }
    }
    property_key(header).map_or(Column::Ignored, Column::Data)
}

/// The outcome of a catalog import.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportReport {
    pub parts: Vec<CatalogPart>,
    /// Lines that were skipped (1-based line in the file) and why.
    pub skipped: Vec<(usize, String)>,
    /// Headers that became technical data, for the user to check.
    pub data_columns: Vec<String>,
}

/// Why a file could not be imported at all.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    #[error("the file is empty")]
    Empty,
    #[error(
        "no order number column found; expected a header such as \"Order number\", \"Artikelnummer\" or \"Bestellnummer\""
    )]
    NoOrderNumber,
    #[error("columns {first} and {second} both set the same field; rename or remove one")]
    DuplicateColumn { first: String, second: String },
    #[error(transparent)]
    Text(#[from] ParseError),
}

/// Reads catalog parts from CSV text with a header row. Rows without an
/// order number are skipped and reported; a missing manufacturer falls
/// back to `default_manufacturer` (a price list from one maker often has no
/// manufacturer column).
pub fn import_catalog(text: &str, default_manufacturer: &str) -> Result<ImportReport, ImportError> {
    let rows = parse_delimited(text)?;
    let Some((header, body)) = rows.split_first() else {
        return Err(ImportError::Empty);
    };
    let header = &header.fields;
    let columns: Vec<Column> = header.iter().map(|h| column_for(h)).collect();
    // Two columns for one field (`Text` and `Description`) would make the
    // later one win silently.
    for (i, column) in columns.iter().enumerate() {
        if *column == Column::Ignored {
            continue;
        }
        if let Some(j) = columns[..i].iter().position(|c| c == column) {
            return Err(ImportError::DuplicateColumn {
                first: header[j].trim().to_owned(),
                second: header[i].trim().to_owned(),
            });
        }
    }
    if !columns.contains(&Column::OrderNumber) {
        return Err(ImportError::NoOrderNumber);
    }
    let mut report = ImportReport {
        data_columns: header
            .iter()
            .zip(&columns)
            .filter(|(_, c)| matches!(c, Column::Data(_)))
            .map(|(h, _)| h.trim().to_owned())
            .collect(),
        ..Default::default()
    };
    for record in body {
        let (line, row) = (record.line, &record.fields);
        let mut part = CatalogPart::new(default_manufacturer, "");
        let mut dims = [None::<f64>; 3];
        for (column, raw) in columns.iter().zip(row) {
            let value = raw.trim();
            if value.is_empty() {
                continue;
            }
            match column {
                Column::Manufacturer => part.manufacturer = value.into(),
                Column::OrderNumber => part.order_number = value.into(),
                Column::TypeNumber => part.type_number = value.into(),
                Column::Description => part.description = value.into(),
                Column::LetterCodes => {
                    part.letter_codes = split_list(&value.replace([';', ' '], ","))
                        .into_iter()
                        .map(|c| c.trim_start_matches('-').to_uppercase())
                        .collect();
                }
                Column::Keywords => part.keywords = split_list(&value.replace(';', ",")),
                Column::Width => dims[0] = dimension_mm(value),
                Column::Height => dims[1] = dimension_mm(value),
                Column::Depth => dims[2] = dimension_mm(value),
                Column::Data(key) => {
                    part.data.insert(key.clone(), cell_value(value));
                }
                Column::Ignored => {}
            }
        }
        if part.order_number.is_empty() {
            report.skipped.push((line, "no order number".into()));
            continue;
        }
        if part.manufacturer.is_empty() {
            report.skipped.push((line, "no manufacturer".into()));
            continue;
        }
        match dims {
            [Some(width_mm), Some(height_mm), Some(depth_mm)] => {
                part.dimensions = Some(Dimensions {
                    width_mm,
                    height_mm,
                    depth_mm,
                });
            }
            _ => {
                use crate::catalog::keys;
                for (key, v) in [keys::WIDTH_MM, keys::HEIGHT_MM, keys::DEPTH_MM]
                    .into_iter()
                    .zip(dims)
                {
                    if let Some(v) = v {
                        part.data.insert(key.into(), Value::Float(v));
                    }
                }
            }
        }
        crate::panel::migrate_properties(&mut part.data);
        report.parts.push(part);
    }
    Ok(report)
}

/// A size cell in mm: a plain number, or a number with a length unit
/// (`60 mm`, `6 cm`).
fn dimension_mm(text: &str) -> Option<f64> {
    parse_number(text).or_else(|| {
        let q = crate::recipes::parse_quantity(text)?;
        (!q.range)
            .then(|| {
                crate::recipes::Dim::Length
                    .factor(&q.unit)
                    .map(|f| q.low * f)
            })
            .flatten()
    })
}

/// A spreadsheet cell as a property value: whole numbers, decimal numbers
/// (with a decimal comma too) and text. Codes with leading zeros
/// (`0012`) and order numbers stay text.
pub fn cell_value(text: &str) -> Value {
    let t = text.trim();
    let leading_zero =
        t.len() > 1 && t.starts_with('0') && !t.starts_with("0.") && !t.starts_with("0,");
    if !leading_zero {
        if let Ok(i) = t.parse::<i64>() {
            return Value::Int(i);
        }
        if t.chars().any(|c| c.is_ascii_digit())
            && let Some(f) = parse_number(t)
        {
            return Value::Float(f);
        }
    }
    Value::Text(t.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(text: &str) -> Vec<Vec<String>> {
        parse_delimited(text)
            .unwrap()
            .into_iter()
            .map(|r| r.fields)
            .collect()
    }

    #[test]
    fn a_title_line_with_a_comma_does_not_decide_the_delimiter() {
        let rows = fields("Price list 2026, example\n\nMaker;Order;Price\nA;B-1;1,5\nA;B-2;2,5\n");
        assert_eq!(rows[2], ["A", "B-1", "1,5"]);
        // Two real comma rows still win over one stray semicolon.
        let rows = fields("a,b,c\n1,2;x,3\n4,5,6\n");
        assert_eq!(rows[0], ["a", "b", "c"]);
    }

    #[test]
    fn detects_delimiters_and_quotes() {
        let rows = fields("a;b;\"c;d\"\r\n1;\"say \"\"hi\"\"\";3\n\n");
        assert_eq!(
            rows,
            vec![vec!["a", "b", "c;d"], vec!["1", "say \"hi\"", "3"]]
        );
        let rows = fields("a\tb,c\n1\t2,5");
        assert_eq!(rows, vec![vec!["a", "b,c"], vec!["1", "2,5"]]);
        let rows = fields("\u{feff}a,b\n\"multi\nline\",x");
        assert_eq!(rows, vec![vec!["a", "b"], vec!["multi\nline", "x"]]);
        // An inch mark inside a cell neither opens a quote nor hides the
        // delimiters after it.
        let rows = fields("Size;Order\n5\";A1\n");
        assert_eq!(rows, vec![vec!["Size", "Order"], vec!["5\"", "A1"]]);
    }

    #[test]
    fn rows_know_their_lines() {
        let text = "a;b\r\n\r\n\"two\nlines\";x\ny;z\rlast;row";
        let lines: Vec<usize> = parse_delimited(text)
            .unwrap()
            .iter()
            .map(|r| r.line)
            .collect();
        assert_eq!(lines, vec![1, 3, 5, 6]);
    }

    #[test]
    fn unterminated_quote_is_an_error() {
        assert_eq!(
            parse_delimited("a;b\n1;\"open\n2;3\n"),
            Err(ParseError::UnterminatedQuote { line: 2 })
        );
    }

    #[test]
    fn duplicate_catalog_columns_are_refused() {
        let err = import_catalog("Order number;Text;Description\nA1;x;y", "M").unwrap_err();
        assert!(matches!(err, ImportError::DuplicateColumn { .. }), "{err}");
        let report = import_catalog("Order number;;\nA1;x;y", "M").unwrap();
        assert_eq!(report.parts.len(), 1);
    }

    #[test]
    fn skipped_rows_report_file_lines() {
        let report =
            import_catalog("Order number;Note\nA1;\"two\nlines\"\n\n;orphan", "M").unwrap();
        assert_eq!(report.skipped, vec![(5, "no order number".into())]);
    }

    #[test]
    fn maps_headers_in_three_languages() {
        assert_eq!(column_for("Bestellnummer"), Column::OrderNumber);
        assert_eq!(column_for("Artikelnummer"), Column::OrderNumber);
        assert_eq!(column_for("Order No."), Column::OrderNumber);
        assert_eq!(column_for("Fabrikant"), Column::Manufacturer);
        assert_eq!(column_for("Breite [mm]"), Column::Width);
        assert_eq!(
            column_for("Rated current A"),
            Column::Data("rated_current_a".into())
        );
        assert_eq!(column_for(""), Column::Ignored);
    }

    #[test]
    fn cell_values() {
        assert_eq!(cell_value("9"), Value::Int(9));
        assert_eq!(cell_value("2,5"), Value::Float(2.5));
        assert_eq!(cell_value("0012"), Value::Text("0012".into()));
        assert_eq!(cell_value("24 V DC"), Value::Text("24 V DC".into()));
    }
}
