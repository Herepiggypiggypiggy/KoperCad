//! Cross references (WS-10), always live.
//!
//! A device drawn more than once (a contactor's coil on one page, its
//! contacts on others) and interruption points of the same name refer to
//! each other. This crate derives those references from the model; nothing
//! is stored, so there is no "update cross references" step: rebuild the
//! [`XrefIndex`] (or let [`LiveXref`] do it) and every reference is current.
//!
//! - [`XrefIndex::label`]: text drawn next to a symbol. A coil shows its
//!   contact image (`NO 13-14 /3.4`), a contact the coil location (`/2.5`),
//!   an interruption point its partners.
//! - [`Navigator`]: G jumps coil to contact and back, Shift+G lists every
//!   occurrence ([`XrefIndex::all_references`]), Alt+Left/Right walk the
//!   jump history.
//! - [`XrefIndex::highlight`]: what selecting a coil lights up on its page,
//!   and the status bar line for the rest.
//! - [`link_command`]: right click, "Link cross reference", click the
//!   partner. Returns a model [`Command`](novo_core::Command), so it is one
//!   undo step and scripts can do the same.
//! - [`XrefIndex::find_tag`]: every place a typed tag occurs.
//!
//! Locations are `page.column`; [`PageGrid`] divides each page into
//! columns (10 by default, see the `xref.*` page properties).
//!
//! ```
//! use novo_core::{DeviceTag, Document, Pin, Placement, Point, SymbolRole};
//! use novo_xref::{FromKey, XrefIndex};
//!
//! let mut doc = Document::default();
//! let (coil, contact) = doc.transact("Draw", |tx| {
//!     let p2 = tx.add_page("2")?;
//!     let p3 = tx.add_page("3")?;
//!     let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
//!     let coil = tx.place_symbol(p2, "iec/coils/relay",
//!         Placement::at(Point::new(190.0, 100.0)),
//!         vec![Pin::new("A1", 0.0, -8.0), Pin::new("A2", 0.0, 8.0)],
//!         SymbolRole::Device { device: k1, main: true })?;
//!     let contact = tx.place_symbol(p3, "iec/contacts/no",
//!         Placement::at(Point::new(140.0, 100.0)),
//!         vec![Pin::new("13", 0.0, -8.0), Pin::new("14", 0.0, 8.0)],
//!         SymbolRole::Device { device: k1, main: false })?;
//!     Ok((coil, contact))
//! })?.0;
//!
//! let xref = XrefIndex::build(doc.project(), &FromKey);
//! assert_eq!(xref.label(coil).unwrap().lines, ["NO 13-14 /3.4"]);
//! assert_eq!(xref.label(contact).unwrap().lines, ["/2.5"]);
//! # Ok::<(), novo_core::ModelError>(())
//! ```

mod function;
mod grid;
mod index;
mod link;
mod nav;

pub use function::{ContactKind, FromKey, Functions};
pub use grid::{
    COLUMNS_PROPERTY, DEFAULT_COLUMNS, Location, MARGIN_LEFT_PROPERTY, MARGIN_RIGHT_PROPERTY,
    PageGrid,
};
pub use index::{Highlight, Issue, Label, LiveXref, Owner, Reference, XrefIndex};
pub use link::{LINK_LABEL, LinkError, can_link, link_command};
pub use nav::{Go, JumpHistory, Navigator, Place};
