//! Problem triangles (docs/design/problem-triangles.md, sections 3 and 6).
//!
//! A [`Flag`] is anything the program says is wrong, doubtful, unfinished or
//! failed. It is drawn as a yellow or red triangle on the thing that causes it.
//! The [`FlagStore`] is the one place that holds all of them: one list, one
//! count, stable ids across re-checks, waivers, roll-ups for tabs and zones,
//! and the order F8 steps through.
//!
//! This crate is pure and depends on `serde` only. Engines do not depend on it;
//! the app has one adapter per engine that builds flags and calls
//! [`FlagStore::replace_source`].

mod anchor;
mod flag;
mod id;
mod severity;
mod store;

pub use anchor::Anchor;
pub use flag::{Action, Detail, Flag, FlagDraft, Life, Source};
pub use id::FlagId;
pub use severity::{Mapping, Severity, map_word};
pub use store::{Area, Counts, FlagStore, RollUp};
