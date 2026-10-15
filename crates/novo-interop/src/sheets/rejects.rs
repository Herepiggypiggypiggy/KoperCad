//! The rejected rows as a workbook to fix and import again.

use super::grid::{Cell, Row, Table, Workbook};
use super::plan::ImportPlan;

/// Header of the column that says why a row was rejected. The import does
/// not know it and notes it as an ignored column; delete it or leave it.
pub const WHY_HEADER: &str = "Why it was rejected";

/// A workbook of only the rows `plan` rejected, copied from `source` (the
/// workbook the plan was made from), each with a final column giving the
/// reasons. Fix the cells, save, and import it like any export: the
/// baseline is carried over, so conflicts are still found. `None` when
/// nothing was rejected.
pub fn rejected_workbook(plan: &ImportPlan, source: &Workbook) -> Option<Workbook> {
    let mut out = Workbook {
        baseline: source.baseline.clone(),
        meta: source.meta.clone(),
        ..Workbook::default()
    };
    for table in &source.tables {
        let mut kept = Table::new(table.kind, table.headers.clone());
        kept.headers.push(WHY_HEADER.to_owned());
        for rejected in plan.rejected.iter().filter(|r| r.table == table.kind) {
            let Some(row) = table.rows.iter().find(|r| r.line == rejected.line) else {
                continue;
            };
            let mut cells = row.cells.clone();
            cells.resize(table.headers.len(), Cell::Empty);
            cells.push(Cell::text(rejected.reasons.join("; ")));
            kept.rows.push(Row {
                line: kept.rows.len() + 2,
                cells,
            });
        }
        if !kept.rows.is_empty() {
            out.tables.push(kept);
        }
    }
    (!out.tables.is_empty()).then_some(out)
}
