//! Key presses to novo-workflow commands (WS-08's keymap).
//!
//! Each frame the shell turns egui key events into [`Chord`]s, decides what
//! has keyboard focus ([`Focus`]) and asks [`Keymap::resolve`] which command
//! a press runs (while a wire is being drawn, [`Keymap::resolve_while_wiring`],
//! so Q ends the wire on a part and Tab cycles its pins). Presses the shell acts on are taken out of the input; the
//! rest (the editor's W, R, X, F2, Delete, Escape, Page Up/Down) stay for the
//! schematic editor, which reads them itself.

use eframe::egui;
use novo_workflow::keymap::{Chord, Focus, Key, Keymap};

/// The keymap chord for an egui key press; `None` for keys the keymap has
/// no name for.
pub fn chord_of(key: egui::Key, modifiers: egui::Modifiers) -> Option<Chord> {
    use egui::Key as E;
    let key = match key {
        E::Enter => Key::Enter,
        E::Escape => Key::Escape,
        E::Tab => Key::Tab,
        E::Space => Key::Space,
        E::Delete => Key::Delete,
        E::Home => Key::Home,
        E::End => Key::End,
        E::PageUp => Key::PageUp,
        E::PageDown => Key::PageDown,
        E::ArrowLeft => Key::ArrowLeft,
        E::ArrowRight => Key::ArrowRight,
        E::ArrowUp => Key::ArrowUp,
        E::ArrowDown => Key::ArrowDown,
        E::Num0 => Key::Digit(0),
        E::Num1 => Key::Digit(1),
        E::Num2 => Key::Digit(2),
        E::Num3 => Key::Digit(3),
        E::Num4 => Key::Digit(4),
        E::Num5 => Key::Digit(5),
        E::Num6 => Key::Digit(6),
        E::Num7 => Key::Digit(7),
        E::Num8 => Key::Digit(8),
        E::Num9 => Key::Digit(9),
        other => {
            let name = other.name();
            if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
                Key::F(n)
            } else {
                let mut chars = name.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if c.is_ascii_alphabetic() => Key::letter(c),
                    _ => return None,
                }
            }
        }
    };
    Some(Chord {
        ctrl: modifiers.command || modifiers.ctrl || modifiers.mac_cmd,
        shift: modifiers.shift,
        alt: modifiers.alt,
        key,
    })
}

/// Takes every key press this frame that `wants` claims, in order, and
/// returns the commands. A claimed letter or Space also takes this frame's
/// typed text, so the field it opens does not start with that letter.
pub fn take_commands(
    ctx: &egui::Context,
    keymap: &Keymap,
    focus: Focus,
    wiring: bool,
    mut wants: impl FnMut(&'static str) -> bool,
) -> Vec<&'static str> {
    let mut fired = Vec::new();
    let mut typed_key = false;
    ctx.input_mut(|input| {
        input.events.retain(|event| {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                repeat,
                ..
            } = event
            else {
                return true;
            };
            let Some(chord) = chord_of(*key, *modifiers) else {
                return true;
            };
            let resolved = if wiring {
                keymap.resolve_while_wiring(chord, focus)
            } else {
                keymap.resolve(chord, focus)
            };
            match resolved {
                Some(command) if wants(command) => {
                    // A held Shift+W would flip the mode on every repeat, and
                    // a held place key would re-arm its hold and, on the
                    // release, open the part search under the wheel.
                    if !(*repeat && matches!(command, "wire.mode" | "tool.place")) {
                        fired.push(command);
                    }
                    typed_key |= chord.types_text();
                    false
                }
                _ => true,
            }
        });
        if typed_key {
            input.events.retain(|e| !matches!(e, egui::Event::Text(_)));
        }
    });
    fired
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_digits_and_function_keys_map() {
        let none = egui::Modifiers::NONE;
        assert_eq!(
            chord_of(egui::Key::G, egui::Modifiers::SHIFT),
            Some(Chord::shift(Key::letter('g')))
        );
        assert_eq!(
            chord_of(egui::Key::Num3, egui::Modifiers::COMMAND),
            Some(Chord::ctrl(Key::Digit(3)))
        );
        assert_eq!(chord_of(egui::Key::F8, none), Some(Chord::plain(Key::F(8))));
        assert_eq!(
            chord_of(egui::Key::ArrowLeft, egui::Modifiers::ALT),
            Some(Chord::alt(Key::ArrowLeft))
        );
        assert_eq!(chord_of(egui::Key::Minus, none), None);
    }

    #[test]
    fn the_keymap_resolves_mapped_presses() {
        let keymap = Keymap::workflow();
        let resolve = |key, m| keymap.resolve(chord_of(key, m).unwrap(), Focus::Canvas);
        assert_eq!(
            resolve(egui::Key::K, egui::Modifiers::COMMAND),
            Some("view.command_palette")
        );
        assert_eq!(
            resolve(egui::Key::I, egui::Modifiers::NONE),
            Some("tool.place")
        );
        assert_eq!(
            resolve(egui::Key::Space, egui::Modifiers::NONE),
            Some("view.command_field_here")
        );
        assert_eq!(
            resolve(egui::Key::Num3, egui::Modifiers::COMMAND),
            Some("view.terminals_drawer")
        );
    }
}
