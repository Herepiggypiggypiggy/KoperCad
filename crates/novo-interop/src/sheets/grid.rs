//! The neutral table model the spreadsheet round trip reads and writes.
//!
//! A [`Workbook`] is plain data: tables of [`Cell`]s with no file format in
//! it. [`super::files`] turns it into `.xlsx` or CSV and back.

/// One cell. Spreadsheets hand numbers back as numbers (Excel turns a wire
/// number `101` into the number 101), so the import reads both kinds.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Cell {
    #[default]
    Empty,
    Text(String),
    Number(f64),
}

impl Cell {
    pub fn text(text: impl Into<String>) -> Cell {
        let text = text.into();
        if text.is_empty() {
            Cell::Empty
        } else {
            Cell::Text(text)
        }
    }

    /// The cell as typed text: numbers without a trailing `.0`, so a whole
    /// number reads back as `101`, not `101.0`.
    pub fn as_text(&self) -> String {
        match self {
            Cell::Empty => String::new(),
            Cell::Text(t) => t.trim().to_owned(),
            Cell::Number(n) => number_text(*n),
        }
    }

    pub fn is_blank(&self) -> bool {
        self.as_text().is_empty()
    }
}

/// `101.0` as `101`, `2.5` as `2.5`.
pub fn number_text(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// The record kinds that make a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TableKind {
    Devices,
    Terminals,
    Wires,
    Cables,
    /// One row per cable core.
    Cores,
}

impl TableKind {
    pub const ALL: [TableKind; 5] = [
        TableKind::Devices,
        TableKind::Terminals,
        TableKind::Wires,
        TableKind::Cables,
        TableKind::Cores,
    ];

    /// The sheet name in a workbook.
    pub fn name(self) -> &'static str {
        match self {
            TableKind::Devices => "Devices",
            TableKind::Terminals => "Terminals",
            TableKind::Wires => "Wires",
            TableKind::Cables => "Cables",
            TableKind::Cores => "Cores",
        }
    }

    pub fn from_name(name: &str) -> Option<TableKind> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|k| k.name().eq_ignore_ascii_case(name))
    }

    pub(crate) fn noun(self) -> &'static str {
        match self {
            TableKind::Devices => "device",
            TableKind::Terminals => "terminal",
            TableKind::Wires => "wire",
            TableKind::Cables => "cable",
            TableKind::Cores => "cable core",
        }
    }
}

/// One row with the line it came from, for messages.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// 1-based row in the sheet, or line in the CSV file. The header is 1.
    pub line: usize,
    pub cells: Vec<Cell>,
}

/// A header row and the rows below it.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub kind: TableKind,
    pub headers: Vec<String>,
    /// Columns shown for orientation that an import ignores (greyed out in
    /// the workbook). Empty when read from a file.
    pub locked: Vec<bool>,
    pub rows: Vec<Row>,
}

impl Table {
    pub fn new(kind: TableKind, headers: Vec<String>) -> Table {
        Table {
            kind,
            headers,
            locked: Vec::new(),
            rows: Vec::new(),
        }
    }

    pub fn column(&self, header: &str) -> Option<usize> {
        let want = header.trim();
        self.headers
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(want))
    }

    pub fn push(&mut self, cells: Vec<Cell>) {
        let line = self.rows.len() + 2;
        self.rows.push(Row { line, cells });
    }
}

/// What the file says about where it came from.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Meta {
    /// The project revision the export was made from.
    pub revision: Option<u64>,
    pub project: String,
}

/// The tables of an export, and (for `.xlsx`) the baseline: a hidden copy
/// of what each table held when it was exported. The baseline is what lets
/// the import tell "the sheet changed this" from "the project changed this
/// after the export".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Workbook {
    pub tables: Vec<Table>,
    pub baseline: Vec<Table>,
    pub meta: Meta,
    /// Sheets of a read workbook that are not one of the tables.
    pub ignored: Vec<String>,
}

impl Workbook {
    pub fn table(&self, kind: TableKind) -> Option<&Table> {
        self.tables.iter().find(|t| t.kind == kind)
    }

    pub fn baseline_of(&self, kind: TableKind) -> Option<&Table> {
        self.baseline.iter().find(|t| t.kind == kind)
    }
}
