//! Channel addresses and how they are written.

use std::fmt;
use std::str::FromStr;

use crate::layout::{Channel, ChannelType, Space};

/// How the project writes PLC addresses: the rule "PLC addresses are
/// written [IEC 61131 | Siemens | Rockwell | Plain]". It changes text
/// only, never which channel is which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AddressStyle {
    /// `%IX4.3`, `%QW66`.
    #[default]
    Iec,
    /// `%I4.3`, `%QW66`.
    Siemens,
    /// `Local:3:I.Data.3`, `Local:5:O.Ch1Data`: slot and channel.
    Rockwell,
    /// `I4.3`, `QW66`.
    Plain,
}

impl AddressStyle {
    pub const ALL: [AddressStyle; 4] = [
        AddressStyle::Iec,
        AddressStyle::Siemens,
        AddressStyle::Rockwell,
        AddressStyle::Plain,
    ];

    /// The value stored in `plc.address_style`.
    pub fn key(self) -> &'static str {
        match self {
            AddressStyle::Iec => "iec",
            AddressStyle::Siemens => "siemens",
            AddressStyle::Rockwell => "rockwell",
            AddressStyle::Plain => "plain",
        }
    }

    /// The name shown in the Rules document.
    pub fn label(self) -> &'static str {
        match self {
            AddressStyle::Iec => "IEC 61131",
            AddressStyle::Siemens => "Siemens",
            AddressStyle::Rockwell => "Rockwell",
            AddressStyle::Plain => "Plain",
        }
    }
}

impl FromStr for AddressStyle {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        AddressStyle::ALL
            .into_iter()
            .find(|style| style.key().eq_ignore_ascii_case(s.trim()))
            .ok_or(())
    }
}

/// Where one channel sits for the PLC program: derived from its card's
/// start and slot and its place in the card's layout, never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Address {
    pub kind: ChannelType,
    /// The byte in the channel's space (`4` in `%IX4.3`, `66` in `%QW66`).
    pub byte: u32,
    /// The bit for a digital channel (`3` in `%IX4.3`); 0 for analog.
    pub bit: u8,
    /// The card's slot, which Rockwell addresses read.
    pub slot: Option<u16>,
    /// The channel number of its type on the card.
    pub number: u16,
}

impl Address {
    /// The address of `channel` on a card whose range in the channel's
    /// space starts at byte `start`.
    pub fn of(channel: &Channel, start: u32, slot: Option<u16>) -> Address {
        let (byte, bit) = channel.byte_bit();
        Address {
            kind: channel.kind,
            byte: start + byte,
            bit,
            slot,
            number: channel.number,
        }
    }

    pub fn space(&self) -> Space {
        self.kind.space()
    }

    /// The address as the project writes it.
    pub fn format(&self, style: AddressStyle) -> String {
        let letter = self.space().letter();
        let analog = self.kind.is_analog();
        match style {
            AddressStyle::Iec if analog => format!("%{letter}W{}", self.byte),
            AddressStyle::Iec => format!("%{letter}X{}.{}", self.byte, self.bit),
            AddressStyle::Siemens if analog => format!("%{letter}W{}", self.byte),
            AddressStyle::Siemens => format!("%{letter}{}.{}", self.byte, self.bit),
            AddressStyle::Plain if analog => format!("{letter}W{}", self.byte),
            AddressStyle::Plain => format!("{letter}{}.{}", self.byte, self.bit),
            AddressStyle::Rockwell => {
                let slot = self.slot.map_or_else(|| "?".to_owned(), |s| s.to_string());
                let io = match self.space() {
                    Space::Input => 'I',
                    Space::Output => 'O',
                };
                if analog {
                    format!("Local:{slot}:{io}.Ch{}Data", self.number)
                } else {
                    format!("Local:{slot}:{io}.Data.{}", self.number)
                }
            }
        }
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format(AddressStyle::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::CardLayout;

    fn address(layout: &str, kind: ChannelType, number: u16, start: u32) -> Address {
        let layout: CardLayout = layout.parse().unwrap();
        Address::of(&layout.channel(kind, number).unwrap(), start, Some(3))
    }

    #[test]
    fn digital_input_in_every_style() {
        let a = address("DI16", ChannelType::DigitalInput, 11, 4);
        assert_eq!(a.format(AddressStyle::Iec), "%IX5.3");
        assert_eq!(a.format(AddressStyle::Siemens), "%I5.3");
        assert_eq!(a.format(AddressStyle::Plain), "I5.3");
        assert_eq!(a.format(AddressStyle::Rockwell), "Local:3:I.Data.11");
    }

    #[test]
    fn analog_output_in_every_style() {
        let a = address("AQ2", ChannelType::AnalogOutput, 1, 64);
        assert_eq!(a.format(AddressStyle::Iec), "%QW66");
        assert_eq!(a.format(AddressStyle::Siemens), "%QW66");
        assert_eq!(a.format(AddressStyle::Plain), "QW66");
        assert_eq!(a.format(AddressStyle::Rockwell), "Local:3:O.Ch1Data");
    }

    #[test]
    fn relay_outputs_are_outputs() {
        let a = address("RO4", ChannelType::RelayOutput, 2, 0);
        assert_eq!(a.to_string(), "%QX0.2");
    }

    #[test]
    fn styles_parse_from_their_keys() {
        for style in AddressStyle::ALL {
            assert_eq!(style.key().parse(), Ok(style));
        }
        assert_eq!(" Siemens ".parse(), Ok(AddressStyle::Siemens));
        assert_eq!("s7".parse::<AddressStyle>(), Err(()));
    }
}
