//! PLC cards read from the model: a card is a `Device` with
//! `kind = Plc`, and everything PLC-specific is in its `plc.*` properties.

use std::ops::Range;

use novo_core::{Device, DeviceId, DeviceKind, Project, Properties, Value};

use crate::address::{Address, AddressStyle};
use crate::layout::{CardLayout, ChannelType, LayoutError, Space};

/// Card layout (`DI16 DQ8`), on a part, or on a device as an override.
pub const CHANNELS: &str = "plc.channels";
/// The station a card belongs to (text). Empty or absent is the project's
/// one unnamed station.
pub const STATION: &str = "plc.station";
/// The card's slot in its station (integer).
pub const SLOT: &str = "plc.slot";
/// The card's first byte in the input space (integer).
pub const START_INPUT: &str = "plc.start.i";
/// The card's first byte in the output space (integer).
pub const START_OUTPUT: &str = "plc.start.q";
/// Project property holding the address style (`iec`, `siemens`,
/// `rockwell`, `plain`).
pub const ADDRESS_STYLE: &str = "plc.address_style";
/// On a channel symbol: which channel it draws, type and number in one
/// text, `"DI 3"` (plc.md §2).
pub const CHANNEL: &str = "plc.channel";
/// On a channel symbol: the signal text, when typed.
pub const SIGNAL: &str = "plc.signal";

/// The property holding a card's start in a space.
pub fn start_key(space: Space) -> &'static str {
    match space {
        Space::Input => START_INPUT,
        Space::Output => START_OUTPUT,
    }
}

/// The project's address style; an absent or unknown value reads as the
/// default (IEC 61131).
pub fn address_style(project: &Project) -> AddressStyle {
    match project.info().properties.get(ADDRESS_STYLE) {
        Some(Value::Text(s)) => s.parse().unwrap_or_default(),
        _ => AddressStyle::default(),
    }
}

/// Where a card's layout came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutSource {
    /// The card's own `plc.channels`, an override of its part.
    Device,
    /// The first assigned part with `plc.channels`.
    Part(novo_core::PartId),
    /// Neither: the card has no channels.
    None,
}

/// A PLC card as the PLC engine sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub device: DeviceId,
    /// Station name; empty for the project's unnamed station.
    pub station: String,
    pub slot: Option<u16>,
    /// The card's channels. `Err` when the layout text is wrong, which the
    /// design checks report; an empty layout when it has none.
    pub layout: Result<CardLayout, LayoutError>,
    pub layout_source: LayoutSource,
    /// Stored start byte per space (`[input, output]`), if allocated.
    pub start: [Option<u32>; 2],
}

impl Card {
    /// Reads a card, or `None` when the device is not a PLC device.
    pub fn read(project: &Project, device: &Device) -> Option<Card> {
        if device.kind != DeviceKind::Plc {
            return None;
        }
        let props = &device.properties;
        let (layout_text, layout_source) = match text(props, CHANNELS) {
            Some(t) => (Some(t), LayoutSource::Device),
            None => device
                .parts
                .iter()
                .find_map(|&id| {
                    let t = text(&project.part(id)?.properties, CHANNELS)?;
                    Some((Some(t), LayoutSource::Part(id)))
                })
                .unwrap_or((None, LayoutSource::None)),
        };
        let layout = match layout_text {
            Some(t) => t.parse(),
            None => Ok(CardLayout::default()),
        };
        Some(Card {
            device: device.id,
            station: text(props, STATION).unwrap_or_default(),
            slot: int(props, SLOT).and_then(|n| u16::try_from(n).ok()),
            layout,
            layout_source,
            start: Space::ALL
                .map(|space| int(props, start_key(space)).and_then(|n| u32::try_from(n).ok())),
        })
    }

    /// The layout, or an empty one when its text is wrong.
    pub fn channels(&self) -> CardLayout {
        self.layout.clone().unwrap_or_default()
    }

    /// Bytes the card takes in a space.
    pub fn bytes(&self, space: Space) -> u32 {
        self.channels().bytes(space)
    }

    /// The byte range the card takes in a space, when it has channels
    /// there and a start.
    pub fn range(&self, space: Space) -> Option<Range<u32>> {
        let bytes = self.bytes(space);
        let start = self.start[space as usize]?;
        (bytes > 0).then(|| start..start + bytes)
    }

    /// The address of channel `number` of a type, when the card has that
    /// channel and a start in its space.
    pub fn address(&self, kind: ChannelType, number: u16) -> Option<Address> {
        let channel = self.channels().channel(kind, number)?;
        let start = self.start[kind.space() as usize]?;
        Some(Address::of(&channel, start, self.slot))
    }
}

/// Every PLC card, by station, then slot (cards without a slot last), then
/// device id.
pub fn cards(project: &Project) -> Vec<Card> {
    let mut cards: Vec<Card> = project
        .devices()
        .filter_map(|d| Card::read(project, d))
        .collect();
    cards.sort_by(|a, b| {
        (&a.station, a.slot.is_none(), a.slot, a.device).cmp(&(
            &b.station,
            b.slot.is_none(),
            b.slot,
            b.device,
        ))
    });
    cards
}

/// The start a new card of `layout` would get in `space` of `station`: the
/// first byte after every range already there, on a word when the card has
/// analog channels in that space. `None` when the layout has nothing in
/// that space. `except` leaves one card out (the card being re-allocated).
/// Cards already there never move.
pub fn next_start(
    project: &Project,
    station: &str,
    layout: &CardLayout,
    space: Space,
    except: Option<DeviceId>,
) -> Option<u32> {
    if layout.bytes(space) == 0 {
        return None;
    }
    let end = cards(project)
        .iter()
        .filter(|c| c.station == station && Some(c.device) != except)
        .filter_map(|c| c.range(space))
        .map(|r| r.end)
        .max()
        .unwrap_or(0);
    Some(if layout.has_analog(space) {
        end.div_ceil(2) * 2
    } else {
        end
    })
}

/// The start properties a new card gets in `station`, to be written with
/// the command that adds it: one `(key, value)` per space it uses.
pub fn allocate(
    project: &Project,
    station: &str,
    layout: &CardLayout,
    except: Option<DeviceId>,
) -> Vec<(&'static str, Value)> {
    Space::ALL
        .into_iter()
        .filter_map(|space| {
            let start = next_start(project, station, layout, space, except)?;
            Some((start_key(space), Value::Int(i64::from(start))))
        })
        .collect()
}

fn text(props: &Properties, key: &str) -> Option<String> {
    match props.get(key)? {
        Value::Text(t) if !t.trim().is_empty() => Some(t.trim().to_owned()),
        Value::Int(n) => Some(n.to_string()),
        _ => None,
    }
}

fn int(props: &Properties, key: &str) -> Option<i64> {
    match props.get(key)? {
        Value::Int(n) => Some(*n),
        Value::Text(t) => t.trim().parse().ok(),
        _ => None,
    }
}
