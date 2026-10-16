//! Functional safety engine: ISO 13849-1 performance level (PL) calculation.
//!
//! **Calculation aid, not a certification.** Results follow the simplified
//! method of ISO 13849-1 (MTTFd, DCavg, CCF, category) but the PFHd of
//! categories 2, 3 and 4 comes from a closed-form model, not from the
//! standard's Table K.1 (Markov models). It is validated against the anchor
//! values listed in `tests/`, not against the full table. Check safety
//! functions that matter with SISTEMA or the standard itself, and have them
//! assessed by a competent person.

mod components;
mod pl;
mod subsystem;

pub use components::{Component, channel_mttfd, mttfd_from_b10d, operations_per_year};
pub use pl::{Frequency, PerformanceLevel, Probability, Severity, required_pl};
pub use subsystem::{
    Category, Combined, Dc, HOURS_PER_YEAR, Mttfd, Problem, Subsystem, SubsystemResult,
    combine_parallel, combine_series, dc_avg,
};

/// Disclaimer to show wherever a result is displayed.
pub const DISCLAIMER: &str = "Calculation aid only, not a certification. PFHd for categories 2 to 4 is a closed-form estimate, not ISO 13849-1 Table K.1.";
