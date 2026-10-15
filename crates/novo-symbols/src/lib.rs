//! Symbol library (owned by WS-05, packs by WS-25): IEC 60617 and NFPA
//! symbol geometry, connection points, function types and the behavior
//! slots used by simulation.
//!
//! Symbols are data: one TOML file per symbol under `symbols/`, embedded in
//! the binary at build time. `FORMAT.md` documents the format.
//!
//! ```
//! let library = novo_symbols::Library::builtin().unwrap();
//! let contact = library.get("iec/contacts/no").unwrap();
//! assert_eq!(contact.connections.len(), 2);
//! ```

mod letter;
mod library;
mod pack;
mod svg;
mod symbol;
mod validate;

pub use letter::{BreakerCode, ContactorCode, LetterRules, letter_code, letter_code_with};
pub use library::{Library, LoadError};
pub use pack::Pack;
pub use svg::{SvgOptions, to_svg};
pub use symbol::{
    ALWAYS_CLOSED, Align, Anchor, Behavior, Connection, Direction, FORMAT_VERSION, GRID, Graphic,
    PASS_THROUGH, PassThrough, Point, Stroke, Symbol,
};
pub use validate::{ANCHOR_ROLES, BEHAVIOR_KINDS, Issue, MAX_ALIAS_LEN, behavior_keys, validate};
