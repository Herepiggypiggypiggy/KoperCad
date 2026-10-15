//! What a symbol is, by its library key, and wire colours by their codes.
//!
//! The standards rules reason about kinds of equipment (a disconnecting
//! device, an overload relay, an emergency stop), not about library keys, so
//! the keys are read once here. Both symbol packs name their symbols alike
//! (`iec/protection/fuse_3p`, `nfpa/protection/fuse_3p`), so the pack prefix
//! is ignored.

use novo_core::SymbolInstance;

/// A kind of equipment, for the rules that ask "is there a ...".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Class {
    /// Disconnector, switch-disconnector, main switch.
    Disconnector,
    /// Switch-disconnector with fuses: isolates and protects.
    FuseSwitch,
    /// Miniature or moulded-case circuit breaker, RCBO.
    Breaker,
    /// Fuse or fuse holder.
    Fuse,
    /// Thermal or electronic overload relay.
    OverloadRelay,
    /// Motor protection breaker: overload and short-circuit in one.
    MotorProtectionBreaker,
    /// Magnetic-only motor circuit protector: short-circuit, no overload.
    MotorCircuitProtector,
    /// Residual-current device.
    Rcd,
    SurgeArrester,
    /// Main contacts of a contactor.
    Contactor,
    Coil,
    /// A contact of a relay, contactor or sensor (auxiliary).
    Contact,
    EmergencyStop,
    PullCord,
    Pushbutton,
    Selector,
    KeySwitch,
    Footswitch,
    Lamp,
    Sounder,
    Motor,
    /// Frequency converter.
    Drive,
    SoftStarter,
    Transformer,
    PowerSupply,
    Heater,
    Terminal,
    Earth,
    Sensor,
    PlcIo,
    Source,
    Other,
}

impl Class {
    /// Overcurrent protective devices that interrupt a short circuit.
    pub(crate) fn is_short_circuit_protector(self) -> bool {
        matches!(
            self,
            Class::Fuse
                | Class::FuseSwitch
                | Class::Breaker
                | Class::MotorProtectionBreaker
                | Class::MotorCircuitProtector
        )
    }

    /// Devices that isolate the supply when opened (a breaker only when it
    /// is marked as suitable for isolation; see the rules).
    pub(crate) fn is_isolator(self) -> bool {
        matches!(self, Class::Disconnector | Class::FuseSwitch)
    }

    /// Devices that protect a motor against overload.
    pub(crate) fn is_overload_protector(self) -> bool {
        matches!(
            self,
            Class::OverloadRelay | Class::MotorProtectionBreaker | Class::Drive
        )
    }

    /// A load on the power side: it takes power from the supply and turns it
    /// into work.
    pub(crate) fn is_power_load(self) -> bool {
        matches!(
            self,
            Class::Motor | Class::Drive | Class::SoftStarter | Class::Heater
        )
    }

    /// A load or operator of the control circuit.
    pub(crate) fn is_control(self) -> bool {
        matches!(
            self,
            Class::Coil
                | Class::Contact
                | Class::EmergencyStop
                | Class::PullCord
                | Class::Pushbutton
                | Class::Selector
                | Class::KeySwitch
                | Class::Footswitch
                | Class::Lamp
                | Class::Sounder
                | Class::Sensor
                | Class::PlcIo
        )
    }

    pub(crate) fn is_operator(self) -> bool {
        matches!(
            self,
            Class::EmergencyStop
                | Class::PullCord
                | Class::Pushbutton
                | Class::Selector
                | Class::KeySwitch
                | Class::Footswitch
        )
    }
}

/// The last two parts of a library key: `("protection", "fuse_3p")`.
fn parts(key: &str) -> (&str, &str) {
    let mut it = key.rsplit('/');
    let name = it.next().unwrap_or_default();
    let group = it.next().unwrap_or_default();
    (group, name)
}

/// The class of a library key.
pub(crate) fn classify(key: &str) -> Class {
    let (group, name) = parts(key);
    match group {
        "switches" => {
            if name.starts_with("changeover") {
                Class::Other
            } else {
                Class::Disconnector
            }
        }
        "protection" => {
            if name.starts_with("fuse_switch") {
                Class::FuseSwitch
            } else if name.starts_with("fuse") {
                Class::Fuse
            } else if name.starts_with("mcb") || name.starts_with("rcbo") {
                Class::Breaker
            } else if name.starts_with("rcd") {
                Class::Rcd
            } else if name.starts_with("overload") {
                Class::OverloadRelay
            } else if name.starts_with("motor_protection") {
                Class::MotorProtectionBreaker
            } else if name.starts_with("motor_circuit") {
                Class::MotorCircuitProtector
            } else if name.starts_with("surge") {
                Class::SurgeArrester
            } else {
                Class::Other
            }
        }
        "contactors" => Class::Contactor,
        "coils" => Class::Coil,
        "contacts" => Class::Contact,
        "operators" => {
            if name == "emergency_stop" || name.starts_with("pushbutton_mushroom") {
                Class::EmergencyStop
            } else if name.starts_with("pullcord") {
                Class::PullCord
            } else if name.starts_with("pushbutton") {
                Class::Pushbutton
            } else if name.starts_with("selector") {
                Class::Selector
            } else if name.starts_with("keyswitch") {
                Class::KeySwitch
            } else if name.starts_with("footswitch") {
                Class::Footswitch
            } else {
                Class::Other
            }
        }
        "signalling" => {
            if name.starts_with("lamp") {
                Class::Lamp
            } else {
                Class::Sounder
            }
        }
        "motors" => {
            if name.starts_with("generator") {
                Class::Other
            } else {
                Class::Motor
            }
        }
        "drives" => {
            if name == "soft_starter" {
                Class::SoftStarter
            } else {
                Class::Drive
            }
        }
        "transformers" => {
            if name.starts_with("transformer") {
                Class::Transformer
            } else {
                Class::Other
            }
        }
        "supplies" => Class::PowerSupply,
        "passive" => {
            if name.starts_with("heater") {
                Class::Heater
            } else {
                Class::Other
            }
        }
        "terminals" => Class::Terminal,
        "earth" => Class::Earth,
        "sensors" => Class::Sensor,
        "plc" => {
            if name == "power_supply" {
                Class::PowerSupply
            } else {
                Class::PlcIo
            }
        }
        "sources" => Class::Source,
        _ => Class::Other,
    }
}

pub(crate) fn class_of(symbol: &SymbolInstance) -> Class {
    classify(&symbol.symbol.0)
}

/// Whether the symbol's contact is a break contact (`_nc`) or a make contact
/// (`_no`), when the key says.
pub(crate) fn is_break_contact(key: &str) -> Option<bool> {
    let (_, name) = parts(key);
    if name.ends_with("_nc") || name == "emergency_stop" {
        Some(true)
    } else if name.ends_with("_no") {
        Some(false)
    } else {
        None
    }
}

/// The colour a lamp symbol is drawn in (`lamp_red`), when it has one.
pub(crate) fn lamp_colour(key: &str) -> Option<Colour> {
    let (_, name) = parts(key);
    Some(match name.strip_prefix("lamp_")? {
        "red" => Colour::Red,
        "amber" => Colour::Yellow,
        "green" => Colour::Green,
        "blue" => Colour::Blue,
        "white" => Colour::White,
        _ => return None,
    })
}

/// A conductor, actuator or lens colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Colour {
    Black,
    Brown,
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Violet,
    Grey,
    White,
    Pink,
    Turquoise,
    /// The green-yellow bi-colour of the protective conductor.
    GreenYellow,
    /// White with a blue stripe (the grounded dc conductor in NFPA 79).
    WhiteBlue,
}

impl Colour {
    /// A name for messages.
    pub fn name(self) -> &'static str {
        match self {
            Colour::Black => "black",
            Colour::Brown => "brown",
            Colour::Red => "red",
            Colour::Orange => "orange",
            Colour::Yellow => "yellow",
            Colour::Green => "green",
            Colour::Blue => "blue",
            Colour::Violet => "violet",
            Colour::Grey => "grey",
            Colour::White => "white",
            Colour::Pink => "pink",
            Colour::Turquoise => "turquoise",
            Colour::GreenYellow => "green-yellow",
            Colour::WhiteBlue => "white with blue stripe",
        }
    }

    /// Reads a colour from a code (`BK`, IEC 60757), a name (`black`,
    /// `light blue`) or the common bi-colour spellings (`GNYE`, `GN/YE`,
    /// `green/yellow`, `green-yellow`). `None` for anything else, which the
    /// rules then report as unknown rather than guess.
    pub fn parse(text: &str) -> Option<Colour> {
        let t: String = text
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| !matches!(c, ' ' | '-' | '_' | '/' | '.' | '+'))
            .collect();
        Some(match t.as_str() {
            "bk" | "black" | "blk" | "sw" | "schwarz" | "zwart" => Colour::Black,
            "bn" | "brown" | "brn" | "br" | "braun" | "bruin" => Colour::Brown,
            "rd" | "red" | "rot" | "rood" => Colour::Red,
            "og" | "orange" | "or" | "oranje" => Colour::Orange,
            "ye" | "yellow" | "yel" | "gelb" | "geel" => Colour::Yellow,
            "gn" | "green" | "grn" | "grün" | "gruen" | "groen" => Colour::Green,
            "bu" | "blue" | "blu" | "lightblue" | "lbu" | "blau" | "blauw" => Colour::Blue,
            "vt" | "violet" | "purple" | "vio" => Colour::Violet,
            "gy" | "grey" | "gray" | "gry" | "naturalgray" | "naturalgrey" | "grau" | "grijs" => {
                Colour::Grey
            }
            "wh" | "white" | "wht" | "ws" | "weiss" | "wit" => Colour::White,
            "pk" | "pink" | "rosa" | "roze" => Colour::Pink,
            "tq" | "turquoise" => Colour::Turquoise,
            "gnye" | "greenyellow" | "yegn" | "yellowgreen" | "gryl" | "grye" | "gnge"
            | "grngelb" | "groengeel" => Colour::GreenYellow,
            "whbu" | "whitebluestripe" | "whitewithbluestripe" | "whitebl" => Colour::WhiteBlue,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_classify_the_same_in_both_packs() {
        for pack in ["iec", "nfpa"] {
            assert_eq!(classify(&format!("{pack}/protection/fuse_3p")), Class::Fuse);
            assert_eq!(
                classify(&format!("{pack}/switches/main_switch_3p")),
                Class::Disconnector
            );
            assert_eq!(
                classify(&format!("{pack}/protection/fuse_switch_3p")),
                Class::FuseSwitch
            );
            assert_eq!(
                classify(&format!("{pack}/operators/emergency_stop")),
                Class::EmergencyStop
            );
            assert_eq!(
                classify(&format!("{pack}/operators/pushbutton_mushroom_nc")),
                Class::EmergencyStop
            );
            assert_eq!(classify(&format!("{pack}/motors/motor_3ph")), Class::Motor);
            assert_eq!(
                classify(&format!("{pack}/protection/overload_relay")),
                Class::OverloadRelay
            );
        }
        assert_eq!(classify("something/else"), Class::Other);
    }

    #[test]
    fn contacts_and_lamps_are_read_from_the_key() {
        assert_eq!(is_break_contact("iec/operators/emergency_stop"), Some(true));
        assert_eq!(
            is_break_contact("iec/operators/pushbutton_mushroom_no"),
            Some(false)
        );
        assert_eq!(is_break_contact("iec/coils/relay"), None);
        assert_eq!(
            lamp_colour("iec/signalling/lamp_amber"),
            Some(Colour::Yellow)
        );
        assert_eq!(lamp_colour("iec/signalling/lamp"), None);
    }

    #[test]
    fn colours_parse_codes_names_and_bicolours() {
        for text in ["GNYE", "GN/YE", "green-yellow", "Green/Yellow", "gn ye"] {
            assert_eq!(Colour::parse(text), Some(Colour::GreenYellow), "{text}");
        }
        assert_eq!(Colour::parse("BK"), Some(Colour::Black));
        assert_eq!(Colour::parse("light blue"), Some(Colour::Blue));
        assert_eq!(Colour::parse("LBU"), Some(Colour::Blue));
        assert_eq!(Colour::parse("natural gray"), Some(Colour::Grey));
        assert_eq!(Colour::parse("WH/BU"), Some(Colour::WhiteBlue));
        assert_eq!(Colour::parse("striped"), None);
        assert_eq!(Colour::parse(""), None);
    }
}
