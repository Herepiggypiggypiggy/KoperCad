//! Schematic editor (owned by WS-06): placing symbols as tagged devices,
//! wiring pin to pin with automatic T-junctions, moving, rotating, mirroring,
//! deleting, renaming in place and declaring potential sources, as specified
//! in `docs/DESIGN.md` (§2 what is derived, §4 commands and keys, §6
//! journeys).
//!
//! The crate has two layers:
//!
//! - Headless editing logic, always built: [`catalog`] (symbol search and
//!   classification), [`naming`] (automatic tags and contact numbers),
//!   [`geometry`] (orthogonal routes, hit testing), [`edit`] (the edits,
//!   each run inside a `novo_core::Tx`), [`potential`] (sources),
//!   [`pages`] and [`tool`]
//!   (the tool state machine, [`Editor`]). Tests, scripts (WS-28) and the AI
//!   assistant (WS-23) drive this layer without a window.
//! - Library artwork (feature `art`): [`art::LibraryArt`] draws placed
//!   symbols with their IEC graphics through `novo-canvas`'s page model,
//!   without the widget or egui, for headless rendering (WS-28).
//! - The interactive view (feature `egui`, on by default): [`view::SchematicView`]
//!   puts the editor on a `novo-canvas` canvas with real IEC artwork
//!   ([`art::LibraryArt`]), the insert search, in-place rename and the
//!   context menu. The app shell (WS-01) hosts it in its central area.
//!
//! Every edit goes through `Document::transact`, one user gesture per undo
//! step, so undo/redo, autosave and scripts share one history.
//!
//! ```
//! use novo_core::{Document, Point};
//! use novo_editor::{Editor, tool::Clicked};
//!
//! let mut doc = Document::default();
//! let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
//! let mut editor = Editor::default();
//! editor.begin_place("iec/coils/relay", None);
//! assert_eq!(editor.click(&mut doc, page, Point::new(100.0, 80.0), 1.0), Clicked::Edited);
//! assert_eq!(editor.click(&mut doc, page, Point::new(140.0, 80.0), 1.0), Clicked::Edited);
//! let tags: Vec<String> = doc.project().devices().map(|d| d.tag.to_string()).collect();
//! assert_eq!(tags, ["-K1", "-K2"]);
//! ```

pub mod catalog;
pub mod edit;
pub mod geometry;
pub mod naming;
pub mod overlap;
pub mod pages;
pub mod potential;
pub mod tool;

#[cfg(feature = "art")]
pub mod art;
#[cfg(feature = "egui")]
pub mod view;

pub use catalog::Catalog;
pub use tool::{Editor, EditorKey, Tool};
