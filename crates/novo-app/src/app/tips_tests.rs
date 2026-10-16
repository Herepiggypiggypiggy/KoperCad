//! Tips, hint counters and the dead-command audit.

use eframe::egui;
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
const COMES_LATER: &[&str] = &[];

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

/// The ids `run_command` handles in its `match id` block, read from the
/// source: the string patterns before each `=>`.
fn handled_in_run_command() -> std::collections::BTreeSet<String> {
    let src = include_str!("workbench.rs");
    let start = src
        .find("match id {\n            \"view.cabinet_table\"")
        .unwrap();
    let end = src[start..].find("other => {").unwrap() + start;
    let mut ids = std::collections::BTreeSet::new();
    let mut group: Vec<String> = Vec::new();
    for line in src[start..end].lines() {
        let line = line.trim_start();
        let arm_part = line.split("=>").next().unwrap_or("");
        let looks_like_pattern = line.starts_with('"') || line.starts_with("| \"");
        if looks_like_pattern || !group.is_empty() {
            let mut rest = arm_part;
            while let Some(i) = rest.find('"') {
                let after = &rest[i + 1..];
                let Some(j) = after.find('"') else { break };
                group.push(after[..j].to_owned());
                rest = &after[j + 1..];
            }
        }
        if line.contains("=>") {
            if looks_like_pattern || group.len() > 1 {
                ids.extend(group.drain(..));
            }
            group.clear();
        }
    }
    ids.retain(|id| novo_workflow::commands::is_known(id));
    ids
}

/// `APP_COMMANDS` (what menus, the `?` sheet and the command field treat as
/// working) and the `match` in `run_command` (what actually runs) are two
/// lists. A command in only one is either greyed "Comes later" while it
/// works, or live in the menu and answers "not in the app yet".
#[test]
fn the_handler_match_and_the_list_of_working_commands_agree() {
    let handled = handled_in_run_command();
    assert!(
        handled.len() > 100,
        "scan found only {} arms",
        handled.len()
    );
    let listed: std::collections::BTreeSet<&str> =
        super::workbench::APP_COMMANDS.iter().copied().collect();
    let works_but_listed_later: Vec<_> = handled
        .iter()
        .filter(|id| !super::workbench::app_runs(id))
        .collect();
    assert!(
        works_but_listed_later.is_empty(),
        "handled in run_command but not in APP_COMMANDS (shown greyed Comes later): {works_but_listed_later:?}"
    );
    let listed_but_unhandled: Vec<_> = listed
        .iter()
        .filter(|id| {
            !handled.contains(**id)
                && !super::reports_view::REPORT_COMMANDS
                    .iter()
                    .any(|(c, _)| c == *id)
                && !id.starts_with("learn.")
                && novo_workflow::keymap::FAVOURITE_COMMANDS
                    .iter()
                    .all(|f| f != *id)
                && crate::actions::Action::from_id(id).is_none()
        })
        .collect();
    assert!(
        listed_but_unhandled.is_empty(),
        "in APP_COMMANDS but no arm in run_command (answers \"not in the app yet\"): {listed_but_unhandled:?}"
    );
}

/// Every `.rs` file under `dir` that is not test code.
fn source_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            source_files(&path, out);
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let test_code = name.ends_with("_tests.rs") || name.starts_with("lesson_verify");
        if name.ends_with(".rs") && !test_code {
            out.push(path);
        }
    }
}

/// A key typed into a tooltip goes stale when the person remaps it, so a
/// tooltip names its key through `tip_text`, `tip()` or `hover()`.
#[test]
fn no_tooltip_has_a_key_typed_into_its_text() {
    let mut files = Vec::new();
    source_files(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() > 50, "the scan finds the app's sources");
    let mut typed = Vec::new();
    for file in files {
        let src = std::fs::read_to_string(&file).unwrap();
        for (at, _) in src.match_indices(".on_hover_text(") {
            let rest = &src[at + ".on_hover_text(".len()..];
            let rest = &rest[..rest.len().min(200)];
            let Some(open) = rest.find('"') else { continue };
            // Only a literal argument: nothing but whitespace before it.
            if !rest[..open].trim().is_empty() {
                continue;
            }
            let body = &rest[open + 1..];
            let literal = &body[..body.find('"').unwrap_or(body.len())];
            if ["Ctrl+", "Shift+", "Alt+"]
                .iter()
                .any(|k| literal.contains(k))
            {
                typed.push(format!("{}: {literal}", file.display()));
            }
        }
    }
    assert!(typed.is_empty(), "keys typed into tooltips: {typed:#?}");
}

#[test]
fn every_tip_command_is_known_and_hover_reads_its_live_key() {
    use super::tips::{TIP_COMMANDS, hover, publish_keys};
    let keymap = Keymap::workflow();
    let ctx = egui::Context::default();
    publish_keys(&ctx, &keymap);
    for c in TIP_COMMANDS {
        assert!(novo_workflow::commands::is_known(c), "{c}");
        assert!(keymap.shortcut_text(c).is_some(), "{c} has a key");
    }
    assert_eq!(
        hover(&ctx, "Open or close the door", "view.3d_door"),
        "Open or close the door · Shift+D"
    );
}
