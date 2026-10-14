//! Fast, keyboard-first workflow (owned by WS-08).
//!
//! The keymap and command list are provisional: docs/DESIGN.md may reshape
//! commands and keys, and the search engine is kept independent of how the
//! palette or command bar is shown.
//!
//! Everything here is plain logic with no UI toolkit, so the app shell
//! (WS-01), the schematic editor (WS-06) and tests drive the same code. The
//! behaviour follows `docs/WORKFLOW.md`, which is the spec:
//!
//! - [`keymap`]: the shortcut map, with canvas-only letter keys and Ctrl
//!   chords that work anywhere (principle 10).
//! - [`commands`]: every command with its id, label, group and shortcut;
//!   the empty command field lists them by group (it is the menu).
//! - [`palette`]: the command field's search and grammar (Ctrl+K, I, Space) over commands, pages, devices and
//!   symbols; an exact tag lists every location first (J6, J8).
//! - [`insert`]: the I search: mnemonics (`no`, `nc`, `coil`), favourites
//!   1 to 9, recents, child symbols (`K1 no`) and the IEC 81346-2 letter
//!   code for the next free tag (J2).
//! - [`orientation`]: R and X on a symbol or text.
//! - [`locate`]: where a device occurs, as page and column (`/3.4`), on top
//!   of novo-core's cross references.
//! - [`renumber`]: renumber devices with a preview, as one undo step (J4).
//! - [`history`]: back and forward through jumps (Alt+Left, Alt+Right).
//! - [`duplicate`]: smart duplicate and paste with the next free tags (J4).
//!
//! Model edits go through [`novo_core::Tx`], so each gesture is one undo
//! step.

pub mod commands;
pub mod duplicate;
pub mod fuzzy;
pub mod history;
pub mod insert;
pub mod keymap;
pub mod locate;
pub mod orientation;
pub mod palette;
pub mod renumber;
