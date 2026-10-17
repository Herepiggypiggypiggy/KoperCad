use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::check::{CheckKind, CheckRecord, CheckState, Target};
use crate::punch::{PunchItem, PunchState, Severity};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommissioningError {
    #[error("{check:?} does not apply to {target:?}")]
    NotApplicable { target: Target, check: CheckKind },
    #[error("the commissioning data is unreadable: {0}")]
    Unreadable(String),
    #[error("no punch item {0}")]
    NoPunchItem(u64),
}

/// All commissioning state of one project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoreData", into = "StoreData")]
pub struct CommissioningStore {
    checks: BTreeMap<(Target, CheckKind), CheckRecord>,
    punch: BTreeMap<u64, PunchItem>,
    next_punch: u64,
}

#[derive(Serialize, Deserialize)]
struct StoreData {
    #[serde(default)]
    checks: Vec<CheckRecord>,
    #[serde(default)]
    punch: Vec<PunchItem>,
}

impl From<StoreData> for CommissioningStore {
    fn from(data: StoreData) -> Self {
        let next_punch = data.punch.iter().map(|p| p.id).max().map_or(1, |m| m + 1);
        CommissioningStore {
            checks: data
                .checks
                .into_iter()
                .map(|r| ((r.target, r.check), r))
                .collect(),
            punch: data.punch.into_iter().map(|p| (p.id, p)).collect(),
            next_punch,
        }
    }
}

impl From<CommissioningStore> for StoreData {
    fn from(store: CommissioningStore) -> Self {
        StoreData {
            checks: store.checks.into_values().collect(),
            punch: store.punch.into_values().collect(),
        }
    }
}

impl CommissioningStore {
    pub fn new() -> Self {
        CommissioningStore {
            next_punch: 1,
            ..Default::default()
        }
    }

    /// The store as the JSON document kept in the project file.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("the store is plain data")
    }

    /// Reads a document written by [`CommissioningStore::to_json`].
    pub fn from_json(json: &str) -> Result<Self, CommissioningError> {
        serde_json::from_str(json).map_err(|e| CommissioningError::Unreadable(e.to_string()))
    }

    pub fn is_empty(&self) -> bool {
        self.checks.is_empty() && self.punch.is_empty()
    }

    /// The state of one check; `Pending` when nothing was decided.
    pub fn state(&self, target: Target, check: CheckKind) -> CheckState {
        self.record(target, check)
            .map_or(CheckState::Pending, |r| r.state)
    }

    pub fn record(&self, target: Target, check: CheckKind) -> Option<&CheckRecord> {
        self.checks.get(&(target, check))
    }

    pub fn records(&self) -> impl Iterator<Item = &CheckRecord> {
        self.checks.values()
    }

    /// Decides a check. A failure opens a linked punch item (Major) unless an
    /// unfinished one for this check exists; returns its id when it did.
    pub fn set_check(&mut self, record: CheckRecord) -> Result<Option<u64>, CommissioningError> {
        let (target, check) = (record.target, record.check);
        if !target.checks().contains(&check) {
            return Err(CommissioningError::NotApplicable { target, check });
        }
        let failed = record.state == CheckState::Failed;
        let (by, at, note) = (record.by.clone(), record.at.clone(), record.note.clone());
        self.checks.insert((target, check), record);
        if !failed || self.linked_open(target, check).is_some() {
            return Ok(None);
        }
        let id = self.add_punch(PunchItem {
            id: 0,
            title: format!("{} failed", check.label()),
            note,
            severity: Severity::Major,
            state: PunchState::Open,
            target: Some(target),
            check: Some(check),
            raised_by: by,
            raised_at: at,
            closed_by: None,
            closed_at: None,
            signature: None,
        });
        Ok(Some(id))
    }

    /// Drops the decisions about objects the project no longer has. Punch
    /// items stay, with their link, so a report can still list them.
    pub fn prune(&mut self, project: &novo_core::Project) {
        self.checks.retain(|(target, _), _| match target {
            Target::Wire(id) => project.wire(*id).is_some(),
            Target::Terminal(id) => project.terminal(*id).is_some(),
            Target::Device(id) => project.device(*id).is_some(),
        });
    }

    /// Stores a record as it is, with no punch item opened (merging field results).
    pub(crate) fn put_record(&mut self, record: CheckRecord) {
        self.checks.insert((record.target, record.check), record);
    }

    /// Replaces the item with the same id.
    pub(crate) fn put_punch(&mut self, item: PunchItem) {
        self.punch.insert(item.id, item);
    }

    /// Forgets a decision, back to `Pending`.
    pub fn clear_check(&mut self, target: Target, check: CheckKind) {
        self.checks.remove(&(target, check));
    }

    /// Adds an item and returns its new id (the id in `item` is ignored).
    pub fn add_punch(&mut self, mut item: PunchItem) -> u64 {
        self.next_punch = self.next_punch.max(1);
        let id = self.next_punch;
        item.id = id;
        self.next_punch += 1;
        self.punch.insert(id, item);
        id
    }

    pub fn punch(&self, id: u64) -> Option<&PunchItem> {
        self.punch.get(&id)
    }

    /// Every item, oldest first.
    pub fn punch_items(&self) -> impl Iterator<Item = &PunchItem> {
        self.punch.values()
    }

    /// Items not yet verified or waived.
    pub fn open_punch_count(&self) -> usize {
        self.punch.values().filter(|p| !p.state.is_closed()).count()
    }

    /// Moves an item along. Deciding a check as passed does not close its
    /// linked item: the person verifies it. Reopening a `Fixed` item does
    /// not bring the failure back; the check stays `Pending` until decided
    /// again. `Fixed` puts the linked failed check back to
    /// `Pending` so it is tested again; `Verified` and `WontFix` record who
    /// closed it and when.
    pub fn set_punch_state(
        &mut self,
        id: u64,
        state: PunchState,
        by: &str,
        at: &str,
    ) -> Result<(), CommissioningError> {
        let item = self
            .punch
            .get_mut(&id)
            .ok_or(CommissioningError::NoPunchItem(id))?;
        item.state = state;
        if state.is_closed() {
            item.closed_by = Some(by.to_owned());
            item.closed_at = Some(at.to_owned());
        } else {
            item.closed_by = None;
            item.closed_at = None;
        }
        let link = item.target.zip(item.check);
        if state == PunchState::Fixed
            && let Some((target, check)) = link
            && self.state(target, check) == CheckState::Failed
        {
            self.checks.remove(&(target, check));
        }
        Ok(())
    }

    fn linked_open(&self, target: Target, check: CheckKind) -> Option<u64> {
        self.punch
            .values()
            .find(|p| p.target == Some(target) && p.check == Some(check) && !p.state.is_closed())
            .map(|p| p.id)
    }
}
