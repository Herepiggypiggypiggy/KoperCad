use serde::{Deserialize, Serialize};

use crate::check::{CheckKind, Target};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Minor,
    Major,
    Blocking,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PunchState {
    #[default]
    Open,
    Fixed,
    Verified,
    WontFix,
}

impl PunchState {
    /// Fixed items still wait for a re-test, so only these two are finished.
    pub fn is_closed(self) -> bool {
        matches!(self, PunchState::Verified | PunchState::WontFix)
    }
}

/// An open issue found while commissioning, drawn or not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PunchItem {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub note: String,
    pub severity: Severity,
    pub state: PunchState,
    /// What it is about, so a click can jump to it.
    #[serde(default)]
    pub target: Option<Target>,
    /// Set when a failed check opened this item.
    #[serde(default)]
    pub check: Option<CheckKind>,
    pub raised_by: String,
    pub raised_at: String,
    #[serde(default)]
    pub closed_by: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
}
