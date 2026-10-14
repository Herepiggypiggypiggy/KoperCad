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

/// Splits delimited text into rows of fields. The delimiter is detected
/// from the first line (tab, `;` or `,`, whichever occurs most outside
/// quotes). Handles quoted fields with doubled quotes and line breaks, a
/// leading byte order mark, and CRLF line ends. Blank lines are skipped.
pub fn parse_delimited(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let delimiter = detect_delimiter(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
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
        } else if c == '"' && field.is_empty() {
            quoted = true;
        } else if c == delimiter {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            if row.iter().any(|f| !f.trim().is_empty()) {
                rows.push(std::mem::take(&mut row));
            } else {
                row.clear();
            }
        } else {
            field.push(c);
        }
    }
    row.push(field);
    if row.iter().any(|f| !f.trim().is_empty()) {
        rows.push(row);
    }
    rows
}

fn detect_delimiter(text: &str) -> char {
    let mut counts = [('\t', 0usize), (';', 0), (',', 0)];
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            '\n' if !quoted => break,
            _ if !quoted => {
                if let Some(entry) = counts.iter_mut().find(|(d, _)| *d == c) {
                    entry.1 += 1;
                }
            }
            _ => {}
        }
    }
    // Ties go to the earlier entry: tab (Excel paste), then `;`.
    let mut best = (',', 0);
    for (d, n) in counts {
        if n > best.1 {
            best = (d, n);
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

/// Maps one header to its column.
pub fn column_for(header: &str) -> Column {
    let squashed: String = header
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
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
}

/// Reads catalog parts from CSV text with a header row. Rows without an
/// order number are skipped and reported; a missing manufacturer falls
/// back to `default_manufacturer` (a price list from one maker often has no
/// manufacturer column).
pub fn import_catalog(text: &str, default_manufacturer: &str) -> Result<ImportReport, ImportError> {
    let rows = parse_delimited(text);
    let Some((header, body)) = rows.split_first() else {
        return Err(ImportError::Empty);
    };
    let columns: Vec<Column> = header.iter().map(|h| column_for(h)).collect();
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
    for (index, row) in body.iter().enumerate() {
        let line = index + 2;
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
                Column::Width => dims[0] = parse_number(value),
                Column::Height => dims[1] = parse_number(value),
                Column::Depth => dims[2] = parse_number(value),
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
        report.parts.push(part);
    }
    Ok(report)
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

    #[test]
    fn detects_delimiters_and_quotes() {
        let rows = parse_delimited("a;b;\"c;d\"\r\n1;\"say \"\"hi\"\"\";3\n\n");
        assert_eq!(
            rows,
            vec![vec!["a", "b", "c;d"], vec!["1", "say \"hi\"", "3"]]
        );
        let rows = parse_delimited("a\tb,c\n1\t2,5");
        assert_eq!(rows, vec![vec!["a", "b,c"], vec!["1", "2,5"]]);
        let rows = parse_delimited("\u{feff}a,b\n\"multi\nline\",x");
        assert_eq!(rows, vec![vec!["a", "b"], vec!["multi\nline", "x"]]);
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
