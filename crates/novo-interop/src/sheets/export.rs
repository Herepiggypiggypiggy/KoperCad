//! Project to [`Workbook`].

use novo_core::Project;

use super::fields::{fields, ids, read};
use super::grid::{Meta, Table, TableKind, Workbook};

/// One table of the project: every row, every column.
pub fn export_table(project: &Project, kind: TableKind) -> Table {
    let fields = fields(project, kind);
    let mut table = Table::new(kind, fields.iter().map(|f| f.header.clone()).collect());
    table.locked = fields.iter().map(|f| !f.editable()).collect();
    for id in ids(project, kind) {
        table.push(fields.iter().map(|f| read(project, id, f)).collect());
    }
    table
}

/// The device list, terminals, wires and cables of the project, with a
/// baseline copy of each for the import to compare against. Only tables
/// with rows are included (a project without cables has no Cables sheet).
pub fn export_project(project: &Project) -> Workbook {
    let tables: Vec<Table> = TableKind::ALL
        .into_iter()
        .map(|kind| export_table(project, kind))
        .filter(|t| !t.rows.is_empty())
        .collect();
    Workbook {
        baseline: tables.clone(),
        tables,
        meta: Meta {
            revision: Some(project.revision()),
            project: project.info().name.clone(),
        },
        ignored: Vec::new(),
    }
}
