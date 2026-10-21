//! What project data may leave this computer.
//!
//! The assistant sees nothing of the project unless the person picked a
//! scope and agreed to its description. The scope limits what the tools can
//! return (see [`crate::tools`]); the description below is what the settings
//! screen shows next to the choice, so the words and the behaviour stay in
//! one place.

use serde::{Deserialize, Serialize};

/// How much of the open project the assistant may read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataScope {
    /// Only what the person types: no project data at all.
    #[default]
    None,
    /// Counts, page names and device tags: no part numbers, notes or geometry.
    Outline,
    /// The outline plus whatever is selected right now, in full.
    Selection,
    /// The whole open project: devices, wires, terminals, cabinet, notes.
    Project,
}

impl DataScope {
    pub const ALL: [DataScope; 4] = [Self::None, Self::Outline, Self::Selection, Self::Project];

    pub fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Outline => "outline",
            Self::Selection => "selection",
            Self::Project => "project",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Nothing from the project",
            Self::Outline => "Project outline",
            Self::Selection => "Outline and selection",
            Self::Project => "Whole project",
        }
    }

    /// What is sent, in plain words, shown beside the choice.
    pub fn describes(self) -> &'static str {
        match self {
            Self::None => {
                "Only the messages you type are sent, and any picture of the window you attach \
                 yourself. Claude cannot look at your drawing on its own."
            }
            Self::Outline => {
                "The project name, page names and titles, device tags and counts are sent \
                 when Claude asks for them. No part numbers, notes, wires or positions."
            }
            Self::Selection => {
                "The outline, plus the full details of what you have selected at the moment \
                 Claude asks."
            }
            Self::Project => {
                "Everything in the open project can be sent when Claude asks: devices, \
                 parts, wires, terminals, cabinet layout and notes. Use this only for \
                 projects you are allowed to share with Anthropic."
            }
        }
    }

    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.id() == id)
            .unwrap_or_default()
    }

    /// Whether this scope lets `needed` through.
    pub fn allows(self, needed: DataScope) -> bool {
        self >= needed
    }
}

/// Bump when the wording of what is sent changes, so people agree again.
pub const CONSENT_VERSION: u32 = 1;

/// What the person agreed to.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Consent {
    /// The scope agreed to; the effective scope is never wider than this.
    pub scope: DataScope,
    /// `CONSENT_VERSION` at the time; stale versions count as no consent.
    pub version: u32,
    /// Unix seconds when it was given, for the settings screen.
    pub given_at: u64,
}

impl Consent {
    pub fn give(scope: DataScope, now_unix: u64) -> Self {
        Self {
            scope,
            version: CONSENT_VERSION,
            given_at: now_unix,
        }
    }

    pub fn is_current(&self) -> bool {
        self.version == CONSENT_VERSION
    }

    /// The scope the assistant works under: `None` when consent is missing or stale.
    pub fn effective(&self) -> DataScope {
        if self.is_current() {
            self.scope
        } else {
            DataScope::None
        }
    }
}

/// The standing notice shown before the first message.
pub const NOTICE: &str = "Your messages, and the project data allowed above, are sent over the \
internet to Anthropic's Claude API using your own API key. Anthropic's terms and your account's \
data settings apply to them. Claude can be wrong: check what it says, especially about \
electrical safety, ratings and standards. While this feature is off, the only thing ever sent \
is your key, when you press Check key.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_widen_in_order() {
        assert!(DataScope::Project.allows(DataScope::Selection));
        assert!(DataScope::Selection.allows(DataScope::Outline));
        assert!(!DataScope::Outline.allows(DataScope::Selection));
        assert!(!DataScope::None.allows(DataScope::Outline));
    }

    #[test]
    fn stale_or_missing_consent_means_no_data() {
        assert_eq!(Consent::default().effective(), DataScope::None);
        let mut c = Consent::give(DataScope::Project, 1);
        assert_eq!(c.effective(), DataScope::Project);
        c.version = 0;
        assert_eq!(c.effective(), DataScope::None);
    }

    #[test]
    fn ids_round_trip_and_unknown_ids_are_the_safest_scope() {
        for s in DataScope::ALL {
            assert_eq!(DataScope::from_id(s.id()), s);
        }
        assert_eq!(DataScope::from_id("everything"), DataScope::None);
    }
}
