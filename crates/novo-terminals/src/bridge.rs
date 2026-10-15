//! Connected terminals: terminals of one strip joined by a jumper and drawn
//! as one bridged symbol (a heavy bar across the upper leads, a dot on each
//! lead), side by side at the multi-level pitch. Connecting swaps their
//! single-terminal drawings for the bridged symbol and moves their wires
//! onto it; disconnecting does the reverse. The jumper in the model is what
//! the netlist reads, so the join holds with or without the drawing.

use novo_core::{
    Endpoint, ModelError, Placement, Result, Rotation, SymbolId, SymbolInstance, SymbolKey,
    SymbolRole, TerminalId, Tx, Wire,
};

use crate::geometry::{library_pins, symbol_pins};
use crate::strings;
use crate::strip::{free_jumper, set_jumper};
use crate::style::style_of;

/// Distance between neighbouring terminals of a bridged symbol, in
/// millimetres (the pitch of the multi-level terminals, on the 4 mm grid).
pub const BRIDGE_PITCH: f64 = 8.0;

/// The bridged symbol for `count` terminals (2 or 3).
pub fn bridged_symbol(count: usize) -> Option<SymbolKey> {
    matches!(count, 2 | 3).then(|| SymbolKey(format!("iec/terminals/terminal_bridged_{count}")))
}

/// Moves every wire end on `from`'s pin `pin` to `to`'s pin `to_pin`.
fn move_wire_ends(
    tx: &mut Tx<'_>,
    from: SymbolId,
    pin: u16,
    to: SymbolId,
    to_pin: u16,
) -> Result<()> {
    let old = Endpoint::pin(from, pin);
    let new = Endpoint::pin(to, to_pin);
    let wires: Vec<_> = tx.project().wires_at(old).collect();
    for wire in wires {
        tx.update::<Wire>(wire, |w| {
            if w.from == old {
                w.from = new;
            }
            if w.to == old {
                w.to = new;
            }
        })?;
    }
    Ok(())
}

/// The one single-terminal drawing of each terminal, in terminal order, or
/// the reason there is not exactly one.
fn single_drawings(tx: &Tx<'_>, terminals: &[TerminalId]) -> Result<Vec<SymbolInstance>> {
    let mut out = Vec::new();
    for &t in terminals {
        let mut symbols = tx.project().symbols_of_terminal(t).into_iter();
        let (Some(symbol), None) = (symbols.next(), symbols.next()) else {
            return Err(ModelError::Format(strings::cannot_connect(
                "each must be drawn exactly once",
            )));
        };
        let single =
            matches!(&symbol.role, SymbolRole::Terminal { terminals } if terminals.len() == 1);
        if !single || !symbol.feeds.is_empty() {
            return Err(ModelError::Format(strings::cannot_connect(
                "one is already part of another symbol or has a potential feed",
            )));
        }
        out.push(symbol.clone());
    }
    Ok(out)
}

/// Joins 2 or 3 terminals of one strip with a jumper and redraws them as one
/// bridged symbol. They must be drawn on one page, single, upright, side by
/// side and [`BRIDGE_PITCH`] apart in a row, with the same height as the
/// bridged symbol; nothing is changed otherwise. Returns the jumper group.
pub fn connect(tx: &mut Tx<'_>, terminals: &[TerminalId]) -> Result<u32> {
    let Some(key) = bridged_symbol(terminals.len()) else {
        return Err(ModelError::Format(strings::cannot_connect(
            "select 2 or 3 terminals",
        )));
    };
    let drawings = single_drawings(tx, terminals)?;
    let mut order: Vec<usize> = (0..drawings.len()).collect();
    order.sort_by(|&a, &b| {
        drawings[a]
            .placement
            .origin
            .x
            .total_cmp(&drawings[b].placement.origin.x)
    });
    let first = drawings[order[0]].clone();
    let upright = Placement {
        rotation: Rotation::R0,
        mirrored: false,
        ..first.placement
    };
    let pins = library_pins(&key)?;
    for (i, &d) in order.iter().enumerate() {
        let s = &drawings[d];
        let expected = first.placement.origin.x + BRIDGE_PITCH * i as f64;
        let fits = s.page == first.page
            && s.placement.rotation == Rotation::R0
            && !s.placement.mirrored
            && (s.placement.origin.x - expected).abs() < 1e-6
            && (s.placement.origin.y - first.placement.origin.y).abs() < 1e-6
            && s.pins.len() == 2
            && s.pins[1].offset == pins[1].offset;
        if !fits {
            return Err(ModelError::Format(strings::cannot_connect(
                "they must sit side by side, 8 mm apart, upright and as tall as a standard terminal",
            )));
        }
    }
    let mut ordered: Vec<TerminalId> = Vec::new();
    for &d in &order {
        let SymbolRole::Terminal { terminals } = &drawings[d].role else {
            unreachable!("single_drawings checked the role")
        };
        ordered.push(terminals[0]);
    }
    let group = terminals
        .iter()
        .find_map(|&t| tx.project().terminal(t).and_then(|t| t.jumper))
        .unwrap_or_else(|| {
            let strip = tx.project().terminal(terminals[0]).map(|t| t.strip);
            strip.map_or(1, |s| free_jumper(tx.project(), s))
        });
    set_jumper(tx, terminals, Some(group))?;
    let bridges = (0..terminals.len() as u16)
        .map(|i| vec![2 * i, 2 * i + 1])
        .collect();
    // The model checks a multi-terminal symbol when it is inserted, so its
    // bridges go in with it.
    let page = first.page;
    let new = tx.insert::<SymbolInstance>(|id| SymbolInstance {
        id,
        page,
        symbol: key,
        placement: upright,
        role: SymbolRole::Terminal { terminals: ordered },
        pins,
        bridges,
        pass_through: Vec::new(),
        feeds: Vec::new(),
        properties: Default::default(),
    })?;
    for (i, &d) in order.iter().enumerate() {
        for pin in 0..2u16 {
            move_wire_ends(tx, drawings[d].id, pin, new, 2 * i as u16 + pin)?;
        }
        tx.remove(drawings[d].id)?;
    }
    Ok(group)
}

/// Splits a bridged symbol back into one drawing per terminal, in the
/// strip's style, and takes the terminals out of their jumper. Does nothing
/// and returns `false` when `symbol` is not a bridged symbol.
pub fn disconnect(tx: &mut Tx<'_>, symbol: SymbolId) -> Result<bool> {
    let Some(current) = tx.project().symbol(symbol).cloned() else {
        return Err(ModelError::NotFound(symbol.into()));
    };
    let SymbolRole::Terminal { terminals } = &current.role else {
        return Ok(false);
    };
    if !current
        .symbol
        .0
        .starts_with("iec/terminals/terminal_bridged_")
    {
        return Ok(false);
    }
    let mut singles = Vec::new();
    for (i, &t) in terminals.iter().enumerate() {
        let key = style_of(tx.project(), t)
            .map(|s| s.symbol)
            .ok_or(ModelError::NotFound(t.into()))?;
        let pins = symbol_pins(&key)?;
        if pins[1].offset.y != current.pins[2 * i + 1].offset.y {
            return Err(ModelError::Format(strings::cannot_connect(
                "the strip's symbol is taller than the bridged one",
            )));
        }
        singles.push((t, key, pins));
    }
    for (i, (t, key, pins)) in singles.into_iter().enumerate() {
        let placement = Placement {
            origin: novo_core::Point::new(
                current.placement.origin.x + BRIDGE_PITCH * i as f64,
                current.placement.origin.y,
            ),
            ..current.placement
        };
        let id = tx.place_symbol(
            current.page,
            key,
            placement,
            pins,
            SymbolRole::Terminal { terminals: vec![t] },
        )?;
        for pin in 0..2u16 {
            move_wire_ends(tx, symbol, 2 * i as u16 + pin, id, pin)?;
        }
    }
    tx.remove(symbol)?;
    let terminals = terminals.clone();
    set_jumper(tx, &terminals, None)?;
    Ok(true)
}
