//! Reads a parts table from a file: CSV, TSV and text exports, and Excel
//! `.xlsx` workbooks, into plain rows of text for the import mapping.
//!
//! Every cell becomes text exactly as the person sees it, so the mapping
//! step reads one thing whatever the file type was. A number cell is written
//! with the shortest text that reads back the same (`24`, `0.5`), never with
//! a made-up unit or format.

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};

use crate::csv::{ParseError, parse_delimited};
use crate::import::decode_text;

/// A table file: one sheet for a text file, one per worksheet in a workbook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFile {
    /// File name, for the part's provenance and messages.
    pub name: String,
    pub sheets: Vec<Sheet>,
}

/// One sheet of rows. Rows are not padded to the same length.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
    /// The row number each of `rows` has in the file (1-based, as the
    /// spreadsheet shows it). Blank rows are not in `rows`, so this is not
    /// just the index plus one. Empty means the index plus one.
    pub lines: Vec<usize>,
}

impl Sheet {
    /// The row number of `rows[index]` in the file.
    pub fn line_of(&self, index: usize) -> usize {
        self.lines.get(index).copied().unwrap_or(index + 1)
    }
}

/// Why a table file could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TableError {
    #[error("the file is empty")]
    Empty,
    #[error("{0}")]
    Text(#[from] ParseError),
    #[error("the workbook could not be read: {0}")]
    Workbook(String),
    #[error(
        "{0} is an old Excel format; open it in Excel and save it as .xlsx or CSV, then import that"
    )]
    OldExcel(String),
    #[error("{0} is not a table file; use CSV, TSV, TXT or XLSX")]
    Unsupported(String),
}

/// File extensions the table reader accepts.
pub const TABLE_EXTENSIONS: &[&str] = &["csv", "tsv", "txt", "xlsx"];

impl TableFile {
    /// Reads `bytes` as the file type `name` says. Text files are UTF-8 or
    /// the Windows-1252 that German and Dutch Excel writes; the delimiter
    /// (`;`, `,`, tab) is detected.
    pub fn read(name: &str, bytes: &[u8]) -> Result<TableFile, TableError> {
        let ext = name
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        let sheets = match ext.as_str() {
            "csv" | "tsv" | "txt" => vec![read_text(bytes)?],
            "xlsx" | "xlsm" => read_workbook(bytes)?,
            "xls" | "xlsb" | "ods" => return Err(TableError::OldExcel(name.to_owned())),
            _ => return Err(TableError::Unsupported(name.to_owned())),
        };
        if sheets.iter().all(|s| s.rows.is_empty()) {
            return Err(TableError::Empty);
        }
        Ok(TableFile {
            name: name.to_owned(),
            sheets,
        })
    }

    /// The first sheet that has rows.
    pub fn first_sheet(&self) -> usize {
        self.sheets
            .iter()
            .position(|s| !s.rows.is_empty())
            .unwrap_or(0)
    }
}

fn read_text(bytes: &[u8]) -> Result<Sheet, TableError> {
    let text = decode_text(bytes);
    let parsed = parse_delimited(&text)?;
    let lines = parsed.iter().map(|r| r.line).collect();
    let rows = parsed
        .into_iter()
        .map(|r| r.fields.into_iter().map(|f| f.trim().to_owned()).collect())
        .collect();
    Ok(Sheet {
        name: String::new(),
        rows,
        lines,
    })
}

fn read_workbook(bytes: &[u8]) -> Result<Vec<Sheet>, TableError> {
    let mut book: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e: calamine::XlsxError| TableError::Workbook(e.to_string()))?;
    let mut sheets = Vec::new();
    for name in book.sheet_names() {
        let range = book
            .worksheet_range(&name)
            .map_err(|e| TableError::Workbook(format!("sheet {name}: {e}")))?;
        let Some((first_row, first_col)) = range.start() else {
            sheets.push(Sheet {
                name,
                ..Sheet::default()
            });
            continue;
        };
        // Column A is index 0 even when the used range starts further right.
        let (mut rows, mut lines) = (Vec::new(), Vec::new());
        for (i, cells) in range.rows().enumerate() {
            let mut row = vec![String::new(); first_col as usize];
            row.extend(cells.iter().map(cell_text));
            while row.last().is_some_and(String::is_empty) {
                row.pop();
            }
            if !row.is_empty() {
                rows.push(row);
                lines.push(first_row as usize + i + 1);
            }
        }
        sheets.push(Sheet { name, rows, lines });
    }
    Ok(sheets)
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty | Data::Error(_) => String::new(),
        Data::String(s) => s.trim().to_owned(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => float_text(*f),
        Data::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

fn float_text(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{f:.0}")
    } else {
        f.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_semicolon_csv_in_windows_1252_reads_with_its_accents() {
        // "Größe;Artikel" with ö and ß as Windows-1252 bytes.
        let bytes = b"Gr\xf6\xdfe;Artikel\n12,5;ABC\n";
        let file = TableFile::read("teile.csv", bytes).unwrap();
        let rows = &file.sheets[0].rows;
        assert_eq!(rows[0], ["Größe", "Artikel"]);
        assert_eq!(rows[1], ["12,5", "ABC"]);
    }

    #[test]
    fn an_empty_file_and_unknown_types_are_refused_in_words() {
        assert_eq!(TableFile::read("a.csv", b""), Err(TableError::Empty));
        assert!(matches!(
            TableFile::read("a.xls", b"x"),
            Err(TableError::OldExcel(_))
        ));
        assert!(matches!(
            TableFile::read("a.pdf", b"x"),
            Err(TableError::Unsupported(_))
        ));
    }

    #[test]
    fn numbers_read_as_the_text_a_person_sees() {
        assert_eq!(float_text(24.0), "24");
        assert_eq!(float_text(0.5), "0.5");
        assert_eq!(float_text(2.5), "2.5");
    }
}
