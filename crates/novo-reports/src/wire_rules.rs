//! The project's wire colour and size rules.
//!
//! Panel builders have house rules, so the Autopilot does not hard-code one
//! rule set. The project states a preset (IEC 60204-1, or a house set whose
//! colours and minimum sections the project types in) in its properties; the
//! Autopilot reads it and names the rule in every reason.

use novo_core::{Project, Value};

/// Property keys the rules are stored under (project info).
pub const PRESET: &str = "wire_rules.preset";
pub const MIN_POWER_MM2: &str = "wire_rules.min_mm2.power";
pub const MIN_CONTROL_MM2: &str = "wire_rules.min_mm2.control";

/// Colour codes a rule can name (IEC 60757).
pub const CODES: [&str; 13] = [
    "BK", "BN", "RD", "OG", "YE", "GN", "BU", "VT", "GY", "WH", "PK", "TQ", "GNYE",
];

/// What a wire colour is chosen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColourClass {
    ProtectiveEarth,
    Pen,
    Neutral,
    /// 0 V / DC minus.
    DcNegative,
    AcPower,
    AcControl,
    DcPower,
    DcControl,
    /// The net carries no potential.
    Unknown,
}

impl ColourClass {
    /// Classes a project can give its own colour. Protective earth and PEN
    /// are not here: they are always green-yellow (IEC 60204-1, IEC 60445).
    pub const EDITABLE: [ColourClass; 6] = [
        ColourClass::Neutral,
        ColourClass::DcNegative,
        ColourClass::AcPower,
        ColourClass::AcControl,
        ColourClass::DcPower,
        ColourClass::DcControl,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ColourClass::ProtectiveEarth => "Protective earth (PE)",
            ColourClass::Pen => "PEN",
            ColourClass::Neutral => "Neutral",
            ColourClass::DcNegative => "0 V / DC minus",
            ColourClass::AcPower => "AC power",
            ColourClass::AcControl => "AC control",
            ColourClass::DcPower => "DC power",
            ColourClass::DcControl => "DC control",
            ColourClass::Unknown => "No potential",
        }
    }

    /// Property key of this class's house colour.
    pub fn key(self) -> String {
        let name = match self {
            ColourClass::ProtectiveEarth => "pe",
            ColourClass::Pen => "pen",
            ColourClass::Neutral => "neutral",
            ColourClass::DcNegative => "dc_negative",
            ColourClass::AcPower => "ac_power",
            ColourClass::AcControl => "ac_control",
            ColourClass::DcPower => "dc_power",
            ColourClass::DcControl => "dc_control",
            ColourClass::Unknown => "unknown",
        };
        format!("wire_rules.colour.{name}")
    }
}

/// Which rule set applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    /// IEC 60204-1 colours and the program's smallest sections.
    #[default]
    Iec60204,
    /// The colours and minimum sections the project states; anything left
    /// blank follows IEC 60204-1.
    House,
}

impl Preset {
    pub fn label(self) -> &'static str {
        match self {
            Preset::Iec60204 => "IEC 60204-1",
            Preset::House => "House rules",
        }
    }

    fn text(self) -> &'static str {
        match self {
            Preset::Iec60204 => "iec60204",
            Preset::House => "house",
        }
    }
}

/// The rules of one project.
#[derive(Debug, Clone, PartialEq)]
pub struct WireRules {
    pub preset: Preset,
    house_colours: Vec<(ColourClass, String)>,
    /// Smallest section, mm², for power and control wires (house only).
    pub min_power_mm2: Option<f64>,
    pub min_control_mm2: Option<f64>,
}

impl Default for WireRules {
    fn default() -> Self {
        WireRules {
            preset: Preset::Iec60204,
            house_colours: Vec::new(),
            min_power_mm2: None,
            min_control_mm2: None,
        }
    }
}

fn text(project: &Project, key: &str) -> Option<String> {
    match project.info().properties.get(key) {
        Some(Value::Text(t)) if !t.trim().is_empty() => Some(t.trim().to_owned()),
        _ => None,
    }
}

fn number(project: &Project, key: &str) -> Option<f64> {
    match project.info().properties.get(key) {
        Some(Value::Float(f)) => Some(*f),
        Some(Value::Int(i)) => Some(*i as f64),
        _ => None,
    }
    .filter(|v| v.is_finite() && *v > 0.0)
}

/// Smallest sections a house rule may name, mm². Anything else in a file or
/// script reads as unset.
pub const MIN_RANGE_MM2: (f64, f64) = (0.5, 4.0);

fn in_range(v: &f64) -> bool {
    (MIN_RANGE_MM2.0..=MIN_RANGE_MM2.1).contains(v)
}

/// Normal form of a colour code the project typed (`gnye` -> `GNYE`), or
/// `None` when it is not a code of [`CODES`].
pub fn normal_code(code: &str) -> Option<&'static str> {
    let up = code.trim().to_uppercase();
    CODES.iter().copied().find(|c| *c == up)
}

impl WireRules {
    /// The rules the project states; IEC 60204-1 when it states none.
    pub fn of(project: &Project) -> WireRules {
        let preset = match text(project, PRESET).as_deref() {
            Some("house") => Preset::House,
            _ => Preset::Iec60204,
        };
        if preset == Preset::Iec60204 {
            return WireRules::default();
        }
        let house_colours = ColourClass::EDITABLE
            .iter()
            .filter_map(|c| {
                let code = normal_code(&text(project, &c.key())?)?;
                Some((*c, code.to_owned()))
            })
            .collect();
        WireRules {
            preset,
            house_colours,
            min_power_mm2: number(project, MIN_POWER_MM2).filter(in_range),
            min_control_mm2: number(project, MIN_CONTROL_MM2).filter(in_range),
        }
    }

    /// The project's own colour for a class, if the house set names one.
    pub fn house_colour(&self, class: ColourClass) -> Option<&str> {
        self.house_colours
            .iter()
            .find(|(c, _)| *c == class)
            .map(|(_, code)| code.as_str())
    }

    /// Smallest section for the class of wire, if the house set states one.
    pub fn min_mm2(&self, control: bool) -> Option<f64> {
        if control {
            self.min_control_mm2
        } else {
            self.min_power_mm2
        }
    }

    /// Value for the preset property.
    pub fn preset_text(&self) -> &'static str {
        self.preset.text()
    }
}
