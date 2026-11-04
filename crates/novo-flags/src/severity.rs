use serde::{Deserialize, Serialize};

/// The one vocabulary. Red is wrong (cannot be built or run as drawn); yellow is
/// doubtful or unfinished. No flag means nothing to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    Yellow,
    Red,
}

impl Severity {
    /// Red before yellow.
    pub fn rank(self) -> u8 {
        match self {
            Severity::Red => 0,
            Severity::Yellow => 1,
        }
    }
}

/// Result of mapping an engine's severity word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapping {
    Flag(Severity),
    /// Information, not a problem: no triangle.
    NoFlag,
    /// A word this table does not know. The adapter must decide; never guess.
    Unknown,
}

/// Maps the words of the engine vocabularies (design section 3) into [`Severity`].
/// Case-insensitive.
pub fn map_word(word: &str) -> Mapping {
    match word.trim().to_ascii_lowercase().as_str() {
        "error" | "problem" | "blocking" | "red" | "over" => Mapping::Flag(Severity::Red),
        "warning" | "warn" | "major" | "busy" | "yellow" => Mapping::Flag(Severity::Yellow),
        "info" | "note" | "minor" | "idle" | "ok" | "s1" | "s2" => Mapping::NoFlag,
        _ => Mapping::Unknown,
    }
}
