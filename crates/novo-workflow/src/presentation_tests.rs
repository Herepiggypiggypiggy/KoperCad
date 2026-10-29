//! The command registry and the presentation table (`novo_theme::presentation`)
//! kept together: every command has an icon and a short label, no row is left
//! over, and the short labels meet the label policy (docs/design/
//! visual-redesign.md section 5.2). The table lives in `novo-theme` so this
//! crate stays free of presentation; the test lives here because only here
//! both sides are visible.

use std::collections::{HashMap, HashSet};

use novo_theme::presentation::{self, Glyph, MAX_SHORT};

use crate::commands::COMMANDS;
use crate::menus;

#[test]
fn every_command_has_an_icon_and_a_short_label() {
    let missing: Vec<_> = COMMANDS
        .iter()
        .filter(|c| presentation::of(c.id).is_none())
        .map(|c| c.id)
        .collect();
    assert!(
        missing.is_empty(),
        "commands with no row in novo-theme/src/presentation.rs (add one line: id, icon, short label): {missing:?}"
    );
}

#[test]
fn every_row_names_a_command() {
    let ids: HashSet<_> = COMMANDS.iter().map(|c| c.id).collect();
    let stale: Vec<_> = presentation::TABLE
        .iter()
        .filter(|r| !ids.contains(r.id))
        .map(|r| r.id)
        .collect();
    assert!(
        stale.is_empty(),
        "rows for commands that do not exist: {stale:?}"
    );
    assert_eq!(presentation::TABLE.len(), COMMANDS.len());
}

#[test]
fn short_labels_fit_the_label_policy_and_are_never_longer_than_the_label() {
    for c in COMMANDS {
        let Some(row) = presentation::of(c.id) else {
            continue;
        };
        let n = row.short.chars().count();
        assert!(
            n <= MAX_SHORT,
            "{}: {:?} has {n} characters",
            c.id,
            row.short
        );
        assert!(
            n <= c.label.chars().count(),
            "{}: short {:?} is longer than the label {:?}",
            c.id,
            row.short,
            c.label
        );
    }
}

#[test]
fn short_labels_are_unique_within_a_menu_level() {
    // Two rows of the same submenu that read the same would be a coin toss.
    let mut seen: HashMap<(&str, Option<&str>, &str), &str> = HashMap::new();
    for c in COMMANDS {
        let (Some(row), Some(place)) = (presentation::of(c.id), menus::place(c)) else {
            continue;
        };
        let key = (place.menu, place.submenu, row.short);
        if let Some(other) = seen.insert(key, c.id) {
            panic!(
                "{} and {} are both {:?} in {:?}",
                c.id, other, row.short, place
            );
        }
    }
}

#[test]
fn commands_of_the_electrical_families_use_the_drawn_icons() {
    let drawn = |id: &str| matches!(presentation::glyph(id), Glyph::Drawn(_));
    for id in [
        "tool.place",
        "tool.cable",
        "xref.link",
        "wires.number",
        "terminal.connect",
        "plc.add_card",
        "sim.live",
        "sim.faults",
        "view.schematic",
        "view.cabinet",
        "view.reports",
        "cabinet.rails",
        "cabinet.plate",
        "export.ferrules",
        "view.bench",
        "macro.browser",
        "check.emc",
    ] {
        assert!(drawn(id), "{id} should show a drawn icon");
    }
    for id in ["file.save", "edit.undo", "view.fit"] {
        assert!(!drawn(id), "{id} is a generic action (Phosphor)");
    }
}
