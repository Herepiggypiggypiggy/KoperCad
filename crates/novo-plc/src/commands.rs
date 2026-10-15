//! PLC edits as single transactions of existing model commands (plc.md
//! §10.4): each runs inside one [`Tx`], so it is one undo step on the path
//! scripts and the AI assistant share. Nothing here adds a `Command`
//! variant.

use novo_core::{
    DeviceId, DeviceKind, DeviceTag, Endpoint, ModelError, PageId, PartId, Pin, Placement, Project,
    Result, SymbolId, SymbolRole, Tx, Value,
};

use crate::card::{CHANNEL, CHANNELS, Card, SLOT, STATION, allocate, cards, start_key};
use crate::index::{Functions, PlcIndex, channel_key};
use crate::layout::{CardLayout, ChannelType, Space};

/// Where a new card's channels come from.
#[derive(Debug, Clone, PartialEq)]
pub enum NewCardLayout {
    /// A catalog part already in the project, with `plc.channels`.
    Part(PartId),
    /// A layout typed for a card without a part yet (`DI16`), stored on
    /// the device as an override.
    Typed(CardLayout),
}

/// Adds a PLC card: the device, its station and slot, its part or layout,
/// and its start addresses after every card already in the station.
/// Adding a card never moves another card's addresses.
pub fn add_card(
    tx: &mut Tx,
    tag: DeviceTag,
    description: &str,
    layout: NewCardLayout,
    station: &str,
    slot: Option<u16>,
) -> Result<DeviceId> {
    let card = tx.add_device_of_kind(tag, description, DeviceKind::Plc)?;
    if !station.is_empty() {
        tx.set_property(card, STATION, Some(Value::from(station)))?;
    }
    if let Some(slot) = slot {
        tx.set_property(card, SLOT, Some(Value::Int(i64::from(slot))))?;
    }
    match layout {
        NewCardLayout::Part(part) => tx.assign_part(card, part)?,
        NewCardLayout::Typed(layout) => {
            tx.set_property(card, CHANNELS, Some(Value::from(layout.to_string())))?;
        }
    }
    let device = tx
        .project()
        .device(card)
        .ok_or(ModelError::NotFound(card.into()))?;
    let read = Card::read(tx.project(), device).ok_or(ModelError::NotFound(card.into()))?;
    let layout = read.layout.map_err(|e| ModelError::Format(e.to_string()))?;
    for (key, value) in allocate(tx.project(), station, &layout, Some(card)) {
        tx.set_property(card, key, Some(value))?;
    }
    Ok(card)
}

/// Places a channel symbol of `card` and gives it the next channel of
/// `kind` not drawn yet (`"DI 3"`). `symbol` and `pins` come from the
/// symbol library, as for any placement. Fails when the card has no free
/// channel of that type.
#[allow(clippy::too_many_arguments)]
pub fn place_channel(
    tx: &mut Tx,
    functions: &dyn Functions,
    card: DeviceId,
    kind: ChannelType,
    page: PageId,
    symbol: &str,
    placement: Placement,
    pins: Vec<Pin>,
) -> Result<SymbolId> {
    let number = PlcIndex::build(tx.project(), functions)
        .next_channel(card, kind)
        .ok_or_else(|| ModelError::Format(format!("the card has no free {kind} channel")))?;
    let placed = tx.place_symbol(
        page,
        symbol,
        placement,
        pins,
        SymbolRole::Device {
            device: card,
            main: false,
        },
    )?;
    tx.set_property(
        placed,
        CHANNEL,
        Some(Value::from(channel_key(kind, number))),
    )?;
    Ok(placed)
}

/// The channel type `to plc` gives a signal symbol, from its function: a
/// contact (NO, NC or changeover; fault and E-stop feedback is usually NC)
/// is read on a DI, a transmitter or RTD on an AI, and a coil, valve, lamp,
/// horn, bell or buzzer is driven from a DQ. `None` for anything else.
pub fn signal_channel_type(function: &str) -> Option<ChannelType> {
    let head = function.split('.').next().unwrap_or("");
    match head {
        "contact" => Some(ChannelType::DigitalInput),
        "sensor" => Some(ChannelType::AnalogInput),
        "coil" | "signal" => Some(ChannelType::DigitalOutput),
        _ => None,
    }
}

/// `to plc`: places the next free channel of the right type for `signal`
/// on `card` and connects it to the signal's pin, in one step.
#[allow(clippy::too_many_arguments)]
pub fn to_plc(
    tx: &mut Tx,
    functions: &dyn Functions,
    card: DeviceId,
    signal: Endpoint,
    kind: ChannelType,
    symbol: &str,
    placement: Placement,
    pins: Vec<Pin>,
) -> Result<SymbolId> {
    let page = signal
        .as_pin()
        .and_then(|p| tx.project().symbol(p.symbol))
        .map(|s| s.page)
        .ok_or_else(|| ModelError::Format("to plc needs a pin to connect".into()))?;
    let placed = place_channel(tx, functions, card, kind, page, symbol, placement, pins)?;
    tx.connect(signal, Endpoint::pin(placed, 0))?;
    Ok(placed)
}

/// One start address `renumber io` would change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartMove {
    pub card: DeviceId,
    pub space: Space,
    pub from: Option<u32>,
    pub to: u32,
}

/// What `renumber io` would do to a station: its cards re-packed in slot
/// order from byte 0 in each space, analog cards on a word. Shown as the
/// preview.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenumberPlan {
    /// Start addresses that move; empty when nothing moves.
    pub moves: Vec<StartMove>,
    /// Cards left where they are because their layout does not parse, so
    /// their size is unknown. The preview lists them; their ranges are not
    /// reserved, so fix the layout and renumber again.
    pub skipped: Vec<DeviceId>,
}

impl RenumberPlan {
    /// True when nothing moves and nothing was skipped.
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty() && self.skipped.is_empty()
    }
}

/// Plans `renumber io` for one station.
pub fn renumber_plan(project: &Project, station: &str) -> RenumberPlan {
    let mut plan = RenumberPlan::default();
    let mut next = [0u32; 2];
    for card in cards(project).iter().filter(|c| c.station == station) {
        let Ok(layout) = &card.layout else {
            plan.skipped.push(card.device);
            continue;
        };
        for space in Space::ALL {
            let bytes = layout.bytes(space);
            if bytes == 0 {
                continue;
            }
            let cursor = &mut next[space as usize];
            if layout.has_analog(space) {
                *cursor = cursor.div_ceil(2) * 2;
            }
            let from = card.start[space as usize];
            if from != Some(*cursor) {
                plan.moves.push(StartMove {
                    card: card.device,
                    space,
                    from,
                    to: *cursor,
                });
            }
            *cursor += bytes;
        }
    }
    plan
}

/// Applies a [`renumber_plan`]: the only edit that moves addresses.
pub fn apply_renumber(tx: &mut Tx, plan: &RenumberPlan) -> Result<()> {
    for m in &plan.moves {
        tx.set_property(
            m.card,
            start_key(m.space),
            Some(Value::Int(i64::from(m.to))),
        )?;
    }
    Ok(())
}
