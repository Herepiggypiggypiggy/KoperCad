//! Generated pages and reports (WS-13): bill of materials, device list,
//! wire list, table of contents, and the framework other generated pages
//! (terminal diagrams, cable diagrams) plug into.
//!
//! Reports are live views of the model, never documents to "update": a
//! [`Report`] is a pure function from a [`Project`] to typed tables
//! ([`ReportOutput`]: sections of rows of [`Cell`]s), and [`LiveReports`]
//! keeps every report current by checking each edit's
//! [`ChangeSet`](novo_core::ChangeSet) against what the report reads.
//!
//! ```
//! use novo_core::Document;
//! use novo_reports::{LiveReports, PageSetup, layout_page, paginate};
//!
//! let mut doc = Document::default();
//! let mut reports = LiveReports::standard();
//! let (_, changes) = doc.transact("Add page", |tx| tx.add_page("1"))?;
//! reports.notify(doc.project(), &changes);
//! let toc = reports.get(doc.project(), "toc").expect("built in");
//! assert_eq!(toc.sections[0].text_rows()[0][0], "1");
//!
//! let setup = PageSetup::default();
//! let pages = paginate(toc, &setup);
//! let layout = layout_page(toc, &pages[0], &setup); // text boxes in mm
//! assert!(layout.texts.iter().any(|t| t.text == "Page"));
//! # Ok::<(), novo_core::ModelError>(())
//! ```
//!
//! # Pieces
//!
//! - **Model**: [`ReportOutput`] → [`Section`] (one table, each starting a
//!   new page) → [`Row`] (cells plus the model objects it describes) →
//!   [`Cell`] (`Text`, `Int`, `Number`, `Empty`; numbers stay numbers for
//!   Excel). [`Column`] carries the header, alignment and a relative width.
//! - **Reports**: [`TableOfContents`] (`toc`), [`BillOfMaterials`] (`bom`),
//!   [`DeviceList`] (`devices`), [`WireList`] (`wires`), [`TerminalDiagram`]
//!   (`terminals`, from novo-terminals) and [`CableDiagram`] (`cables`, from
//!   novo-cables). Ids double as the `generator` of a
//!   [`PageKind::Generated`](novo_core::PageKind) page.
//! - **Generated pages**: a generated page record prints as one sheet named
//!   like the page (`20`), or as `20a`, `20b`, ... when its report needs
//!   more, skipping `i`, `l` and `o` ([`LiveReports::sheets_of_page`],
//!   [`LiveReports::layout_sheet`], [`sheet_names`]).
//! - **Live cache**: [`LiveReports`]. Call [`LiveReports::notify`] with every
//!   transaction's change set; [`LiveReports::get`] regenerates only stale
//!   reports. On the 10k-object stress project an edit plus regenerating and
//!   paging every report takes well under a second (`tests/live.rs`).
//! - **Paging**: [`paginate`] splits a report to the usable area of a
//!   [`PageSetup`] (header row repeated on every page), [`layout_page`]
//!   turns one page into [`TextBox`]es and [`Line`]s in mm.
//! - **Export**: [`xlsx_bytes`] / [`save_xlsx`] (one sheet per report) and
//!   [`to_csv`].
//!
//! # Plugging in a generated page
//!
//! Implement [`Report`] for the page type, returning one [`Section`] per
//! independent table (key and title = the strip's or cable's tag), name in
//! [`Report::dependencies`] every record kind it reads, and register it:
//! `reports.register(Box::new(MyReport))`. It is then paged, laid out,
//! exported to Excel and kept live like the built-in reports.
//! [`TerminalDiagram`] and [`CableDiagram`] are written this way over the
//! data WS-11 and WS-12 already compute (`novo_terminals::screen`,
//! `novo_cables::cable_screens`), so the drawers and the printed pages show
//! the same rows. Those two crates must never depend on `novo-reports`.
//!
//! # Drawing (WS-03 canvas, WS-09 PDF)
//!
//! This crate draws nothing. [`layout_page`] gives positioned text boxes
//! and table rules in mm on the sheet (origin top-left, y down, like
//! schematic pages); the renderer draws them inside its own page frame and
//! title block, clipping text to its box. Set [`PageSetup::margins`] and
//! [`PageSetup::title_block_mm`] to match that frame.

mod bom;
mod devices;
mod diagrams;
mod export;
mod order;
mod page;
mod report;
mod sheets;
mod table;
mod toc;
mod wires;

pub use bom::BillOfMaterials;
pub use devices::DeviceList;
pub use diagrams::{CableDiagram, TerminalDiagram};
pub use export::{CsvOptions, ExportError, save_xlsx, to_csv, xlsx_bytes};
pub use order::{function_of, location_of, location_text, tag_cmp};
pub use page::{Line, Margins, PageLayout, PageSetup, ReportPage, TextBox, layout_page, paginate};
pub use report::{Dependencies, LiveReports, Report};
pub use sheets::{GeneratedSheet, sheet_name, sheet_names};
pub use table::{Align, Cell, Column, ReportOutput, Row, Section};
pub use toc::TableOfContents;
pub use wires::WireList;

use novo_core::Project;

/// The built-in reports, in the order they are listed and exported.
pub fn standard_reports() -> Vec<Box<dyn Report>> {
    vec![
        Box::new(TableOfContents),
        Box::new(BillOfMaterials),
        Box::new(DeviceList),
        Box::new(WireList),
        Box::new(TerminalDiagram),
        Box::new(CableDiagram),
    ]
}

/// Generates every built-in report once, without a cache: for exports and
/// scripts.
pub fn generate_all(project: &Project) -> Vec<ReportOutput> {
    standard_reports()
        .iter()
        .map(|report| report.generate(project))
        .collect()
}
