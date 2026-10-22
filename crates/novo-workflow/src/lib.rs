//! Fast, keyboard-first workflow (owned by WS-08).
//!
//! Everything here is plain logic with no UI toolkit, so the app shell
//! (WS-01), the schematic editor (WS-06) and tests drive the same code. The
//! behaviour follows `docs/DESIGN.md` (sections 3 and 4), which is the spec:
//!
//! - [`keymap`]: the shortcut map, with canvas-only letter keys and Ctrl
//!   chords that work anywhere (principle 10).
//! - [`commands`]: every command with its id, label, group and shortcut;
//!   the empty command field lists them by group (it is the menu).
//! - [`language`]: Dutch words for every command; English and Dutch both
//!   always match.
//! - [`palette`]: the command field's search and grammar (Ctrl+K, Q, Space) over commands, pages, devices and
//!   symbols; an exact tag lists every location first (J6, J8).
//! - [`insert`]: the Q search: mnemonics (`no`, `nc`, `coil`), favourites
//!   1 to 9, recents, child symbols (`K1 no`) and the IEC 81346-2 letter
//!   code for the next free tag (J2).
//! - [`macro_place`]: place a macro from the user library, with a preview.
//! - [`orientation`]: R and X on a symbol or text.
//! - [`manual`]: the offline user manual, with reference pages generated from
//!   the command list, key map and menus.
//! - [`menus`]: where each command sits in the menu bar.
//! - [`locate`]: where a device occurs, as page and column (`/3.4`), on top
//!   of novo-core's cross references.
//! - [`renumber`]: renumber devices with a preview, as one undo step (J4).
//! - [`switch`]: switch the project between IEC and NFPA, with a preview of
//!   what has no counterpart, as one undo step.
//! - [`userkeys`]: the user's keymap file, applied on top of the defaults.
//! - [`history`]: back and forward through jumps (Alt+Left, Alt+Right).
//! - [`duplicate`]: smart duplicate and paste with the next free tags (J4).
//!
//! Model edits go through [`novo_core::Tx`], so each gesture is one undo
//! step.

pub mod clipboard;
pub mod commands;
pub mod duplicate;
pub mod expr;
pub mod fuzzy;
pub mod generate;
pub mod history;
pub mod insert;
pub mod keymap;
pub mod language;
pub mod locate;
pub mod macro_place;
pub mod manual;
pub mod menus;
pub mod options;
pub mod orientation;
pub mod palette;
pub mod parametric;
pub mod renumber;
pub mod switch;
pub mod userkeys;
