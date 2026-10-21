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
//! - [`import_inbox`]: the bulk import of every catalog file in one folder
//!   (`kopercad-cli parts import`), with a per-maker report.
//! - [`TableFile`], [`Mapping`], [`build_preview`]: the table import with a
//!   mapping step (CSV and Excel files with any column layout), a row by
//!   row preview with checks, and honest labelling of what the file does
//!   not say.
//! - [`ImportJob`], [`LocalLibrary`], [`import_bmecat`], [`step_box`]: the
//!   one-click part import and the per-user library of downloaded maker
//!   files (design: `docs/PARTS-LIBRARY-DESIGN.md`). No network code here.
//!
//! There is deliberately no panel layout here yet: the screen design comes
//! from `docs/DESIGN.md`, and any UI built on top reads rows and returns
//! commands from these types.
//!
//! Nothing here edits the model directly: every change is a
//! [`novo_core::Command`] the caller runs with `Document::execute`.

mod assign;
mod bmecat;
mod browser;
mod bulk;
mod catalog;
mod csv;
mod fit;
mod import;
mod library;
mod macros;
mod panel;
mod pins;
mod quality;
mod recipes;
mod schema;
mod search;
mod sheet;
mod step;
mod table;
mod tablefile;
mod tablemap;
mod tablepreview;

pub use assign::{
    AssignMode, PartProposal, Suggestion, assign_cable_part, assign_part, assign_terminal_part,
    auto_assign_parts, catalog_entry, clear_parts, find_project_part, is_terminal_block,
    missing_parts, missing_parts_summary, propose_parts, remove_unused_parts, suggest_for_device,
    suggest_for_terminal, suggestions_for_device, terminal_kind_of, terminal_part, unused_parts,
};
pub use bmecat::{BmecatError, ECLASS_CLASS_KEY, ETIM_CLASS_KEY, import_bmecat};
pub use browser::{BrowserRow, RowAction, browser_rows};
pub use bulk::{
    InboxError, InboxOptions, InboxReport, MakerSummary, default_inbox, import_inbox, maker_guide,
    prepare_inbox,
};
pub use catalog::{
    CATALOG_FORMAT_VERSION, Catalog, CatalogError, CatalogPart, Dimensions, MergePreview, PartKey,
    keys, user_catalog_path,
};
pub use csv::{
    Column, DelimitedRow, ImportError, ImportReport, ParseError, column_for, import_catalog,
    parse_delimited,
};
pub use fit::{
    DeviceNeed, Fit, Role, Supply, coil_supply, device_role, fit, parse_supply, part_role,
};
pub use import::{
    Arrival, DownloadsWatcher, FallbackReason, FetchError, Fetcher, ImportJob, ImportOutcome,
    ImportState, MTIME_SLACK, MakerSource, RemoteFile, ScriptedAnswer, ScriptedFetcher,
    SourceTable, default_downloads_dir,
};
pub use library::{
    FileKind, ImportFlow, LIBRARY_FORMAT_VERSION, LibraryError, LocalLibrary, NewSource,
    SourceRecord, default_library_root,
};
pub use macros::{
    Imported, MACRO_BASE_VERSION, MACRO_EXTENSION, MACRO_FORMAT, MACRO_FORMAT_VERSION,
    MACRO_PACK_EXTENSION, MACRO_PACK_FORMAT, MacroEntry, MacroError, MacroFile, MacroLibrary,
    MacroPack, MacroPart, MacroUpdate, OnConflict, PackEntry, ParamDraft, ParamKind,
    STARTER_FOLDER, SYMBOL_PAD, ThumbSymbol, Thumbnail, macro_updates, param_drafts,
    starter_macros, thumbnail, user_macros_dir, validate_parameters,
};
pub use panel::{Clearances, Mounting, PanelData, is_panel_number_key, migrate_properties};
pub use pins::{
    POSITIONS_SOURCE_KEY, POSITIONS_SOURCE_PROPERTY, placed_terminals, positions_estimated,
    terminal_position, write_properties as write_terminal_positions,
};
pub use quality::{
    Completeness, Conflict, Criterion, Decision, DuplicateGroup, DuplicateReason, Fill, Finding,
    Grade, PartQuality, QualityReport, QueueItem, REVIEW_FILE_NAME, REVIEW_FORMAT_VERSION,
    ReviewEntry, ReviewError, ReviewLedger, Trust, assess, conflicts_between, duplicate_groups,
    fingerprint, incoming_conflicts, report as quality_report, review_path_for,
};
pub use recipes::{
    ApplyContext, Dim, GENERIC, Login, MakerRecipe, Mapped, Quantity, RECIPES, RecipeReport, Route,
    Skipped, Verified, local_keys, parse_quantity, recipe_for,
};
pub use schema::eng;
pub use schema::{
    Asset, AssetKind, Classification, ConnectionKind, DataSource, LetThroughPoint, Lifecycle,
    PartRef, PartTerminal, Price, Provenance, Side,
};
pub use search::{Matcher, normalize_code};
pub use sheet::{
    EditError, Group, INTERNAL_KEY_PREFIXES, PropertySheet, Row, RowKey, RowValue, display_value,
    is_internal_key, parse_number, parse_value, property_key, property_label, resolve_selection,
};
pub use step::{StepBox, StepError, step_box};
pub use table::{
    BulkError, BulkOutcome, BulkRow, BulkTable, TableColumn, apply_table, fill, find_device,
    set_cell,
};
pub use tablefile::{
    MAX_FILE_BYTES, MAX_ROWS, Sheet, TABLE_EXTENSIONS, TableError, TableFile, too_large_file,
};
pub use tablemap::{
    ColumnMap, MEASURES, Mapping, Measure, SavedMappings, Target, guess_header_row, measure,
    saved_mappings_path, split_header, stored_unit,
};
pub use tablepreview::{
    Counts, Coverage, ImportPreview, Issue, PreviewRow, RowStatus, Severity, build_preview,
};
