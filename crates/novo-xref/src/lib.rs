//! Cross references (WS-10), always live.
//!
//! A device drawn more than once (a contactor's coil on one page, its
//! contacts on others) and interruption points of the same name refer to
//! each other. This crate derives those references from the model; nothing
//! is stored, so there is no "update cross references" step: rebuild the
//! [`XrefIndex`] (or let [`LiveXref`] do it) and every reference is current.
//!
//! - [`XrefIndex::label`]: text drawn next to a symbol. A coil shows its
//!   contact image below it (`13-14 3.4`), a contact the coil location
//!   (`2.5`) at its right,
//!   an interruption point its partners.
//! - [`Navigator`]: G jumps coil to contact and back, Shift+G lists every
//!   occurrence ([`XrefIndex::all_references`]), Alt+Left/Right walk the
//!   jump history.
//! - [`XrefIndex::highlight`]: what selecting a coil lights up on its page,
//!   and the dock hint line for the rest.
//! - [`link_command`]: "Link cross reference", then click the partner;
//!   [`link_to_device_command`] when a device is named instead (renaming a
//!   contact to a coil's tag).
//!   Returns a model [`Command`](novo_core::Command) (`SetRole`), so it is
//!   one undo step and scripts can do the same.
//! - [`XrefAction::for_symbol`]: the actions a selection offers, one list
//!   for the context menu and the command bar.
//! - [`XrefIndex::find`] and [`XrefIndex::go_to_tag`]: every place a typed
//!   tag occurs, through [`Project::search`](novo_core::Project::search);
//!   an exact tag jumps to the device's main location.
//! - [`XrefIndex::issues`]: contacts without a coil, more contacts drawn
//!   than the device has, lone interruption points (for WS-14).
//!
//! References read `page.column` (`3.4`), prefixed with the target page's
//! structure ([`Page::structure`](novo_core::Page::structure)) when it
//! differs from the reader's page (`=A2/3.4`).
//! Locations and reading order come from the model
//! ([`Project::location_of`](novo_core::Project::location_of),
//! [`PageFormat::columns`](novo_core::PageFormat::columns)).
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
//! assert_eq!(xref.label(coil).unwrap().lines, ["13-14 3.4"]);
//! assert_eq!(xref.label(contact).unwrap().lines, ["2.5"]);
//! # Ok::<(), novo_core::ModelError>(())
//! ```

mod actions;
mod function;
mod index;
mod link;
mod nav;
mod number;

pub use actions::XrefAction;
pub use function::{ContactKind, FromKey, Functions};
pub use index::{
    Highlight, Issue, Label, LabelAnchor, LabelRow, LiveXref, Owner, Reference, TagJump, XrefIndex,
    contact_limit_property,
};
pub use link::{LINK_LABEL, LinkError, can_link, link_command, link_to_device_command};
pub use nav::{Go, JumpHistory, Navigator, Place};
pub use number::{
    auto_name_interruption, auto_name_interruptions, next_interruption_name, renumber_interruptions,
};
