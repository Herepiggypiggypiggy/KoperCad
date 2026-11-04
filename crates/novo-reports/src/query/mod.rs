//! A read-only query layer over the project, and saved custom reports.
//!
//! The project is exposed as a few flat tables (`pages`, `devices`,
//! `terminals`, `wires`, `cables`, `cores`, `parts`) that a small subset of
//! SQL can read: `SELECT ... FROM ... WHERE ... GROUP BY ... ORDER BY`.
//! Nothing can change the project through it. The person writes (or builds
//! with [`Builder`]) a query, sees the answer as a report table, and keeps
//! it as a preset that is saved inside the project file and printed as
//! "My reports" ([`SavedQueries`]) in every export.
//!
//! This is our own implementation of the idea; the language is a plain SQL
//! subset and shares no code with any other program.

mod builder;
mod eval;
mod parse;
mod preset;
mod tables;
mod value;

pub use builder::{Builder, Filter, FilterOp};
pub use eval::{MAX_ROWS, QueryResult, ResultColumn, check, run, run_query};
pub use parse::{MAX_DEPTH, MAX_OPERATORS, MAX_QUERY_CHARS, Query, QueryError, parse};
pub use preset::{
    EXAMPLES, MAX_NAME, Preset, Presets, STORE_ADDON, STORE_BLOCK, SavedQueries, to_section,
};
pub use tables::{TABLES, Table, TableColumn, build as build_table, schema};
pub use value::QValue;
