//! Import wizard engine (lane C4): turns a standard I/O page into a controller profile.
//!
//! The ladder, cheapest first: a pasted table ([`Table::from_text`]), the pins of a device in
//! the open drawing ([`Table::from_pins`]), a grid read from a spreadsheet or an EPLAN export
//! by the application ([`Table::from_grid`]). All of them end in the same [`propose`] step,
//! which only guesses: it returns a [`Proposal`] with a warning for every row it was not sure
//! about, and writes nothing. Dutch and English words are data files ([`WordList`]).
//!
//! Plain `std`, no files or clock, like the rest of the crate. Profiles made here are the
//! user's own; the library in [`library`] is generic and never names a product.

mod device;
pub mod library;
mod output;
mod propose;
mod read;
mod table;
mod words;

pub use device::DevicePin;
pub use propose::{
    Binding, Control, Options, Outcome, Proposal, ProposedProfile, RowOutcome, Severity, Signal,
    Supply, Warning, propose, slug,
};
pub use read::{Reading, Voltage, read_row};
pub use table::{Columns, MAX_ROWS, Row, Table, parse_step_cell, rows_to_text};
pub use words::{Dir, Hit, Role, Tag, WordList, tokenize};
