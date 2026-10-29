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
use novo_drive::brake_system::{BrakeSource, BrakeSystem, ConicalPull, Rectifier, RectifierKind};
use novo_drive::drive::MotorNameplate;
use novo_drive::hoist_thermal::{MotorThermal, OverloadRelay, ThermalParams, WindingElectrics};
use novo_drive::hoist_unit::{BrakeFeed, FineLift, HoistElectrics, TwoSpeedHoistUnit};
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
    tag(
        "mech_brake_kind",
        "Brake kind (rectifier, or conical: lifted by the motor field)",
    ),
    tag(
        "mech_rectifier",
        "Rectifier (bridge, half-wave, fast or controlled)",
    ),
    num(
        "mech_rectifier_boost",
        "Fast rectifier: over-excitation",
        "x",
        "2",
    ),
    num(
        "mech_rectifier_boost_s",
        "Fast rectifier: over-excitation time",
        "s",
        "0.2",
    ),
    num(
        "mech_brake_pull_in",
        "Brake pull-in (share of rated voltage)",
        "",
        "0.8",
    ),
    num(
        "mech_brake_drop_out",
        "Brake drop-out (share of rated voltage)",
        "",
        "0.4",
    ),
    num(
        "mech_conical_phase_pull",
        "Conical brake: pull left with a phase lost",
        "",
        "0.5",
    ),
    num(
        "mech_conical_fine_pull",
        "Conical brake: pull on the fine winding vs coarse",
        "",
        "1",
    ),
    tag("mech_fine_lift_motor", "Fine-lift motor device"),
    num("mech_fl_poles", "Fine-lift motor poles", "", "4"),
    num("mech_fl_kw", "Fine-lift motor power", "kW", "0.37"),
    num("mech_fl_rpm", "Fine-lift motor rated speed", "rpm", "1380"),
    num(
        "mech_fl_ratio",
        "Fine-lift speed ratio (fine motor over main shaft)",
        "",
        "10",
    ),
    num(
        "mech_fl_inertia",
        "Fine-lift rotor inertia",
        "kg m\u{b2}",
        "0.002",
    ),
    num(
        "mech_fl_brake_torque_nm",
        "Fine-lift brake torque",
        "N m",
        "8",
    ),
    tag(
        "mech_fl_releases_main_brake",
        "Fine-lift motor also lifts the main brake (yes or no)",
    ),
    num(
        "mech_voltage_v",
        "Supply voltage (line to line)",
        "V",
        "400",
    ),
    num(
        "mech_fine_amps",
        "Fine winding rated current (empty = from power, with an assumed efficiency)",
        "A",
        "",
    ),
    num("mech_fine_pf", "Fine winding power factor", "", "0.6"),
    num(
        "mech_coarse_amps",
        "Coarse winding rated current (empty = from power, with an assumed efficiency)",
        "A",
        "",
    ),
    num("mech_coarse_pf", "Coarse winding power factor", "", "0.85"),
    num(
        "mech_fl_amps",
        "Fine-lift motor rated current (empty = from power, with an assumed efficiency)",
        "A",
        "",
    ),
    num("mech_fl_pf", "Fine-lift motor power factor", "", "0.75"),
    num("mech_rated_rise_k", "Winding rise at rated load", "K", "80"),
    num("mech_rise_limit_k", "Winding rise limit", "K", "100"),
    num(
        "mech_thermal_tau_s",
        "Winding heat time constant",
        "s",
        "40",
    ),
    num("mech_frame_tau_s", "Frame heat time constant", "s", "1500"),
    num(
        "mech_overload_amps",
        "Overload relay setting, main motor only (empty = no relay)",
        "A",
        "",
    ),
    num(
        "mech_overload_class_s",
        "Overload trip class (s at 7.2 x setting)",
        "s",
        "10",
    ),
    tag(
        "mech_overload_contact",
        "Overload relay contact device (opens on trip)",
    ),
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
    /// A word that is not one of the known choices.
    UnknownWord { key: &'static str, text: String },
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
            Self::UnknownWord { key, text } => {
                write!(f, "{key}: \"{text}\" is not a known choice")
            }
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

/// Reads a typed number. A comma is a decimal comma ("1,5" is 1.5), but a comma followed by
/// three digits ("5,000") could as well be a thousands separator: refused, not guessed. An
/// integer part of 0 ("0,500", "-0,125") can never be a thousands group, so it reads as decimal.
pub(crate) fn decimal(key: &'static str, raw: String) -> Result<f64, SpecError> {
    if let Some((before, after)) = raw.split_once(',')
        && after.chars().filter(char::is_ascii_digit).count() >= 3
        && !matches!(before.trim(), "0" | "-0" | "+0")
    {
        return Err(SpecError::NotANumber { key, text: raw });
    }
    raw.replace(',', ".")
        .parse::<f64>()
        .map_err(|_| SpecError::NotANumber { key, text: raw })
}

type TextFn<'a> = dyn Fn(&'static str) -> Option<String> + 'a;
type NumFn<'a> = dyn Fn(&'static str) -> Result<f64, SpecError> + 'a;

/// What lifts the brake, from `mech_brake_kind` (empty = a rectifier) and its details.
fn brake_source(text: &TextFn<'_>, number: &NumFn<'_>) -> Result<BrakeSource, SpecError> {
    let word = |key: &'static str| text(key).map(|t| t.to_ascii_lowercase());
    match word("mech_brake_kind").as_deref() {
        Some("conical") => Ok(BrakeSource::Conical(ConicalPull::new(
            number("mech_conical_phase_pull")?,
            number("mech_conical_fine_pull")?,
        )?)),
        None | Some("rectifier") => {
            let kind = match word("mech_rectifier").as_deref() {
                None | Some("bridge") => RectifierKind::Bridge,
                Some("half-wave" | "halfwave" | "half wave") => RectifierKind::HalfWave,
                Some("controlled") => RectifierKind::Controlled,
                Some("fast") => RectifierKind::FastExcitation {
                    boost: number("mech_rectifier_boost")?,
                    boost_s: number("mech_rectifier_boost_s")?,
                },
                Some(other) => {
                    return Err(SpecError::UnknownWord {
                        key: "mech_rectifier",
                        text: other.to_owned(),
                    });
                }
            };
            Ok(BrakeSource::Rectifier(Rectifier::new(kind)?))
        }
        Some(other) => Err(SpecError::UnknownWord {
            key: "mech_brake_kind",
            text: other.to_owned(),
        }),
    }
}

/// The fine-lift motor from its `mech_fl_*` properties. Its brake is of the same kind as the
/// main one (`source`) with its own torque; delays and thresholds follow the main brake.
fn fine_lift(
    text: &TextFn<'_>,
    number: &NumFn<'_>,
    hz: f64,
    source: BrakeSource,
) -> Result<FineLift, SpecError> {
    let poles = number("mech_fl_poles")?;
    if poles.fract() != 0.0 || poles < 0.0 || poles > f64::from(u32::MAX) {
        return Err(SpecError::NotWhole {
            key: "mech_fl_poles",
            value: poles,
        });
    }
    let winding = Winding::new(
        poles as u32,
        hz,
        number("mech_fl_kw")? * 1000.0,
        number("mech_fl_rpm")?,
        2.2,
    )?;
    let mut brake = SpringBrake::new(
        number("mech_fl_brake_torque_nm")?,
        number("mech_brake_release_s")?,
        number("mech_brake_apply_ac_s")?,
        number("mech_brake_apply_dc_s")?,
        0.02,
        Switching::AcSide,
    )?;
    brake.set_thresholds(
        number("mech_brake_pull_in")?,
        number("mech_brake_drop_out")?,
    )?;
    let releases = match text("mech_fl_releases_main_brake").map(|t| t.to_ascii_lowercase()) {
        None => true,
        Some(t) if matches!(t.as_str(), "yes" | "y" | "true" | "1") => true,
        Some(t) if matches!(t.as_str(), "no" | "n" | "false" | "0") => false,
        Some(t) => {
            return Err(SpecError::UnknownWord {
                key: "mech_fl_releases_main_brake",
                text: t,
            });
        }
    };
    Ok(FineLift::new(
        winding,
        number("mech_fl_ratio")?,
        number("mech_fl_inertia")?,
        BrakeSystem::new(brake, source),
        releases,
    )?)
}

/// Generic efficiency assumed to derive a rated current from the power when none is entered.
/// A pole-change fine winding is far less efficient than the coarse one.
const ASSUMED_EFFICIENCY: [(&str, f64); 3] = [("fine", 0.6), ("coarse", 0.85), ("fine-lift", 0.7)];

/// One winding's electrical model from its `mech_*` numbers. The rated current is the entered
/// one, or derived from the power with a generic efficiency and the power factor.
fn winding_electrics(
    text: &TextFn<'_>,
    number: &NumFn<'_>,
    volts: f64,
    hz: f64,
    keys: [&'static str; 5],
    which: &str,
) -> Result<WindingElectrics, SpecError> {
    let [poles, kw, rpm, amps, pf] = keys;
    let p_kw = number(kw)?;
    let pf_v = number(pf)?;
    let current = match text(amps) {
        Some(_) => number(amps)?,
        None => {
            let eta = ASSUMED_EFFICIENCY
                .iter()
                .find(|(w, _)| *w == which)
                .map_or(0.8, |(_, e)| *e);
            p_kw * 1000.0 / (3f64.sqrt() * volts * eta * pf_v)
        }
    };
    let np = MotorNameplate {
        voltage_v: volts,
        current_a: current,
        frequency_hz: hz,
        rpm: number(rpm)?,
        power_kw: p_kw,
        power_factor: pf_v,
    };
    Ok(WindingElectrics::from_nameplate(&np, number(poles)? as u32)?.0)
}

/// The current, heat and overload relay model of a hoist.
fn electrics(
    text: &TextFn<'_>,
    number: &NumFn<'_>,
    hz: f64,
    fine_lift: bool,
) -> Result<HoistElectrics, SpecError> {
    let volts = number("mech_voltage_v")?;
    let fine = winding_electrics(
        text,
        number,
        volts,
        hz,
        [
            "mech_fine_poles",
            "mech_fine_kw",
            "mech_fine_rpm",
            "mech_fine_amps",
            "mech_fine_pf",
        ],
        "fine",
    )?;
    let coarse = winding_electrics(
        text,
        number,
        volts,
        hz,
        [
            "mech_coarse_poles",
            "mech_coarse_kw",
            "mech_coarse_rpm",
            "mech_coarse_amps",
            "mech_coarse_pf",
        ],
        "coarse",
    )?;
    let params = ThermalParams {
        winding_rise_rated_k: number("mech_rated_rise_k")?,
        limit_rise_k: number("mech_rise_limit_k")?,
        tau_winding_s: number("mech_thermal_tau_s")?,
        tau_frame_s: number("mech_frame_tau_s")?,
        ..ThermalParams::GENERIC
    };
    // The heat model is sized on the larger winding's rated losses.
    let rated_losses = fine.rated_losses_w().max(coarse.rated_losses_w());
    let thermal = MotorThermal::new(params, rated_losses)?;
    let relay = match text("mech_overload_amps") {
        Some(_) => Some(OverloadRelay::new(
            number("mech_overload_amps")?,
            number("mech_overload_class_s")?,
        )?),
        None => None,
    };
    let fine_lift = if fine_lift {
        let w = winding_electrics(
            text,
            number,
            volts,
            hz,
            [
                "mech_fl_poles",
                "mech_fl_kw",
                "mech_fl_rpm",
                "mech_fl_amps",
                "mech_fl_pf",
            ],
            "fine-lift",
        )?;
        let t = MotorThermal::new(params, w.rated_losses_w())?;
        Some((w, t))
    } else {
        None
    };
    Ok(HoistElectrics {
        fine,
        coarse,
        thermal,
        relay,
        fine_lift,
    })
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
        decimal(key, raw)
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
            return Err(SpecError::UnknownWord {
                key: "mech_brake_switching",
                text: t,
            });
        }
    };
    let mut brake = SpringBrake::new(
        number("mech_brake_torque_nm")?,
        number("mech_brake_release_s")?,
        number("mech_brake_apply_ac_s")?,
        number("mech_brake_apply_dc_s")?,
        0.02,
        switching,
    )?;
    brake.set_thresholds(
        number("mech_brake_pull_in")?,
        number("mech_brake_drop_out")?,
    )?;
    let source = brake_source(&text, &number)?;
    let system = BrakeSystem::new(brake, source);
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
    let mut unit = TwoSpeedHoistUnit::with_system(motor, hoist, sequencer, system, feed)?;
    let fine_motor = text("mech_fine_lift_motor");
    unit = unit.with_electrics(electrics(&text, &number, hz, fine_motor.is_some())?);
    if fine_motor.is_some() {
        unit = unit.with_fine_lift(fine_lift(
            &text,
            &number,
            hz,
            brake_source(&text, &number)?,
        )?);
    }

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
            fine_motor,
            overload_contact: text("mech_overload_contact"),
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
        assert!(bad(&[("mech_mass_kg", "5,000")]).contains("mech_mass_kg"));
        assert!(bad(&[("mech_upper_limit", "=S1")]).contains("upper limit"));
        assert!(bad(&[("mech_lower_limit_m", "0.2")]).contains("lower limit"));
        assert!(bad(&[("mech_brake_switching", "both")]).contains("mech_brake_switching"));
        assert!(bad(&[("mech_drum_radius_m", "-1")]).contains("greater than zero"));
    }

    #[test]
    fn brake_kind_and_rectifier_are_read() {
        let kind = |extra: &[(&str, &str)]| {
            let mut p = props(&[("mech_mass_kg", "500")]);
            p.extend(props(extra));
            build(&p)
                .unwrap()
                .unwrap()
                .unit
                .brake_system()
                .source()
                .to_owned()
        };
        assert!(matches!(
            kind(&[]),
            BrakeSource::Rectifier(r) if r.kind() == RectifierKind::Bridge
        ));
        assert!(matches!(
            kind(&[("mech_brake_kind", "Conical")]),
            BrakeSource::Conical(c) if c.phase_lost_pull() == 0.5
        ));
        assert!(matches!(
            kind(&[("mech_rectifier", "half-wave")]),
            BrakeSource::Rectifier(r) if r.kind() == RectifierKind::HalfWave
        ));
        assert!(matches!(
            kind(&[("mech_rectifier", "fast"), ("mech_rectifier_boost", "3")]),
            BrakeSource::Rectifier(r)
                if r.kind() == RectifierKind::FastExcitation { boost: 3.0, boost_s: 0.2 }
        ));
    }

    #[test]
    fn a_fine_lift_motor_is_built_with_its_binding() {
        let h = build(&props(&[
            ("mech_mass_kg", "500"),
            ("mech_brake_kind", "conical"),
            ("mech_fine_lift_motor", "=M2"),
            ("mech_fl_releases_main_brake", "no"),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(h.binding.fine_motor.as_deref(), Some("=M2"));
        let f = h.unit.fine_lift().unwrap();
        assert_eq!(f.ratio(), 10.0);
        assert!(!f.releases_main_brake());
        assert_eq!(f.winding().poles(), 4);
        // No marker, no fine-lift motor.
        let h = build(&props(&[("mech_mass_kg", "500"), ("mech_fl_ratio", "7")]))
            .unwrap()
            .unwrap();
        assert!(h.unit.fine_lift().is_none() && h.binding.fine_motor.is_none());
    }

    #[test]
    fn bad_family_values_are_named() {
        let bad = |extra: &[(&str, &str)]| {
            let mut p = props(&[("mech_mass_kg", "500")]);
            p.extend(props(extra));
            build(&p).unwrap_err().to_string()
        };
        assert!(bad(&[("mech_brake_kind", "magnetic")]).contains("mech_brake_kind"));
        assert!(bad(&[("mech_rectifier", "triac")]).contains("not a known choice"));
        assert!(
            bad(&[
                ("mech_brake_pull_in", "0.3"),
                ("mech_brake_drop_out", "0.5")
            ])
            .contains("drop-out")
        );
        assert!(
            bad(&[("mech_fine_lift_motor", "=M2"), ("mech_fl_poles", "3")])
                .contains("mech_fl_poles")
                || bad(&[("mech_fine_lift_motor", "=M2"), ("mech_fl_poles", "3")]).contains("pole")
        );
        assert!(
            bad(&[
                ("mech_fine_lift_motor", "=M2"),
                ("mech_fl_releases_main_brake", "maybe")
            ])
            .contains("mech_fl_releases_main_brake")
        );
        assert!(
            bad(&[
                ("mech_brake_kind", "conical"),
                ("mech_conical_phase_pull", "0")
            ])
            .contains("phase_lost_pull")
        );
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
