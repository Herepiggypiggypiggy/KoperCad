//! The shortcut sheet (`?`, Help menu, `view.shortcuts`): a searchable list
//! generated from the live keymap and the command registry, so a remapped
//! key shows its real key and a new command appears with no list to edit
//! (docs/design/discoverability.md, layer 1).

use eframe::egui;
use novo_workflow::commands::{COMMANDS, GROUPS};
use novo_workflow::keymap::{Keymap, Scope};
use novo_workflow::language;

use super::NovoApp;
use super::workbench::app_runs;

/// Key and pointer behaviours that are not a single key press, so the
/// keymap cannot list them. Shown as text rows under "Hold and tap".
pub(super) const GESTURES: &[(&str, &str, &[&str])] = &[
    (
        "Q (hold)",
        "Opens the place wheel; a tap places a symbol",
        &["wheel", "place", "radial"],
    ),
    (
        "Space (hold) + drag",
        "Pans the drawing",
        &["pan", "move", "scroll"],
    ),
    (
        "Shift while moving a symbol",
        "Does the other of leave wires / take wires along",
        &["move", "wires", "carry"],
    ),
    (
        "Shift+W while drawing a wire",
        "Switches between corners and straight lines",
        &["wire", "corner", "lines"],
    ),
    (
        "Q while drawing a wire",
        "Ends the wire on the part under the pointer",
        &["wire", "end", "part"],
    ),
    (
        "Tab while drawing a wire",
        "Cycles the connection point of a multi-pin part",
        &["wire", "pin", "cycle"],
    ),
    (
        "Mouse wheel",
        "Zooms around the pointer",
        &["zoom", "scroll"],
    ),
];

/// One line of the sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    /// The command, for a click; `None` for a text row.
    pub command: Option<&'static str>,
    pub label: &'static str,
    /// Keys as the keymap writes them, the first one the main key.
    pub keys: Vec<String>,
    /// "on the drawing" and the like, when the key is not global.
    pub scope: Option<&'static str>,
    /// Why the row cannot be clicked ("Comes later"), when it cannot.
    pub unavailable: Option<&'static str>,
    /// For text rows: the behaviour in a sentence.
    pub note: Option<&'static str>,
}

/// A group of rows under a heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Section {
    pub heading: &'static str,
    pub rows: Vec<Row>,
}

/// The scope text for a binding, `None` when the key works everywhere.
fn scope_text(scope: Scope) -> Option<&'static str> {
    match scope {
        Scope::Anywhere => None,
        Scope::Canvas => Some("on the drawing"),
        Scope::OutsideText => Some("outside text fields"),
        Scope::PlaceFieldEmpty => Some("in the empty place field"),
    }
}

/// Whether `row` matches the filter: every word of it must start a word of
/// the label, the command's keywords, verbs or Dutch words, or a key.
/// English and Dutch both always match, as in the command field.
fn matches(filter: &str, haystack: &[String]) -> bool {
    let wanted: Vec<String> = filter.split_whitespace().map(str::to_lowercase).collect();
    wanted.iter().all(|w| {
        haystack
            .iter()
            .flat_map(|h| h.split(|c: char| !c.is_alphanumeric() && c != '+'))
            .any(|word| word.to_lowercase().starts_with(w.as_str()))
    })
}

/// The sheet for `keymap` and `filter`: one section per command group, then
/// the hold-and-tap behaviours. Commands without a key are listed too (the
/// palette is their way in); a command the app cannot run yet is greyed
/// "Comes later". Empty sections are left out.
pub(crate) fn sections(keymap: &Keymap, filter: &str) -> Vec<Section> {
    let mut out = Vec::new();
    for group in GROUPS {
        let mut rows = Vec::new();
        for info in COMMANDS.iter().filter(|c| c.group == group) {
            let bindings: Vec<_> = keymap
                .bindings()
                .iter()
                .filter(|b| b.command == info.id)
                .collect();
            let keys: Vec<String> = bindings.iter().map(|b| b.chord.to_string()).collect();
            if !filter.trim().is_empty() {
                let mut hay: Vec<String> = vec![info.label.to_owned(), info.id.to_owned()];
                hay.extend(info.keywords.iter().map(|s| (*s).to_owned()));
                hay.extend(info.verbs.iter().map(|s| (*s).to_owned()));
                if let Some(nl) = language::dutch(info.id) {
                    hay.extend(nl.verbs.iter().map(|s| (*s).to_owned()));
                    hay.extend(nl.words.iter().map(|s| (*s).to_owned()));
                }
                hay.extend(keys.iter().cloned());
                if !matches(filter, &hay) {
                    continue;
                }
            }
            let key_only = EDITOR_ONLY.contains(&info.id);
            rows.push(Row {
                command: Some(info.id),
                label: info.label,
                scope: bindings.first().and_then(|b| scope_text(b.scope)),
                keys,
                unavailable: if app_runs(info.id) {
                    None
                } else {
                    Some("Comes later")
                },
                note: key_only.then_some("Works with its key on the drawing"),
            });
        }
        if !rows.is_empty() {
            out.push(Section {
                heading: group,
                rows,
            });
        }
    }
    let gestures: Vec<Row> = GESTURES
        .iter()
        .filter(|(keys, note, words)| {
            filter.trim().is_empty() || {
                let mut hay = vec![(*keys).to_owned(), (*note).to_owned()];
                hay.extend(words.iter().map(|w| (*w).to_owned()));
                matches(filter, &hay)
            }
        })
        .map(|(keys, note, _)| Row {
            command: None,
            label: keys,
            keys: Vec::new(),
            scope: None,
            unavailable: None,
            note: Some(note),
        })
        .collect();
    if !gestures.is_empty() {
        out.push(Section {
            heading: "Hold and tap",
            rows: gestures,
        });
    }
    out
}

/// Commands whose key the editor reads itself: the row runs from the key,
/// and a click would do nothing useful.
const EDITOR_ONLY: &[&str] = &["wire.end_on_part", "wire.next_pin"];

/// State of the open sheet.
#[derive(Default)]
pub(crate) struct ShortcutSheet {
    pub open: bool,
    pub filter: String,
}

impl NovoApp {
    /// `view.shortcuts`: opens the sheet, or closes it when it is open.
    pub(super) fn toggle_shortcuts(&mut self) {
        self.wb.shortcuts.open = !self.wb.shortcuts.open;
        self.wb.shortcuts.filter.clear();
    }

    pub(super) fn shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.wb.shortcuts.open {
            return;
        }
        // Esc clears the filter first, a second Esc closes (the modal sees
        // an Esc that is still there).
        if !self.wb.shortcuts.filter.is_empty()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.wb.shortcuts.filter.clear();
        }
        let sections = sections(&self.wb.keymap, &self.wb.shortcuts.filter);
        let mut run: Option<&'static str> = None;
        let modal = egui::Modal::new(egui::Id::new("shortcut_sheet")).show(ctx, |ui| {
            ui.set_width(novo_theme::dialog_width::MEDIUM);
            ui.horizontal(|ui| {
                super::dialog_ui::title(ui, "Keyboard shortcuts", "");
                ui.weak("Ctrl+K finds every command");
            });
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.wb.shortcuts.filter)
                    .hint_text("Type to filter (English or Dutch)")
                    .desired_width(f32::INFINITY),
            );
            field.request_focus();
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(ctx.content_rect().height() * 0.6)
                .show(ui, |ui| {
                    if sections.is_empty() {
                        ui.weak("Nothing matches. Ctrl+K searches every command.");
                    }
                    for section in &sections {
                        ui.add_space(6.0);
                        novo_theme::widgets::section_label(ui, section.heading);
                        for row in &section.rows {
                            if let Some(id) = row_ui(ui, row) {
                                run = Some(id);
                            }
                        }
                    }
                });
        });
        if let Some(id) = run {
            self.wb.shortcuts.open = false;
            self.settings.remember_action(id);
            self.wb.queued.push(id);
        } else if modal.should_close() {
            self.wb.shortcuts.open = false;
        }
    }
}

/// Draws one row; returns the command when it was clicked.
fn row_ui(ui: &mut egui::Ui, row: &Row) -> Option<&'static str> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        match (row.command, row.unavailable) {
            (Some(id), None) if row.note.is_none() => {
                if ui.link(row.label).clicked() {
                    clicked = Some(id);
                }
            }
            (Some(_), Some(_)) => {
                ui.add_enabled(false, egui::Label::new(row.label));
            }
            _ => {
                ui.label(row.label);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(why) = row.unavailable {
                ui.weak(why);
            } else if let Some(note) = row.note {
                ui.weak(note);
            }
            if !row.keys.is_empty() {
                ui.monospace(row.keys.join("  /  "));
            }
            if let Some(scope) = row.scope {
                ui.weak(scope);
            }
        });
    });
    clicked
}
