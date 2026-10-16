//! Tips and hints (docs/design/discoverability.md layers 3 and 4).
//!
//! [`tip_text`] gives a control its tooltip with the key read from the
//! live keymap, so a remapped key reads right and no key is typed by hand.
//! Hints teach once: each has an id, a command that follows it, and a
//! counter in `Settings::hints_seen`; after [`HINT_LIMIT`] follows it stops
//! showing. Help > Show hints again clears the counters.

use eframe::egui;
use novo_workflow::keymap::Keymap;

use super::NovoApp;
use crate::settings::{HINT_LIMIT, Settings};

/// A hint that teaches a command and fades once it has been used.
pub(super) struct Hint {
    /// Stable key in `Settings::hints_seen`.
    pub id: &'static str,
    /// The command that follows the hint; running it counts one use.
    pub command: &'static str,
    /// The text, with `{key}` for the command's key.
    pub text: &'static str,
}

/// Every hint, declared next to its text so a new hint is one line.
pub(super) const HINTS: &[Hint] = &[Hint {
    id: "empty_page",
    command: "tool.place",
    text: "Press {key} to place a symbol · {keys} for keys",
}];

/// `Label · Key`, or just the label when the command has no key.
pub(crate) fn tip_text(keymap: &Keymap, label: &str, command: &str) -> String {
    match keymap.shortcut_text(command) {
        Some(key) => format!("{label} · {key}"),
        None => label.to_owned(),
    }
}

/// Commands whose tooltips are drawn where the keymap is out of reach (a
/// free function with only the `Ui`); [`publish_keys`] puts their keys in
/// the egui context each frame and [`hover`] reads them back.
pub(super) const TIP_COMMANDS: &[&str] = &[
    "view.3d_door",
    "heat.toggle",
    "view.cabinet_table",
    "sim.live",
    "cabinet.rails",
    "view.command_palette",
    "edit.undo",
];

type TipKeys = std::sync::Arc<std::collections::BTreeMap<&'static str, String>>;

fn tip_keys_id() -> egui::Id {
    egui::Id::new("tip_keys")
}

/// The live keys of [`TIP_COMMANDS`], for [`hover`].
pub(super) fn publish_keys(ctx: &egui::Context, keymap: &Keymap) {
    let keys: std::collections::BTreeMap<_, _> = TIP_COMMANDS
        .iter()
        .filter_map(|c| keymap.shortcut_text(c).map(|k| (*c, k)))
        .collect();
    ctx.data_mut(|d| d.insert_temp(tip_keys_id(), TipKeys::new(keys)));
}

/// The live key of a [`TIP_COMMANDS`] command, or `fallback` when it has none.
pub(super) fn key_or(ctx: &egui::Context, command: &str, fallback: &str) -> String {
    debug_assert!(
        TIP_COMMANDS.contains(&command),
        "add `{command}` to TIP_COMMANDS"
    );
    ctx.data(|d| d.get_temp::<TipKeys>(tip_keys_id()))
        .and_then(|keys| keys.get(command).cloned())
        .unwrap_or_else(|| fallback.to_owned())
}

/// `Label · Key` for a [`TIP_COMMANDS`] command, from any drawing code
/// that has the context. Never type a key into a tooltip: add the command
/// to the list and call this.
pub(super) fn hover(ctx: &egui::Context, label: &str, command: &str) -> String {
    debug_assert!(
        TIP_COMMANDS.contains(&command),
        "add `{command}` to TIP_COMMANDS"
    );
    let key = ctx
        .data(|d| d.get_temp::<TipKeys>(tip_keys_id()))
        .and_then(|keys| keys.get(command).cloned());
    match key {
        Some(key) => format!("{label} · {key}"),
        None => label.to_owned(),
    }
}

/// A hint's text with its key filled in.
pub(super) fn hint_text(keymap: &Keymap, hint: &Hint) -> String {
    let key = keymap
        .shortcut_text(hint.command)
        .unwrap_or_else(|| "Ctrl+K".to_owned());
    let sheet = keymap
        .shortcut_text("view.shortcuts")
        .unwrap_or_else(|| "Ctrl+K".to_owned());
    hint.text.replace("{key}", &key).replace("{keys}", &sheet)
}

/// Whether a hint still shows.
pub(super) fn hint_due(settings: &Settings, id: &str) -> bool {
    settings.hints_seen.get(id).copied().unwrap_or(0) < HINT_LIMIT
}

impl NovoApp {
    /// A command ran: counts a use for the hint that teaches it.
    pub(super) fn hint_command_run(&mut self, command: &str) {
        for hint in HINTS.iter().filter(|h| h.command == command) {
            let n = self
                .settings
                .hints_seen
                .entry(hint.id.to_owned())
                .or_insert(0);
            *n = (*n + 1).min(HINT_LIMIT);
        }
    }

    /// The hint for an empty page, while it is still due.
    pub(super) fn empty_page_hint(&self) -> Option<String> {
        let hint = HINTS.iter().find(|h| h.id == "empty_page")?;
        if !hint_due(&self.settings, hint.id) {
            return None;
        }
        let page = self.current_page()?;
        let doc = self.backend.document()?;
        let empty = !doc.project().symbols().any(|s| s.page == page);
        empty.then(|| hint_text(&self.wb.keymap, hint))
    }

    /// `help.show_hints`: every hint shows again.
    pub(super) fn show_hints_again(&mut self) {
        self.settings.hints_seen.clear();
        self.message(
            super::Level::Info,
            "Hints will show again until you have used them",
        );
    }
}
