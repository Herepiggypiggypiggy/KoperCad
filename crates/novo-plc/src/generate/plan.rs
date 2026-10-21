//! From a list to cards: which cards the signals need, which channel each
//! signal gets, and what the review list should say. Nothing here touches
//! the project; [`apply`](super::apply) builds what the plan describes.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{DeviceTag, Project, SymbolRole, Value};

use crate::card::{CHANNEL, cards};
use crate::layout::{CardLayout, ChannelType, Space};
use crate::link::addr::AddrKey;

use super::FIELD;
use super::list::{IoList, Note};

/// How the list is turned into cards.
#[derive(Debug, Clone)]
pub struct Options {
    /// The device letter of a new card (`K`, giving `-K4`).
    pub card_letter: String,
    /// Most digital channels on a card: 8, 16 or 32.
    pub digital_per_card: u16,
    /// Most analog channels on a card: 2, 4 or 8.
    pub analog_per_card: u16,
    /// Put a signal at the address the list gives it (cards start where
    /// that needs and unused channels in between stay spare), when the
    /// whole group has addresses and they fit. Off, the project assigns
    /// every address.
    pub keep_addresses: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            card_letter: "K".into(),
            digital_per_card: 16,
            analog_per_card: 4,
            keep_addresses: true,
        }
    }
}

/// One card to add.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedCard {
    pub tag: DeviceTag,
    pub kind: ChannelType,
    pub station: String,
    pub slot: Option<u16>,
    pub layout: CardLayout,
    /// The start byte the list's addresses need; `None` takes the next
    /// free range of the station.
    pub start: Option<u32>,
    /// One entry per channel of the layout: the list item it carries, or
    /// `None` for a spare.
    pub channels: Vec<Option<usize>>,
}

impl PlannedCard {
    pub fn signals(&self) -> usize {
        self.channels.iter().flatten().count()
    }

    pub fn spares(&self) -> usize {
        self.channels.len() - self.signals()
    }
}

/// What importing a list would do.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    pub cards: Vec<PlannedCard>,
    /// Everything the review list shows besides the cards: rows left out,
    /// addresses that could not be kept, signals already in the project.
    pub notes: Vec<Note>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn signals(&self) -> usize {
        self.cards.iter().map(PlannedCard::signals).sum()
    }

    /// One sentence for the top of the review.
    pub fn summary(&self) -> String {
        let (c, s) = (self.cards.len(), self.signals());
        if c == 0 {
            return "Nothing to add: no signal of the list is new.".into();
        }
        format!(
            "{s} signal{} on {c} new card{}, one page and one terminal strip per card.",
            if s == 1 { "" } else { "s" },
            if c == 1 { "" } else { "s" },
        )
    }
}

/// The unit a card's channel count is rounded up to, and its largest size.
fn sizes(kind: ChannelType, options: &Options) -> (u16, u16) {
    match kind {
        ChannelType::DigitalInput | ChannelType::DigitalOutput => (8, options.digital_per_card),
        ChannelType::RelayOutput => (4, options.digital_per_card.min(8)),
        ChannelType::AnalogInput | ChannelType::AnalogOutput => (2, options.analog_per_card),
    }
}

/// Signals already drawn from a list: `plc.field` on a channel symbol,
/// lower-cased, with where.
fn known(project: &Project) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for symbol in project.symbols() {
        let Some(Value::Text(tag)) = symbol.properties.get(FIELD) else {
            continue;
        };
        let SymbolRole::Device { device, .. } = symbol.role else {
            continue;
        };
        let card = project
            .device(device)
            .map(|d| d.tag.to_string())
            .unwrap_or_default();
        let channel = match symbol.properties.get(CHANNEL) {
            Some(Value::Text(c)) => c.clone(),
            _ => String::new(),
        };
        out.entry(tag.trim().to_lowercase())
            .or_insert_with(|| format!("{card} {channel}").trim().to_owned());
    }
    out
}

/// Plans the cards for a list.
pub fn plan(project: &Project, list: &IoList, options: &Options) -> Plan {
    let mut plan = Plan {
        notes: list.notes.clone(),
        ..Plan::default()
    };
    let known = known(project);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut groups: BTreeMap<(String, ChannelType), Vec<usize>> = BTreeMap::new();
    for (i, item) in list.items.iter().enumerate() {
        let key = item.tag.trim().to_lowercase();
        if !key.is_empty() {
            if let Some(at) = known.get(&key) {
                plan.notes.push(Note {
                    line: Some(item.line),
                    text: format!(
                        "{} is already in the project at {at}; left as it is",
                        item.tag
                    ),
                });
                continue;
            }
            if !seen.insert(key) {
                plan.notes.push(Note {
                    line: Some(item.line),
                    text: format!("{} is in the list twice; the first row is used", item.tag),
                });
                continue;
            }
        }
        groups
            .entry((item.station.clone(), item.kind))
            .or_default()
            .push(i);
    }
    let existing = cards(project);
    let mut taken: BTreeMap<(String, Space), Vec<std::ops::Range<u32>>> = BTreeMap::new();
    for c in &existing {
        for space in Space::ALL {
            if let Some(r) = c.range(space) {
                taken.entry((c.station.clone(), space)).or_default().push(r);
            }
        }
    }
    let mut next_slot: BTreeMap<String, u16> = BTreeMap::new();
    for c in &existing {
        if let Some(slot) = c.slot {
            let n = next_slot.entry(c.station.clone()).or_insert(1);
            *n = (*n).max(slot.saturating_add(1));
        }
    }
    let letter_number = project.next_tag_number(&DeviceTag::default(), &options.card_letter);
    let mut explicit_cards = Vec::new();
    let mut sequential_cards = Vec::new();
    for ((station, kind), members) in &groups {
        let (unit, cap) = sizes(*kind, options);
        let placed = options
            .keep_addresses
            .then(|| by_address(list, members, *kind, unit, cap, &mut plan.notes))
            .flatten();
        let mut chunks: Vec<(Vec<Option<usize>>, Option<u32>)> = Vec::new();
        if let Some(found) = placed {
            {
                // Every card of the group must fit beside the cards already
                // in the station; otherwise the whole group is assigned by
                // the project, so no two cards of one list disagree.
                let space = kind.space();
                let clash = found.iter().find_map(|(channels, start)| {
                    let layout = layout_of(*kind, channels.len());
                    let bytes = layout.bytes(space);
                    let range = (*start)..(*start + bytes);
                    taken
                        .get(&(station.clone(), space))
                        .into_iter()
                        .flatten()
                        .any(|r| range.start < r.end && r.start < range.end)
                        .then_some(range)
                });
                if let Some(range) = clash {
                    plan.notes.push(Note {
                        line: None,
                        text: format!(
                            "{} addresses from the list ({} to {}) overlap a card already in \
                             the project, so the project assigns them",
                            kind.code(),
                            range.start,
                            range.end - 1
                        ),
                    });
                } else {
                    for (channels, start) in found {
                        let layout = layout_of(*kind, channels.len());
                        taken
                            .entry((station.clone(), space))
                            .or_default()
                            .push(start..start + layout.bytes(space));
                        chunks.push((channels, Some(start)));
                    }
                }
            }
        }
        if chunks.is_empty() {
            for part in members.chunks(usize::from(cap)) {
                let count = round_up(part.len(), usize::from(unit));
                let mut channels: Vec<Option<usize>> = part.iter().map(|&i| Some(i)).collect();
                channels.resize(count, None);
                chunks.push((channels, None));
            }
        }
        for (channels, start) in chunks {
            let card = PlannedCard {
                tag: DeviceTag::default(),
                kind: *kind,
                station: station.clone(),
                slot: None,
                layout: layout_of(*kind, channels.len()),
                start,
                channels,
            };
            if start.is_some() {
                explicit_cards.push(card);
            } else {
                sequential_cards.push(card);
            }
        }
    }
    // Cards with their own start come first, so a card without one never
    // takes a range the list's addresses need.
    plan.cards = explicit_cards;
    plan.cards.extend(sequential_cards);
    for (n, card) in plan.cards.iter_mut().enumerate() {
        card.tag = DeviceTag::parse(&format!(
            "-{}{}",
            options.card_letter,
            letter_number + n as u32
        ))
        .unwrap_or_default();
        let slot = next_slot.entry(card.station.clone()).or_insert(1);
        card.slot = Some(*slot);
        *slot += 1;
    }
    plan
}

fn round_up(n: usize, unit: usize) -> usize {
    n.div_ceil(unit.max(1)).max(1) * unit.max(1)
}

fn layout_of(kind: ChannelType, count: usize) -> CardLayout {
    format!("{}{count}", kind.code())
        .parse()
        .unwrap_or_default()
}

/// Cards laid out so every signal sits at the address the list gives it:
/// `(channels, start byte)` per card. `None` unless every signal of the
/// group has a byte-style address of the right shape, with a note when
/// some have one and some do not.
fn by_address(
    list: &IoList,
    members: &[usize],
    kind: ChannelType,
    unit: u16,
    cap: u16,
    notes: &mut Vec<Note>,
) -> Option<Vec<(Vec<Option<usize>>, u32)>> {
    let analog = kind.is_analog();
    let position = |key: AddrKey| match key {
        AddrKey::Bit { byte, bit, .. } if !analog => Some(u64::from(byte) * 8 + u64::from(bit)),
        AddrKey::Word { byte, .. } if analog && byte % 2 == 0 => Some(u64::from(byte / 2)),
        _ => None,
    };
    let with: Vec<(usize, u64)> = members
        .iter()
        .filter_map(|&i| Some((i, position(list.items[i].address?)?)))
        .collect();
    if with.is_empty() {
        return None;
    }
    if with.len() != members.len() {
        notes.push(Note {
            line: None,
            text: format!(
                "only {} of {} {} signals have an address in the list, so the project \
                 assigns the addresses of all of them",
                with.len(),
                members.len(),
                kind.code()
            ),
        });
        return None;
    }
    let mut by_position: BTreeMap<u64, usize> = BTreeMap::new();
    for &(i, pos) in &with {
        if let Some(&first) = by_position.get(&pos) {
            notes.push(Note {
                line: Some(list.items[i].line),
                text: format!(
                    "{} has the same address as {}; the addresses of the {} signals are \
                     assigned by the project",
                    list.items[i].address_text,
                    list.items[first].address_text,
                    kind.code()
                ),
            });
            return None;
        }
        by_position.insert(pos, i);
    }
    let first = *by_position.keys().next()?;
    let base = if analog { first } else { first / 8 * 8 };
    let cap = u64::from(cap);
    let mut cards: BTreeMap<u64, Vec<(u64, usize)>> = BTreeMap::new();
    for (&pos, &i) in &by_position {
        cards
            .entry((pos - base) / cap)
            .or_default()
            .push(((pos - base) % cap, i));
    }
    Some(
        cards
            .into_iter()
            .map(|(chunk, members)| {
                let top = members.iter().map(|m| m.0).max().unwrap_or(0) as usize + 1;
                let mut channels = vec![None; round_up(top, usize::from(unit))];
                for (idx, i) in members {
                    channels[idx as usize] = Some(i);
                }
                let first_channel = base + chunk * cap;
                let start = if analog {
                    first_channel * 2
                } else {
                    first_channel / 8
                };
                (channels, u32::try_from(start).unwrap_or(u32::MAX))
            })
            .collect(),
    )
}
