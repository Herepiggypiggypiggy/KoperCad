//! The user's keymap file (`keymap.txt` in the KoperCAD folder under
//! `%APPDATA%`): a plain text list of overrides applied on top of
//! [`Keymap::workflow`].
//!
//! ```text
//! # A comment. One binding per line: command = [scope:] chord
//! tool.cable = Ctrl+Shift+C
//! edit.duplicate = anywhere: Ctrl+D
//! edit.duplicate = Alt+D          # a second chord for the same command
//! tool.repeat = none              # no key at all
//! ```
//!
//! The first line for a command replaces its default chords; further lines
//! add chords. The scope (`canvas`, `anywhere`, `panel`) defaults to the one
//! the command has already, else `anywhere` for chords with Ctrl, Alt or a
//! function key and `canvas` for plain keys. A line that names no command,
//! has a chord that does not parse, or takes a chord another command already
//! uses is skipped and reported; the rest still apply, so a typo never
//! leaves the app without keys.

use std::collections::BTreeSet;

use crate::commands;
use crate::keymap::{Binding, Chord, FAVOURITE_COMMANDS, Key, Keymap, Scope};

/// A line of the file that was not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 1-based line number.
    pub line: usize,
    pub message: String,
}

impl Keymap {
    /// Applies the user's keymap file; see the module docs for the format.
    /// Returns the lines that were skipped, in file order.
    pub fn apply_user_file(&mut self, text: &str) -> Vec<Problem> {
        let mut problems = Vec::new();
        let mut replaced: BTreeSet<&'static str> = BTreeSet::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let content = raw.split('#').next().unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }
            let mut problem = |message: String| problems.push(Problem { line, message });
            let Some((name, value)) = content.split_once('=') else {
                problem("expected `command = chord`".into());
                continue;
            };
            let name = name.trim();
            let Some(command) = command_id(name) else {
                problem(format!("unknown command `{name}`"));
                continue;
            };
            let value = value.trim();
            if value.eq_ignore_ascii_case("none") {
                self.bindings.retain(|b| b.command != command);
                replaced.insert(command);
                continue;
            }
            let (scope_name, chord_text) = match value.split_once(':') {
                Some((scope, rest)) => (Some(scope.trim()), rest.trim()),
                None => (None, value),
            };
            let Some(chord) = Chord::parse(chord_text) else {
                problem(format!("`{chord_text}` is not a key"));
                continue;
            };
            let scope = match scope_name {
                Some(s) => match parse_scope(s) {
                    Some(scope) => scope,
                    None => {
                        problem(format!("unknown scope `{s}` (canvas, anywhere or panel)"));
                        continue;
                    }
                },
                None => self
                    .bindings
                    .iter()
                    .find(|b| b.command == command)
                    .map(|b| b.scope)
                    .unwrap_or_else(|| default_scope(chord)),
            };
            let candidate = Binding {
                chord,
                scope,
                command,
            };
            if let Some(other) = self.clash(&candidate) {
                problem(format!(
                    "{chord} is already {}",
                    commands::get(other)
                        .map(|c| c.label)
                        .unwrap_or("another command")
                ));
                continue;
            }
            if replaced.insert(command) {
                self.bindings.retain(|b| b.command != command);
            }
            self.bindings.push(candidate);
        }
        problems
    }

    /// The command another binding gives `candidate`'s chord in an
    /// overlapping scope.
    fn clash(&self, candidate: &Binding) -> Option<&'static str> {
        self.bindings
            .iter()
            .find(|b| {
                b.command != candidate.command
                    && b.chord == candidate.chord
                    && crate::keymap::scopes_overlap(b.scope, candidate.scope)
            })
            .map(|b| b.command)
    }
}

fn command_id(name: &str) -> Option<&'static str> {
    commands::get(name)
        .map(|c| c.id)
        .or_else(|| FAVOURITE_COMMANDS.iter().copied().find(|&id| id == name))
}

fn parse_scope(text: &str) -> Option<Scope> {
    match text.to_ascii_lowercase().as_str() {
        "canvas" => Some(Scope::Canvas),
        "anywhere" => Some(Scope::Anywhere),
        "panel" | "outside-text" => Some(Scope::OutsideText),
        _ => None,
    }
}

fn default_scope(chord: Chord) -> Scope {
    if chord.ctrl || chord.alt || matches!(chord.key, Key::F(_)) {
        Scope::Anywhere
    } else {
        Scope::Canvas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Focus;

    #[test]
    fn a_line_replaces_the_default_chords_of_its_command() {
        let mut map = Keymap::workflow();
        let problems = map.apply_user_file("tool.cable = Ctrl+Shift+C\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            map.shortcut_text("tool.cable").as_deref(),
            Some("Ctrl+Shift+C")
        );
        let c = Chord::plain(Key::letter('c'));
        assert_eq!(map.resolve(c, Focus::Canvas), None, "the old key is free");
        assert_eq!(
            map.resolve(Chord::ctrl_shift(Key::letter('c')), Focus::Canvas),
            Some("tool.cable")
        );
    }

    #[test]
    fn more_lines_add_chords_and_none_unbinds() {
        let mut map = Keymap::workflow();
        let text = "edit.duplicate = Alt+D\nedit.duplicate = Alt+Shift+D\ntool.repeat = none\n";
        assert!(map.apply_user_file(text).is_empty());
        assert_eq!(map.chords_for("edit.duplicate").len(), 2);
        assert!(map.chords_for("tool.repeat").is_empty());
    }

    #[test]
    fn mistakes_are_reported_by_line_and_the_rest_still_applies() {
        let mut map = Keymap::workflow();
        let text = "\
# comment
nothing here
no.such.command = F5
tool.cable = Ctrl+Hyper+9
tool.cable = elsewhere: F6
edit.duplicate = Ctrl+Z   # taken by undo
tool.repeat = F7
";
        let problems = map.apply_user_file(text);
        let lines: Vec<usize> = problems.iter().map(|p| p.line).collect();
        assert_eq!(lines, vec![2, 3, 4, 5, 6], "{problems:?}");
        assert!(problems[4].message.contains("Undo"), "{problems:?}");
        assert_eq!(map.shortcut_text("tool.repeat").as_deref(), Some("F7"));
        assert_eq!(
            map.shortcut_text("edit.duplicate").as_deref(),
            Some("Ctrl+D")
        );
        assert!(map.conflicts().is_empty());
    }

    #[test]
    fn an_empty_file_changes_nothing() {
        let mut map = Keymap::workflow();
        assert!(map.apply_user_file("\n# only a comment\n").is_empty());
        assert_eq!(map.bindings(), Keymap::workflow().bindings());
    }
}
