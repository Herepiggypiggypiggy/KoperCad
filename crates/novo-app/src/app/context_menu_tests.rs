//! The schematic right-click menu only offers commands the app runs, and
//! shows the key the keymap binds.

use novo_workflow::commands;

/// Command ids the editor's context menu pushes that are not in the
/// registry: keys of the drawing and a double-click, handled by the shell.
const SHELL_ONLY: &[&str] = &["tool.place_wheel", "edit.properties"];

/// Every `"a.b"` id after `MenuChoice::Command(` or in a `menu_row` or
/// `(label, id)` pair of the context menu in the editor's source.
fn menu_ids() -> Vec<String> {
    let src = include_str!("../../../novo-editor/src/view.rs");
    let start = src.find("fn context_menu(").expect("context_menu");
    let end = src[start..].find("\nenum MenuChoice").expect("enum") + start;
    let body = &src[start..end];
    let mut ids = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find('"') {
        let tail = &rest[at + 1..];
        let Some(close) = tail.find('"') else { break };
        let lit = &tail[..close];
        let looks_like_id = lit.contains('.')
            && !lit.contains(' ')
            && !lit.ends_with('.')
            && lit
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_' || c == '-');
        if looks_like_id {
            ids.push(lit.to_owned());
        }
        rest = &tail[close + 1..];
    }
    ids.sort();
    ids.dedup();
    ids
}

#[test]
fn every_command_the_context_menu_offers_is_real_and_runs() {
    let ids = menu_ids();
    assert!(ids.len() > 25, "scan found only {} ids", ids.len());
    for id in ids {
        if SHELL_ONLY.contains(&id.as_str()) {
            continue;
        }
        assert!(commands::get(&id).is_some(), "{id} is not a command");
        assert!(
            super::workbench::app_runs(&id),
            "{id} is in the right-click menu but the app answers \"not in the app yet\""
        );
    }
}

#[test]
fn the_keys_the_menu_shows_come_from_the_keymap() {
    for id in novo_editor::view::SchematicView::MENU_COMMANDS {
        assert!(commands::get(id).is_some(), "{id} is not a command");
    }
    let keymap = novo_workflow::keymap::Keymap::default();
    for (id, key) in [
        ("edit.paste", "Ctrl+V"),
        ("edit.cut", "Ctrl+X"),
        ("edit.select_all", "Ctrl+A"),
    ] {
        assert_eq!(keymap.shortcut_text(id).as_deref(), Some(key), "{id}");
    }
}
