//! Revisions as plain data, shared by `Project.revisions()` /
//! `Project.compare()` in Python and `kopercad-cli revisions`.

use novo_core::{PageId, Project};
use novo_revisions::{ChangeKind, Comparison, FieldChange, Group, SheetChangeKind, group_of};
use serde_json::{Value, json};

/// The lowercase name of a group, as used in the JSON.
pub fn group_name(group: Group) -> &'static str {
    match group {
        Group::Page => "page",
        Group::Device => "device",
        Group::Terminal => "terminal",
        Group::Cable => "cable",
        Group::Wire => "wire",
        Group::Text => "text",
        Group::Drawing => "drawing",
        Group::Part => "part",
        Group::Project => "project",
    }
}

/// `added`, `removed` or `changed`.
pub fn kind_name(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "added",
        ChangeKind::Removed => "removed",
        ChangeKind::Changed { .. } => "changed",
    }
}

/// The page's name in `project`, or its id when the page is gone.
pub fn page_label(project: &Project, id: Option<PageId>) -> Option<String> {
    let id = id?;
    Some(
        project
            .page(id)
            .map_or_else(|| format!("{id}"), |p| p.name.clone()),
    )
}

fn field_json(f: &FieldChange) -> Value {
    json!({ "name": f.name, "old": f.old, "new": f.new, "note": f.note })
}

/// A comparison as data. `project` names the pages (the project as it is
/// now); `from` and `to` are the labels the caller used (`B`, `now`).
pub fn comparison_json(project: &Project, from: &str, to: &str, c: &Comparison) -> Value {
    let rows: Vec<Value> = c
        .rows
        .iter()
        .map(|r| {
            json!({
                "group": group_name(group_of(r.id)),
                "kind": kind_name(&r.kind),
                "label": r.label,
                "page": page_label(project, r.page),
                "id": r.id.to_string(),
                "fields": r.fields.iter().map(field_json).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "from": from,
        "to": to,
        "rows": rows,
        "info": c.info.iter().map(field_json).collect::<Vec<_>>(),
        "sheets": c.sheets.iter().map(|s| json!({
            "page": page_label(project, Some(s.page)),
            "name": s.name,
            "report": s.report,
            "kind": match s.kind {
                SheetChangeKind::Added => "added",
                SheetChangeKind::Removed => "removed",
                SheetChangeKind::Changed => "changed",
            },
        })).collect::<Vec<_>>(),
    })
}

/// The issued revisions, oldest first; `kept` says which have a snapshot
/// to compare with.
pub fn revisions_json(project: &Project, kept: &[novo_core::RevisionId]) -> Value {
    Value::Array(
        project
            .revisions()
            .map(|r| {
                json!({
                    "index": r.index,
                    "date": r.date,
                    "author": r.author,
                    "description": r.description,
                    "snapshot": kept.contains(&r.id),
                })
            })
            .collect(),
    )
}
