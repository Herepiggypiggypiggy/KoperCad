//! The documentation designer engine (DOC-1): templates for reports and
//! forms, a table builder over the live report data, the title block and
//! frame as data, templates saved per project and in a user folder, and a
//! renderer that turns a template into the same [`PageLayout`]s the PDF
//! export and the canvas already draw.
//!
//! ```
//! use novo_reports::designer::{builtin, render, Fields, preview_data};
//!
//! let template = builtin::report_a4("Device list", "devices");
//! let data = preview_data(&template, 40);
//! let out = render(&template, &data, &Fields::sample());
//! assert!(out.problems.is_empty());
//! assert!(out.pages.len() >= 2); // 40 sample rows do not fit one page
//! ```
//!
//! # Pieces
//!
//! - [`Template`]: page format, [`Frame`], [`TitleBlock`] and free
//!   [`Element`]s (text, line, rectangle, [`TableElement`]). Everything is
//!   plain data (serde), so the editor UI, scripts and files share it.
//! - [`TableSpec`]: the table builder. It picks a report and section, the
//!   columns (header text, width, alignment), filters and sort order, and
//!   [`build_table`] applies them to a [`ReportOutput`]. Reports stay live;
//!   a template never stores rows.
//! - [`Fields`]: values for `{project.name}`, `{revision}`, `{page}` and the
//!   like in text. Unknown fields stay visible as typed and are listed as
//!   problems; nothing is silently blank.
//! - [`render`]: template + data + fields -> one [`PageLayout`] per page,
//!   plus [`DesignProblem`]s. [`preview_data`] gives sample rows for the live
//!   preview while the editor works without a project.
//! - [`store`]: JSON files for a user folder (`%APPDATA%\KoperCAD\templates`)
//!   and a project-property string, and [`store::resolve`] (project wins).
//! - [`raster`]: a small layout rasteriser for pixel-diff tests of frames.
//! - [`ENTRY`]: where the user finds the designer.
//!
//! Text elements and title block captions carry an optional `key`, the
//! translation key the language layer (DOC-2) looks up; the engine itself
//! prints the text as stored.
//!
//! The PDF side is `novo_pdf::export_layouts_pdf`, which writes the pages
//! this module renders.

pub mod builtin;
mod fields;
pub mod raster;
mod render;
pub mod store;
mod table;
mod template;
mod validate;

pub use fields::{Fields, expand};
pub use render::{Rendered, preview_data, render};
pub use table::{ColumnSpec, Filter, FilterOp, SortKey, TableError, TableSpec, build_table};
pub use template::{
    Element, FORMAT_VERSION, Frame, TableElement, Template, TemplateKind, TitleBlock, TitleRow,
};
pub use validate::{DesignProblem, validate};

/// Where the user finds the designer: the Reports workspace, Designer tab.
/// The editor UI (a later lane) registers this command; the engine exposes
/// the entry so the shell, the palette and the tests agree on its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignerEntry {
    pub command_id: &'static str,
    pub label: &'static str,
    pub workspace: &'static str,
    pub tab: &'static str,
}

/// The Reports workspace > Designer entry.
pub const ENTRY: DesignerEntry = DesignerEntry {
    command_id: "reports.designer",
    label: "Designer",
    workspace: "Reports",
    tab: "Designer",
};
