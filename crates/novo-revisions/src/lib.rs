//! Revisions (WS-18, DESIGN §7): the index rule that proposes the next
//! revision, and the per-page changes between two project states that
//! Changes mode, the change marks and the page revision are built on.
//!
//! Issuing itself is a model command ([`novo_core::Command::IssueRevision`]);
//! this crate decides what to propose and compares states.

mod compare;
mod fields;
mod issue;
mod list;
mod report;
mod restore;
mod sheets;

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{AnyId, PageId, Project, Record, Revision, RevisionId};

pub use compare::{CompareError, Comparison, Group, Side, compare, group_of};
pub use fields::{FieldChange, field_changes};
pub use issue::{
    IssueError, Issued, issue_revision, issue_revision_as, issue_revision_locked,
    issue_revision_noted,
};
pub use list::{ChangeRow, change_list, list_of};
pub use report::{REPORT_ID, change_report};
pub use restore::{RestoreError, RestoreScope, Restored, restore};
pub use sheets::{SheetChange, SheetChangeKind, changed_sheet_fingerprints, changed_sheets};

/// How revisions are indexed, a project rule ("Revisions are indexed
/// [A, B, C, ... ▾]").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndexRule {
    /// `A`, `B`, ... `Z`, then `AA`, `AB`, ...; I and O are skipped so they
    /// never read as 1 and 0.
    #[default]
    Letters,
    /// `0`, `1`, `2`, ...
    Numbers,
    /// `01`, `02`, ... (at least two digits).
    Padded,
}

/// The 24 letters an index uses.
const LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

impl IndexRule {
    /// The first index under this rule.
    pub fn first(self) -> String {
        match self {
            IndexRule::Letters => "A".into(),
            IndexRule::Numbers => "0".into(),
            IndexRule::Padded => "01".into(),
        }
    }

    /// The index after `index`, or `None` when `index` does not follow the
    /// rule (the user typed something else; the proposal is then left
    /// empty for them to fill in).
    pub fn after(self, index: &str) -> Option<String> {
        match self {
            IndexRule::Letters => {
                let digits = index
                    .bytes()
                    .map(|c| LETTERS.iter().position(|&l| l == c))
                    .collect::<Option<Vec<usize>>>()?;
                if digits.is_empty() {
                    return None;
                }
                // Bijective base 24: Z is followed by AA.
                let mut digits = digits;
                let mut i = digits.len();
                loop {
                    if i == 0 {
                        digits.insert(0, 0);
                        break;
                    }
                    i -= 1;
                    if digits[i] + 1 < LETTERS.len() {
                        digits[i] += 1;
                        break;
                    }
                    digits[i] = 0;
                }
                Some(digits.iter().map(|&d| char::from(LETTERS[d])).collect())
            }
            IndexRule::Numbers => {
                if index.is_empty() || !index.bytes().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                Some((index.parse::<u64>().ok()? + 1).to_string())
            }
            IndexRule::Padded => {
                if index.is_empty() || !index.bytes().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                let next = index.parse::<u64>().ok()? + 1;
                Some(format!("{next:0width$}", width = index.len().max(2)))
            }
        }
    }

    /// The index to propose for the next revision of `project`.
    /// Skips indexes already taken (one typed by hand ahead of the rule).
    pub fn next(self, project: &Project) -> Option<String> {
        let mut index = match project.latest_revision() {
            None => self.first(),
            Some(latest) => self.after(&latest.index)?,
        };
        while project.revision_by_index(&index).is_some() {
            index = self.after(&index)?;
        }
        Some(index)
    }
}

/// What happened to one object between two states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    /// The object's fields that differ, by their JSON names.
    Changed {
        fields: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub id: AnyId,
    pub kind: ChangeKind,
}

/// The changes between two project states, by page (DESIGN §7: a change
/// lands on every page it shows on). Changes to objects that are on no
/// page (parts, potentials, structure, rules) are under `project`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeSet {
    pub pages: BTreeMap<PageId, Vec<Change>>,
    pub project: Vec<Change>,
    /// The project information fields that differ (`name`,
    /// `description`, `rules`, `properties`). The name and description
    /// print in every title block.
    pub info: Vec<String>,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty() && self.project.is_empty() && self.info.is_empty()
    }

    /// The pages with at least one change, in id order.
    pub fn changed_pages(&self) -> impl Iterator<Item = PageId> + '_ {
        self.pages.keys().copied()
    }

    /// The changes shown on `page`.
    pub fn on_page(&self, page: PageId) -> &[Change] {
        self.pages.get(&page).map_or(&[], Vec::as_slice)
    }
}

/// Compares two states of a project (an older revision's snapshot and
/// now, or two snapshots). Objects are matched by id.
pub fn diff(old: &Project, new: &Project) -> ChangeSet {
    let before: BTreeMap<AnyId, Record> = old.records().map(|r| (r.id(), r)).collect();
    let after: BTreeMap<AnyId, Record> = new.records().map(|r| (r.id(), r)).collect();
    let ids: BTreeSet<AnyId> = before.keys().chain(after.keys()).copied().collect();
    let mut set = ChangeSet {
        info: json_fields_that_differ(old.info(), new.info()),
        ..ChangeSet::default()
    };
    for id in ids {
        let kind = match (before.get(&id), after.get(&id)) {
            (None, Some(_)) => ChangeKind::Added,
            (Some(_), None) => ChangeKind::Removed,
            (Some(a), Some(b)) if a != b => {
                let mut fields = json_fields_that_differ(a, b);
                // Issuing re-takes every page's freeze; that is bookkeeping
                // of the issue, not a change to the drawing.
                if matches!(id, AnyId::Page(_)) {
                    fields.retain(|f| f != "frozen" && f != "locked");
                    if fields.is_empty() {
                        continue;
                    }
                }
                ChangeKind::Changed { fields }
            }
            _ => continue,
        };
        let mut pages = pages_showing(old, id);
        pages.extend(pages_showing(new, id));
        let change = Change { id, kind };
        if pages.is_empty() {
            set.project.push(change);
        } else {
            for page in pages {
                set.pages.entry(page).or_default().push(change.clone());
            }
        }
    }
    set
}

/// The top-level JSON fields of two values that differ.
fn json_fields_that_differ<T: serde::Serialize>(a: &T, b: &T) -> Vec<String> {
    let (Ok(serde_json::Value::Object(a)), Ok(serde_json::Value::Object(b))) =
        (serde_json::to_value(a), serde_json::to_value(b))
    else {
        return Vec::new();
    };
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    keys.into_iter()
        .filter(|k| a.get(*k) != b.get(*k))
        .cloned()
        .collect()
}

/// The pages an object shows on in one state: its own page, or the pages
/// that draw it (a device's or terminal's symbols, a cable's wires).
fn pages_showing(project: &Project, id: AnyId) -> BTreeSet<PageId> {
    let mut pages = BTreeSet::new();
    if let Some(page) = project.record(id).and_then(|r| r.page()) {
        pages.insert(page);
    }
    match id {
        AnyId::Device(d) => pages.extend(project.symbols_of_device(d).iter().map(|s| s.page)),
        AnyId::Terminal(t) => {
            pages.extend(project.symbols_of_terminal(t).iter().map(|s| s.page));
        }
        AnyId::Cable(c) => {
            if let Some(cable) = project.cable(c) {
                pages.extend(
                    cable
                        .cores
                        .iter()
                        .filter_map(|core| core.wire)
                        .filter_map(|w| project.wire(w))
                        .map(|w| w.page),
                );
            }
        }
        AnyId::BlockInstance(b) => pages.extend(project.block_instance(b).map(|b| b.page)),
        _ => {}
    }
    pages
}

/// The revision each page shows in its title block (DESIGN §7): the latest
/// revision in which the page changed. Worked out from the change sets
/// between consecutive issued snapshots, never stored. A page added since
/// the latest revision has none yet.
///
/// The project revision is [`Project::latest_revision`], and a revision
/// table lists [`Project::revisions`] (index, date, author, description).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PageRevisions {
    pages: BTreeMap<PageId, RevisionId>,
}

impl PageRevisions {
    /// Walks the revisions of `project` in issue order, comparing each
    /// one's snapshot with the one before (the first with an empty
    /// project, so every page it holds gets its index). `snapshot` reads a
    /// kept snapshot; a revision without one is skipped, and its changes
    /// count toward the next revision that has one.
    pub fn compute(
        project: &Project,
        mut snapshot: impl FnMut(RevisionId) -> Option<Project>,
    ) -> PageRevisions {
        let mut pages = BTreeMap::new();
        let mut before = Project::default();
        for revision in project.revisions() {
            let Some(after) = snapshot(revision.id) else {
                continue;
            };
            for page in diff(&before, &after).changed_pages() {
                if after.page(page).is_some() {
                    pages.insert(page, revision.id);
                }
            }
            before = after;
        }
        PageRevisions { pages }
    }

    /// The revision `page` shows, if any.
    pub fn of<'p>(&self, project: &'p Project, page: PageId) -> Option<&'p Revision> {
        project.issued_revision(*self.pages.get(&page)?)
    }
}
