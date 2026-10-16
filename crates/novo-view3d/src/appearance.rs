//! What a device looks like by type (3d-look.md section 3): a breaker is not
//! the colour of a terminal block. Pure data, no GPU. The classes follow the
//! IEC 81346 letter code of the tag, with the model's device kind on top.

use crate::palette::{self, Material, Rgba};

/// The family a device belongs to, for its body colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DeviceClass {
    /// Circuit breakers, fuses and isolators (`Q`, `F`).
    Protection,
    /// Contactors, relays and timers (`K`).
    Switching,
    /// Terminal strips (`X`).
    Terminal,
    /// PLC racks, CPUs and I/O cards.
    Controller,
    /// Drives and soft starters (`U`, `V`).
    Drive,
    /// Power supplies and transformers (`G`, `T`).
    Supply,
    /// Plugs and sockets.
    Connector,
    Generic,
}

/// What the model says about a device: its kind and the letter of its tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindHint {
    General,
    TerminalStrip,
    Connector,
    Plc,
}

impl DeviceClass {
    /// Class from the model kind first, then the tag letter (case blind).
    pub fn of(kind: KindHint, letter_code: &str) -> Self {
        match kind {
            KindHint::TerminalStrip => return Self::Terminal,
            KindHint::Connector => return Self::Connector,
            KindHint::Plc => return Self::Controller,
            KindHint::General => {}
        }
        match letter_code.to_ascii_uppercase().as_str() {
            "Q" | "F" | "S" => Self::Protection,
            "K" | "KA" | "KM" | "KT" => Self::Switching,
            "X" => Self::Terminal,
            "A" | "PLC" => Self::Controller,
            "U" | "V" => Self::Drive,
            "G" | "T" => Self::Supply,
            "XS" | "XP" => Self::Connector,
            _ => Self::Generic,
        }
    }

    pub fn material(self) -> Material {
        match self {
            Self::Protection => palette::DEVICE_LIGHT,
            Self::Switching => palette::DEVICE_DARK,
            Self::Terminal => palette::TERMINAL,
            Self::Controller => palette::DEVICE_CONTROLLER,
            Self::Drive => palette::DEVICE_DRIVE,
            Self::Supply => palette::DEVICE_SUPPLY,
            Self::Connector => palette::DEVICE_CONNECTOR,
            Self::Generic => palette::DEVICE_LIGHT,
        }
    }

    /// Body colour as the scene's maker-colour tint.
    pub fn tint(self) -> [u8; 3] {
        let c: Rgba = self.material().color;
        [c[0], c[1], c[2]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_kind_wins_over_the_letter() {
        assert_eq!(
            DeviceClass::of(KindHint::TerminalStrip, "K"),
            DeviceClass::Terminal
        );
        assert_eq!(DeviceClass::of(KindHint::Plc, ""), DeviceClass::Controller);
    }

    #[test]
    fn letters_map_to_families_and_unknown_is_generic() {
        assert_eq!(
            DeviceClass::of(KindHint::General, "q"),
            DeviceClass::Protection
        );
        assert_eq!(
            DeviceClass::of(KindHint::General, "KM"),
            DeviceClass::Switching
        );
        assert_eq!(DeviceClass::of(KindHint::General, "G"), DeviceClass::Supply);
        assert_eq!(
            DeviceClass::of(KindHint::General, "Z"),
            DeviceClass::Generic
        );
        assert_eq!(DeviceClass::of(KindHint::General, ""), DeviceClass::Generic);
    }

    #[test]
    fn families_are_told_apart_by_body_colour() {
        use std::collections::HashSet;
        let classes = [
            DeviceClass::Protection,
            DeviceClass::Switching,
            DeviceClass::Terminal,
            DeviceClass::Controller,
            DeviceClass::Drive,
            DeviceClass::Supply,
            DeviceClass::Connector,
        ];
        let colours: HashSet<_> = classes.iter().map(|c| c.tint()).collect();
        assert_eq!(colours.len(), classes.len());
    }
}
