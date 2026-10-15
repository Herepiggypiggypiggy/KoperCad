//! Connection points of terminal symbols, read from the novo-symbols
//! library, and redrawing a placed terminal with another symbol.
//!
//! A drawn terminal keeps its own copy of its symbol's connection points.
//! When a kind change or a style swap gives it another symbol, the points
//! are refitted to the new symbol, so the drawn connection points stay
//! where the wires end. Pin `1` sits at the symbol origin in every
//! single-level terminal symbol, so the internal wire never moves; pin `2`
//! slides along the terminal's axis, and the wire on it gets shorter or
//! longer. When that would fold or bend the wire, the change is refused.

use std::sync::OnceLock;

use novo_core::{
    Direction, Endpoint, ModelError, Pin, Point, Result, SymbolId, SymbolInstance, SymbolKey, Tx,
};
use novo_symbols::{Library, Symbol};

use crate::strings;

fn library() -> Option<&'static Library> {
    static LIBRARY: OnceLock<Option<Library>> = OnceLock::new();
    LIBRARY.get_or_init(|| Library::builtin().ok()).as_ref()
}

fn direction(d: novo_symbols::Direction) -> Direction {
    match d {
        novo_symbols::Direction::Up => Direction::Up,
        novo_symbols::Direction::Down => Direction::Down,
        novo_symbols::Direction::Left => Direction::Left,
        novo_symbols::Direction::Right => Direction::Right,
    }
}

/// Connection points of a single-level terminal symbol from the library:
/// `1` at the origin and `2` straight below it, with the library's
/// direction and `optional` flag, so they match the pins the editor draws. Any other symbol, or one
/// the library does not know, is refused, because a terminal could not be
/// put into a wire with it.
pub fn symbol_pins(symbol: &SymbolKey) -> Result<Vec<Pin>> {
    library()
        .and_then(|l| l.get(&symbol.0))
        .and_then(terminal_pins)
        .ok_or_else(|| ModelError::Format(strings::not_a_terminal_symbol(symbol)))
}

fn terminal_pins(definition: &Symbol) -> Option<Vec<Pin>> {
    let pins: Vec<Pin> = definition
        .connections
        .iter()
        .map(|c| {
            let pin = Pin::new(c.id.clone(), c.at[0], c.at[1]).with_direction(direction(c.dir));
            if c.optional { pin.optional() } else { pin }
        })
        .collect();
    match pins.as_slice() {
        [one, two]
            if one.offset == Point::new(0.0, 0.0) && two.offset.x == 0.0 && two.offset.y > 0.0 =>
        {
            Some(pins)
        }
        _ => None,
    }
}

/// The one-ended drawing of a terminal symbol: the same terminal with one
/// connection point, a wire ending in the clamp. `None` for a symbol that
/// has no such drawing (fused and disconnect terminals).
pub fn one_ended(symbol: &SymbolKey) -> Option<SymbolKey> {
    let id = match symbol.0.as_str() {
        "iec/terminals/terminal" => "iec/terminals/terminal_end",
        "iec/terminals/terminal_pe" => "iec/terminals/terminal_pe_end",
        "iec/terminals/terminal_n" => "iec/terminals/terminal_n_end",
        "iec/terminals/terminal_bridged" => "iec/terminals/terminal_end_bridged",
        _ => return None,
    };
    Some(SymbolKey(id.into()))
}

/// Connection point of a one-ended terminal symbol from the library: `1` at
/// the origin. Anything else is refused.
fn end_pins(symbol: &SymbolKey) -> Result<Vec<Pin>> {
    let pins = library()
        .and_then(|l| l.get(&symbol.0))
        .map(|d| {
            d.connections
                .iter()
                .map(|c| {
                    let pin =
                        Pin::new(c.id.clone(), c.at[0], c.at[1]).with_direction(direction(c.dir));
                    if c.optional { pin.optional() } else { pin }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    match pins.as_slice() {
        [one] if one.offset == Point::new(0.0, 0.0) => Ok(pins),
        _ => Err(ModelError::Format(strings::not_a_terminal_symbol(symbol))),
    }
}

/// Length of a terminal symbol from `1` to `2`, in millimetres.
pub fn symbol_length(symbol: &SymbolKey) -> Result<f64> {
    Ok(symbol_pins(symbol)?[1].offset.y)
}

/// The point next to an end of a wire: its first or last route point, or
/// the other end.
fn neighbour(tx: &Tx<'_>, wire: novo_core::WireId, end: Endpoint) -> Option<Point> {
    let w = tx.project().wire(wire)?;
    if w.from == end {
        w.route
            .first()
            .copied()
            .or_else(|| tx.project().endpoint_position(w.to))
    } else {
        w.route
            .last()
            .copied()
            .or_else(|| tx.project().endpoint_position(w.from))
    }
}

/// Whether moving a wire end from `old` to `new` keeps the wire's first
/// segment straight and pointing the same way: the neighbour `q` lies on
/// the line through `old` and `new`, beyond `new`.
fn still_fits(old: Point, new: Point, q: Point) -> bool {
    const EPS: f64 = 1e-9;
    let (ax, ay) = (new.x - old.x, new.y - old.y);
    let (bx, by) = (q.x - old.x, q.y - old.y);
    let collinear = (ax * by - ay * bx).abs() < EPS;
    let (cx, cy) = (q.x - new.x, q.y - new.y);
    // `q` is on the far side of `new`, as seen from `old`'s segment.
    collinear && cx * bx + cy * by > EPS
}

/// Redraws a placed terminal with another symbol and refits its connection
/// points, as part of a transaction. Refused when the symbol is not a
/// single-level terminal symbol or the wires leave no room.
pub fn redraw(tx: &mut Tx<'_>, id: SymbolId, symbol: &SymbolKey) -> Result<()> {
    let current = tx
        .project()
        .symbol(id)
        .ok_or(ModelError::NotFound(id.into()))?
        .clone();
    // A one-ended drawing stays one-ended: it takes the one-ended variant of
    // the new symbol.
    let one_ended_drawing = current.pins.len() == 1;
    let symbol = &if one_ended_drawing {
        one_ended(symbol).unwrap_or_else(|| symbol.clone())
    } else {
        symbol.clone()
    };
    let pins = if one_ended_drawing {
        end_pins(symbol)?
    } else {
        symbol_pins(symbol)?
    };
    if current.symbol == *symbol && current.pins == pins {
        return Ok(());
    }
    if current.pins.len() != pins.len() {
        return Err(ModelError::Format(strings::not_a_terminal_symbol(symbol)));
    }
    let refitted = SymbolInstance {
        pins: pins.clone(),
        ..current.clone()
    };
    for pin in 0..u16::try_from(pins.len()).unwrap_or(u16::MAX) {
        let (Some(old), Some(new)) = (current.pin_position(pin), refitted.pin_position(pin)) else {
            continue;
        };
        if old.distance(new) < 1e-9 {
            continue;
        }
        let end = Endpoint::pin(id, pin);
        for wire in tx.project().wires_at(end) {
            let fits = neighbour(tx, wire, end).is_some_and(|q| still_fits(old, new, q));
            if !fits {
                return Err(ModelError::Format(strings::no_room_to_refit(symbol)));
            }
        }
    }
    tx.update::<SymbolInstance>(id, |s| {
        s.symbol = symbol.clone();
        s.pins = pins;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_pins_keep_the_library_optional_flag() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../novo-symbols/symbols/iec/terminals/terminal.toml"
        ))
        .unwrap();
        let pin_two = r#"{ id = "2", at = [0, 8], dir = "down" }"#;
        assert!(
            text.contains(pin_two),
            "terminal.toml no longer has {pin_two}"
        );
        let text = text.replace(
            pin_two,
            r#"{ id = "2", at = [0, 8], dir = "down", optional = true }"#,
        );
        let symbol = Symbol::from_toml(&text).unwrap();
        let pins = terminal_pins(&symbol).unwrap();
        assert!(!pins[0].optional && pins[1].optional);
    }
}
