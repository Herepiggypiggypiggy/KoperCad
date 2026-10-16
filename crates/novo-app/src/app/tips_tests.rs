//! Tips, hint counters and the dead-command audit.

use novo_workflow::commands::COMMANDS;
use novo_workflow::keymap::Keymap;

use super::gate1_tests::Drive;
use super::tips::{HINTS, hint_due, hint_text, tip_text};
use crate::actions::Action;
use crate::settings::HINT_LIMIT;

#[test]
fn a_tip_carries_the_live_key() {
    let keymap = Keymap::workflow();
    assert_eq!(tip_text(&keymap, "Place", "tool.place"), "Place · Q");
    assert_eq!(tip_text(&keymap, "Nothing", "no.such.command"), "Nothing");
}

#[test]
fn every_hint_names_a_command_and_has_a_key_or_falls_back() {
    let keymap = Keymap::workflow();
    let mut ids = std::collections::HashSet::new();
    for h in HINTS {
        assert!(novo_workflow::commands::is_known(h.command), "{}", h.id);
        assert!(ids.insert(h.id), "duplicate hint id {}", h.id);
        assert!(!hint_text(&keymap, h).contains("{key}"));
    }
}

#[test]
fn a_hint_fades_after_it_was_followed_three_times_and_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(
        d.app.empty_page_hint().unwrap().contains("Q"),
        "an empty page names the key"
    );
    for _ in 0..HINT_LIMIT {
        assert!(hint_due(&d.app.settings, "empty_page"));
        d.app.hint_command_run("tool.place");
    }
    assert!(!hint_due(&d.app.settings, "empty_page"));
    assert!(d.app.empty_page_hint().is_none());
    d.app.run_command(&ctx, "help.show_hints", None);
    assert!(hint_due(&d.app.settings, "empty_page"));
    assert!(d.app.empty_page_hint().is_some());
}

/// Commands the app cannot run yet. Listed on purpose: the `?` sheet, the
/// command field and the menus grey them "Comes later". The audit fails for
/// a command with no handler that is not here, so a command never ships
/// dead by accident; remove an id when its handler lands.
const COMES_LATER: &[&str] = &[
    "edit.copy",
    "edit.cut",
    "edit.paste",
    "edit.paste_keep_tags",
    "view.override_marks",
    // No handler anywhere in novo-app yet; found by the audit, owners told.
    "project.rules",
    "draw.tidy",
    "edit.renumber",
    "nav.changes",
];

#[test]
fn dead_command_audit() {
    let dead: Vec<_> = COMMANDS
        .iter()
        .map(|c| c.id)
        // The lesson commands run through the Learn engine, not app_runs.
        .filter(|id| !super::workbench::app_runs(id) && !id.starts_with("learn."))
        .collect();
    let unlisted: Vec<_> = dead.iter().filter(|id| !COMES_LATER.contains(id)).collect();
    assert!(
        unlisted.is_empty(),
        "commands with no handler, not marked Comes later: {unlisted:?}"
    );
    let stale: Vec<_> = COMES_LATER.iter().filter(|id| !dead.contains(id)).collect();
    assert!(
        stale.is_empty(),
        "now handled, remove from COMES_LATER: {stale:?}"
    );
}
