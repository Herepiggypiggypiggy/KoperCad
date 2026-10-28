//! A hoist described by plain device properties (lane L5a, "Mechanical load").
//!
//! The motor's property sheet holds the numbers as `mech_*` properties; this module reads
//! them (a lookup closure keeps `novo-machine` free of the project model), checks them and
//! builds the engine and its bindings. A motor with `mech_mass_kg` set is a hoist. Nothing
//! here is a manufacturer's data: the defaults are a generic 12/2 pole hoist motor and a
//! generic spring brake, labelled as such in the UI.

use std::fmt;

use novo_drive::InputError;
use novo_drive::brake::{SpringBrake, Switching};
use novo_drive::hoist_unit::{BrakeFeed, TwoSpeedHoistUnit};
use novo_drive::mechanics::Hoist;
use novo_drive::motor::{PoleChangeMotor, Winding};
use novo_drive::sequencer::{Sequencer, SequencerConfig};

use crate::bindings::SensorBinding;
use crate::hoist_loop::{HoistBinding, HoistModule, LimitKind, LimitSwitch};

/// Property key that marks a motor as a hoist.
pub const HOIST_KEY: &str = "mech_mass_kg";

/// One row of the "Mechanical load" section.
#[derive(Debug, Clone, Copy)]
pub struct SpecKey {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    /// Shown when the property is empty and used by the run.
    pub default: &'static str,
    /// A device tag, not a number.
    pub text: bool,
}

const fn num(
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    default: &'static str,
) -> SpecKey {
    SpecKey {
        key,
        label,
        unit,
        default,
        text: false,
    }
}

const fn tag(key: &'static str, label: &'static str) -> SpecKey {
    SpecKey {
        key,
        label,
        unit: "",
        default: "",
        text: true,
    }
}

/// The rows, in the order the property sheet shows them. `mech_mass_kg` comes first and has no
/// default: setting it turns the motor into a hoist.
pub const KEYS: &[SpecKey] = &[
    num(HOIST_KEY, "Hook load (payload and block)", "kg", ""),
    num("mech_drum_radius_m", "Drum radius", "m", "0.15"),
    num("mech_gear_ratio", "Gearbox ratio (motor to drum)", "", "40"),
    num("mech_falls", "Rope falls", "", "1"),
    num("mech_efficiency", "Efficiency per direction", "", "0.9"),
    num("mech_fine_poles", "Fine winding poles", "", "12"),
    num("mech_fine_kw", "Fine winding power", "kW", "0.8"),
    num("mech_fine_rpm", "Fine winding rated speed", "rpm", "480"),
    num("mech_coarse_poles", "Coarse winding poles", "", "2"),
    num("mech_coarse_kw", "Coarse winding power", "kW", "6"),
    num(
        "mech_coarse_rpm",
        "Coarse winding rated speed",
        "rpm",
        "2880",
    ),
    num("mech_frequency_hz", "Supply frequency", "Hz", "50"),
    num("mech_rotor_inertia", "Rotor inertia", "kg m\u{b2}", "0.05"),
    num("mech_brake_torque_nm", "Brake torque", "N m", "60"),
    num("mech_brake_release_s", "Brake release delay", "s", "0.05"),
    num(
        "mech_brake_apply_ac_s",
        "Brake apply delay, AC side",
        "s",
        "0.2",
    ),
    num(
        "mech_brake_apply_dc_s",
        "Brake apply delay, DC side",
        "s",
        "0.03",
    ),
    tag("mech_brake_switching", "Brake switched on (AC or DC)"),
    tag("mech_brake_feed", "Brake rectifier feed device"),
    tag("mech_main_contactor", "Main contactor device"),
    tag("mech_upper_limit", "Upper limit switch device"),
    num(
        "mech_upper_limit_m",
        "Upper limit height (m above the hook start)",
        "m",
        "",
    ),
    tag("mech_lower_limit", "Lower limit switch device"),
    num(
        "mech_lower_limit_m",
        "Lower limit height (m above the hook start)",
        "m",
        "",
    ),
];

/// Why a set of properties does not make a hoist.
#[derive(Debug, Clone, PartialEq)]
pub enum SpecError {
    /// A number that is not a number.
    NotANumber { key: &'static str, text: String },
    /// A count that has to be a whole number (poles) is not one.
    NotWhole { key: &'static str, value: f64 },
    /// The engine refused a value.
    Value(InputError),
    /// A limit switch device without a height, or the other way round.
    LimitIncomplete(&'static str),
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotANumber { key, text } => write!(f, "{key}: \"{text}\" is not a number"),
            Self::NotWhole { key, value } => {
                write!(f, "{key}: {value} must be a whole number")
            }
            Self::Value(e) => write!(f, "{e}"),
            Self::LimitIncomplete(which) => {
                write!(f, "the {which} limit needs both a device and a height")
            }
        }
    }
}

impl std::error::Error for SpecError {}

impl From<InputError> for SpecError {
    fn from(e: InputError) -> Self {
        Self::Value(e)
    }
}

/// Reads a hoist from properties. `get(key)` returns the property text if set. Returns
/// `Ok(None)` when the motor is no hoist (`mech_mass_kg` not set).
pub fn hoist_from_properties(
    id: &str,
    motor_tag: &str,
    get: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<HoistModule>, SpecError> {
    let text = |key: &'static str| {
        get(key)
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    if text(HOIST_KEY).is_none() {
        return Ok(None);
    }
    let number = |key: &'static str| -> Result<f64, SpecError> {
        let default = KEYS.iter().find(|k| k.key == key).map_or("", |k| k.default);
        let raw = text(key).unwrap_or_else(|| default.to_owned());
        // A comma is a decimal comma: "1,500" reads as 1.5, not 1500 (the property sheet says so).
        raw.replace(',', ".")
            .parse::<f64>()
            .map_err(|_| SpecError::NotANumber { key, text: raw })
    };
    let hz = number("mech_frequency_hz")?;
    let winding = |poles: &'static str, kw: &'static str, rpm: &'static str, breakdown: f64| {
        let key = poles;
        let poles = number(key)?;
        // A pole count is a whole number; reject 12.5 rather than rounding it silently.
        if poles.fract() != 0.0 || poles < 0.0 || poles > f64::from(u32::MAX) {
            return Err(SpecError::NotWhole { key, value: poles });
        }
        Winding::new(
            poles as u32,
            hz,
            number(kw)? * 1000.0,
            number(rpm)?,
            breakdown,
        )
        .map_err(SpecError::from)
    };
    let motor = PoleChangeMotor::new(
        winding("mech_fine_poles", "mech_fine_kw", "mech_fine_rpm", 2.0)?,
        winding(
            "mech_coarse_poles",
            "mech_coarse_kw",
            "mech_coarse_rpm",
            2.2,
        )?,
        number("mech_rotor_inertia")?,
    )?;
    let falls = number("mech_falls")?;
    if falls.fract() != 0.0 || !(1.0..=64.0).contains(&falls) {
        return Err(SpecError::Value(InputError::OutOfRange {
            field: "mech_falls",
            value: falls,
            min: 1.0,
            max: 64.0,
        }));
    }
    let hoist = Hoist::new(
        number(HOIST_KEY)?,
        number("mech_drum_radius_m")?,
        number("mech_gear_ratio")?,
        falls as u32,
        number("mech_efficiency")?,
    )?;
    let switching = match text("mech_brake_switching").map(|t| t.to_ascii_lowercase()) {
        None => Switching::AcSide,
        Some(t) if t == "ac" => Switching::AcSide,
        Some(t) if t == "dc" => Switching::DcSide,
        Some(t) => {
            return Err(SpecError::NotANumber {
                key: "mech_brake_switching",
                text: t,
            });
        }
    };
    let brake = SpringBrake::new(
        number("mech_brake_torque_nm")?,
        number("mech_brake_release_s")?,
        number("mech_brake_apply_ac_s")?,
        number("mech_brake_apply_dc_s")?,
        0.02,
        switching,
    )?;
    // The circuit decides what is fed; the sequencer is unused here and gets neutral times.
    let sequencer = Sequencer::new(SequencerConfig {
        changeover_dead_time_s: 0.1,
        fine_first_s: 0.5,
        reversal_dead_time_s: 0.2,
    })?;
    let brake_feed = text("mech_brake_feed");
    let feed = if brake_feed.is_some() {
        BrakeFeed::Separate
    } else {
        BrakeFeed::MotorTerminals
    };
    let unit = TwoSpeedHoistUnit::new(motor, hoist, sequencer, brake, feed)?;

    let mut limits = Vec::new();
    for (which, kind, device, height) in [
        (
            "upper",
            LimitKind::Upper,
            "mech_upper_limit",
            "mech_upper_limit_m",
        ),
        (
            "lower",
            LimitKind::Lower,
            "mech_lower_limit",
            "mech_lower_limit_m",
        ),
    ] {
        match (text(device), text(height)) {
            (None, None) => {}
            (Some(device_tag), Some(_)) => limits.push(LimitSwitch {
                binding: SensorBinding {
                    name: which.to_owned(),
                    device_tag,
                    actuate_at_mm: number(height)? * 1000.0,
                },
                kind,
            }),
            _ => return Err(SpecError::LimitIncomplete(which)),
        }
    }
    Ok(Some(HoistModule::new(
        id,
        unit,
        HoistBinding {
            motor: motor_tag.to_owned(),
            brake_feed,
            mains: text("mech_main_contactor"),
            limits,
        },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn build(p: &BTreeMap<String, String>) -> Result<Option<HoistModule>, SpecError> {
        hoist_from_properties("h", "=M1", &|k| p.get(k).cloned())
    }

    #[test]
    fn a_motor_without_a_load_is_no_hoist() {
        assert!(build(&props(&[])).unwrap().is_none());
        assert!(build(&props(&[("mech_mass_kg", "  ")])).unwrap().is_none());
    }

    #[test]
    fn the_mass_alone_gives_a_generic_hoist() {
        let h = build(&props(&[("mech_mass_kg", "500")])).unwrap().unwrap();
        assert_eq!(h.binding.motor, "=M1");
        assert!(h.binding.limits.is_empty());
        assert!(h.binding.mains.is_none());
        assert_eq!(h.unit.brake_feed(), BrakeFeed::MotorTerminals);
    }

    #[test]
    fn limits_and_feeds_are_read_with_a_comma_decimal() {
        let h = build(&props(&[
            ("mech_mass_kg", "500"),
            ("mech_upper_limit", "=S1"),
            ("mech_upper_limit_m", "4,5"),
            ("mech_brake_feed", "=K9"),
            ("mech_main_contactor", "=K1"),
            ("mech_brake_switching", "DC"),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(h.unit.brake_feed(), BrakeFeed::Separate);
        assert_eq!(h.binding.limits[0].binding.actuate_at_mm, 4500.0);
        assert_eq!(h.binding.limits[0].kind, LimitKind::Upper);
        assert_eq!(h.binding.mains.as_deref(), Some("=K1"));
    }

    #[test]
    fn bad_values_are_named() {
        let bad = |extra: &[(&str, &str)]| {
            let mut p = props(&[("mech_mass_kg", "500")]);
            p.extend(props(extra));
            build(&p).unwrap_err().to_string()
        };
        assert!(bad(&[("mech_gear_ratio", "x")]).contains("mech_gear_ratio"));
        assert!(bad(&[("mech_falls", "2.5")]).contains("mech_falls"));
        assert!(bad(&[("mech_fine_poles", "2"), ("mech_coarse_poles", "2")]).contains("poles"));
        assert!(bad(&[("mech_fine_poles", "12.5")]).contains("whole number"));
        // A comma is a decimal comma: 1,500 falls reads as 1.5, a non-whole count.
        assert!(bad(&[("mech_falls", "1,500")]).contains("mech_falls"));
        assert!(bad(&[("mech_upper_limit", "=S1")]).contains("upper limit"));
        assert!(bad(&[("mech_lower_limit_m", "0.2")]).contains("lower limit"));
        assert!(bad(&[("mech_brake_switching", "both")]).contains("mech_brake_switching"));
        assert!(bad(&[("mech_drum_radius_m", "-1")]).contains("greater than zero"));
    }

    #[test]
    fn every_key_is_listed_once() {
        let mut seen = std::collections::BTreeSet::new();
        for k in KEYS {
            assert!(seen.insert(k.key), "{}", k.key);
            assert!(k.key.starts_with("mech_"));
        }
        assert_eq!(KEYS[0].key, HOIST_KEY);
    }
}
