//! Shared fixtures for cross-crate integration tests (owned by the tester
//! thread).
//!
//! The fixtures build the roadmap's gate scenarios through the public
//! `novo-core` API only, the same way the editor, scripts and storage will.
//! Integration tests live in `tests/`; other crates may also use these
//! fixtures as a dev-dependency.

pub mod big;
pub mod distribution;
pub mod motor_starter;
pub mod stress;

pub use big::{Big, BigSpec, big_project};
pub use distribution::{Board, distribution_board, distribution_board_in, distribution_board_wide};
pub use motor_starter::{
    MotorStarter, motor_starter, motor_starter_in, motor_starter_with_changes,
};
pub use stress::stress_page;
