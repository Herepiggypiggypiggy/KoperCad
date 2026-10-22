//! Claude as a co-engineer: it works on a draft copy of the project.
//!
//! Every edit Claude proposes is applied at once to a private copy, so the
//! next tool call sees the result (tags it created, pins still open, the
//! design checks) and a circuit can be built in several steps. Nothing
//! reaches the real project. When the turn ends the steps are one change
//! set: the person reads the diff and the checks computed by the program,
//! then applies it as ONE undo step or rejects it.
//!
//! - [`actions`]: the edits, written in tags, page names and `tag:pin`.
//! - [`resolve`]: finding devices, pages, parts and pins by those names.
//! - [`verify`]: design checks, standards report and diff.
//! - [`host`]: the tool host for a turn, and committing a change set.

pub mod actions;
pub mod host;
pub mod resolve;
pub mod snapshot;
pub mod verify;

pub use host::{
    ChangeSetOutcome, EngineerHost, LogEntry, PlanItem, Question, Resources, Step, commit,
    starter_macros,
};
