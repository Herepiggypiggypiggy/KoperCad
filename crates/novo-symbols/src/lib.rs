//! Symbol library (owned by WS-05): IEC 60617 symbol geometry, connection
//! points, function types and the behavior slots used by simulation.
//!
//! Symbols are data: one TOML file per symbol under `symbols/`, embedded in
//! the binary at build time. `FORMAT.md` documents the format.
//!
//! ```
//! let library = novo_symbols::Library::builtin().unwrap();
//! let contact = library.get("iec/contacts/no").unwrap();
//! assert_eq!(contact.connections.len(), 2);
//! ```

mod library;
mod svg;
mod symbol;
mod validate;

pub use library::{Library, LoadError};
pub use svg::{SvgOptions, to_svg};
pub use symbol::{
    ALWAYS_CLOSED, Align, Anchor, Behavior, Connection, Direction, FORMAT_VERSION, GRID, Graphic,
    PASS_THROUGH, PassThrough, Point, Stroke, Symbol,
};
pub use validate::{ANCHOR_ROLES, BEHAVIOR_KINDS, Issue, MAX_ALIAS_LEN, behavior_keys, validate};
