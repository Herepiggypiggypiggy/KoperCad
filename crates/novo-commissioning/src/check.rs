use novo_core::{AnyId, DeviceId, TerminalId, WireId};
use serde::{Deserialize, Serialize};

/// An object that is checked at the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "target", content = "id", rename_all = "snake_case")]
pub enum Target {
    Wire(WireId),
    Terminal(TerminalId),
    /// A device other than a terminal strip (a strip is checked through its terminals).
    Device(DeviceId),
}

impl Target {
    pub fn any_id(self) -> AnyId {
        match self {
            Target::Wire(id) => id.into(),
            Target::Terminal(id) => id.into(),
            Target::Device(id) => id.into(),
        }
    }

    /// The checks that apply to this kind of target, in the order they are asked.
    pub fn checks(self) -> &'static [CheckKind] {
        match self {
            Target::Wire(_) => &[CheckKind::Continuity, CheckKind::Labelled],
            Target::Terminal(_) => &[CheckKind::Torque, CheckKind::Labelled],
            Target::Device(_) => &[CheckKind::Labelled, CheckKind::Tested],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    Continuity,
    Torque,
    Labelled,
    Tested,
}

impl CheckKind {
    pub fn label(self) -> &'static str {
        match self {
            CheckKind::Continuity => "Continuity",
            CheckKind::Torque => "Torque",
            CheckKind::Labelled => "Labelled",
            CheckKind::Tested => "Tested",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    #[default]
    Pending,
    Passed,
    Failed,
    NotApplicable,
}

impl CheckState {
    pub fn label(self) -> &'static str {
        match self {
            CheckState::Pending => "Pending",
            CheckState::Passed => "Passed",
            CheckState::Failed => "Failed",
            CheckState::NotApplicable => "Not applicable",
        }
    }
}

/// One decided check. A check with no record is [`CheckState::Pending`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRecord {
    pub target: Target,
    pub check: CheckKind,
    pub state: CheckState,
    /// Who decided it (free text).
    pub by: String,
    /// When, ISO 8601 text as given by the caller.
    pub at: String,
    /// A measured value, such as `2.5 Nm` or `0.2 ohm`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}
