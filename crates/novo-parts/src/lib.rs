//! Parts database and properties (WS-07).
//!
//! - [`Catalog`]: manufacturer parts with dimensions and technical data,
//!   searchable by order number, type, mnemonic (`mpcb`, `ol`, `pb`) or
//!   description. A starter catalog ships with the program; the user's own
//!   catalog lives in their profile ([`user_catalog_path`]).
//! - [`assign_part`] and friends: device-to-part assignment as commands, so
//!   assigning a part to 50 devices is one undo step.
//! - [`PropertySheet`]: the searchable properties panel model (WORKFLOW.md
//!   J12): common fields first, a filter over names and values, shared
//!   values across a selection, one command per edit.
//! - [`BulkTable`]: spreadsheet-style bulk edit with Excel copy and paste.
//! - [`import_catalog`]: CSV import with English, German and Dutch headers.
//!
//! There is deliberately no panel layout here yet: the screen design comes
//! from `docs/DESIGN.md`, and any UI built on top reads rows and returns
//! commands from these types.
//!
//! Nothing here edits the model directly: every change is a
//! [`novo_core::Command`] the caller runs with `Document::execute`.

mod assign;
mod catalog;
mod csv;
mod panel;
mod search;
mod sheet;
mod table;

pub use assign::{
    AssignMode, PartProposal, assign_cable_part, assign_part, assign_terminal_part,
    auto_assign_parts, catalog_entry, clear_parts, find_project_part, is_terminal_block,
    missing_parts, missing_parts_summary, propose_parts, remove_unused_parts, suggest_for_device,
    suggest_for_terminal, terminal_kind_of, terminal_part, unused_parts,
};
pub use catalog::{
    CATALOG_FORMAT_VERSION, Catalog, CatalogError, CatalogPart, Dimensions, MergePreview, PartKey,
    keys, user_catalog_path,
};
pub use csv::{
    Column, DelimitedRow, ImportError, ImportReport, ParseError, column_for, import_catalog,
    parse_delimited,
};
pub use panel::{Clearances, Mounting, PanelData, is_panel_number_key, migrate_properties};
pub use search::{Matcher, normalize_code};
pub use sheet::{
    EditError, Group, INTERNAL_KEY_PREFIXES, PropertySheet, Row, RowKey, RowValue, display_value,
    is_internal_key, parse_number, parse_value, property_key, property_label, resolve_selection,
};
pub use table::{
    BulkError, BulkOutcome, BulkRow, BulkTable, TableColumn, apply_table, fill, find_device,
    set_cell,
};
