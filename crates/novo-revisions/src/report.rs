//! A comparison as a report: the tables a printed change report is made
//! of. The PDF is written by novo-pdf's stand-alone report export, so the
//! report reads the same on paper as the generated pages do on screen.

use novo_core::PageId;
use novo_reports::{Cell, Column, ReportOutput, Section};

use crate::{ChangeKind, Comparison, FieldChange, SheetChangeKind};

/// The report id of a change report.
pub const REPORT_ID: &str = "change_report";

/// The change report for `comparison`: one table of changed objects (page,
/// change, object, what changed), one of changed generated sheets and one
/// of project information, leaving out the tables with no rows.
/// `page_name` gives a page's printed name (its number in the title block).
/// `title` is the first line, such as "Changes B to now".
pub fn change_report(
    comparison: &Comparison,
    title: &str,
    page_name: impl Fn(PageId) -> String,
) -> ReportOutput {
    let mut sections = Vec::new();

    let mut objects = Section::new(
        "changes",
        "Changed objects",
        vec![
            Column::new("page", "Page", 14.0),
            Column::new("change", "Change", 20.0),
            Column::new("object", "Object", 60.0),
            Column::new("details", "What changed", 150.0),
        ],
    );
    for row in &comparison.rows {
        let (change, details) = match &row.kind {
            ChangeKind::Added => ("Added", String::new()),
            ChangeKind::Removed => ("Removed", String::new()),
            ChangeKind::Changed { fields } => (
                "Changed",
                if row.fields.is_empty() {
                    fields.join(", ")
                } else {
                    values(&row.fields)
                },
            ),
        };
        objects.push(
            vec![
                Cell::text(row.page.map(&page_name).unwrap_or_default()),
                Cell::text(change),
                Cell::text(row.label.clone()),
                Cell::text(details),
            ],
            vec![row.id],
        );
    }
    if !objects.rows.is_empty() {
        sections.push(objects);
    }

    let mut sheets = Section::new(
        "sheets",
        "Changed generated sheets",
        vec![
            Column::new("page", "Page", 14.0),
            Column::new("sheet", "Sheet", 80.0),
            Column::new("report", "Report", 80.0),
            Column::new("change", "Change", 20.0),
        ],
    );
    for sheet in &comparison.sheets {
        let change = match sheet.kind {
            SheetChangeKind::Added => "Added",
            SheetChangeKind::Removed => "Removed",
            SheetChangeKind::Changed => "Changed",
        };
        sheets.push(
            vec![
                Cell::text(page_name(sheet.page)),
                Cell::text(sheet.name.clone()),
                Cell::text(sheet.report.clone()),
                Cell::text(change),
            ],
            Vec::new(),
        );
    }
    if !sheets.rows.is_empty() {
        sections.push(sheets);
    }

    let mut info = Section::new(
        "info",
        "Project information",
        vec![
            Column::new("field", "Field", 50.0),
            Column::new("details", "What changed", 160.0),
        ],
    );
    for field in &comparison.info {
        info.push(
            vec![Cell::text(field.name.clone()), Cell::text(value(field))],
            Vec::new(),
        );
    }
    if !info.rows.is_empty() {
        sections.push(info);
    }

    ReportOutput {
        report: REPORT_ID.into(),
        title: title.into(),
        revision: 0,
        sections,
    }
}

/// `name: old → new` for each field, joined with `; `.
fn values(fields: &[FieldChange]) -> String {
    fields
        .iter()
        .map(|f| format!("{}: {}", f.name, value(f)))
        .collect::<Vec<_>>()
        .join("; ")
}

/// A field's change as the person reads it: the note when there is one,
/// else `old → new`.
fn value(field: &FieldChange) -> String {
    if let Some(note) = &field.note {
        return note.clone();
    }
    let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "(none)".to_owned());
    format!("{} → {}", show(&field.old), show(&field.new))
}
