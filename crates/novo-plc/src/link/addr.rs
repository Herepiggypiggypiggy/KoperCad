//! Reading addresses back: the text a PLC engineering tool holds for a
//! channel, in any of the styles, as a key that equals the key of the
//! channel's own [`Address`].

use crate::address::Address;
use crate::layout::Space;

/// What an address names, independent of how it is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddrKey {
    /// `%IX4.3`, `%I4.3`, `I4.3`.
    Bit { space: Space, byte: u32, bit: u8 },
    /// `%IW64`, `IW64`.
    Word { space: Space, byte: u32 },
    /// `Local:3:I.Data.7`: slot and point.
    Point { space: Space, slot: u16, point: u32 },
    /// `Local:5:O.Ch1Data`: slot and analog channel.
    Analog {
        space: Space,
        slot: u16,
        channel: u16,
    },
}

impl AddrKey {
    /// The space the address is in.
    pub fn space(self) -> Space {
        match self {
            AddrKey::Bit { space, .. }
            | AddrKey::Word { space, .. }
            | AddrKey::Point { space, .. }
            | AddrKey::Analog { space, .. } => space,
        }
    }

    /// Whether the key names a whole word (an analog value).
    pub fn is_word(self) -> bool {
        matches!(self, AddrKey::Word { .. } | AddrKey::Analog { .. })
    }
}

/// The byte-style key of a channel's address (IEC, Siemens and plain
/// styles all read the same).
pub fn byte_key(a: &Address) -> AddrKey {
    if a.kind.is_analog() {
        AddrKey::Word {
            space: a.space(),
            byte: a.byte,
        }
    } else {
        AddrKey::Bit {
            space: a.space(),
            byte: a.byte,
            bit: a.bit,
        }
    }
}

/// The slot-style key of a channel's address, when its card has a slot.
pub fn slot_key(a: &Address) -> Option<AddrKey> {
    let slot = a.slot?;
    Some(if a.kind.is_analog() {
        AddrKey::Analog {
            space: a.space(),
            slot,
            channel: a.number,
        }
    } else {
        AddrKey::Point {
            space: a.space(),
            slot,
            point: a.point,
        }
    })
}

/// Reads an address in any style. `None` for text that is not a PLC
/// address (a memory bit `M0.0`, a byte `IB4`, a tag path).
pub fn parse_address(text: &str) -> Option<AddrKey> {
    let t = text.trim().trim_start_matches('%');
    if t.len() > 6 && t[..6].eq_ignore_ascii_case("local:") {
        return parse_slot(&t[6..]);
    }
    let mut chars = t.chars();
    let space = match chars.next()?.to_ascii_uppercase() {
        'I' => Space::Input,
        'Q' => Space::Output,
        _ => return None,
    };
    let rest = chars.as_str();
    let (word, rest) = match rest.chars().next()?.to_ascii_uppercase() {
        'W' => (true, &rest[1..]),
        'X' => (false, &rest[1..]),
        c if c.is_ascii_digit() => (false, rest),
        _ => return None,
    };
    if word {
        let byte = rest.parse().ok()?;
        return Some(AddrKey::Word { space, byte });
    }
    let (byte, bit) = rest.split_once('.')?;
    let bit: u8 = bit.parse().ok().filter(|b| *b < 8)?;
    Some(AddrKey::Bit {
        space,
        byte: byte.parse().ok()?,
        bit,
    })
}

fn parse_slot(rest: &str) -> Option<AddrKey> {
    let (slot, rest) = rest.split_once(':')?;
    let slot: u16 = slot.parse().ok()?;
    let mut parts = rest.split('.');
    let space = match parts.next()?.to_ascii_uppercase().as_str() {
        "I" => Space::Input,
        "O" => Space::Output,
        _ => return None,
    };
    let second = parts.next()?;
    if second.eq_ignore_ascii_case("data") {
        let point = parts.next()?.parse().ok()?;
        return Some(AddrKey::Point { space, slot, point });
    }
    let lower = second.to_ascii_lowercase();
    let channel = lower
        .strip_prefix("ch")?
        .strip_suffix("data")?
        .parse()
        .ok()?;
    Some(AddrKey::Analog {
        space,
        slot,
        channel,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_reads_back_to_one_key() {
        let bit = AddrKey::Bit {
            space: Space::Input,
            byte: 4,
            bit: 3,
        };
        for text in ["%IX4.3", "%I4.3", "I4.3", " ix4.3 "] {
            assert_eq!(parse_address(text), Some(bit), "{text}");
        }
        let word = AddrKey::Word {
            space: Space::Output,
            byte: 66,
        };
        for text in ["%QW66", "QW66"] {
            assert_eq!(parse_address(text), Some(word), "{text}");
        }
        assert_eq!(
            parse_address("Local:3:I.Data.11"),
            Some(AddrKey::Point {
                space: Space::Input,
                slot: 3,
                point: 11
            })
        );
        assert_eq!(
            parse_address("Local:5:O.Ch1Data"),
            Some(AddrKey::Analog {
                space: Space::Output,
                slot: 5,
                channel: 1
            })
        );
    }

    #[test]
    fn not_addresses() {
        for text in [
            "",
            "M0.0",
            "IB4",
            "%I4",
            "%I4.9",
            "Pump",
            "Local:x:I.Data.1",
        ] {
            assert_eq!(parse_address(text), None, "{text}");
        }
    }
}
