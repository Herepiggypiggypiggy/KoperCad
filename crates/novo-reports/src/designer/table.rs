//! The table builder: which report, which columns, in which order.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::table::{Align, Cell, Column, ReportOutput, Section};

/// One column of a designed table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnSpec {
    /// The source column's key (`quantity`, `order_number`).
    pub key: String,
    /// Header text; `None` keeps the report's own header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    /// Relative width; `None` keeps the report's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
}

impl ColumnSpec {
    pub fn new(key: &str) -> Self {
        ColumnSpec {
            key: key.to_owned(),
            header: None,
            width: None,
            align: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Equals,
    NotEquals,
    Contains,
    /// The cell has text.
    NotEmpty,
    /// The cell is empty.
    Empty,
}

/// Keeps the rows whose cell in `column` passes. Text comparison ignores
/// case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    pub column: String,
    pub op: FilterOp,
    #[serde(default)]
    pub value: String,
}

impl Filter {
    fn keeps(&self, cell: &Cell) -> bool {
        let text = cell.to_string().to_lowercase();
        let value = self.value.to_lowercase();
        match self.op {
            FilterOp::Equals => text == value,
            FilterOp::NotEquals => text != value,
            FilterOp::Contains => text.contains(&value),
            FilterOp::NotEmpty => !text.is_empty(),
            FilterOp::Empty => text.is_empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortKey {
    pub column: String,
    #[serde(default)]
    pub descending: bool,
}

/// A table definition: a view over one section of one report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSpec {
    /// Report id (`bom`, `devices`, `wires`, ...).
    pub report: String,
    /// Section key; `None` takes the report's first section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// Columns in print order; empty takes every column of the section.
    #[serde(default)]
    pub columns: Vec<ColumnSpec>,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sort: Vec<SortKey>,
    pub header_mm: f64,
    pub row_mm: f64,
    pub text_mm: f64,
    /// Heading printed above the table on every page; may use fields.
    #[serde(default)]
    pub heading: String,
}

impl TableSpec {
    /// Every column of the report's first section, default sizes.
    pub fn new(report: &str) -> Self {
        TableSpec {
            report: report.to_owned(),
            section: None,
            columns: Vec::new(),
            filters: Vec::new(),
            sort: Vec::new(),
            header_mm: 8.0,
            row_mm: 6.0,
            text_mm: 2.5,
            heading: String::new(),
        }
    }
}

/// Something the table builder could not resolve; the table is still built
/// from what exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableError {
    MissingReport(String),
    MissingSection { report: String, section: String },
    MissingColumn { report: String, column: String },
}

fn cmp_cells(a: &Cell, b: &Cell) -> Ordering {
    match (a, b) {
        (Cell::Int(x), Cell::Int(y)) => x.cmp(y),
        (Cell::Int(_) | Cell::Number(_), Cell::Int(_) | Cell::Number(_)) => {
            let f = |c: &Cell| match c {
                Cell::Int(i) => *i as f64,
                Cell::Number(n) => *n,
                _ => 0.0,
            };
            f(a).total_cmp(&f(b))
        }
        _ => natural_cmp(&a.to_string(), &b.to_string()),
    }
}

/// Consumes a run of digits, without leading zeros.
fn digits(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut n = String::new();
    while let Some(c) = it.next_if(char::is_ascii_digit) {
        n.push(c);
    }
    n.trim_start_matches('0').to_owned()
}

/// Natural text order: `K2` before `K10`, case-insensitive.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (na, nb) = (digits(&mut a), digits(&mut b));
                let o = na.len().cmp(&nb.len()).then_with(|| na.cmp(&nb));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(x), Some(y)) => {
                let o = x.to_lowercase().cmp(y.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// Builds the table [`TableSpec`] describes from `outputs` (the generated
/// reports). Resolves what it can and lists the rest in the errors: a
/// missing report gives an empty table with the spec's own columns, a
/// missing column is left out.
pub fn build_table(spec: &TableSpec, outputs: &[ReportOutput]) -> (Section, Vec<TableError>) {
    let mut errors = Vec::new();
    let source = outputs.iter().find(|o| o.report == spec.report);
    let section = source.and_then(|o| match &spec.section {
        Some(key) => o.section(key),
        None => o.sections.first(),
    });
    let Some(section) = section else {
        match (source, &spec.section) {
            (None, _) => errors.push(TableError::MissingReport(spec.report.clone())),
            (Some(_), Some(key)) => errors.push(TableError::MissingSection {
                report: spec.report.clone(),
                section: key.clone(),
            }),
            (Some(_), None) => {} // a report with no sections: nothing to print
        }
        let columns = spec
            .columns
            .iter()
            .map(|c| {
                let mut col = Column::new(
                    &c.key,
                    c.header.as_deref().unwrap_or(&c.key),
                    c.width.unwrap_or(10.0),
                );
                col.align = c.align.unwrap_or_default();
                col
            })
            .collect();
        return (Section::new("", "", columns), errors);
    };

    // Pick columns: (index in source, resolved column).
    let picked: Vec<(usize, Column)> = if spec.columns.is_empty() {
        section.columns.iter().cloned().enumerate().collect()
    } else {
        spec.columns
            .iter()
            .filter_map(|c| match section.column(&c.key) {
                Some(i) => {
                    let mut col = section.columns[i].clone();
                    if let Some(h) = &c.header {
                        col.header = h.clone();
                    }
                    if let Some(w) = c.width {
                        col.width = w;
                    }
                    if let Some(a) = c.align {
                        col.align = a;
                    }
                    Some((i, col))
                }
                None => {
                    errors.push(TableError::MissingColumn {
                        report: spec.report.clone(),
                        column: c.key.clone(),
                    });
                    None
                }
            })
            .collect()
    };

    let mut rows: Vec<&crate::table::Row> = section.rows.iter().collect();
    for filter in &spec.filters {
        match section.column(&filter.column) {
            Some(i) => rows.retain(|r| filter.keeps(r.cells.get(i).unwrap_or(&Cell::Empty))),
            None => errors.push(TableError::MissingColumn {
                report: spec.report.clone(),
                column: filter.column.clone(),
            }),
        }
    }
    let sort: Vec<(usize, bool)> = spec
        .sort
        .iter()
        .filter_map(|k| match section.column(&k.column) {
            Some(i) => Some((i, k.descending)),
            None => {
                errors.push(TableError::MissingColumn {
                    report: spec.report.clone(),
                    column: k.column.clone(),
                });
                None
            }
        })
        .collect();
    if !sort.is_empty() {
        rows.sort_by(|a, b| {
            sort.iter()
                .map(|&(i, desc)| {
                    let o = cmp_cells(
                        a.cells.get(i).unwrap_or(&Cell::Empty),
                        b.cells.get(i).unwrap_or(&Cell::Empty),
                    );
                    if desc { o.reverse() } else { o }
                })
                .find(|o| *o != Ordering::Equal)
                .unwrap_or(Ordering::Equal)
        });
    }

    let mut out = Section::new(
        &section.key,
        &section.title,
        picked.iter().map(|(_, c)| c.clone()).collect(),
    );
    for row in rows {
        let cells = picked
            .iter()
            .map(|(i, _)| row.cells.get(*i).cloned().unwrap_or(Cell::Empty))
            .collect();
        out.push(cells, row.sources.clone());
    }
    (out, errors)
}
