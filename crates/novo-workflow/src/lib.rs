//! Fast, keyboard-first workflow (owned by WS-08).
//!
//! Everything here is plain logic with no UI toolkit, so the app shell
//! (WS-01), the schematic editor (WS-06) and tests drive the same code. The
//! behaviour follows `docs/WORKFLOW.md`, which is the spec:
//!
//! - [`keymap`]: the shortcut map, with canvas-only letter keys and Ctrl
//!   chords that work anywhere (principle 10).
//! - [`commands`]: every command with its id, label, menu and shortcut, so
//!   menus and the palette show the same list (J8).
//! - [`palette`]: the Ctrl+K search over commands, pages, devices and
//!   symbols; an exact tag lists every location first (J6, J8).
//! - [`insert`]: the I search: mnemonics (`no`, `nc`, `coil`), favourites
//!   1 to 9, recents, child symbols (`K1 no`) and the IEC 81346-2 letter
//!   code for the next free tag (J2).
//! - [`orientation`]: Tab cycling through every connection piece (corners,
//!   Ts, cross) and through every symbol orientation (J3).
//! - [`locate`]: where a device occurs, as page and column (`/3.4`).
//! - [`history`]: back and forward through jumps (Alt+Left, Alt+Right).
//! - [`duplicate`]: smart duplicate and paste with the next free tags (J4).
//! - [`escape`]: the one Escape ladder (principle 4).
//!
//! Model edits go through [`novo_core::Tx`], so each gesture is one undo
//! step.

pub mod commands;
pub mod duplicate;
pub mod escape;
pub mod fuzzy;
pub mod history;
pub mod insert;
pub mod keymap;
pub mod locate;
pub mod orientation;
pub mod palette;
