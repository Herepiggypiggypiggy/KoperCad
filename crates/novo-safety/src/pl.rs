//! Performance levels, PFHd bands and the required PL risk graph.

use std::fmt;

/// Performance level a to e.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PerformanceLevel {
    /// PFHd of 1e-4 or worse: below PL a.
    None,
    A,
    B,
    C,
    D,
    E,
}

impl PerformanceLevel {
    /// PL from PFHd in 1/h (ISO 13849-1 table 3 bands).
    pub fn from_pfhd(pfhd: f64) -> Self {
        if pfhd < 1e-7 {
            Self::E
        } else if pfhd < 1e-6 {
            Self::D
        } else if pfhd < 3e-6 {
            Self::C
        } else if pfhd < 1e-5 {
            Self::B
        } else if pfhd < 1e-4 {
            Self::A
        } else {
            Self::None
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "below a",
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
            Self::E => "e",
        }
    }
}

impl fmt::Display for PerformanceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PL {}", self.label())
    }
}

/// Risk graph severity of injury (S1 slight, S2 serious).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    S1,
    S2,
}

/// Frequency and/or duration of exposure (F1 seldom, F2 frequent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    F1,
    F2,
}

/// Possibility of avoiding the hazard (P1 possible, P2 scarcely possible).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probability {
    P1,
    P2,
}

/// Required PL from the risk graph of ISO 13849-1 annex A.
pub fn required_pl(s: Severity, f: Frequency, p: Probability) -> PerformanceLevel {
    use PerformanceLevel::*;
    match (s, f, p) {
        (Severity::S1, Frequency::F1, Probability::P1) => A,
        (Severity::S1, Frequency::F1, Probability::P2) => B,
        (Severity::S1, Frequency::F2, Probability::P1) => B,
        (Severity::S1, Frequency::F2, Probability::P2) => C,
        (Severity::S2, Frequency::F1, Probability::P1) => C,
        (Severity::S2, Frequency::F1, Probability::P2) => D,
        (Severity::S2, Frequency::F2, Probability::P1) => D,
        (Severity::S2, Frequency::F2, Probability::P2) => E,
    }
}
