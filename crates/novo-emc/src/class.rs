//! Wire classes, how loud and how sensitive each is, and the planning
//! distances between them.
//!
//! The numbers are planning values from common installation practice (the
//! kind of table a cabling guide prints), written down in our own words and
//! **not verified against a standard**. Every one can be overridden per
//! project (`emc.distance.<aggressor>.<victim>_mm`).

use std::fmt;

/// What a wire does to its neighbours and what its neighbours do to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EmcClass {
    /// Drive output to a motor: steep edges, the loudest wire in a cabinet.
    Motor,
    /// Mains and other power wiring.
    Power,
    /// 24 V supply and digital control and I/O.
    Control,
    /// Analog and sensor signals.
    Analog,
    /// Fieldbus and Ethernet.
    Fieldbus,
    /// Safety circuits.
    Safety,
    /// Protective earth and bonding: neither judged nor judging here.
    Earth,
}

impl EmcClass {
    pub const ALL: [EmcClass; 7] = [
        EmcClass::Motor,
        EmcClass::Power,
        EmcClass::Control,
        EmcClass::Analog,
        EmcClass::Fieldbus,
        EmcClass::Safety,
        EmcClass::Earth,
    ];

    /// The id stored in the `emc.class` property.
    pub fn id(self) -> &'static str {
        match self {
            EmcClass::Motor => "motor",
            EmcClass::Power => "power",
            EmcClass::Control => "control",
            EmcClass::Analog => "analog",
            EmcClass::Fieldbus => "fieldbus",
            EmcClass::Safety => "safety",
            EmcClass::Earth => "earth",
        }
    }

    pub fn parse(text: &str) -> Option<EmcClass> {
        let t = text.trim().to_ascii_lowercase();
        EmcClass::ALL.into_iter().find(|c| c.id() == t)
    }

    /// A short name for chips and messages.
    pub fn label(self) -> &'static str {
        match self {
            EmcClass::Motor => "Motor (drive output)",
            EmcClass::Power => "Power",
            EmcClass::Control => "Control and 24 V",
            EmcClass::Analog => "Analog and sensor",
            EmcClass::Fieldbus => "Fieldbus",
            EmcClass::Safety => "Safety",
            EmcClass::Earth => "Earth",
        }
    }

    /// How hard the class disturbs others, 0 to 1.
    pub fn aggression(self) -> f64 {
        match self {
            EmcClass::Motor => 1.0,
            EmcClass::Power => 0.7,
            EmcClass::Control => 0.2,
            EmcClass::Fieldbus => 0.1,
            EmcClass::Analog | EmcClass::Safety => 0.05,
            EmcClass::Earth => 0.0,
        }
    }

    /// How easily the class is disturbed, 0 to 1.
    pub fn sensitivity(self) -> f64 {
        match self {
            EmcClass::Analog => 1.0,
            EmcClass::Fieldbus => 0.9,
            EmcClass::Safety => 0.8,
            EmcClass::Control => 0.4,
            EmcClass::Motor | EmcClass::Power => 0.1,
            EmcClass::Earth => 0.0,
        }
    }

    /// Whether the class is a noise source for the others.
    pub fn is_aggressor(self) -> bool {
        matches!(self, EmcClass::Motor | EmcClass::Power)
    }
}

impl fmt::Display for EmcClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Planning distance in mm that two unshielded runs of these classes keep
/// when they run parallel. 0 when the pair is not judged. Symmetric.
pub fn default_distance_mm(a: EmcClass, b: EmcClass) -> f64 {
    use EmcClass::*;
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    match (lo, hi) {
        (Motor, Control) => 200.0,
        (Motor, Analog) => 300.0,
        (Motor, Fieldbus) => 300.0,
        (Motor, Safety) => 200.0,
        (Power, Control) => 100.0,
        (Power, Analog) => 200.0,
        (Power, Fieldbus) => 200.0,
        (Power, Safety) => 100.0,
        (Control, Analog) => 50.0,
        (Control, Fieldbus) => 50.0,
        (Control, Safety) => 30.0,
        _ => 0.0,
    }
}

/// How far a shield cuts the distance a run needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Termination {
    /// Shield bonded all round at the gland or a shield bar.
    Full360,
    /// Shield led away on a wire of this length in mm.
    Pigtail(f64),
    /// Shield not connected at this end.
    Open,
    /// The project does not say.
    Unknown,
}

/// A cable shield and how both ends are made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shield {
    pub a: Termination,
    pub b: Termination,
}

impl Shield {
    /// Factor on the planning distance: 1 means no help from the shield.
    /// Both ends all round 0.25; a pigtail at an end helps less; one end
    /// bonded about 0.6; unknown ends are not counted.
    pub fn factor(&self) -> f64 {
        let end = |t: Termination| match t {
            Termination::Full360 => 2,
            Termination::Pigtail(mm) if mm <= 50.0 => 1,
            _ => 0,
        };
        match end(self.a) + end(self.b) {
            4 => 0.25,
            3 => 0.4,
            2 => 0.6,
            1 => 0.8,
            _ => 1.0,
        }
    }

    /// Both ends bonded all round.
    pub fn is_full(&self) -> bool {
        self.a == Termination::Full360 && self.b == Termination::Full360
    }
}
