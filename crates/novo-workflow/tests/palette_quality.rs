//! What the command field ranks and shows (DESIGN §3): the key next to every
//! command, labels before keywords, a typo that still finds the command,
//! and recent picks first for a short query.

use novo_symbols::Library;
use novo_workflow::commands::Context;
use novo_workflow::keymap::Keymap;
use novo_workflow::palette::{Palette, Row, Sources, Target};

fn context() -> Context {
    Context {
        has_project: true,
        has_page: true,
        ..Context::default()
    }
}

fn search(palette: &Palette, keymap: &Keymap, query: &str) -> Vec<Row> {
    let library = Library::builtin().unwrap();
    let sources = Sources {
        project: None,
        library: Some(&library),
        keymap,
        context: context(),
    };
    palette.search(query, &sources)
}

fn position(rows: &[Row], command: &str) -> Option<usize> {
    rows.iter()
        .position(|r| matches!(&r.target, Target::Command(id) if *id == command))
}

#[test]
fn a_command_row_shows_its_key() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    let rows = search(&palette, &keymap, "dup");
    assert_eq!(rows[0].target, Target::Command("edit.duplicate"));
    assert_eq!(rows[0].shortcut.as_deref(), Some("Ctrl+D"));
}

#[test]
fn a_typo_and_a_dutch_word_still_find_the_command() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    assert_eq!(
        search(&palette, &keymap, "duplcate")[0].target,
        Target::Command("edit.duplicate")
    );
    assert_eq!(
        search(&palette, &keymap, "dupliceren")[0].target,
        Target::Command("edit.duplicate")
    );
}

#[test]
fn the_start_of_a_label_beats_a_keyword() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    let rows = search(&palette, &keymap, "ro");
    let rotate = position(&rows, "edit.rotate").expect("Rotate is listed");
    let wiring = position(&rows, "tool.wiring_mode");
    assert!(
        wiring.is_none_or(|w| rotate < w),
        "Rotate before commands that only have a keyword starting with ro"
    );
}

#[test]
fn recent_picks_lead_a_short_query_but_only_when_they_match() {
    let keymap = Keymap::workflow();
    let mut palette = Palette::default();
    let before = search(&palette, &keymap, "m");
    palette.picked(Target::Command("edit.mirror"));
    let after = search(&palette, &keymap, "m");
    let (was, is) = (
        position(&before, "edit.mirror").unwrap(),
        position(&after, "edit.mirror").unwrap(),
    );
    assert!(is <= was, "a recent pick does not drop: {was} -> {is}");
    assert_eq!(is, 0, "and leads a query it matches");

    // `ro` only scatters across Mirror's letters: no lift over Rotate.
    let rows = search(&palette, &keymap, "ro");
    let rotate = position(&rows, "edit.rotate").unwrap();
    assert!(position(&rows, "edit.mirror").is_none_or(|m| rotate < m));
}

#[test]
fn a_part_word_finds_the_part_before_a_command_that_only_has_it_as_a_keyword() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    for word in [
        "overload",
        "contactor",
        "relay",
        "breaker",
        "terminal",
        "motor",
        "fuse",
        "transformer",
        "coil",
    ] {
        let rows = search(&palette, &keymap, word);
        assert_eq!(
            rows[0].kind,
            novo_workflow::palette::Kind::Symbol,
            "`{word}` starts with a part, not {:?}",
            rows[0].label
        );
    }
}

#[test]
fn overload_is_the_overload_relay_and_fault_scenarios_stay_reachable() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    let rows = search(&palette, &keymap, "overload");
    assert!(rows[0].label.contains("overload"), "{:?}", rows[0].label);
    assert!(position(&rows, "sim.faults").is_some(), "still listed");
    // The command's own words still lead to it.
    let rows = search(&palette, &keymap, "fault");
    assert_eq!(rows[0].target, Target::Command("sim.faults"));
    let rows = search(&palette, &keymap, "short circuit");
    assert_eq!(rows[0].target, Target::Command("sim.faults"));
}
