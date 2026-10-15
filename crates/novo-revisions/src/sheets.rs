//! Which generated sheets changed between two states (DESIGN §8: "a
//! revision marks the generated sheets whose content changed").
//!
//! A generated sheet is not a record, so [`crate::diff`] cannot see it.
//! Instead each sheet's content is boiled down to a hash
//! ([`novo_reports::SheetFingerprint`]) and the two states are compared.
//! A snapshot is a whole project, so the older fingerprints come from
//! running the reports on it; nothing extra is stored in the revision.

use novo_core::{PageId, Project};
use novo_reports::{LiveReports, SheetFingerprint};

/// What happened to one generated sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetChangeKind {
    /// A sheet that was not there (its page is new, or the report grew a
    /// sheet).
    Added,
    /// A sheet that is gone (its page was deleted, or the report shrank).
    Removed,
    /// The sheet shows different content.
    Changed,
}

/// One marked sheet. `name` is the printed name in the newer state, or in
/// the older one for a removed sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetChange {
    pub page: PageId,
    pub name: String,
    pub report: String,
    pub kind: SheetChangeKind,
}

/// Compares two fingerprint lists in page order. Sheets are matched by
/// page and position within that page, so a sheet that was renamed or
/// shifted by an earlier page still lines up. Unchanged sheets are left
/// out; the result is in page order, removed sheets after the pages that
/// remain.
pub fn changed_sheet_fingerprints(
    old: &[SheetFingerprint],
    new: &[SheetFingerprint],
) -> Vec<SheetChange> {
    let old = by_page(old);
    let new = by_page(new);

    let mut changes = Vec::new();
    for (page, now) in &new {
        let before = sheets_of(&old, *page);
        for (i, sheet) in now.iter().enumerate() {
            let kind = match before.get(i) {
                None => SheetChangeKind::Added,
                Some(was) if was.hash != sheet.hash || was.report != sheet.report => {
                    SheetChangeKind::Changed
                }
                Some(_) => continue,
            };
            changes.push(change(sheet, kind));
        }
        for gone in before.iter().skip(now.len()) {
            changes.push(change(gone, SheetChangeKind::Removed));
        }
    }
    for (page, before) in &old {
        if new.iter().all(|(p, _)| p != page) {
            changes.extend(before.iter().map(|s| change(s, SheetChangeKind::Removed)));
        }
    }
    changes
}

/// The generated sheets whose content differs between `old` (an issued
/// snapshot) and `new` (usually the project now).
pub fn changed_sheets(old: &Project, new: &Project) -> Vec<SheetChange> {
    let mut reports = LiveReports::standard();
    let old = reports.sheet_fingerprints(old);
    let new = reports.sheet_fingerprints(new);
    changed_sheet_fingerprints(&old, &new)
}

fn change(sheet: &SheetFingerprint, kind: SheetChangeKind) -> SheetChange {
    SheetChange {
        page: sheet.page,
        name: sheet.name.clone(),
        report: sheet.report.clone(),
        kind,
    }
}

type ByPage<'a> = Vec<(PageId, Vec<&'a SheetFingerprint>)>;

/// The sheets grouped by page, pages in first-seen order.
fn by_page(list: &[SheetFingerprint]) -> ByPage<'_> {
    let mut pages: ByPage<'_> = Vec::new();
    for sheet in list {
        match pages.iter_mut().find(|(page, _)| *page == sheet.page) {
            Some((_, sheets)) => sheets.push(sheet),
            None => pages.push((sheet.page, vec![sheet])),
        }
    }
    pages
}

fn sheets_of<'a>(pages: &ByPage<'a>, page: PageId) -> Vec<&'a SheetFingerprint> {
    pages
        .iter()
        .find(|(p, _)| *p == page)
        .map(|(_, sheets)| sheets.clone())
        .unwrap_or_default()
}
