//! The typed table data every report produces.

use std::fmt;

use novo_core::AnyId;
use serde::{Deserialize, Serialize};

/// One value in a report table. Numbers stay numbers, so a spreadsheet can
/// sum a quantity column and a page can right-align it.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Empty,
    Text(String),
    Int(i64),
    /// A measured value, shown in its shortest form (`1.5`, `12`).
    Number(f64),
}

impl Cell {
    pub fn is_empty(&self) -> bool {
        matches!(self, Cell::Empty)
    }

    /// Text, or `Empty` for an empty string.
    pub fn text(text: impl Into<String>) -> Cell {
        let text = text.into();
        if text.is_empty() {
            Cell::Empty
        } else {
            Cell::Text(text)
        }
    }
}

impl fmt::Display for Cell {
    /// The cell as printed on a page or in CSV.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cell::Empty => Ok(()),
            Cell::Text(text) => f.write_str(text),
            Cell::Int(value) => write!(f, "{value}"),
            Cell::Number(value) => write!(f, "{value}"),
        }
    }
}

impl From<&str> for Cell {
    fn from(text: &str) -> Self {
        Cell::text(text)
    }
}
impl From<String> for Cell {
    fn from(text: String) -> Self {
        Cell::text(text)
    }
}
impl From<i64> for Cell {
    fn from(value: i64) -> Self {
        Cell::Int(value)
    }
}
impl From<f64> for Cell {
    fn from(value: f64) -> Self {
        Cell::Number(value)
    }
}
impl<T: Into<Cell>> From<Option<T>> for Cell {
    fn from(value: Option<T>) -> Self {
        value.map_or(Cell::Empty, Into::into)
    }
}

/// Horizontal alignment of a column's cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// A column of a report table.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// Stable machine name (`order_number`), for scripts and templates.
    pub key: String,
    /// Header text printed above the column.
    pub header: String,
    pub align: Align,
    /// Relative width. A page splits its usable width between the columns
    /// in proportion to these, so `10.0` next to `30.0` is a quarter.
    pub width: f64,
}

impl Column {
    pub fn new(key: &str, header: &str, width: f64) -> Self {
        Column {
            key: key.to_owned(),
            header: header.to_owned(),
            align: Align::Left,
            width,
        }
    }

    /// The same column, right-aligned (numbers).
    pub fn right(mut self) -> Self {
        self.align = Align::Right;
        self
    }
}

/// One row of a report table.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// One cell per column, in column order.
    pub cells: Vec<Cell>,
    /// The model objects the row describes, for "jump to" from a generated
    /// page or a spreadsheet hyperlink later. May be empty.
    pub sources: Vec<AnyId>,
}

impl Row {
    pub fn new(cells: Vec<Cell>, sources: Vec<AnyId>) -> Self {
        Row { cells, sources }
    }
}

/// One table of a report. Most reports have one section; a terminal
/// diagram has one per strip, a bill of materials adds "devices without
/// parts". Every section starts on a new page.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// Stable name within the report (`parts`, or a strip's tag).
    pub key: String,
    /// Printed above the table; may be empty.
    pub title: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    /// A printed report may start this section under the previous one on the
    /// same sheet, when at least a few of its rows fit there, instead of on a
    /// new page. Off by default: every section starts a new page.
    pub flow: bool,
    /// The decimal separator of measured values on pages and in CSV: `.`
    /// unless the report language says otherwise. Spreadsheets keep real
    /// numbers and format them by their own locale.
    pub decimal: char,
}

impl Section {
    pub fn new(key: &str, title: &str, columns: Vec<Column>) -> Self {
        Section {
            key: key.to_owned(),
            title: title.to_owned(),
            columns,
            rows: Vec::new(),
            flow: false,
            decimal: '.',
        }
    }

    /// A cell as printed in this section: measured values use the
    /// section's decimal separator.
    pub fn cell_text(&self, cell: &Cell) -> String {
        match cell {
            Cell::Number(_) if self.decimal != '.' => {
                cell.to_string().replace('.', &self.decimal.to_string())
            }
            _ => cell.to_string(),
        }
    }

    /// This section may flow on under the previous one when printed.
    pub fn flowing(mut self) -> Self {
        self.flow = true;
        self
    }

    /// Adds a row, padding or cutting it to the column count.
    pub fn push(&mut self, mut cells: Vec<Cell>, sources: Vec<AnyId>) {
        cells.resize(self.columns.len(), Cell::Empty);
        self.rows.push(Row::new(cells, sources));
    }

    /// The column with this key.
    pub fn column(&self, key: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.key == key)
    }

    /// Every row as display strings, for tests and plain-text views.
    pub fn text_rows(&self) -> Vec<Vec<String>> {
        self.rows
            .iter()
            .map(|r| r.cells.iter().map(ToString::to_string).collect())
            .collect()
    }
}

/// What a report generated from one project revision.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportOutput {
    /// The report's id ([`Report::id`](crate::Report::id)).
    pub report: String,
    pub title: String,
    /// The project revision this output matches: the one it was generated
    /// from, moved forward by [`LiveReports`](crate::LiveReports) across
    /// edits that cannot change it. Equal to `project.revision()` means
    /// current.
    pub revision: u64,
    pub sections: Vec<Section>,
}

impl ReportOutput {
    /// The section with this key.
    pub fn section(&self, key: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.key == key)
    }

    /// Total number of data rows over all sections.
    pub fn row_count(&self) -> usize {
        self.sections.iter().map(|s| s.rows.len()).sum()
    }
}
