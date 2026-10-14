//! The keys from docs/design/interaction-model.md: a small set for frequent
//! actions, everything else through the command field.
//!
//! Letters act only while the drawing has focus; Ctrl chords are commands
//! that work anywhere. There are deliberately no keys for corners, T-pieces,
//! terminals, cables or potentials: the user does not place those. A plain key never steals typing from a
//! text field (principle 10): while a text field has focus only chords with
//! Ctrl or Alt, function keys and Escape resolve.

use std::fmt;

/// A key, independent of the UI toolkit. The app shell maps its own key
/// events onto these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    Letter(char),
    Digit(u8),
    F(u8),
    Enter,
    Escape,
    Tab,
    Space,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
}

impl Key {
    /// A letter key; lower case is folded to upper case.
    pub fn letter(ch: char) -> Key {
        Key::Letter(ch.to_ascii_uppercase())
    }

    /// Keys that type a character into a text field when pressed without
    /// Ctrl or Alt.
    pub fn types_text(self) -> bool {
        matches!(
            self,
            Key::Letter(_) | Key::Digit(_) | Key::Enter | Key::Tab | Key::Space | Key::Delete
        ) || matches!(
            self,
            Key::Home | Key::End | Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
        )
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Letter(ch) => write!(f, "{ch}"),
            Key::Digit(d) => write!(f, "{d}"),
            Key::F(n) => write!(f, "F{n}"),
            Key::Enter => f.write_str("Enter"),
            Key::Escape => f.write_str("Esc"),
            Key::Tab => f.write_str("Tab"),
            Key::Space => f.write_str("Space"),
            Key::Delete => f.write_str("Delete"),
            Key::Home => f.write_str("Home"),
            Key::End => f.write_str("End"),
            Key::PageUp => f.write_str("Page Up"),
            Key::PageDown => f.write_str("Page Down"),
            Key::ArrowLeft => f.write_str("Left"),
            Key::ArrowRight => f.write_str("Right"),
            Key::ArrowUp => f.write_str("Up"),
            Key::ArrowDown => f.write_str("Down"),
        }
    }
}

/// A key with its modifiers. Ctrl means Cmd on macOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: Key,
}

impl Chord {
    pub const fn plain(key: Key) -> Chord {
        Chord {
            ctrl: false,
            shift: false,
            alt: false,
            key,
        }
    }

    pub const fn ctrl(key: Key) -> Chord {
        Chord {
            ctrl: true,
            ..Chord::plain(key)
        }
    }

    pub const fn ctrl_shift(key: Key) -> Chord {
        Chord {
            ctrl: true,
            shift: true,
            ..Chord::plain(key)
        }
    }

    pub const fn shift(key: Key) -> Chord {
        Chord {
            shift: true,
            ..Chord::plain(key)
        }
    }

    pub const fn alt(key: Key) -> Chord {
        Chord {
            alt: true,
            ..Chord::plain(key)
        }
    }

    /// Whether the chord would type into a focused text field instead of
    /// running a command.
    pub fn types_text(self) -> bool {
        !self.ctrl && !self.alt && self.key.types_text()
    }

    /// Parses `Ctrl+Shift+S`, `Alt+Left`, `G`, `Page Down`. Case-insensitive.
    pub fn parse(text: &str) -> Option<Chord> {
        let mut chord = Chord::plain(Key::Escape);
        let mut key = None;
        for part in text.split('+').map(str::trim) {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "cmd" | "control" => chord.ctrl = true,
                "shift" => chord.shift = true,
                "alt" | "option" => chord.alt = true,
                other => {
                    if key.is_some() {
                        return None;
                    }
                    key = Some(parse_key(other)?);
                }
            }
        }
        chord.key = key?;
        Some(chord)
    }
}

fn parse_key(text: &str) -> Option<Key> {
    Some(match text {
        "enter" | "return" => Key::Enter,
        "esc" | "escape" => Key::Escape,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "delete" | "del" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "page up" | "pageup" | "pgup" => Key::PageUp,
        "page down" | "pagedown" | "pgdn" => Key::PageDown,
        "left" => Key::ArrowLeft,
        "right" => Key::ArrowRight,
        "up" => Key::ArrowUp,
        "down" => Key::ArrowDown,
        _ => {
            let mut chars = text.chars();
            let first = chars.next()?;
            let rest: String = chars.collect();
            if rest.is_empty() {
                if first.is_ascii_alphabetic() {
                    Key::letter(first)
                } else {
                    Key::Digit(u8::try_from(first.to_digit(10)?).ok()?)
                }
            } else if first == 'f' {
                let n: u8 = rest.parse().ok()?;
                if !(1..=24).contains(&n) {
                    return None;
                }
                Key::F(n)
            } else {
                return None;
            }
        }
    })
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        write!(f, "{}", self.key)
    }
}

/// Where a binding is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Everywhere in the window, also while a panel or text field has focus.
    Anywhere,
    /// Only while the drawing has keyboard focus.
    Canvas,
    /// While the drawing or a panel without a text field has focus. A text
    /// field keeps the chord for itself (its own undo and redo).
    OutsideText,
    /// Only while the command field is open in place mode with nothing
    /// typed yet (favourites 1 to 9), so typing `m3` or `13` still searches.
    PlaceFieldEmpty,
}

/// What has keyboard focus right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Focus {
    /// The drawing.
    Canvas,
    /// The command field in place mode, still empty.
    PlaceFieldEmpty,
    /// The command field with text in it, or any other text field.
    TextField,
    /// A panel or drawer without a text field.
    Panel,
}

impl Focus {
    const ALL: [Focus; 4] = [
        Focus::Canvas,
        Focus::PlaceFieldEmpty,
        Focus::TextField,
        Focus::Panel,
    ];

    fn allows(self, scope: Scope) -> bool {
        match scope {
            Scope::Anywhere => true,
            Scope::Canvas => self == Focus::Canvas,
            Scope::OutsideText => !self.is_text_entry(),
            Scope::PlaceFieldEmpty => self == Focus::PlaceFieldEmpty,
        }
    }

    fn is_text_entry(self) -> bool {
        matches!(self, Focus::TextField | Focus::PlaceFieldEmpty)
    }
}

/// One shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub chord: Chord,
    pub scope: Scope,
    /// A command id from [`crate::commands`].
    pub command: &'static str,
}

/// All shortcuts. Several chords may run the same command (Ctrl+Y and
/// Ctrl+Shift+Z both redo); a chord runs at most one command per focus.
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: Vec<Binding>,
}

impl Default for Keymap {
    fn default() -> Self {
        Keymap::workflow()
    }
}

impl Keymap {
    /// The keys of docs/design/interaction-model.md (section 7). Provisional
    /// until docs/DESIGN.md is final.
    pub fn workflow() -> Keymap {
        use Key::*;
        use Scope::*;
        let mut bindings = Vec::new();
        let mut bind = |chord: Chord, scope: Scope, command: &'static str| {
            bindings.push(Binding {
                chord,
                scope,
                command,
            });
        };
        let l = Key::letter;

        // Project and the command field.
        bind(Chord::ctrl(l('k')), Anywhere, "view.command_palette");
        bind(Chord::plain(Space), Canvas, "view.command_field_here");
        bind(Chord::ctrl(l('n')), Anywhere, "file.new");
        bind(Chord::ctrl(l('o')), Anywhere, "file.open");
        bind(Chord::ctrl(l('s')), Anywhere, "file.save");
        bind(Chord::ctrl_shift(l('s')), Anywhere, "file.save_as");
        bind(Chord::ctrl(l('w')), Anywhere, "file.close");
        bind(Chord::ctrl(l('q')), Anywhere, "file.quit");

        // Edit. Undo and redo belong to a text field while one has focus
        // (F2 tag editing, the command field), so the project's undo runs
        // only outside text. Clipboard and selection chords are
        // canvas-scoped for the same reason.
        bind(Chord::ctrl(l('z')), OutsideText, "edit.undo");
        bind(Chord::ctrl_shift(l('z')), OutsideText, "edit.redo");
        bind(Chord::ctrl(l('y')), OutsideText, "edit.redo");
        bind(Chord::ctrl(l('c')), Canvas, "edit.copy");
        bind(Chord::ctrl(l('x')), Canvas, "edit.cut");
        bind(Chord::ctrl(l('v')), Canvas, "edit.paste");
        bind(Chord::ctrl_shift(l('v')), Canvas, "edit.paste_keep_tags");
        bind(Chord::ctrl(l('d')), Canvas, "edit.duplicate");
        bind(Chord::ctrl(l('a')), Canvas, "edit.select_all");
        bind(Chord::plain(Delete), Canvas, "edit.delete");
        bind(Chord::plain(F(2)), Canvas, "edit.rename");
        bind(Chord::plain(Escape), Anywhere, "edit.escape");

        // Draw.
        bind(Chord::plain(l('i')), Canvas, "tool.place");
        bind(Chord::plain(l('w')), Canvas, "tool.connect");
        bind(Chord::plain(l('r')), Canvas, "edit.rotate");
        bind(Chord::plain(l('x')), Canvas, "edit.mirror");
        for digit in 1..=9 {
            bind(
                Chord::plain(Digit(digit)),
                PlaceFieldEmpty,
                FAVOURITE_COMMANDS[usize::from(digit - 1)],
            );
        }

        // Proposals and problems.
        bind(Chord::plain(Enter), Canvas, "proposal.accept");
        bind(Chord::plain(Tab), Canvas, "proposal.next");
        bind(Chord::shift(l('p')), Canvas, "view.toggle_marks");
        bind(Chord::plain(F(8)), Anywhere, "check.next_problem");
        bind(Chord::shift(F(8)), Anywhere, "check.previous_problem");

        // Navigate.
        bind(Chord::plain(l('g')), Canvas, "nav.partner");
        bind(Chord::shift(l('g')), Canvas, "nav.all_references");
        bind(Chord::alt(ArrowLeft), Anywhere, "nav.back");
        bind(Chord::alt(ArrowRight), Anywhere, "nav.forward");
        bind(Chord::plain(PageUp), Canvas, "nav.previous_page");
        bind(Chord::plain(PageDown), Canvas, "nav.next_page");

        // View: lenses and drawers.
        bind(Chord::plain(Home), Canvas, "view.fit");
        bind(Chord::ctrl(Digit(1)), Anywhere, "view.schematic");
        bind(Chord::ctrl(Digit(2)), Anywhere, "view.cabinet");
        bind(Chord::ctrl(Digit(3)), Anywhere, "view.terminals_drawer");
        bind(Chord::ctrl(Digit(4)), Anywhere, "view.cables_drawer");
        bind(Chord::ctrl(Digit(5)), Anywhere, "view.devices_drawer");
        bind(Chord::ctrl_shift(l('f')), Anywhere, "view.focus");
        bind(Chord::plain(F(11)), Anywhere, "view.full_screen");
        bind(Chord::ctrl(l('t')), Anywhere, "view.cabinet_table");
        Keymap { bindings }
    }

    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// The command a key press runs with this focus, if any.
    pub fn resolve(&self, chord: Chord, focus: Focus) -> Option<&'static str> {
        // Typing keys type, except the empty place field's favourite digits.
        let favourite = focus == Focus::PlaceFieldEmpty && matches!(chord.key, Key::Digit(_));
        if focus.is_text_entry() && chord.types_text() && !favourite {
            return None;
        }
        self.bindings
            .iter()
            .find(|b| b.chord == chord && focus.allows(b.scope))
            .map(|b| b.command)
    }

    /// The chords for a command, in the order they were bound. The first
    /// one is what menus and the palette show.
    pub fn chords_for(&self, command: &str) -> Vec<Chord> {
        self.bindings
            .iter()
            .filter(|b| b.command == command)
            .map(|b| b.chord)
            .collect()
    }

    /// The shortcut text menus and the palette show for a command.
    pub fn shortcut_text(&self, command: &str) -> Option<String> {
        self.chords_for(command).first().map(Chord::to_string)
    }

    /// Pairs of bindings that would fire for the same chord in some focus,
    /// for different commands. The default keymap has none; a user keymap
    /// is checked with this before it is used.
    pub fn conflicts(&self) -> Vec<(&Binding, &Binding)> {
        let mut out = Vec::new();
        for (i, a) in self.bindings.iter().enumerate() {
            for b in &self.bindings[i + 1..] {
                if a.chord == b.chord && a.command != b.command && scopes_overlap(a.scope, b.scope)
                {
                    out.push((a, b));
                }
            }
        }
        out
    }

    /// Rebinds a command, replacing its chords. For user keymaps.
    pub fn rebind(&mut self, command: &'static str, chord: Chord, scope: Scope) {
        self.bindings.retain(|b| b.command != command);
        self.bindings.push(Binding {
            chord,
            scope,
            command,
        });
    }
}

/// Command ids for favourites 1 to 9.
pub const FAVOURITE_COMMANDS: [&str; 9] = [
    "insert.favourite_1",
    "insert.favourite_2",
    "insert.favourite_3",
    "insert.favourite_4",
    "insert.favourite_5",
    "insert.favourite_6",
    "insert.favourite_7",
    "insert.favourite_8",
    "insert.favourite_9",
];

fn scopes_overlap(a: Scope, b: Scope) -> bool {
    Focus::ALL.iter().any(|f| f.allows(a) && f.allows(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_act_only_on_the_canvas() {
        let map = Keymap::workflow();
        let w = Chord::plain(Key::letter('w'));
        assert_eq!(map.resolve(w, Focus::Canvas), Some("tool.connect"));
        assert_eq!(map.resolve(w, Focus::TextField), None);
        assert_eq!(map.resolve(w, Focus::Panel), None);
        assert_eq!(map.resolve(w, Focus::PlaceFieldEmpty), None);
    }

    #[test]
    fn ctrl_chords_work_anywhere() {
        let map = Keymap::workflow();
        let k = Chord::ctrl(Key::letter('k'));
        for focus in [Focus::Canvas, Focus::TextField, Focus::Panel] {
            assert_eq!(map.resolve(k, focus), Some("view.command_palette"));
        }
        // Ctrl+C in a text field is the text field's copy.
        assert_eq!(
            map.resolve(Chord::ctrl(Key::letter('c')), Focus::TextField),
            None
        );
    }

    #[test]
    fn undo_and_redo_yield_to_text_fields() {
        let map = Keymap::workflow();
        let chords = [
            (Chord::ctrl(Key::letter('z')), "edit.undo"),
            (Chord::ctrl_shift(Key::letter('z')), "edit.redo"),
            (Chord::ctrl(Key::letter('y')), "edit.redo"),
        ];
        for (chord, command) in chords {
            // Typing in F2 tag editing or the command field: the field's
            // own undo, not the project's.
            assert_eq!(map.resolve(chord, Focus::TextField), None, "{chord}");
            assert_eq!(map.resolve(chord, Focus::PlaceFieldEmpty), None, "{chord}");
            // On the drawing or in a panel without a text field: the
            // project's undo.
            assert_eq!(map.resolve(chord, Focus::Canvas), Some(command), "{chord}");
            assert_eq!(map.resolve(chord, Focus::Panel), Some(command), "{chord}");
        }
    }

    #[test]
    fn tab_and_enter_work_proposals_not_orientation() {
        let map = Keymap::workflow();
        assert_eq!(
            map.resolve(Chord::plain(Key::Tab), Focus::Canvas),
            Some("proposal.next")
        );
        assert_eq!(
            map.resolve(Chord::plain(Key::Enter), Focus::Canvas),
            Some("proposal.accept")
        );
        assert_eq!(map.resolve(Chord::plain(Key::Tab), Focus::TextField), None);
        assert_eq!(
            map.resolve(Chord::plain(Key::letter('r')), Focus::Canvas),
            Some("edit.rotate")
        );
        assert_eq!(
            map.resolve(Chord::plain(Key::letter('x')), Focus::Canvas),
            Some("edit.mirror")
        );
    }

    #[test]
    fn no_keys_for_things_the_user_does_not_place() {
        let map = Keymap::workflow();
        for letter in ['t', 'c', 'p', 'j'] {
            assert_eq!(
                map.resolve(Chord::plain(Key::letter(letter)), Focus::Canvas),
                None
            );
        }
    }

    #[test]
    fn favourites_only_in_the_empty_place_field() {
        let map = Keymap::workflow();
        let three = Chord::plain(Key::Digit(3));
        assert_eq!(
            map.resolve(three, Focus::PlaceFieldEmpty),
            Some("insert.favourite_3")
        );
        assert_eq!(map.resolve(three, Focus::Canvas), None);
        // Once something is typed the field is a text field: 3 types.
        assert_eq!(map.resolve(three, Focus::TextField), None);
    }

    #[test]
    fn lenses_and_drawers() {
        let map = Keymap::workflow();
        let ctrl = |d| map.resolve(Chord::ctrl(Key::Digit(d)), Focus::Canvas);
        assert_eq!(ctrl(1), Some("view.schematic"));
        assert_eq!(ctrl(2), Some("view.cabinet"));
        assert_eq!(ctrl(3), Some("view.terminals_drawer"));
        assert_eq!(ctrl(4), Some("view.cables_drawer"));
        assert_eq!(ctrl(5), Some("view.devices_drawer"));
        assert_eq!(
            map.resolve(Chord::plain(Key::F(8)), Focus::Panel),
            Some("check.next_problem")
        );
    }

    #[test]
    fn default_keymap_has_no_conflicts() {
        let map = Keymap::workflow();
        let conflicts = map.conflicts();
        assert!(conflicts.is_empty(), "{conflicts:?}");
    }

    #[test]
    fn no_typing_key_is_bound_anywhere() {
        // A plain letter, digit, Tab or Delete bound outside the canvas
        // would steal typing from a text field.
        for b in Keymap::workflow().bindings() {
            if b.chord.types_text() {
                assert_ne!(b.scope, Scope::Anywhere, "{} {}", b.chord, b.command);
            }
        }
    }

    #[test]
    fn redo_has_two_chords_and_shows_the_first() {
        let map = Keymap::workflow();
        assert_eq!(map.chords_for("edit.redo").len(), 2);
        // Ctrl+Shift+Z first, as the app shell's menu shows it.
        assert_eq!(
            map.shortcut_text("edit.redo").as_deref(),
            Some("Ctrl+Shift+Z")
        );
    }

    #[test]
    fn chords_round_trip_through_text() {
        for text in [
            "Ctrl+Shift+S",
            "Alt+Left",
            "G",
            "Shift+G",
            "Page Down",
            "F2",
            "Ctrl+1",
        ] {
            let chord = Chord::parse(text).unwrap();
            assert_eq!(chord.to_string(), text);
        }
        assert_eq!(Chord::parse("ctrl+k"), Some(Chord::ctrl(Key::letter('K'))));
        assert_eq!(Chord::parse("Ctrl+K+J"), None);
        assert_eq!(Chord::parse("F99"), None);
    }

    #[test]
    fn rebinding_can_introduce_a_conflict_that_is_reported() {
        let mut map = Keymap::workflow();
        map.rebind("tool.cable", Chord::plain(Key::letter('w')), Scope::Canvas);
        assert_eq!(map.conflicts().len(), 1);
    }
}
