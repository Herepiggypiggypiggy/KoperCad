//! Sample projects and lint for the lessons in `assets/lessons/` (format:
//! docs/design/learning-modules.md). Each topic module builds its lessons'
//! starting and finished projects through the same engine calls the app
//! uses, so a lesson project cannot drift from the program.
//!
//! Regenerate the files after changing a builder:
//! `UPDATE_LESSONS=1 cargo test -p novo-tests --test lessons`

mod cables;
mod common;
mod documents;
mod plc;
mod progression;
mod standard;
mod terminals;
