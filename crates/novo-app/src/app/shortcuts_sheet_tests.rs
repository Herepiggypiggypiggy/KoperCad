//! The shortcut sheet (docs/design/discoverability.md, layer 1).

use novo_workflow::commands::COMMANDS;
use novo_workflow::keymap::{Chord, Key, Keymap, Scope};

use super::gate1_tests::Drive;
use super::shortcuts_sheet::sections;

fn all_rows(keymap: &Keymap, filter: &str) -> Vec<super::shortcuts_sheet::Row> {
    sections(keymap, filter)
        .into_iter()
        .flat_map(|s| s.rows)
        .collect()
}

#[test]
fn every_command_is_listed_once() {
    let rows = all_rows(&Keymap::workflow(), "");
    for info in COMMANDS {
        let n = rows.iter().filter(|r| r.command == Some(info.id)).count();
        assert_eq!(n, 1, "{}", info.id);
    }
}

#[test]
fn a_command_that_does_not_run_is_listed_greyed_comes_later() {
    let rows = all_rows(&Keymap::workflow(), "");
    let later: Vec<_> = rows.iter().filter(|r| r.unavailable.is_some()).collect();
    assert!(!later.is_empty() || COMMANDS.iter().all(|c| super::workbench::app_runs(c.id)));
    for r in later {
        assert_eq!(r.unavailable, Some("Comes later"));
        assert!(!super::workbench::app_runs(r.command.unwrap()));
    }
    // The sheet itself now runs.
    let row = rows
        .iter()
        .find(|r| r.command == Some("view.shortcuts"))
        .unwrap();
    assert_eq!(row.unavailable, None);
    assert_eq!(row.keys, vec!["?".to_owned()]);
}

#[test]
fn a_remapped_key_shows_its_real_key() {
    let mut keymap = Keymap::workflow();
    keymap.rebind("edit.rotate", Chord::plain(Key::letter('t')), Scope::Canvas);
    let rows = all_rows(&keymap, "rotate");
    let row = rows
        .iter()
        .find(|r| r.command == Some("edit.rotate"))
        .unwrap();
    assert_eq!(row.keys, vec!["T".to_owned()]);
    assert_eq!(row.scope, Some("on the drawing"));
}

#[test]
fn the_filter_matches_english_and_dutch_words() {
    let keymap = Keymap::workflow();
    let rows = all_rows(&keymap, "sneltoetsen");
    assert!(rows.iter().any(|r| r.command == Some("view.shortcuts")));
    let rows = all_rows(&keymap, "keyboard");
    assert!(rows.iter().any(|r| r.command == Some("view.shortcuts")));
    assert!(all_rows(&keymap, "zzzzqq").is_empty());
}

#[test]
fn hold_and_tap_behaviours_are_text_rows() {
    let sections = sections(&Keymap::workflow(), "wheel");
    let hold = sections
        .iter()
        .find(|s| s.heading == "Hold and tap")
        .expect("a Hold and tap section");
    assert!(hold.rows.iter().any(|r| r.label.starts_with("Q (hold)")
        && r.command.is_none()
        && r.note.is_some_and(|n| n.contains("place wheel"))));
}

#[test]
fn the_command_opens_and_closes_the_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.queued.push("view.shortcuts");
    d.frame();
    d.frame();
    assert!(d.app.wb.shortcuts.open);
    d.app.wb.queued.push("view.shortcuts");
    d.frame();
    assert!(!d.app.wb.shortcuts.open);
}

/// A command with a key in the default keymap works, so it must never be
/// listed or greyed as "Comes later" (command field, `?` sheet, menus).
/// `KEY_ONLY_GAPS` are keys whose command has no handler yet; remove an id
/// from it when the command lands.
#[test]
fn a_command_with_a_key_is_never_comes_later() {
    const KEY_ONLY_GAPS: &[&str] = &[
        "edit.copy",
        "edit.cut",
        "edit.paste",
        "edit.paste_keep_tags",
        "view.override_marks",
    ];
    let dead: Vec<_> = Keymap::workflow()
        .bindings()
        .iter()
        .map(|b| b.command)
        .filter(|c| !super::workbench::app_runs(c) && !KEY_ONLY_GAPS.contains(c))
        .collect();
    assert_eq!(dead, Vec::<&str>::new(), "keys that only toast");
}

#[test]
fn ctrl_a_selects_everything_on_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    d.place("mcb3", 80.0, 48.0);
    d.place("mcb3", 120.0, 48.0);
    d.frame();
    d.app.run_command(&ctx, "edit.select_all", None);
    d.frame();
    assert_eq!(d.app.selection().len(), 2);
}
