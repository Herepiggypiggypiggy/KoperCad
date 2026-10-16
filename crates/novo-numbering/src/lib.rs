//! Wire and potential numbering.
//!
//! One engine for every way a wire gets its number: automatic (schemes by
//! page, column, potential, cable or function), manual (typed, then locked),
//! and settling after edits. Numbers live on the model's wires, so the wire
//! list, labels, cut list, cable schedule and terminal screens read them
//! from there and always agree.
//!
//! * [`plan`] computes what a numbering run would do (a preview); apply it
//!   with [`Plan::apply`] inside one transaction and it is one undo step.
//! * [`Mode::Fill`] only numbers what has no number and settles what is
//!   unsettled: an existing unique number is never touched, so a small edit
//!   never renumbers the drawing. [`Mode::Renumber`] starts over.
//! * [`check`] reports duplicates, split and missing numbers.
//! * [`set_number`] types a number on a whole net and locks it.

mod check;
mod group;
mod manual;
mod plan;
mod scheme;

pub use check::{Conflict, ConflictKind, Severity, check};
pub use group::{Group, LOCKED, groups};
pub use manual::{is_locked, set_number, unlock};
pub use plan::{Change, GroupPlan, Mode, Plan, Reason, Scope, number_new_wires, plan};
pub use scheme::{Scheme, SchemeKind};
