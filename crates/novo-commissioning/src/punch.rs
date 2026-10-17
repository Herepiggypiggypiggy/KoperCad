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

/// A hand signature taken on a tablet: the typed name and, when drawn, the
/// pen strokes (pad units, 0..=1000 in both directions).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub name: String,
    /// When it was signed, ISO 8601 text.
    pub at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strokes: Vec<Vec<[u16; 2]>>,
}

/// A photo taken for a punch item. The picture itself is a file beside the
/// project (`<project> field photos/<name>`); the store keeps only its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Photo {
    /// File name, content-addressed (see [`photo_name`](crate::photo_name)),
    /// so the same picture from two devices is one file.
    pub name: String,
    /// When it was attached, ISO 8601 text.
    pub at: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
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
    /// Pictures of the issue, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub photos: Vec<Photo>,
    /// Who accepted the fix at the machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,
}
