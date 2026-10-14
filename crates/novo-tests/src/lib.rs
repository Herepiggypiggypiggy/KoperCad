//! Shared fixtures for cross-crate integration tests (owned by the tester
//! thread).
//!
//! The fixtures build the roadmap's gate scenarios through the public
//! `novo-core` API only, the same way the editor, scripts and storage will.
//! Integration tests live in `tests/`; other crates may also use these
//! fixtures as a dev-dependency.

pub mod motor_starter;
pub mod stress;

pub use motor_starter::{MotorStarter, motor_starter};
pub use stress::stress_page;
