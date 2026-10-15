//! Card layouts: which channels a card has (`DI16 DQ8 AI4`).

use std::fmt;
use std::str::FromStr;

use thiserror::Error;

/// The address space a channel lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Space {
    /// Read by the PLC program (`%I`).
    Input,
    /// Written by the PLC program (`%Q`).
    Output,
}

impl Space {
    pub const ALL: [Space; 2] = [Space::Input, Space::Output];

    /// The address letter: `I` or `Q`.
    pub fn letter(self) -> char {
        match self {
            Space::Input => 'I',
            Space::Output => 'Q',
        }
    }
}

/// What a channel does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChannelType {
    DigitalInput,
    DigitalOutput,
    RelayOutput,
    AnalogInput,
    AnalogOutput,
}

impl ChannelType {
    pub const ALL: [ChannelType; 5] = [
        ChannelType::DigitalInput,
        ChannelType::DigitalOutput,
        ChannelType::RelayOutput,
        ChannelType::AnalogInput,
        ChannelType::AnalogOutput,
    ];

    /// The code written in a layout and shown in lists: `DI`, `DQ`, `RO`,
    /// `AI`, `AQ`.
    pub fn code(self) -> &'static str {
        match self {
            ChannelType::DigitalInput => "DI",
            ChannelType::DigitalOutput => "DQ",
            ChannelType::RelayOutput => "RO",
            ChannelType::AnalogInput => "AI",
            ChannelType::AnalogOutput => "AQ",
        }
    }

    /// Reads a code, accepting `DO` for `DQ` and `AO` for `AQ`, in any case.
    pub fn from_code(code: &str) -> Option<ChannelType> {
        match code.to_ascii_uppercase().as_str() {
            "DI" => Some(ChannelType::DigitalInput),
            "DQ" | "DO" => Some(ChannelType::DigitalOutput),
            "RO" => Some(ChannelType::RelayOutput),
            "AI" => Some(ChannelType::AnalogInput),
            "AQ" | "AO" => Some(ChannelType::AnalogOutput),
            _ => None,
        }
    }

    pub fn space(self) -> Space {
        match self {
            ChannelType::DigitalInput | ChannelType::AnalogInput => Space::Input,
            _ => Space::Output,
        }
    }

    pub fn is_analog(self) -> bool {
        matches!(self, ChannelType::AnalogInput | ChannelType::AnalogOutput)
    }

    /// The symbol function (novo-symbols) that draws this channel type.
    pub fn function(self) -> &'static str {
        match self {
            ChannelType::DigitalInput => "plc.input.digital",
            ChannelType::DigitalOutput => "plc.output.digital",
            ChannelType::RelayOutput => "plc.output.relay",
            ChannelType::AnalogInput => "plc.input.analog",
            ChannelType::AnalogOutput => "plc.output.analog",
        }
    }

    /// The channel type a symbol function draws, if it is a channel.
    pub fn from_function(function: &str) -> Option<ChannelType> {
        ChannelType::ALL
            .into_iter()
            .find(|t| t.function() == function)
    }
}

impl fmt::Display for ChannelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// A run of channels of one type, such as `DI16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Group {
    pub kind: ChannelType,
    pub count: u16,
}

/// One channel of a card, with where it sits in its card's range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Channel {
    pub kind: ChannelType,
    /// The group it belongs to, in layout order.
    pub group: usize,
    /// Its number among the card's channels of its type, from 0 (`DI 3`).
    pub number: u16,
    /// Its bit offset from the card's start in its space. Analog channels
    /// start on a word (16 bits).
    pub bit_offset: u32,
}

impl Channel {
    /// Byte and bit relative to the card's start in its space.
    pub fn byte_bit(&self) -> (u32, u8) {
        (self.bit_offset / 8, (self.bit_offset % 8) as u8)
    }
}

/// What a layout string got wrong.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LayoutError {
    #[error("a card layout names at least one group, such as DI16")]
    Empty,
    #[error("\"{0}\" is not a channel group; write a type and a count, such as DI16 or AQ2")]
    BadGroup(String),
    #[error("\"{0}\" has no channels")]
    ZeroCount(String),
}

/// The channels a card has, as written in the part's or device's
/// `plc.channels` (`DI8 DQ8`). Channels are numbered from 0 per type in
/// the order written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CardLayout {
    pub groups: Vec<Group>,
}

/// The widest layout accepted, so a typo like `DI16000` is caught.
const MAX_CHANNELS_PER_GROUP: u16 = 1024;

impl CardLayout {
    /// Every channel in layout order, with its bit offset in its space.
    /// Digital channels pack bit by bit; an analog channel takes a word,
    /// starting on the next word boundary.
    pub fn channels(&self) -> Vec<Channel> {
        let mut next_bit = [0u32; 2];
        let mut numbers = [0u16; ChannelType::ALL.len()];
        let mut out = Vec::new();
        for (group, g) in self.groups.iter().enumerate() {
            let cursor = &mut next_bit[g.kind.space() as usize];
            for _ in 0..g.count {
                let number = &mut numbers[g.kind as usize];
                if g.kind.is_analog() {
                    *cursor = cursor.div_ceil(16) * 16;
                }
                out.push(Channel {
                    kind: g.kind,
                    group,
                    number: *number,
                    bit_offset: *cursor,
                });
                *number += 1;
                *cursor += if g.kind.is_analog() { 16 } else { 1 };
            }
        }
        out
    }

    /// Bytes the card takes in a space: whole bytes, whole words when it
    /// has analog channels there. Zero when it has no channel there.
    pub fn bytes(&self, space: Space) -> u32 {
        let bits = self
            .channels()
            .iter()
            .filter(|c| c.kind.space() == space)
            .map(|c| c.bit_offset + if c.kind.is_analog() { 16 } else { 1 })
            .max()
            .unwrap_or(0);
        let bytes = bits.div_ceil(8);
        if self.has_analog(space) {
            bytes.div_ceil(2) * 2
        } else {
            bytes
        }
    }

    /// Whether the card has analog channels in a space, so its start there
    /// must be on a word (an even byte).
    pub fn has_analog(&self, space: Space) -> bool {
        self.groups
            .iter()
            .any(|g| g.kind.is_analog() && g.kind.space() == space)
    }

    /// Channel `number` of a type. Numbers count across groups of the same
    /// type (`DI8 DQ8 DI8`: DI 8 is the first channel of the second DI
    /// group).
    pub fn channel(&self, kind: ChannelType, number: u16) -> Option<Channel> {
        self.channels_of(kind).into_iter().nth(usize::from(number))
    }

    /// The channels of one type in layout order.
    pub fn channels_of(&self, kind: ChannelType) -> Vec<Channel> {
        self.channels()
            .into_iter()
            .filter(|c| c.kind == kind)
            .collect()
    }

    /// How many channels of a type the card has.
    pub fn count(&self, kind: ChannelType) -> u16 {
        self.groups
            .iter()
            .filter(|g| g.kind == kind)
            .map(|g| g.count)
            .sum()
    }
}

impl FromStr for CardLayout {
    type Err = LayoutError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut groups = Vec::new();
        for word in s.split(|c: char| c.is_whitespace() || c == ',' || c == '+') {
            if word.is_empty() {
                continue;
            }
            let split = word
                .find(|c: char| c.is_ascii_digit())
                .ok_or_else(|| LayoutError::BadGroup(word.into()))?;
            let (code, count) = word.split_at(split);
            let kind =
                ChannelType::from_code(code).ok_or_else(|| LayoutError::BadGroup(word.into()))?;
            let count: u16 = count
                .parse()
                .ok()
                .filter(|&n| n <= MAX_CHANNELS_PER_GROUP)
                .ok_or_else(|| LayoutError::BadGroup(word.into()))?;
            if count == 0 {
                return Err(LayoutError::ZeroCount(word.into()));
            }
            groups.push(Group { kind, count });
        }
        if groups.is_empty() {
            return Err(LayoutError::Empty);
        }
        Ok(CardLayout { groups })
    }
}

impl fmt::Display for CardLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, g) in self.groups.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{}{}", g.kind.code(), g.count)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(s: &str) -> CardLayout {
        s.parse().unwrap()
    }

    #[test]
    fn parses_and_prints_groups() {
        let l = layout("di8, do8 + AI2");
        assert_eq!(l.to_string(), "DI8 DQ8 AI2");
        assert_eq!(l.count(ChannelType::DigitalOutput), 8);
    }

    #[test]
    fn rejects_bad_layouts() {
        assert_eq!("".parse::<CardLayout>(), Err(LayoutError::Empty));
        assert_eq!(
            "XX4".parse::<CardLayout>(),
            Err(LayoutError::BadGroup("XX4".into()))
        );
        assert_eq!(
            "DI".parse::<CardLayout>(),
            Err(LayoutError::BadGroup("DI".into()))
        );
        assert_eq!(
            "DI0".parse::<CardLayout>(),
            Err(LayoutError::ZeroCount("DI0".into()))
        );
        assert!("DI16000".parse::<CardLayout>().is_err());
    }

    #[test]
    fn digital_channels_pack_bits() {
        let l = layout("DI16");
        let c = l.channel(ChannelType::DigitalInput, 11).unwrap();
        assert_eq!(c.byte_bit(), (1, 3));
        assert_eq!(l.bytes(Space::Input), 2);
        assert_eq!(l.bytes(Space::Output), 0);
    }

    #[test]
    fn analog_channels_take_aligned_words() {
        let l = layout("DI4 AI2");
        let ai1 = l.channel(ChannelType::AnalogInput, 1).unwrap();
        // DI0-3 use byte 0; AI0 starts on the next word (byte 2), AI1 at 4.
        assert_eq!(ai1.byte_bit(), (4, 0));
        assert_eq!(l.bytes(Space::Input), 6);
        assert!(l.has_analog(Space::Input));
    }

    #[test]
    fn mixed_card_uses_two_spaces() {
        let l = layout("DI8 DQ8");
        assert_eq!(l.bytes(Space::Input), 1);
        assert_eq!(l.bytes(Space::Output), 1);
        let dq0 = l.channel(ChannelType::DigitalOutput, 0).unwrap();
        assert_eq!((dq0.group, dq0.bit_offset), (1, 0));
    }

    #[test]
    fn numbers_continue_across_groups_of_a_type() {
        let l = layout("DI8 DQ8 DI8");
        let di8 = l.channel(ChannelType::DigitalInput, 8).unwrap();
        assert_eq!((di8.group, di8.number, di8.bit_offset), (2, 8, 8));
        assert_eq!(l.channel(ChannelType::DigitalInput, 16), None);
    }

    #[test]
    fn functions_round_trip() {
        for t in ChannelType::ALL {
            assert_eq!(ChannelType::from_function(t.function()), Some(t));
        }
        assert_eq!(ChannelType::from_function("contact.no"), None);
    }
}
