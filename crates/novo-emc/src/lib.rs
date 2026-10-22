//! EMC rule engine (plan: `docs/EMC.md`).
//!
//! A pure engine: it reads a project through queries and returns wire
//! classes, geometric indices, estimates, findings and a risk score per run,
//! device, plate and net. It never edits the project (fixes go through
//! [`project::apply`], one undo step each) and never certifies compliance.
//! Every result carries one of the levels of `docs/EMC.md` section 4
//! ([`Level`]): a rule on what is drawn, a geometric index, a closed-form
//! estimate with its assumptions, or (later) a quasi-static model. What it
//! cannot judge is listed in [`NOT_CHECKED`].
//!
//! * [`analyse`] runs the rules over an [`Input`] built by hand (tests) or
//!   from a project ([`project::input_of`], [`project::analyse_project`]).
//! * The score of a subject is the sum of its findings' points, capped at
//!   100, and every point is listed with its reason in [`Score::reasons`].

pub mod class;
pub mod geometry;
pub mod input;
pub mod picture;
pub mod project;
pub mod report;
pub mod rules;

pub use class::{EmcClass, Shield, Termination, default_distance_mm};
pub use input::{Device, Duct, FilterState, Finish, Input, Origin, Params, Plate, Role, Run};
pub use picture::{DESIGN_AID, Picture, picture};
pub use report::*;
pub use rules::{RULES, Rule, analyse};
