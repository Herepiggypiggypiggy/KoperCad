//! Spreadsheet round trip (WS-24): the device list, terminals, wires and
//! cables out to `.xlsx` or CSV, edited in bulk, and back in with a
//! preview.
//!
//! ```
//! use novo_core::{DeviceTag, Document};
//! use novo_interop::sheets::{export_project, plan_import};
//!
//! let mut doc = Document::default();
//! doc.transact("Add", |tx| tx.add_device(DeviceTag::parse("-K1")?, "Contactor"))?;
//!
//! // Export, edit a cell (here in memory; normally in Excel), import.
//! let mut book = export_project(doc.project());
//! let description = book.tables[0].column("Description").unwrap();
//! book.tables[0].rows[0].cells[description] = novo_interop::sheets::Cell::text("Main contactor");
//! let plan = plan_import(doc.project(), &book);
//! assert_eq!(plan.selected(), 1);
//! plan.apply(&mut doc)?; // one undo step
//! assert_eq!(doc.project().devices().next().unwrap().description, "Main contactor");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! - [`export_project`] builds a [`Workbook`]: one table per kind of
//!   record, every row with a stable `Id`, plus a hidden baseline copy.
//! - [`xlsx_bytes`] / [`read_xlsx`] and [`to_csv`] / [`read_csv`] turn it
//!   into files and back. Pure Rust: no Excel install, no admin rights.
//! - [`plan_import`] previews: changes, conflicts, rejected rows.
//!   [`ImportPlan::apply`] makes the selected changes as one undo step.
//!
//! Greyed columns (Id, Strip, From, To, ...) are shown for orientation and
//! ignored on import. Property columns are `properties.<key>`; a property
//! column the project does not have yet adds the property.

mod create;
mod decode;
mod export;
mod fields;
mod files;
mod grid;
mod plan;
mod rejects;

pub use create::NewDevice;
pub use decode::decode_csv;
pub use export::{export_project, export_table};
pub use fields::PROPERTY_PREFIX;
pub use files::{CsvOptions, FORMAT_VERSION, SheetError, read_csv, read_xlsx, to_csv, xlsx_bytes};
pub use grid::{Cell, Meta, Row, Table, TableKind, Workbook, number_text};
pub use plan::{
    ApplyError, Change, ImportPlan, Rejected, Resolution, plan_import, plan_import_with,
};
pub use rejects::{WHY_HEADER, rejected_workbook};
