//! `.xlsx` and CSV for a [`Workbook`].
//!
//! Both are pure functions between bytes (or text) and values; the caller
//! owns the disk. Tags are always written as text cells, so Excel never
//! reads `=A1+CAB-K1` as a formula.

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use rust_xlsxwriter::{
    Color, DocProperties, ExcelDateTime, Format, Workbook as XlsxBook, XlsxError,
};

use super::grid::{Cell, Meta, Row, Table, TableKind, Workbook, number_text};

/// Why a file could not be written or read.
#[derive(Debug, thiserror::Error)]
pub enum SheetError {
    #[error("could not write the workbook: {0}")]
    Write(#[from] XlsxError),
    #[error("could not read the workbook: {0}")]
    Read(String),
    #[error("the file is empty")]
    Empty,
    #[error("a quote that opens a cell on line {0} is never closed")]
    UnclosedQuote(usize),
    #[error(
        "this file does not look like a KoperCAD export: no sheet is called Devices, Terminals, Wires or Cables"
    )]
    NoTables,
    #[error(
        "cannot tell which table this CSV is; its columns match none of Devices, Terminals, Wires, Cables"
    )]
    UnknownCsv,
}

/// Increase when the layout of the sheets changes incompatibly.
pub const FORMAT_VERSION: u32 = 1;

const BASE_PREFIX: char = '~';
const META_SHEET: &str = "~Meta";

fn col(index: usize) -> Result<u16, XlsxError> {
    u16::try_from(index).map_err(|_| XlsxError::RowColumnLimitError)
}

fn row_index(index: usize) -> Result<u32, XlsxError> {
    u32::try_from(index).map_err(|_| XlsxError::RowColumnLimitError)
}

fn write_table(
    book: &mut XlsxBook,
    table: &Table,
    name: &str,
    hidden: bool,
) -> Result<(), XlsxError> {
    let bold = Format::new().set_bold();
    let grey = Format::new().set_background_color(Color::RGB(0xF0F0F0));
    let grey_bold = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE0E0E0));
    let sheet = book.add_worksheet();
    sheet.set_name(name)?;
    let locked = |c: usize| table.locked.get(c).copied().unwrap_or(false);
    let mut widths: Vec<usize> = table.headers.iter().map(|h| h.chars().count()).collect();
    for (c, header) in table.headers.iter().enumerate() {
        let format = if locked(c) { &grey_bold } else { &bold };
        sheet.write_string_with_format(0, col(c)?, header, format)?;
    }
    for (r, row) in table.rows.iter().enumerate() {
        let r = row_index(r + 1)?;
        for (c, cell) in row.cells.iter().enumerate() {
            let c16 = col(c)?;
            let grey_cell = locked(c).then_some(&grey);
            match cell {
                Cell::Empty => {
                    if let Some(format) = grey_cell {
                        sheet.write_blank(r, c16, format)?;
                    }
                }
                Cell::Text(text) => {
                    match grey_cell {
                        Some(format) => sheet.write_string_with_format(r, c16, text, format)?,
                        None => sheet.write_string(r, c16, text)?,
                    };
                    widths[c] = widths[c].max(text.chars().count());
                }
                Cell::Number(n) => {
                    match grey_cell {
                        Some(format) => sheet.write_number_with_format(r, c16, *n, format)?,
                        None => sheet.write_number(r, c16, *n)?,
                    };
                    widths[c] = widths[c].max(number_text(*n).chars().count());
                }
            }
        }
    }
    for (c, chars) in widths.into_iter().enumerate() {
        sheet.set_column_width(col(c)?, (chars + 2).clamp(6, 50) as f64)?;
    }
    sheet.set_freeze_panes(1, 0)?;
    if !table.rows.is_empty() && !table.headers.is_empty() {
        sheet.autofilter(
            0,
            0,
            row_index(table.rows.len())?,
            col(table.headers.len() - 1)?,
        )?;
    }
    if hidden {
        sheet.set_hidden(true);
    }
    Ok(())
}

/// The workbook as `.xlsx` bytes: one visible sheet per table, a hidden
/// `~Name` copy of each as baseline, and a hidden `~Meta` sheet. The same
/// workbook always gives the same bytes (the creation date is fixed).
pub fn xlsx_bytes(workbook: &Workbook) -> Result<Vec<u8>, SheetError> {
    let mut book = XlsxBook::new();
    let created = ExcelDateTime::from_ymd(2000, 1, 1)?;
    book.set_properties(&DocProperties::new().set_creation_datetime(&created));
    for table in &workbook.tables {
        write_table(&mut book, table, table.kind.name(), false)?;
    }
    for table in &workbook.baseline {
        write_table(
            &mut book,
            table,
            &format!("{BASE_PREFIX}{}", table.kind.name()),
            true,
        )?;
    }
    let sheet = book.add_worksheet();
    sheet.set_name(META_SHEET)?;
    sheet.write_string(0, 0, "format")?;
    sheet.write_number(0, 1, f64::from(FORMAT_VERSION))?;
    sheet.write_string(1, 0, "project")?;
    sheet.write_string(1, 1, &workbook.meta.project)?;
    if let Some(revision) = workbook.meta.revision {
        sheet.write_string(2, 0, "revision")?;
        sheet.write_number(2, 1, revision as f64)?;
    }
    sheet.set_hidden(true);
    Ok(book.save_to_buffer()?)
}

fn data_cell(data: &Data) -> Cell {
    match data {
        Data::Empty => Cell::Empty,
        Data::String(s) => Cell::Text(s.clone()),
        Data::Int(i) => Cell::Number(*i as f64),
        Data::Float(f) => Cell::Number(*f),
        Data::Bool(b) => Cell::Text(b.to_string()),
        Data::Error(e) => Cell::Text(e.to_string()),
        Data::DateTime(d) => Cell::Number(d.as_f64()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => Cell::Text(s.clone()),
    }
}

/// The used rows of one sheet as rows of cells, padded so column A is
/// index 0 even when the sheet starts further right.
fn sheet_rows(range: &calamine::Range<Data>) -> Vec<Row> {
    let Some((first_row, first_col)) = range.start() else {
        return Vec::new();
    };
    range
        .rows()
        .enumerate()
        .map(|(i, cells)| {
            let mut padded = vec![Cell::Empty; first_col as usize];
            padded.extend(cells.iter().map(data_cell));
            Row {
                line: first_row as usize + i + 1,
                cells: padded,
            }
        })
        .collect()
}

fn table_from(kind: TableKind, rows: Vec<Row>) -> Option<Table> {
    let mut rows = rows.into_iter();
    let header = rows.next()?;
    let mut table = Table::new(kind, header.cells.iter().map(Cell::as_text).collect());
    table.rows = rows.collect();
    Some(table)
}

/// Reads an `.xlsx` workbook. Sheets named like a table are the tables
/// (case does not matter); `~Name` sheets are the baseline; others are
/// listed in [`Workbook::ignored`] and otherwise left alone.
pub fn read_xlsx(bytes: &[u8]) -> Result<Workbook, SheetError> {
    let mut book: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e: calamine::XlsxError| SheetError::Read(e.to_string()))?;
    let mut out = Workbook::default();
    for name in book.sheet_names() {
        let range = book
            .worksheet_range(&name)
            .map_err(|e| SheetError::Read(format!("sheet {name}: {e}")))?;
        let rows = sheet_rows(&range);
        if name == META_SHEET {
            for row in rows {
                let key = row.cells.first().map(Cell::as_text).unwrap_or_default();
                let value = row.cells.get(1).cloned().unwrap_or_default();
                match key.as_str() {
                    "revision" => {
                        if let Cell::Number(n) = value
                            && n >= 0.0
                        {
                            out.meta.revision = Some(n as u64);
                        }
                    }
                    "project" => out.meta.project = value.as_text(),
                    _ => {}
                }
            }
        } else if let Some(base) = name.strip_prefix(BASE_PREFIX) {
            if let Some(kind) = TableKind::from_name(base)
                && let Some(table) = table_from(kind, rows)
            {
                out.baseline.push(table);
            }
        } else if let Some(kind) = TableKind::from_name(&name) {
            if let Some(table) = table_from(kind, rows) {
                out.tables.push(table);
            }
        } else {
            out.ignored.push(name);
        }
    }
    if out.tables.is_empty() {
        return Err(SheetError::NoTables);
    }
    Ok(out)
}

/// How to write CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsvOptions {
    /// `,` by default; `;` is what Excel expects where the decimal mark is
    /// a comma (Dutch, German), and numbers are then written with a comma.
    pub delimiter: char,
    /// Start with a UTF-8 byte order mark so Excel reads `mm²` correctly.
    pub bom: bool,
    /// Put an apostrophe before text starting with `=`, `+`, `-` or `@`
    /// (every tag does), so a spreadsheet does not read it as a formula.
    /// The import removes it again.
    pub excel_safe: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        CsvOptions {
            delimiter: ',',
            bom: true,
            excel_safe: true,
        }
    }
}

fn csv_field(text: &str, delimiter: char) -> String {
    if text.contains([delimiter, '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// One table as CSV (RFC 4180, CRLF line ends). CSV has no baseline: an
/// import from it cannot tell a sheet edit from a later project edit.
pub fn to_csv(table: &Table, options: &CsvOptions) -> String {
    let mut out = String::new();
    if options.bom {
        out.push('\u{feff}');
    }
    let delimiter = options.delimiter.to_string();
    let safe = |t: &str| {
        if options.excel_safe && t.starts_with(['=', '+', '-', '@', '\t', '\r']) {
            format!("'{t}")
        } else {
            t.to_owned()
        }
    };
    let line = |fields: Vec<String>, out: &mut String| {
        let fields: Vec<String> = fields
            .iter()
            .map(|f| csv_field(f, options.delimiter))
            .collect();
        out.push_str(&fields.join(&delimiter));
        out.push_str("\r\n");
    };
    line(table.headers.iter().map(|h| safe(h)).collect(), &mut out);
    for row in &table.rows {
        let fields = row
            .cells
            .iter()
            .map(|cell| match cell {
                Cell::Empty => String::new(),
                Cell::Text(t) => safe(t),
                Cell::Number(n) if options.delimiter == ';' => number_text(*n).replace('.', ","),
                Cell::Number(n) => number_text(*n),
            })
            .collect();
        line(fields, &mut out);
    }
    out
}

/// Which table a CSV holds, from its header.
fn detect_kind(headers: &[String]) -> Option<TableKind> {
    let has = |name: &str| headers.iter().any(|h| h.trim().eq_ignore_ascii_case(name));
    if has("Strip") {
        Some(TableKind::Terminals)
    } else if has("From") && has("To") {
        Some(TableKind::Wires)
    } else if has("Cores") || has("Length [m]") {
        Some(TableKind::Cables)
    } else if has("Tag") {
        Some(TableKind::Devices)
    } else {
        None
    }
}

/// Reads CSV or tab-separated text (delimiter detected, quotes and a byte
/// order mark handled) as a one-table workbook. `kind` says which table it
/// is; `None` works it out from the columns. The text must already be
/// decoded: Excel's "CSV" is often Windows-1252, not UTF-8.
pub fn read_csv(text: &str, kind: Option<TableKind>) -> Result<Workbook, SheetError> {
    let rows = novo_parts::parse_delimited(text).map_err(|e| match e {
        novo_parts::ParseError::UnterminatedQuote { line } => SheetError::UnclosedQuote(line),
    })?;
    let mut rows = rows.into_iter();
    let header = rows.next().ok_or(SheetError::Empty)?;
    let headers: Vec<String> = header.fields.iter().map(|h| h.trim().to_owned()).collect();
    let kind = kind
        .or_else(|| detect_kind(&headers))
        .ok_or(SheetError::UnknownCsv)?;
    let mut table = Table::new(kind, headers);
    for row in rows {
        let cells = row
            .fields
            .iter()
            .map(|f| {
                // Undo the apostrophe `to_csv` put before a tag.
                let t = f
                    .strip_prefix('\'')
                    .filter(|r| r.starts_with(['=', '+', '-', '@']));
                Cell::text(t.unwrap_or(f).trim())
            })
            .collect();
        table.rows.push(Row {
            line: row.line,
            cells,
        });
    }
    Ok(Workbook {
        tables: vec![table],
        baseline: Vec::new(),
        meta: Meta::default(),
        ignored: Vec::new(),
    })
}
