//! The changes between two states as one flat, readable list: what the
//! Changes list in the app jumps through and what a printed change report
//! lists. Each row names the object in words (`Device -K1`, `Wire 12`) as
//! it reads in the newer state, or in the older one for a removed object.

use novo_core::{AnyId, PageId, Project, SymbolRole};

use crate::{Change, ChangeKind, ChangeSet, diff};

/// One row of the change list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRow {
    /// The page the row is on; `None` for changes on no page (parts,
    /// potentials, structure, rules).
    pub page: Option<PageId>,
    /// The object, to select and reveal it.
    pub id: AnyId,
    /// What the object is: `Device -K1`, `Wire 12`, `Text "Pump 1"`.
    pub label: String,
    pub kind: ChangeKind,
}

/// The change list between `old` and `new`: pages in page order (pages
/// that exist only in `old` after them), then the changes on no page, and
/// within a page by label. A change that shows on several pages (a device
/// drawn on two) has a row on each.
pub fn change_list(old: &Project, new: &Project) -> Vec<ChangeRow> {
    list_of(&diff(old, new), old, new)
}

/// [`change_list`] for a comparison already made.
pub fn list_of(set: &ChangeSet, old: &Project, new: &Project) -> Vec<ChangeRow> {
    let mut pages: Vec<PageId> = new
        .pages_in_order()
        .into_iter()
        .map(|p| p.id)
        .filter(|id| set.pages.contains_key(id))
        .collect();
    let gone: Vec<PageId> = set
        .pages
        .keys()
        .filter(|id| !pages.contains(id))
        .copied()
        .collect();
    pages.extend(gone);

    let mut rows = Vec::new();
    for page in pages {
        rows.extend(rows_of(set.on_page(page), Some(page), old, new));
    }
    rows.extend(rows_of(&set.project, None, old, new));
    rows
}

fn rows_of(
    changes: &[Change],
    page: Option<PageId>,
    old: &Project,
    new: &Project,
) -> Vec<ChangeRow> {
    let mut rows: Vec<ChangeRow> = changes
        .iter()
        .map(|c| ChangeRow {
            page,
            id: c.id,
            label: label(c, old, new),
            kind: c.kind.clone(),
        })
        .collect();
    rows.sort_by(|a, b| a.label.cmp(&b.label));
    rows
}

fn label(change: &Change, old: &Project, new: &Project) -> String {
    let project = if matches!(change.kind, ChangeKind::Removed) {
        old
    } else {
        new
    };
    name_of(project, change.id).unwrap_or_else(|| "Object".to_owned())
}

/// A short name for an object, from the state it exists in.
fn name_of(project: &Project, id: AnyId) -> Option<String> {
    Some(match id {
        AnyId::Page(p) => {
            let p = project.page(p)?;
            format!("Page {}", p.name)
        }
        AnyId::Device(d) => format!("Device {}", project.device(d)?.tag),
        AnyId::Symbol(s) => {
            let s = project.symbol(s)?;
            match &s.role {
                SymbolRole::Device { device, .. } => {
                    format!("Symbol of {}", project.device(*device)?.tag)
                }
                _ => format!("Symbol {}", s.symbol.0),
            }
        }
        AnyId::Wire(w) => match project.wire(w)?.number.as_deref() {
            Some(n) if !n.is_empty() => format!("Wire {n}"),
            _ => "Wire".to_owned(),
        },
        AnyId::Terminal(t) => {
            let t = project.terminal(t)?;
            let strip = project.device(t.strip).map(|d| d.tag.to_string());
            format!("Terminal {}:{}", strip.unwrap_or_default(), t.name)
        }
        AnyId::Cable(c) => format!("Cable {}", project.cable(c)?.tag),
        AnyId::Part(p) => {
            let p = project.part(p)?;
            let number = if p.type_number.is_empty() {
                &p.order_number
            } else {
                &p.type_number
            };
            format!("Part {} {number}", p.manufacturer)
                .trim()
                .to_owned()
        }
        AnyId::Potential(p) => format!("Potential {}", project.potential(p)?.name),
        AnyId::Text(t) => match &project.text(t)?.content {
            novo_core::TextContent::Literal { text } => format!("Text \"{text}\""),
            _ => "Text".to_owned(),
        },
        AnyId::Junction(_) => "Junction".to_owned(),
        _ => return None,
    })
}
