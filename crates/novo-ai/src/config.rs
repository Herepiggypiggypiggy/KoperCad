//! The person's Claude settings, kept with the other preferences.

use crate::consent::Consent;
use crate::models::DEFAULT_MODEL;
use serde::{Deserialize, Serialize};

/// Everything the assistant needs from the settings screen. Off by default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    /// The feature switch. Off, nothing is sent and no panel is shown.
    pub enabled: bool,
    /// Model id from [`crate::models::MODELS`].
    pub model: String,
    /// `low`, `medium`, `high`: how long the model thinks before answering.
    pub effort: String,
    pub consent: Consent,
    /// Monthly spend limit in USD; 0 is no limit. The assistant stops asking
    /// once this month's estimated cost reaches it.
    pub monthly_cap_usd: f64,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            model: DEFAULT_MODEL.to_string(),
            effort: "medium".to_string(),
            consent: Consent::default(),
            monthly_cap_usd: 5.0,
        }
    }
}

pub const EFFORTS: [&str; 3] = ["low", "medium", "high"];

impl AiSettings {
    /// The effort to send: a stored value that is not on the list becomes `medium`.
    pub fn effort_or_default(&self) -> &str {
        EFFORTS
            .iter()
            .copied()
            .find(|e| *e == self.effort)
            .unwrap_or("medium")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_off_and_shares_nothing_by_default() {
        let s = AiSettings::default();
        assert!(!s.enabled);
        assert_eq!(s.consent.effective(), crate::consent::DataScope::None);
    }

    #[test]
    fn old_settings_files_without_the_section_still_load() {
        let s: AiSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, AiSettings::default());
        let s: AiSettings = serde_json::from_str(r#"{"effort":"turbo"}"#).unwrap();
        assert_eq!(s.effort_or_default(), "medium");
    }
}
