//! An open-loop drive hoist described by plain device properties (lane L2c-open, "Drive hoist").
//!
//! The motor's property sheet holds the numbers as `dh_*` properties; this module reads them (a
//! lookup closure keeps `novo-machine` free of the project model), checks them and builds the
//! drive, the brake logic, the fitted motor, the brake and the load. A motor with `dh_mass_kg`
//! set is a drive hoist. Nothing here is a manufacturer's data: every default is a generic
//! value for a 5.5 kW 4 pole hoist motor behind a generic drive, labelled as such in the UI, and
//! a drive's real parameter set never appears in a drawing.

use novo_drive::brake::{SpringBrake, Switching};
use novo_drive::drive::terminals::DigitalInput;
use novo_drive::drive::{
    Drive, DriveConfig, MotorNameplate, ReferenceSource, TerminalFunction, TerminalMacro,
};
use novo_drive::hoist_logic::{BrakeLogic, HoistLogic, HoistLogicConfig, LoadSense, ReversalJump};
use novo_drive::induction::{FitOptions, InductionMotor};
use novo_drive::mechanics::Hoist;

use crate::bindings::SensorBinding;
use crate::drive_hoist_loop::{DriveHoistBinding, DriveHoistModule, DriveHoistParts};
use crate::hoist_loop::{LimitKind, LimitSwitch};
use crate::spec::{SpecError, SpecKey};

/// Property key that marks a motor as a drive hoist.
pub const DRIVE_HOIST_KEY: &str = "dh_mass_kg";

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

/// The rows, in the order the property sheet shows them. `dh_mass_kg` comes first and has no
/// default: setting it turns the motor into a drive hoist.
pub const KEYS: &[SpecKey] = &[
    num(DRIVE_HOIST_KEY, "Hook load (payload and block)", "kg", ""),
    num("dh_drum_radius_m", "Drum radius", "m", "0.15"),
    num("dh_gear_ratio", "Gearbox ratio (motor to drum)", "", "63"),
    num("dh_falls", "Rope falls", "", "2"),
    num("dh_efficiency", "Efficiency per direction", "", "0.85"),
    num("dh_motor_kw", "Motor power", "kW", "5.5"),
    num("dh_motor_v", "Motor voltage", "V", "400"),
    num("dh_motor_a", "Motor current", "A", "11.2"),
    num("dh_motor_hz", "Motor frequency", "Hz", "50"),
    num("dh_motor_rpm", "Motor rated speed", "rpm", "1440"),
    num("dh_motor_pf", "Motor power factor", "", "0.82"),
    num("dh_rotor_inertia", "Rotor inertia", "kg m\u{b2}", "0.03"),
    num(
        "dh_drive_a",
        "Drive rated current (blank: 1.25 x motor)",
        "A",
        "",
    ),
    num(
        "dh_accel_s",
        "Acceleration time (to motor frequency)",
        "s",
        "2",
    ),
    num(
        "dh_decel_s",
        "Deceleration time (from motor frequency)",
        "s",
        "2",
    ),
    num("dh_boost_v", "Voltage boost at 0 Hz", "V", "20"),
    num(
        "dh_resistor_ohm",
        "Braking resistor (0: none fitted)",
        "\u{3a9}",
        "60",
    ),
    num("dh_step1_hz", "Speed step 1 frequency", "Hz", "10"),
    num("dh_step2_hz", "Speed step 2 frequency", "Hz", "25"),
    num("dh_step3_hz", "Speed step 3 frequency", "Hz", "50"),
    num(
        "dh_step4_hz",
        "Speed step 4 frequency (blank: three steps)",
        "Hz",
        "",
    ),
    num("dh_release_hz", "Brake release frequency", "Hz", "3"),
    num(
        "dh_release_a",
        "Brake release current (blank: 0.9 x motor)",
        "A",
        "",
    ),
    num("dh_release_s", "Brake release time", "s", "0.3"),
    num("dh_engage_hz", "Brake engage frequency", "Hz", "2"),
    num("dh_engage_s", "Brake engage time", "s", "0.4"),
    num("dh_restart_s", "Time to restart", "s", "0.5"),
    num(
        "dh_jump_hz",
        "Reversal with the brake open: fast band (blank: full brake cycle)",
        "Hz",
        "",
    ),
    num("dh_jump_ramp_s", "Reversal fast-band ramp time", "s", "0.3"),
    num(
        "dh_sense_boost_hz",
        "Load sensing: highest top step (blank: off)",
        "Hz",
        "",
    ),
    num(
        "dh_sense_probe_hz",
        "Load sensing: probe frequency",
        "Hz",
        "20",
    ),
    num("dh_sense_probe_s", "Load sensing: probe time", "s", "0.5"),
    num(
        "dh_sense_light",
        "Load sensing: light load (share of rated torque)",
        "",
        "0.25",
    ),
    num(
        "dh_sense_margin",
        "Load sensing: torque margin kept at the boost",
        "",
        "1.5",
    ),
    num("dh_brake_torque_nm", "Brake torque", "N m", "80"),
    num(
        "dh_brake_release_delay_s",
        "Brake opening delay",
        "s",
        "0.08",
    ),
    num(
        "dh_brake_apply_ac_s",
        "Brake apply delay, AC side",
        "s",
        "0.15",
    ),
    num(
        "dh_brake_apply_dc_s",
        "Brake apply delay, DC side",
        "s",
        "0.03",
    ),
    tag("dh_brake_switching", "Brake switched on (AC or DC)"),
    tag("dh_slow_up", "Upper slow-down switch device (blank: none)"),
    num(
        "dh_slow_up_m",
        "Upper slow-down height (m above the hook start)",
        "m",
        "",
    ),
    tag(
        "dh_slow_down",
        "Lower slow-down switch device (blank: none)",
    ),
    num(
        "dh_slow_down_m",
        "Lower slow-down height (m above the hook start)",
        "m",
        "",
    ),
    num(
        "dh_clearance_up_m",
        "Clearance behind the upper final limit to the stop (blank: not checked)",
        "m",
        "",
    ),
    num(
        "dh_clearance_down_m",
        "Clearance behind the lower final limit to the stop (blank: not checked)",
        "m",
        "",
    ),
    tag("dh_in_up", "Up command device"),
    tag("dh_in_down", "Down command device"),
    tag("dh_in_step2", "Speed step 2 input device"),
    tag("dh_in_step3", "Speed step 3 input device"),
    tag("dh_in_step4", "Speed step 4 input device"),
    tag("dh_mains", "Main contactor device"),
    tag("dh_brake_relay", "Drive brake relay contact device"),
    tag("dh_brake_feed", "Brake contactor / rectifier device"),
    tag("dh_brake_feedback", "Brake feedback device (blank: none)"),
    tag("dh_reset", "Fault reset input device"),
    tag("dh_limit_up", "Upper limit switch device"),
    num(
        "dh_limit_up_m",
        "Upper limit height (m above the hook start)",
        "m",
        "",
    ),
    tag("dh_limit_down", "Lower limit switch device"),
    num(
        "dh_limit_down_m",
        "Lower limit height (m above the hook start)",
        "m",
        "",
    ),
];

/// A drive hoist from properties. `get(key)` returns the property text if set. Returns
/// `Ok(None)` when the motor is no drive hoist (`dh_mass_kg` not set). The strings are the notes
/// of the nameplate fit and of the assumptions made (what to show next to the run).
pub fn drive_hoist_from_properties(
    id: &str,
    get: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<(DriveHoistModule, Vec<String>)>, SpecError> {
    let text = |key: &'static str| {
        get(key)
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    if text(DRIVE_HOIST_KEY).is_none() {
        return Ok(None);
    }
    let number = |key: &'static str| -> Result<f64, SpecError> {
        let default = KEYS.iter().find(|k| k.key == key).map_or("", |k| k.default);
        let raw = text(key).unwrap_or_else(|| default.to_owned());
        crate::spec::decimal(key, raw)
    };
    let np = MotorNameplate {
        voltage_v: number("dh_motor_v")?,
        current_a: number("dh_motor_a")?,
        frequency_hz: number("dh_motor_hz")?,
        rpm: number("dh_motor_rpm")?,
        power_kw: number("dh_motor_kw")?,
        power_factor: number("dh_motor_pf")?,
    };
    let (motor, report) = InductionMotor::from_nameplate(&np, None, &FitOptions::typical())?;
    let drive_a = match text("dh_drive_a") {
        Some(_) => number("dh_drive_a")?,
        None => 1.25 * np.current_a,
    };

    let mut steps_hz = vec![
        number("dh_step1_hz")?,
        number("dh_step2_hz")?,
        number("dh_step3_hz")?,
    ];
    if text("dh_step4_hz").is_some() {
        steps_hz.push(number("dh_step4_hz")?);
    }
    let release_a = match text("dh_release_a") {
        Some(_) => number("dh_release_a")?,
        None => 0.9 * np.current_a,
    };
    let mut brake_logic = BrakeLogic::generic(np.current_a);
    brake_logic.release_hz = number("dh_release_hz")?;
    brake_logic.release_current_up_a = release_a;
    brake_logic.release_current_down_a = release_a;
    brake_logic.release_time_s = number("dh_release_s")?;
    brake_logic.engage_hz = number("dh_engage_hz")?;
    brake_logic.engage_time_s = number("dh_engage_s")?;
    brake_logic.restart_time_s = number("dh_restart_s")?;
    let sense = match text("dh_sense_boost_hz") {
        None => None,
        Some(_) => Some(LoadSense {
            probe_hz: number("dh_sense_probe_hz")?,
            probe_time_s: number("dh_sense_probe_s")?,
            light_fraction: number("dh_sense_light")?,
            boost_hz: number("dh_sense_boost_hz")?,
            margin: number("dh_sense_margin")?,
        }),
    };
    let top = steps_hz.iter().copied().fold(0.0_f64, f64::max);
    let max_hz = sense.as_ref().map_or(top, |s| top.max(s.boost_hz));
    let jump = match text("dh_jump_hz") {
        None => None,
        Some(_) => Some(ReversalJump {
            jump_hz: number("dh_jump_hz")?,
            ramp_time_s: number("dh_jump_ramp_s")?,
        }),
    };
    let logic = HoistLogic::new(HoistLogicConfig {
        jump,
        brake: brake_logic,
        steps_hz: steps_hz.clone(),
        sense,
        rated_torque_nm: motor.rated_torque_nm(),
    })?;

    let mut cfg = DriveConfig::generic_travel(np, drive_a);
    cfg.reference = ReferenceSource::Word;
    cfg.macro_ = TerminalMacro {
        name: "open-loop hoist".into(),
        inputs: vec![
            DigitalInput::new(TerminalFunction::Forward),
            DigitalInput::new(TerminalFunction::Reverse),
            DigitalInput::new(TerminalFunction::FaultReset),
        ],
    };
    cfg.accel_time_s = number("dh_accel_s")?;
    cfg.decel_time_s = number("dh_decel_s")?;
    cfg.max_hz = max_hz.max(np.frequency_hz);
    cfg.vf.boost_v = number("dh_boost_v")?;
    cfg.vf.boost_end_hz = np.frequency_hz * 0.3;
    let ohm = number("dh_resistor_ohm")?;
    cfg.bus.resistor_ohm = (ohm > 0.0).then_some(ohm);

    let switching = match text("dh_brake_switching").map(|t| t.to_ascii_lowercase()) {
        None => Switching::AcSide,
        Some(t) if t == "ac" => Switching::AcSide,
        Some(t) if t == "dc" => Switching::DcSide,
        Some(t) => {
            return Err(SpecError::NotANumber {
                key: "dh_brake_switching",
                text: t,
            });
        }
    };
    let brake = SpringBrake::new(
        number("dh_brake_torque_nm")?,
        number("dh_brake_release_delay_s")?,
        number("dh_brake_apply_ac_s")?,
        number("dh_brake_apply_dc_s")?,
        0.02,
        switching,
    )?;
    let falls = number("dh_falls")?;
    if falls.fract() != 0.0 || !(1.0..=64.0).contains(&falls) {
        return Err(SpecError::NotWhole {
            key: "dh_falls",
            value: falls,
        });
    }
    let hoist = Hoist::new(
        number(DRIVE_HOIST_KEY)?,
        number("dh_drum_radius_m")?,
        number("dh_gear_ratio")?,
        falls as u32,
        number("dh_efficiency")?,
    )?;

    let mut limits = Vec::new();
    for (which, kind, device, height) in [
        (
            "slow-down upper",
            LimitKind::Upper,
            "dh_slow_up",
            "dh_slow_up_m",
        ),
        (
            "slow-down lower",
            LimitKind::Lower,
            "dh_slow_down",
            "dh_slow_down_m",
        ),
        ("upper", LimitKind::Upper, "dh_limit_up", "dh_limit_up_m"),
        (
            "lower",
            LimitKind::Lower,
            "dh_limit_down",
            "dh_limit_down_m",
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
    let mut slow_notes = Vec::new();
    for (slow, fin, what) in [
        ("slow-down upper", "upper", "upper"),
        ("slow-down lower", "lower", "lower"),
    ] {
        let at = |name: &str| {
            limits
                .iter()
                .find(|l: &&LimitSwitch| l.binding.name == name)
                .map(|l| l.binding.actuate_at_mm)
        };
        if let (Some(s_mm), Some(f_mm)) = (at(slow), at(fin)) {
            let before = if what == "upper" {
                s_mm < f_mm
            } else {
                s_mm > f_mm
            };
            if !before {
                slow_notes.push(format!(
                    "The {what} slow-down switch ({:.2} m) is not before the {what} final limit \
                     ({:.2} m): the hook reaches the final limit first.",
                    s_mm / 1000.0,
                    f_mm / 1000.0
                ));
            }
        }
    }
    let speed_inputs: Vec<Option<String>> = ["dh_in_step2", "dh_in_step3", "dh_in_step4"]
        .iter()
        .take(steps_hz.len() - 1)
        .map(|k| text(k))
        .collect();
    let binding = DriveHoistBinding {
        up: text("dh_in_up"),
        down: text("dh_in_down"),
        speed_inputs,
        mains: text("dh_mains"),
        brake_relay: text("dh_brake_relay"),
        brake_feed: text("dh_brake_feed"),
        brake_feedback: text("dh_brake_feedback"),
        reset: text("dh_reset"),
        limits,
    };
    let mut notes: Vec<String> = report.notes.iter().map(|n| (*n).to_owned()).collect();
    notes.extend(slow_notes);
    if let Some(ratio) = report.assumed_breakdown_ratio {
        notes.push(format!(
            "The motor's breakdown torque was not on the plate: {ratio:.1} x rated is assumed."
        ));
    }
    if top > np.frequency_hz * 1.0 + 1e-9 {
        notes.push(format!(
            "The top step ({top:.0} Hz) is above the motor frequency: the motor runs in field \
             weakening and loses torque there."
        ));
    }
    if cfg.bus.resistor_ohm.is_none() {
        notes.push(
            "No braking resistor: lowering a load returns energy to the DC bus and the drive \
             is expected to trip on over-voltage."
                .to_owned(),
        );
    }
    let rotor_inertia = number("dh_rotor_inertia")?;
    let clearance = |key: &'static str| -> Result<Option<f64>, SpecError> {
        match text(key) {
            None => Ok(None),
            Some(_) => number(key).map(Some),
        }
    };
    let (clear_up, clear_down) = (
        clearance("dh_clearance_up_m")?,
        clearance("dh_clearance_down_m")?,
    );
    let module = DriveHoistModule::new(
        id,
        DriveHoistParts {
            drive: Drive::new(cfg),
            logic,
            motor,
            hoist,
            brake,
            rotor_inertia_kg_m2: rotor_inertia,
        },
        binding,
    )?
    .with_clearance(clear_up, clear_down);
    Ok(Some((module, notes)))
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

    fn build(
        p: &BTreeMap<String, String>,
    ) -> Result<Option<(DriveHoistModule, Vec<String>)>, SpecError> {
        drive_hoist_from_properties("=M1", &|k| p.get(k).cloned())
    }

    #[test]
    fn no_mass_means_no_drive_hoist() {
        assert!(build(&props(&[])).unwrap().is_none());
    }

    #[test]
    fn defaults_make_a_generic_three_step_hoist() {
        let (m, _) = build(&props(&[(DRIVE_HOIST_KEY, "2000"), ("dh_in_up", "=K1")]))
            .unwrap()
            .unwrap();
        assert_eq!(m.logic.config().steps_hz, vec![10.0, 25.0, 50.0]);
        assert_eq!(m.binding.speed_inputs.len(), 2);
        assert_eq!(m.binding.up.as_deref(), Some("=K1"));
        assert!((m.logic.config().brake.release_current_up_a - 0.9 * 11.2).abs() < 1e-9);
        assert!(m.logic.config().sense.is_none());
    }

    #[test]
    fn a_fourth_step_is_optional() {
        let (m, _) = build(&props(&[(DRIVE_HOIST_KEY, "2000"), ("dh_step4_hz", "80")]))
            .unwrap()
            .unwrap();
        assert_eq!(m.logic.config().steps_hz.len(), 4);
        assert_eq!(m.binding.speed_inputs.len(), 3);
    }

    #[test]
    fn load_sensing_switches_on_with_a_boost_frequency() {
        let (m, _) = build(&props(&[
            (DRIVE_HOIST_KEY, "2000"),
            ("dh_sense_boost_hz", "80"),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(m.logic.config().sense.as_ref().unwrap().boost_hz, 80.0);
        assert_eq!(m.drive.cfg.max_hz, 80.0);
    }

    #[test]
    fn the_jump_at_reversal_is_off_unless_a_band_is_entered() {
        let (m, _) = build(&props(&[(DRIVE_HOIST_KEY, "2000")]))
            .unwrap()
            .unwrap();
        assert!(m.logic.config().jump.is_none());
        let (m, _) = build(&props(&[(DRIVE_HOIST_KEY, "2000"), ("dh_jump_hz", "8")]))
            .unwrap()
            .unwrap();
        let j = m.logic.config().jump.as_ref().unwrap();
        assert_eq!((j.jump_hz, j.ramp_time_s), (8.0, 0.3));
    }

    #[test]
    fn a_slow_down_switch_is_a_limit_before_the_final_one_and_a_wrong_order_is_noted() {
        let base = [
            (DRIVE_HOIST_KEY, "2000"),
            ("dh_limit_up", "=LS-F"),
            ("dh_limit_up_m", "1.6"),
            ("dh_slow_up", "=LS-S"),
            ("dh_slow_up_m", "1.0"),
        ];
        let (m, notes) = build(&props(&base)).unwrap().unwrap();
        let names: Vec<&str> = m
            .binding
            .limits
            .iter()
            .map(|l| l.binding.name.as_str())
            .collect();
        assert_eq!(names, ["slow-down upper", "upper"]);
        assert!(notes.iter().all(|n| !n.contains("slow-down")), "{notes:?}");
        // A slow-down switch behind the final limit does nothing useful: say so.
        let mut wrong = base.to_vec();
        wrong.push(("dh_slow_up_m", "1.8"));
        let (_, notes) = build(&props(&wrong)).unwrap().unwrap();
        assert!(
            notes
                .iter()
                .any(|n| n.contains("not before the upper final limit")),
            "{notes:?}"
        );
        // A device without a height is refused like any other limit.
        assert!(
            build(&props(&[
                (DRIVE_HOIST_KEY, "2000"),
                ("dh_slow_up", "=LS-S")
            ]))
            .is_err()
        );
    }

    #[test]
    fn a_zero_resistor_means_none_and_says_so() {
        let (m, notes) = build(&props(&[
            (DRIVE_HOIST_KEY, "2000"),
            ("dh_resistor_ohm", "0"),
        ]))
        .unwrap()
        .unwrap();
        assert!(m.drive.cfg.bus.resistor_ohm.is_none());
        assert!(notes.iter().any(|n| n.contains("No braking resistor")));
    }

    #[test]
    fn bad_values_are_named() {
        let bad = |extra: &[(&str, &str)]| {
            let mut p = vec![(DRIVE_HOIST_KEY, "2000")];
            p.extend_from_slice(extra);
            build(&props(&p)).err().map(|e| e.to_string()).unwrap()
        };
        assert!(bad(&[("dh_falls", "1.5")]).contains("dh_falls"));
        assert!(bad(&[("dh_brake_switching", "both")]).contains("dh_brake_switching"));
        assert!(bad(&[("dh_limit_up", "=S1")]).contains("upper limit"));
        assert!(bad(&[("dh_limit_down_m", "0.2")]).contains("lower limit"));
        assert!(bad(&[(DRIVE_HOIST_KEY, "5,000")]).contains("dh_mass_kg"));
        assert!(bad(&[("dh_drum_radius_m", "-1")]).contains("greater than zero"));
        assert!(bad(&[("dh_release_s", "-1")]).contains("release_time_s"));
    }

    #[test]
    fn keys_are_unique_and_prefixed() {
        let mut seen = std::collections::BTreeSet::new();
        for k in KEYS {
            assert!(seen.insert(k.key), "{}", k.key);
            assert!(k.key.starts_with("dh_"));
        }
        assert_eq!(KEYS[0].key, DRIVE_HOIST_KEY);
    }
}
