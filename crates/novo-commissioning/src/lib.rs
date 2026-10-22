//! Commissioning (docs/design/commissioning.md): proving the built machine
//! against the drawings.
//!
//! A [`Target`] (wire, terminal or device) carries the [`CheckKind`]s that
//! apply to it. Each check has a [`CheckState`] and, once decided, who and
//! when. A failed check opens a linked [`PunchItem`]. Everything lives in a
//! [`CommissioningStore`], which is not part of the design model: the project
//! file keeps it in side tables (novo-storage), ticks are not undoable design
//! edits, and `novo-core` is untouched.
//!
//! The crate is pure. Callers pass the time (`at`, ISO 8601 text) and the
//! person (`by`), so tests are deterministic. [`progress`] and [`targets`]
//! derive totals from the [`Project`](novo_core::Project) every time; nothing
//! about the design is cached in the store.

mod build;
mod check;
mod field;
mod photo;
mod progress;
mod punch;
mod redline;
mod report;
mod store;
mod walk;

pub use build::{
    BuildPhase, BuildPlan, BuildProgress, BuildStep, BuildTick, CabinetPlan, Next, StepKey,
    StepStatus, TickState, keys as build_keys, mark_done, mark_problem, undo as undo_build,
};
pub use check::{CheckKind, CheckRecord, CheckState, Target};
pub use field::{FieldError, FieldPackage, MergeReport};
pub use photo::{is_photo_name, photo_name};
pub use progress::{
    Progress, Scope, TargetInfo, progress, progress_by_cabinet, progress_by_page, targets,
};
pub use punch::{Photo, PunchItem, PunchState, Severity, Signature};
pub use redline::{Change, Redline, RedlineState, apply_commands, revision_description};
pub use report::to_report_input;
pub use store::{CommissioningError, CommissioningStore};
pub use walk::{
    WalkOrder, WalkScope, WireKind, WirePlan, WireStep, next_open, next_open_with, walk_order,
    walk_order_with,
};
