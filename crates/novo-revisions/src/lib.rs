//! Revisions (WS-18, DESIGN §7): the index rule that proposes the next
//! revision, and the per-page changes between two project states that
//! Changes mode, the change marks and the page revision are built on.
//!
//! Issuing itself is a model command ([`novo_core::Command::IssueRevision`]);
//! this crate decides what to propose and compares states.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{AnyId, PageId, Project, Record};

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
    pub fn next(self, project: &Project) -> Option<String> {
        match project.latest_revision() {
            None => Some(self.first()),
            Some(latest) => self.after(&latest.index),
        }
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
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty() && self.project.is_empty()
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
    let mut set = ChangeSet::default();
    for id in ids {
        let kind = match (before.get(&id), after.get(&id)) {
            (None, Some(_)) => ChangeKind::Added,
            (Some(_), None) => ChangeKind::Removed,
            (Some(a), Some(b)) if a != b => ChangeKind::Changed {
                fields: changed_fields(a, b),
            },
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

/// The top-level fields of two versions of a record that differ.
fn changed_fields(a: &Record, b: &Record) -> Vec<String> {
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
