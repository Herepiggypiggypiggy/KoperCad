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
//! - novo-core's project rule `ProjectInfo.rules.terminals` ([`rules`])
//!   decides what one name in several places means. **Join** (new
//!   projects): the drawings and records are one terminal, built from
//!   1-in/1-out clamps linked by connectors (or a jumper), one row in the
//!   strip editor, one node in the netlist. **Refuse**: a second record
//!   with the name is refused. **Off** (files without a rule): records stay
//!   separate and [`check`] lists them.
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
mod earth;
mod geometry;
mod move_strip;
mod naming;
mod occurrence;
mod place;
mod rules;
mod screen;
pub mod strings;
mod strip;
mod style;
mod suggest;

pub use check::{Issue, IssueKind, Severity, check};
pub use earth::{
    earthed_terminals, has_pe_override, is_earth_symbol, mark_earthed, reset_pe_override, toggle_pe,
};
pub use geometry::{redraw, symbol_length, symbol_pins};
pub use move_strip::{MoveTo, move_to_strip};
pub use naming::{
    base_name_for_potential, compare_names, kind_for_name, kind_for_potential, kind_key,
    named_potential, next_free_name, next_free_number, next_order, potential_fits,
};
pub use occurrence::{Occurrence, Side, State, Target, occurrence_in, occurrences};
pub use place::{
    Drop, Placed, place_named, place_terminals, strip_by_tag, strip_in_use, terminal_length,
    terminal_pins,
};
pub use rules::{
    LEGACY_JOIN_LINK_KEY, LEGACY_SAME_NAME_KEY, migrate_legacy_rules, rules, set_rules,
};
pub use screen::{Screen, ScreenRow, screen};
pub use strings::{kind_label, rule_sentence};
pub use strip::{
    Build, RenameChange, RenameProblem, Row, StripView, add_spares, connect_terminals, explain,
    free_jumper, join_duplicates, rename, rename_preview, renumber, renumber_preview, reorder,
    set_jumper, set_kind, set_level, strip_view, strips,
};
pub use style::{
    Style, clear_override, default_symbol, overrides, set_strip_style, strip_part_key, strip_style,
    style_of, terminal_part,
};
pub use suggest::{
    DISMISS_KEY, Suggestion, accept_suggestions, dismiss_suggestion, migrate_legacy_dismissals,
    restore_suggestion, suggest_terminals, summarize_suggestions,
};
