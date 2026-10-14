//! One look and one part per terminal type on a strip.
//!
//! All terminals of one kind on a strip share a symbol and a part, stored
//! once on the strip device, so 400 terminals look and report the same and
//! changing the type changes all of them. A single terminal differs only
//! when it carries an override on purpose, and [`overrides`] lists those.
//!
//! | Where | Key | Value |
//! | --- | --- | --- |
//! | strip device | `terminal_symbol.<kind>` | symbol key, e.g. `iec/terminals/terminal` |
//! | strip device | `terminal_part.<kind>` | part id |
//! | terminal | `symbol` | symbol key override |
//! | terminal | `part` | part id override |
//!
//! `<kind>` is [`kind_key`]: `feedthrough`, `protective_earth`, `neutral`,
//! `fuse` or `disconnect`.

use novo_core::{
    DeviceId, PartId, Project, Result, SymbolInstance, SymbolKey, SymbolRole, TerminalId,
    TerminalKind, Tx, Value,
};

use crate::geometry::{redraw, symbol_pins};
use crate::naming::kind_key;

/// The built-in symbol for a terminal kind (novo-symbols keys).
pub fn default_symbol(kind: TerminalKind) -> &'static str {
    match kind {
        TerminalKind::Feedthrough => "iec/terminals/terminal",
        TerminalKind::ProtectiveEarth => "iec/terminals/terminal_pe",
        TerminalKind::Neutral => "iec/terminals/terminal_n",
        TerminalKind::Fuse => "iec/terminals/terminal_fused",
        TerminalKind::Disconnect => "iec/terminals/terminal_disconnect",
    }
}

fn symbol_key(kind: TerminalKind) -> String {
    format!("terminal_symbol.{}", kind_key(kind))
}

fn part_key(kind: TerminalKind) -> String {
    format!("terminal_part.{}", kind_key(kind))
}

fn as_part(value: Option<&Value>) -> Option<PartId> {
    match value {
        Some(Value::Int(raw)) => u64::try_from(*raw).ok().map(PartId),
        _ => None,
    }
}

fn as_text(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::Text(t)) if !t.is_empty() => Some(t.clone()),
        _ => None,
    }
}

/// The symbol and part a terminal kind uses on a strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    pub symbol: SymbolKey,
    pub part: Option<PartId>,
}

/// The strip-wide style of a terminal kind.
pub fn strip_style(project: &Project, strip: DeviceId, kind: TerminalKind) -> Style {
    let props = project.device(strip).map(|d| &d.properties);
    let get = |key: String| props.and_then(|p| p.get(&key));
    Style {
        symbol: SymbolKey(
            as_text(get(symbol_key(kind))).unwrap_or_else(|| default_symbol(kind).into()),
        ),
        part: as_part(get(part_key(kind))).filter(|&p| project.part(p).is_some()),
    }
}

/// The style a terminal actually uses: its own override, else its strip's.
pub fn style_of(project: &Project, terminal: TerminalId) -> Option<Style> {
    let t = project.terminal(terminal)?;
    let strip = strip_style(project, t.strip, t.kind);
    Some(Style {
        symbol: as_text(t.properties.get("symbol"))
            .map(SymbolKey)
            .unwrap_or(strip.symbol),
        part: as_part(t.properties.get("part"))
            .filter(|&p| project.part(p).is_some())
            .or(strip.part),
    })
}

/// Terminals that deliberately differ from their strip's style.
pub fn overrides(project: &Project, strip: DeviceId) -> Vec<TerminalId> {
    project
        .terminals_of_strip(strip)
        .into_iter()
        .filter(|t| t.properties.contains_key("symbol") || t.properties.contains_key("part"))
        .map(|t| t.id)
        .collect()
}

fn draws_only_inherited(project: &Project, symbol: &SymbolInstance) -> bool {
    let SymbolRole::Terminal { terminals } = &symbol.role else {
        return false;
    };
    terminals.len() == 1
        && project
            .terminal(terminals[0])
            .is_some_and(|t| !t.properties.contains_key("symbol"))
}

/// Sets the style of one terminal kind on a strip and redraws every terminal
/// of that kind that has no override of its own with the new symbol,
/// refitting its connection points (see [`crate::redraw`]). Refused, with
/// nothing changed, for a symbol that is not a single-level terminal symbol
/// or when a drawing's wires leave no room for it.
pub fn set_strip_style(
    tx: &mut Tx<'_>,
    strip: DeviceId,
    kind: TerminalKind,
    style: &Style,
) -> Result<()> {
    symbol_pins(&style.symbol)?;
    tx.set_property(strip, symbol_key(kind), Some(style.symbol.0.clone().into()))?;
    let part = style
        .part
        .map(|p| Value::Int(i64::try_from(p.raw()).unwrap_or(i64::MAX)));
    tx.set_property(strip, part_key(kind), part)?;
    let symbols: Vec<_> = tx
        .project()
        .terminals_of_strip(strip)
        .into_iter()
        .filter(|t| t.kind == kind)
        .flat_map(|t| tx.project().symbols_of_terminal(t.id))
        .filter(|s| draws_only_inherited(tx.project(), s))
        .map(|s| s.id)
        .collect();
    for symbol in symbols {
        redraw(tx, symbol, &style.symbol)?;
    }
    Ok(())
}

/// Removes a terminal's own symbol and part, so it follows its strip again,
/// and redraws it with the strip's symbol (see [`crate::redraw`]).
pub fn clear_override(tx: &mut Tx<'_>, terminal: TerminalId) -> Result<()> {
    tx.set_property(terminal, "symbol", None)?;
    tx.set_property(terminal, "part", None)?;
    let Some(t) = tx.project().terminal(terminal) else {
        return Ok(());
    };
    let symbol = strip_style(tx.project(), t.strip, t.kind).symbol;
    let symbols: Vec<_> = tx
        .project()
        .symbols_of_terminal(terminal)
        .into_iter()
        .filter(|s| draws_only_inherited(tx.project(), s))
        .map(|s| s.id)
        .collect();
    for id in symbols {
        redraw(tx, id, &symbol)?;
    }
    Ok(())
}
