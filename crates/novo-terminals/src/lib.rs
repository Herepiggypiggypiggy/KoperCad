//! Terminal strips (WS-11): putting terminals into wires, the project rule
//! for same-named terminals, the strip editor's rows, terminal connection
//! screens and terminal checks.
//!
//! Everything here reads the model through `novo-core` queries and changes
//! it inside a [`novo_core::Tx`], so every edit is one undo step and scripts
//! (WS-28) get the same behaviour as the UI. Nothing is stored that the
//! model can derive: strip rows, screens and checks are computed on demand,
//! so they are always current.
//!
//! When terminals connect is never a guess:
//!
//! - A terminal conducts end to end. Drawings of one terminal record are one
//!   node; jumpers join terminals with the same jumper group.
//! - The project rule [`Settings`] decides what one name in several places
//!   means. **Join** (default): the drawings are one terminal, built from
//!   1-in/1-out clamps linked by connectors (or a jumper), one row in the
//!   strip editor. **Refuse**: later drawings are flagged as duplicates.
//! - [`explain`] says in plain words which of these applies to a terminal.
//!
//! ```
//! use novo_core::{Document, DeviceTag, Endpoint, Pin, Placement, Point, SymbolRole};
//! use novo_terminals::{Drop, place_terminals, strip_view};
//!
//! let mut doc = Document::default();
//! let (strip, _) = doc.transact("Draw", |tx| {
//!     let page = tx.add_page("1")?;
//!     let m1 = tx.add_device(DeviceTag::parse("-M1")?, "Motor")?;
//!     let k1 = tx.add_device(DeviceTag::parse("-K1")?, "Contactor")?;
//!     let motor = tx.place_symbol(page, "iec/motors/motor_1ph", Placement::at(Point::new(40.0, 100.0)),
//!         vec![Pin::new("U", 0.0, 0.0)], SymbolRole::Device { device: m1, main: true })?;
//!     let contact = tx.place_symbol(page, "iec/contacts/no", Placement::at(Point::new(40.0, 20.0)),
//!         vec![Pin::new("13", 0.0, 0.0), Pin::new("14", 0.0, 10.0)], SymbolRole::Device { device: k1, main: false })?;
//!     let wire = tx.connect(Endpoint::pin(contact, 1), Endpoint::pin(motor, 0))?;
//!     let strip = novo_terminals::strip_in_use(tx, page)?;
//!     place_terminals(tx, strip, &[Drop { wire, at: Point::new(40.0, 60.0) }])?;
//!     Ok(strip)
//! })?;
//! let view = strip_view(doc.project(), strip);
//! assert_eq!(view.tag, "-X1");
//! assert_eq!(view.rows[0].name, "1");
//! assert_eq!(view.rows[0].internal_targets()[0].designation, "-K1:14");
//! assert_eq!(view.rows[0].external_targets()[0].designation, "-M1:U");
//! # Ok::<(), novo_core::ModelError>(())
//! ```

mod check;
mod naming;
mod occurrence;
mod place;
mod screen;
mod settings;
mod strip;
mod style;
mod suggest;

pub use check::{Issue, IssueKind, Severity, check};
pub use naming::{
    compare_names, kind_for_name, kind_for_potential, kind_key, kind_label, next_free_name,
    next_free_number, next_order, potential_kind_for_name,
};
pub use occurrence::{Occurrence, Side, State, Target, occurrence_in, occurrences};
pub use place::{
    Drop, Placed, place_named, place_terminals, strip_by_tag, strip_in_use, terminal_length,
    terminal_pins,
};
pub use screen::{Screen, ScreenRow, screen};
pub use settings::{JOIN_LINK_KEY, JoinLink, SAME_NAME_KEY, SameName, Settings};
pub use strip::{
    Build, Row, StripView, add_spares, explain, free_jumper, join_duplicates, renumber,
    renumber_preview, reorder, set_jumper, set_kind, set_level, strip_view, strips,
};
pub use style::{
    Style, clear_override, default_symbol, overrides, set_strip_style, strip_style, style_of,
};
pub use suggest::{
    DISMISS_KEY, Suggestion, accept_suggestions, dismiss_suggestion, restore_suggestion,
    suggest_terminals,
};
