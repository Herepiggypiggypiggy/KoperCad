//! Which symbol draws which channel, and what is wrong with it.

use std::collections::BTreeMap;

use novo_core::{
    AnyId, DeviceId, Endpoint, EntityKind, Project, SymbolId, SymbolKey, SymbolRole, Value,
};

use crate::address::{Address, AddressStyle};
use crate::card::{ADDRESS_TEXT, CHANNEL, Card, LayoutSource, SPARE, address_style, cards};
use crate::layout::{ChannelType, LayoutError, Space};

/// Looks up a symbol's function type (`plc.input.digital`).
/// [`novo_symbols::Library`] implements it; [`FromKey`] reads the built-in
/// channel symbols' keys (`iec/plc/digital_input`) for tools without a
/// library.
pub trait Functions {
    fn function(&self, symbol: &SymbolKey) -> Option<&str>;
}

impl Functions for novo_symbols::Library {
    fn function(&self, symbol: &SymbolKey) -> Option<&str> {
        self.get(&symbol.0).map(|s| s.function.as_str())
    }
}

/// Knows the built-in channel symbols by key only.
#[derive(Debug, Clone, Copy, Default)]
pub struct FromKey;

impl Functions for FromKey {
    fn function(&self, symbol: &SymbolKey) -> Option<&str> {
        Some(match novo_symbols::Pack::path_of(&symbol.0) {
            "plc/digital_input" => "plc.input.digital",
            "plc/digital_output" => "plc.output.digital",
            "plc/relay_output" => "plc.output.relay",
            "plc/analog_input" => "plc.input.analog",
            "plc/analog_output" => "plc.output.analog",
            _ => return None,
        })
    }
}

/// The record kinds the PLC engine reads, for live reports' dependencies:
/// pages for where channels are drawn ([`PlcIndex::io_rows`]). It also
/// reads the project info (the address style).
pub const READS: &[EntityKind] = &[
    EntityKind::Device,
    EntityKind::Part,
    EntityKind::Symbol,
    EntityKind::Wire,
    EntityKind::Page,
];

/// A drawn channel: a channel symbol of a PLC card.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnChannel {
    pub symbol: SymbolId,
    pub card: DeviceId,
    /// The type the symbol draws (from its function).
    pub kind: ChannelType,
    /// The symbol's `plc.channel` (`"DI 3"`), when set and readable.
    pub stored: Option<(ChannelType, u16)>,
    /// The channel number, when `stored` names the symbol's own type.
    pub number: Option<u16>,
    /// The derived address, when the card has that channel and a start.
    pub address: Option<Address>,
}

/// Something about a card or channel the design checks report (plc.md §7).
#[derive(Debug, Clone, PartialEq)]
pub enum Issue {
    /// Two cards of one station take overlapping bytes in a space.
    Overlap { cards: [DeviceId; 2], space: Space },
    /// One channel is drawn by more than one symbol.
    DrawnTwice {
        card: DeviceId,
        kind: ChannelType,
        number: u16,
        symbols: Vec<SymbolId>,
    },
    /// A symbol names a channel the card does not have (`count` is how
    /// many of that type it has; 0 when the type does not match).
    NoSuchChannel {
        symbol: SymbolId,
        card: DeviceId,
        kind: ChannelType,
        number: u16,
        count: u16,
    },
    /// A channel symbol without a readable `plc.channel`.
    Unnumbered { symbol: SymbolId, card: DeviceId },
    /// A channel symbol whose stored channel is of another type (a DI
    /// symbol with `"DQ 3"`).
    TypeMismatch {
        symbol: SymbolId,
        card: DeviceId,
        drawn: ChannelType,
        stored: ChannelType,
    },
    /// A card with neither a part layout nor its own `plc.channels`.
    NoLayout { card: DeviceId },
    /// A card whose `plc.channels` text does not parse.
    BadLayout { card: DeviceId, error: LayoutError },
    /// A card with channels in a space but no start there.
    NoStart { card: DeviceId, space: Space },
    /// A drawn channel with no wire on any of its pins.
    Unconnected { symbol: SymbolId, card: DeviceId },
    /// Two cards of one station in the same slot. `clash` is set under
    /// the Rockwell style, where slot and channel are the address, so the
    /// cards' addresses collide and it is an error.
    SharedSlot {
        cards: [DeviceId; 2],
        slot: u16,
        clash: bool,
    },
}

impl Issue {
    /// The objects to mark: symbols where the issue has any, else the
    /// card devices.
    pub fn at(&self) -> Vec<AnyId> {
        match self {
            Issue::Overlap { cards, .. } | Issue::SharedSlot { cards, .. } => {
                cards.iter().map(|&c| c.into()).collect()
            }
            Issue::DrawnTwice { symbols, .. } => symbols.iter().map(|&s| s.into()).collect(),
            Issue::NoSuchChannel { symbol, .. }
            | Issue::Unnumbered { symbol, .. }
            | Issue::TypeMismatch { symbol, .. }
            | Issue::Unconnected { symbol, .. } => vec![(*symbol).into()],
            Issue::NoLayout { card }
            | Issue::BadLayout { card, .. }
            | Issue::NoStart { card, .. } => {
                vec![(*card).into()]
            }
        }
    }

    /// One plain sentence for the Issues list, without a trailing period.
    pub fn message(&self, project: &Project) -> String {
        let tag = |d: &DeviceId| {
            project
                .device(*d)
                .map_or_else(|| "a card".to_owned(), |d| d.tag.to_string())
        };
        let space = |s: &Space| match s {
            Space::Input => "inputs",
            Space::Output => "outputs",
        };
        match self {
            Issue::Overlap { cards, space: s } => format!(
                "The {} of {} and {} overlap",
                space(s),
                tag(&cards[0]),
                tag(&cards[1])
            ),
            Issue::DrawnTwice {
                card, kind, number, ..
            } => format!("{kind} {number} of {} is drawn more than once", tag(card)),
            Issue::NoSuchChannel {
                card,
                kind,
                number,
                count: 0,
                ..
            } => format!(
                "{} has no {kind} channels, so {kind} {number} does not exist",
                tag(card)
            ),
            Issue::NoSuchChannel {
                card,
                kind,
                number,
                count,
                ..
            } => format!(
                "{} has {count} {kind} channels, so {kind} {number} does not exist",
                tag(card)
            ),
            Issue::Unnumbered { card, .. } => {
                format!("This channel of {} has no channel chosen", tag(card))
            }
            Issue::TypeMismatch {
                card,
                drawn,
                stored,
                ..
            } => format!(
                "A {drawn} symbol of {} is set to a {stored} channel",
                tag(card)
            ),
            Issue::NoLayout { card } => format!(
                "{} has no channels: assign a part or set its channels",
                tag(card)
            ),
            Issue::BadLayout { card, error } => format!("{}: {error}", tag(card)),
            Issue::NoStart { card, space: s } => {
                format!("{} has no start address for its {}", tag(card), space(s))
            }
            Issue::Unconnected { card, .. } => {
                format!("This channel of {} is connected to nothing", tag(card))
            }
            Issue::SharedSlot { cards, slot, .. } => format!(
                "{} and {} are both in slot {slot}",
                tag(&cards[0]),
                tag(&cards[1])
            ),
        }
    }

    /// Whether the checks report it as an error rather than a warning.
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Issue::SharedSlot { clash: true, .. }
                | Issue::Overlap { .. }
                | Issue::DrawnTwice { .. }
                | Issue::NoSuchChannel { .. }
                | Issue::TypeMismatch { .. }
                | Issue::BadLayout { .. }
        )
    }
}

/// Every PLC card and drawn channel of a project, built in one pass.
/// Rebuild it after an edit; nothing here is stored.
#[derive(Debug, Clone)]
pub struct PlcIndex {
    pub style: AddressStyle,
    cards: Vec<Card>,
    channels: Vec<DrawnChannel>,
    by_symbol: BTreeMap<SymbolId, usize>,
    issues: Vec<Issue>,
}

/// Sets the address text of every channel symbol that carries one to the
/// address its card gives that channel. Returns how many changed. Symbols
/// without the property (drawn by hand) are left alone.
pub fn sync_address_texts(
    tx: &mut novo_core::Tx<'_>,
    functions: &dyn Functions,
) -> novo_core::Result<usize> {
    let index = PlcIndex::build(tx.project(), functions);
    let mut changes = Vec::new();
    for ch in &index.channels {
        let Some(symbol) = tx.project().symbol(ch.symbol) else {
            continue;
        };
        if !symbol.properties.contains_key(ADDRESS_TEXT) {
            continue;
        }
        let text = ch
            .address
            .map(|a| a.format(index.style))
            .unwrap_or_default();
        if symbol.properties.get(ADDRESS_TEXT) != Some(&Value::from(text.clone())) {
            changes.push((ch.symbol, text));
        }
    }
    let n = changes.len();
    for (symbol, text) in changes {
        tx.set_property(symbol, ADDRESS_TEXT, Some(Value::from(text)))?;
    }
    Ok(n)
}

impl PlcIndex {
    pub fn build(project: &Project, functions: &dyn Functions) -> PlcIndex {
        let cards = cards(project);
        let style = address_style(project);
        let mut channels = Vec::new();
        for card in &cards {
            for symbol in project.symbols_of_device(card.device) {
                if let SymbolRole::Device { main: true, .. } = symbol.role {
                    continue;
                }
                if symbol.properties.contains_key(SPARE) {
                    continue;
                }
                let Some(kind) = functions
                    .function(&symbol.symbol)
                    .and_then(ChannelType::from_function)
                else {
                    continue;
                };
                let stored = match symbol.properties.get(CHANNEL) {
                    Some(Value::Text(t)) => parse_channel(t),
                    _ => None,
                };
                let number = stored.filter(|(k, _)| *k == kind).map(|(_, n)| n);
                channels.push(DrawnChannel {
                    symbol: symbol.id,
                    card: card.device,
                    kind,
                    stored,
                    number,
                    address: number.and_then(|n| card.address_in(kind, n, style)),
                });
            }
        }
        channels.sort_by_key(|c| (c.card, c.kind, c.number, c.symbol));
        let by_symbol = channels
            .iter()
            .enumerate()
            .map(|(i, c)| (c.symbol, i))
            .collect();
        let mut index = PlcIndex {
            style,
            cards,
            channels,
            by_symbol,
            issues: Vec::new(),
        };
        index.issues = index.find_issues(project);
        index
    }

    /// Cards by station and slot.
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn card(&self, device: DeviceId) -> Option<&Card> {
        self.cards.iter().find(|c| c.device == device)
    }

    /// Every drawn channel, by card, type and number.
    pub fn channels(&self) -> &[DrawnChannel] {
        &self.channels
    }

    /// The channel a symbol draws, if it is a channel symbol of a card.
    pub fn channel_of(&self, symbol: SymbolId) -> Option<&DrawnChannel> {
        self.by_symbol.get(&symbol).map(|&i| &self.channels[i])
    }

    /// The address text drawn at a channel symbol's `address` anchor, in
    /// the project's style.
    pub fn address_label(&self, symbol: SymbolId) -> Option<String> {
        Some(self.channel_of(symbol)?.address?.format(self.style))
    }

    /// The symbols that draw one channel.
    pub fn drawn(&self, card: DeviceId, kind: ChannelType, number: u16) -> Vec<SymbolId> {
        self.channels
            .iter()
            .filter(|c| c.card == card && c.kind == kind && c.number == Some(number))
            .map(|c| c.symbol)
            .collect()
    }

    /// The card's channels of a type that no symbol draws, in order.
    pub fn free_channels(&self, card: DeviceId, kind: ChannelType) -> Vec<u16> {
        let Some(c) = self.card(card) else {
            return Vec::new();
        };
        (0..c.channels().count(kind))
            .filter(|&n| self.drawn(card, kind, n).is_empty())
            .collect()
    }

    /// The channel the next placement of `kind` on `card` takes: the
    /// lowest one not drawn yet.
    pub fn next_channel(&self, card: DeviceId, kind: ChannelType) -> Option<u16> {
        self.free_channels(card, kind).first().copied()
    }

    /// Cards with a free channel of a type, by station and slot: what
    /// `di` without a card offers.
    pub fn cards_with_free(&self, kind: ChannelType) -> Vec<DeviceId> {
        self.cards
            .iter()
            .filter(|c| self.next_channel(c.device, kind).is_some())
            .map(|c| c.device)
            .collect()
    }

    /// What the design checks report.
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    fn find_issues(&self, project: &Project) -> Vec<Issue> {
        let mut issues = Vec::new();
        for card in &self.cards {
            match (&card.layout, card.layout_source) {
                (Err(error), _) => issues.push(Issue::BadLayout {
                    card: card.device,
                    error: error.clone(),
                }),
                (Ok(_), LayoutSource::None) => issues.push(Issue::NoLayout { card: card.device }),
                (Ok(_), _) => {}
            }
            for space in Space::ALL {
                if self.style != AddressStyle::Rockwell
                    && card.bytes(space) > 0
                    && card.start[space as usize].is_none()
                {
                    issues.push(Issue::NoStart {
                        card: card.device,
                        space,
                    });
                }
            }
        }
        for (i, a) in self.cards.iter().enumerate() {
            for b in &self.cards[i + 1..] {
                if a.station != b.station {
                    continue;
                }
                // Rockwell addresses are slot and channel: bytes never clash.
                for space in Space::ALL
                    .into_iter()
                    .filter(|_| self.style != AddressStyle::Rockwell)
                {
                    if let (Some(ra), Some(rb)) = (a.range(space), b.range(space))
                        && ra.start < rb.end
                        && rb.start < ra.end
                    {
                        issues.push(Issue::Overlap {
                            cards: [a.device, b.device],
                            space,
                        });
                    }
                }
                if let (Some(sa), Some(sb)) = (a.slot, b.slot)
                    && sa == sb
                {
                    issues.push(Issue::SharedSlot {
                        cards: [a.device, b.device],
                        slot: sa,
                        clash: self.style == AddressStyle::Rockwell,
                    });
                }
            }
        }
        let mut seen: BTreeMap<(DeviceId, ChannelType, u16), Vec<SymbolId>> = BTreeMap::new();
        for ch in &self.channels {
            let Some(number) = ch.number else {
                issues.push(match ch.stored {
                    Some((stored, _)) => Issue::TypeMismatch {
                        symbol: ch.symbol,
                        card: ch.card,
                        drawn: ch.kind,
                        stored,
                    },
                    None => Issue::Unnumbered {
                        symbol: ch.symbol,
                        card: ch.card,
                    },
                });
                continue;
            };
            let Some(card) = self.card(ch.card) else {
                continue;
            };
            // A card whose layout does not parse is reported once, as
            // BadLayout, not again on every channel drawn for it.
            if card.layout.is_err() {
                continue;
            }
            let count = card.channels().count(ch.kind);
            if number >= count {
                issues.push(Issue::NoSuchChannel {
                    symbol: ch.symbol,
                    card: ch.card,
                    kind: ch.kind,
                    number,
                    count,
                });
            }
            seen.entry((ch.card, ch.kind, number))
                .or_default()
                .push(ch.symbol);
        }
        for ((card, kind, number), symbols) in seen {
            if symbols.len() > 1 {
                issues.push(Issue::DrawnTwice {
                    card,
                    kind,
                    number,
                    symbols,
                });
            }
        }
        for ch in &self.channels {
            let Some(symbol) = project.symbol(ch.symbol) else {
                continue;
            };
            let wired = (0..symbol.pins.len()).any(|pin| {
                u16::try_from(pin).is_ok_and(|pin| {
                    project
                        .wires_at(Endpoint::pin(ch.symbol, pin))
                        .next()
                        .is_some()
                })
            });
            if !wired {
                issues.push(Issue::Unconnected {
                    symbol: ch.symbol,
                    card: ch.card,
                });
            }
        }
        issues
    }
}

/// The stored form of a channel: `"DI 3"`.
pub fn channel_key(kind: ChannelType, number: u16) -> String {
    format!("{} {number}", kind.code())
}

/// Reads a stored channel, `"DI 3"` (also `"DI3"` and `"do 3"`).
pub fn parse_channel(text: &str) -> Option<(ChannelType, u16)> {
    let text = text.trim();
    let split = text.find(|c: char| c.is_ascii_digit() || c.is_whitespace())?;
    let (code, number) = text.split_at(split);
    Some((ChannelType::from_code(code)?, number.trim().parse().ok()?))
}
