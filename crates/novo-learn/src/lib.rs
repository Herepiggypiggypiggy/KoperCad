//! Learning modules (docs/design/learning-modules.md): the lesson file
//! format, step text, checks, the step runner, the lesson catalog, the lint
//! and the person's progress.
//!
//! This crate is pure. It knows nothing of egui, the editor or the keymap:
//! the application feeds it [`Event`]s and [`Facts`] and reads back what the
//! card should show. Everything a person reads is data in the lesson files.

pub mod catalog;
pub mod check;
pub mod demo;
pub mod lesson;
pub mod lint;
pub mod markup;
pub mod progress;
pub mod runner;
pub mod ui;

pub use catalog::{Catalog, Entry, Files, ProjectRef, Track, TrackSummary};
pub use check::{Check, Event, Facts};
pub use demo::{Demo, DemoOp};
pub use lesson::{Lang, Lesson, LessonError, Status, Step, Target};
pub use lint::{Finding, Registry, Report, Severity, lint};
pub use markup::{Span, parse_markup};
pub use progress::{Progress, Standing};
pub use runner::{Phase, Run};
pub use ui::UiStrings;
