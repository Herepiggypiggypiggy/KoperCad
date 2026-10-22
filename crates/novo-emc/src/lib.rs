//! EMC rule engine (plan: `docs/EMC.md`).
//!
//! A pure engine: it reads the project through queries and returns classes,
//! geometric indices, estimates and findings. It never edits the project
//! (fixes go through commands) and never certifies compliance; every result
//! says which of the five levels of `docs/EMC.md` section 4 it comes from.
//!
//! This crate is registered empty so that the engine pull request does not
//! touch the shared crate list.
