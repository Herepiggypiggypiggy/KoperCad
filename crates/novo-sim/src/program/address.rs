//! Direct addresses (`%IX0.0`, `%QW66`) in the notations the I/O list
//! prints: IEC 61131 (`%IX4.3`), Siemens (`%I4.3`) and plain (`I4.3`).

use std::fmt;

/// Which image an address lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Space {
    Input,
    Output,
    Memory,
}

/// The size of the addressed location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Size {
    Bit,
    Byte,
    Word,
    DWord,
}

/// A direct address, normalized: `%I4.3` and `I4.3` are both `%IX4.3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address {
    pub space: Space,
    pub size: Size,
    pub byte: u32,
    /// Bit number, for a bit address; 0 otherwise.
    pub bit: u8,
}

impl Address {
    /// Reads an address in any supported notation. `None` when the text is
    /// not an address (Rockwell tags are not).
    pub fn parse(text: &str) -> Option<Address> {
        let t = text.trim();
        let t = t.strip_prefix('%').unwrap_or(t);
        let mut chars = t.chars();
        let space = match chars.next()?.to_ascii_uppercase() {
            'I' => Space::Input,
            'Q' => Space::Output,
            'M' => Space::Memory,
            _ => return None,
        };
        let rest = chars.as_str();
        let (size, rest) = match rest.chars().next()?.to_ascii_uppercase() {
            'X' => (Size::Bit, &rest[1..]),
            'B' => (Size::Byte, &rest[1..]),
            'W' => (Size::Word, &rest[1..]),
            'D' => (Size::DWord, &rest[1..]),
            c if c.is_ascii_digit() => (Size::Bit, rest),
            _ => return None,
        };
        if size == Size::Bit {
            let (byte, bit) = rest.split_once('.')?;
            let bit: u8 = bit.parse().ok().filter(|b| *b < 8)?;
            Some(Address {
                space,
                size,
                byte: byte.parse().ok()?,
                bit,
            })
        } else {
            Some(Address {
                space,
                size,
                byte: rest.parse().ok()?,
                bit: 0,
            })
        }
    }

    pub fn is_bit(&self) -> bool {
        self.size == Size::Bit
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let space = match self.space {
            Space::Input => 'I',
            Space::Output => 'Q',
            Space::Memory => 'M',
        };
        match self.size {
            Size::Bit => write!(f, "%{space}X{}.{}", self.byte, self.bit),
            Size::Byte => write!(f, "%{space}B{}", self.byte),
            Size::Word => write!(f, "%{space}W{}", self.byte),
            Size::DWord => write!(f, "%{space}D{}", self.byte),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notations_normalize() {
        let iec = Address::parse("%IX4.3").unwrap();
        assert_eq!(Address::parse("%I4.3"), Some(iec));
        assert_eq!(Address::parse("I4.3"), Some(iec));
        assert_eq!(iec.to_string(), "%IX4.3");
        assert_eq!(Address::parse("%QW66").unwrap().to_string(), "%QW66");
        assert_eq!(Address::parse("QW66"), Address::parse("%QW66"));
    }

    #[test]
    fn non_addresses_are_none() {
        assert_eq!(Address::parse("Local:3:I.Data.3"), None);
        assert_eq!(Address::parse("%IX4.9"), None);
        assert_eq!(Address::parse("start"), None);
    }
}
