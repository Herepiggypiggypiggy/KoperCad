//! Tips and hints (docs/design/discoverability.md layers 3 and 4).
//!
//! [`tip_text`] gives a control its tooltip with the key read from the
//! live keymap, so a remapped key reads right and no key is typed by hand.
//! Hints teach once: each has an id, a command that follows it, and a
//! counter in `Settings::hints_seen`; after [`HINT_LIMIT`] follows it stops
//! showing. Help > Show hints again clears the counters.

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
pub(super) const HINTS: &[Hint] = &[
    Hint {
        id: "empty_page",
        command: "tool.place",
        text: "Press {key} to place a symbol · {keys} for keys",
    },
    Hint {
        id: "first_wire",
        command: "tool.connect",
        text: "Press {key} to connect pins · click one pin, then another",
    },
];

/// `Label · Key`, or just the label when the command has no key.
pub(crate) fn tip_text(keymap: &Keymap, label: &str, command: &str) -> String {
    match keymap.shortcut_text(command) {
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

/// What a page still needs said (see [`NovoApp::empty_page_hint`]).
pub(super) fn page_hint(
    settings: &Settings,
    keymap: &Keymap,
    project: &novo_core::Project,
    page: novo_core::PageId,
) -> Option<String> {
    let symbols = project.symbols().filter(|s| s.page == page).count();
    let id = match symbols {
        0 => "empty_page",
        1 => return None,
        _ if project.wires().any(|w| w.page == page) => return None,
        _ => "first_wire",
    };
    let hint = HINTS.iter().find(|h| h.id == id)?;
    hint_due(settings, id).then(|| hint_text(keymap, hint))
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

    /// The hint for the page being drawn, while it is still due: the first
    /// move on an empty page, the first wire once two symbols sit there.
    pub(super) fn empty_page_hint(&self) -> Option<String> {
        let page = self.current_page()?;
        let project = self.backend.document()?.project();
        page_hint(&self.settings, &self.wb.keymap, project, page)
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
