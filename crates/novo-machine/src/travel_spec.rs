//! A travel axis (trolley, bridge) described by plain device properties (lane L5a part 3,
//! "Travel drive").
//!
//! The motor's property sheet holds the numbers as `trav_*` properties; this module reads them
//! (a lookup closure keeps `novo-machine` free of the project model), checks them and builds
//! the drive, the fitted motor and the load. A motor with `trav_mass_kg` set is a travel axis.
//! Nothing here is a manufacturer's data: the defaults are a generic 1.5 kW 4 pole motor on a
//! generic drive, labelled as such in the UI.

use novo_drive::brake::{SpringBrake, Switching};
use novo_drive::drive::{Drive, DriveConfig, MotorNameplate, ReferenceSource, TerminalMacro};
use novo_drive::induction::{FitOptions, InductionMotor};

use crate::bindings::SensorBinding;
use crate::hoist_loop::{LimitKind, LimitSwitch};
use crate::spec::{SpecError, SpecKey};
use crate::travel_loop::{Grip, Gust, TravelBinding, TravelLoad, TravelModule, Wind};

/// Property key that marks a motor as a travel axis.
pub const TRAVEL_KEY: &str = "trav_mass_kg";

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

/// The rows, in the order the property sheet shows them. `trav_mass_kg` comes first and has no
/// default: setting it turns the motor into a travel axis.
pub const KEYS: &[SpecKey] = &[
    num(TRAVEL_KEY, "Moving mass (crab, bridge, load)", "kg", ""),
    num("trav_wheel_radius_m", "Wheel radius", "m", "0.15"),
    num(
        "trav_gear_ratio",
        "Gearbox ratio (motor to wheel)",
        "",
        "30",
    ),
    num("trav_efficiency", "Drivetrain efficiency", "", "0.9"),
    num(
        "trav_rolling",
        "Rolling resistance (force over weight)",
        "",
        "0.01",
    ),
    num(
        "trav_wind_n",
        "Constant wind force, forward positive",
        "N",
        "0",
    ),
    num(
        "trav_wind_ms",
        "Wind speed along the track, forward positive",
        "m/s",
        "0",
    ),
    num(
        "trav_wind_area_m2",
        "Wind area of crane and load",
        "m\u{b2}",
        "0",
    ),
    num("trav_wind_cd", "Wind drag coefficient", "", "1.2"),
    num(
        "trav_gust_ms",
        "Gust: extra wind speed at the top (0: no gust)",
        "m/s",
        "0",
    ),
    num("trav_gust_start_s", "Gust starts after", "s", "10"),
    num("trav_gust_rise_s", "Gust rises over", "s", "2"),
    num("trav_gust_hold_s", "Gust holds for", "s", "5"),
    num("trav_gust_fall_s", "Gust falls over", "s", "3"),
    num(
        "trav_driven_share",
        "Share of the weight on driven and braked wheels",
        "",
        "0.5",
    ),
    num(
        "trav_adhesion",
        "Wheel on rail friction coefficient",
        "",
        "0.15",
    ),
    num(
        "trav_brake_torque_nm",
        "Travel brake torque on the motor shaft (blank: no brake)",
        "N m",
        "",
    ),
    num("trav_brake_release_s", "Brake release delay", "s", "0.15"),
    num(
        "trav_brake_apply_ac_s",
        "Brake apply delay, AC side switched",
        "s",
        "0.25",
    ),
    num(
        "trav_brake_apply_dc_s",
        "Brake apply delay, DC side switched",
        "s",
        "0.03",
    ),
    num(
        "trav_brake_rise_s",
        "Brake torque build-up after the apply delay",
        "s",
        "0.05",
    ),
    tag("trav_brake_switching", "Brake switched on (AC or DC)"),
    num(
        "trav_brake_release_hz",
        "Drive brake output on from",
        "Hz",
        "2",
    ),
    tag("trav_brake_relay", "Drive brake relay (R2) contact device"),
    tag(
        "trav_brake_feed",
        "Brake contactor / rectifier device (blank: follows R2)",
    ),
    num("trav_motor_kw", "Motor power", "kW", "1.5"),
    num("trav_motor_v", "Motor voltage", "V", "400"),
    num("trav_motor_a", "Motor current", "A", "3.6"),
    num("trav_motor_hz", "Motor frequency", "Hz", "50"),
    num("trav_motor_rpm", "Motor rated speed", "rpm", "1430"),
    num("trav_motor_pf", "Motor power factor", "", "0.8"),
    num("trav_rotor_inertia", "Rotor inertia", "kg m\u{b2}", "0.005"),
    num(
        "trav_drive_a",
        "Drive rated current (blank: 1.2 x motor)",
        "A",
        "",
    ),
    num("trav_step1_hz", "Step 1 frequency", "Hz", "10"),
    num("trav_step2_hz", "Step 2 frequency", "Hz", "50"),
    num("trav_accel_s", "Acceleration time", "s", "4"),
    num("trav_decel_s", "Deceleration time", "s", "4"),
    tag("trav_in_fwd", "Forward input device"),
    tag("trav_in_rev", "Reverse input device"),
    tag("trav_in_high", "Step 2 (high speed) input device"),
    tag("trav_in_enable", "Enable input device (blank: not wired)"),
    tag("trav_mains", "Main contactor device"),
    tag("trav_limit_fwd", "Forward end limit device"),
    num(
        "trav_limit_fwd_m",
        "Forward end limit (m from the start)",
        "m",
        "",
    ),
    tag("trav_limit_rev", "Reverse end limit device"),
    num(
        "trav_limit_rev_m",
        "Reverse end limit (m from the start)",
        "m",
        "",
    ),
];

/// A travel axis from properties. `get(key)` returns the property text if set. Returns
/// `Ok(None)` when the motor is no travel axis (`trav_mass_kg` not set). The strings are the
/// notes of the nameplate fit (what was assumed), to show next to the run.
pub fn travel_from_properties(
    id: &str,
    get: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<(TravelModule, Vec<String>)>, SpecError> {
    let text = |key: &'static str| {
        get(key)
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    if text(TRAVEL_KEY).is_none() {
        return Ok(None);
    }
    let number = |key: &'static str| -> Result<f64, SpecError> {
        let default = KEYS.iter().find(|k| k.key == key).map_or("", |k| k.default);
        let raw = text(key).unwrap_or_else(|| default.to_owned());
        crate::spec::decimal(key, raw)
    };
    let np = MotorNameplate {
        voltage_v: number("trav_motor_v")?,
        current_a: number("trav_motor_a")?,
        frequency_hz: number("trav_motor_hz")?,
        rpm: number("trav_motor_rpm")?,
        power_kw: number("trav_motor_kw")?,
        power_factor: number("trav_motor_pf")?,
    };
    let (motor, report) = InductionMotor::from_nameplate(&np, None, &FitOptions::typical())?;
    let drive_a = match text("trav_drive_a") {
        Some(_) => number("trav_drive_a")?,
        None => 1.2 * np.current_a,
    };
    let step1 = number("trav_step1_hz")?;
    let step2 = number("trav_step2_hz")?;
    let mut cfg = DriveConfig::generic_travel(np, drive_a);
    cfg.reference = ReferenceSource::Panel(step1);
    cfg.presets_hz[2] = step2;
    cfg.max_hz = cfg.max_hz.max(step2);
    cfg.brake_release_hz = number("trav_brake_release_hz")?;
    cfg.accel_time_s = number("trav_accel_s")?;
    cfg.decel_time_s = number("trav_decel_s")?;
    cfg.macro_ = TerminalMacro::two_wire_two_speed();
    let enable = text("trav_in_enable");
    if enable.is_none() {
        // No enable wired: the macro must not wait for one.
        cfg.macro_.inputs.truncate(3);
    }
    let mut inputs = vec![
        text("trav_in_fwd"),
        text("trav_in_rev"),
        text("trav_in_high"),
    ];
    if enable.is_some() {
        inputs.push(enable);
    }
    let gust_peak = number("trav_gust_ms")?;
    let gust = (gust_peak != 0.0).then(|| Gust {
        peak_m_s: gust_peak,
        start_s: number("trav_gust_start_s").unwrap_or(0.0),
        rise_s: number("trav_gust_rise_s").unwrap_or(0.0),
        hold_s: number("trav_gust_hold_s").unwrap_or(0.0),
        fall_s: number("trav_gust_fall_s").unwrap_or(0.0),
    });
    if gust.is_some() {
        // Surface a bad gust number as the parse error it is, not as a zero.
        for key in [
            "trav_gust_start_s",
            "trav_gust_rise_s",
            "trav_gust_hold_s",
            "trav_gust_fall_s",
        ] {
            number(key)?;
        }
    }
    let wind = Wind {
        speed_m_s: number("trav_wind_ms")?,
        gust,
        area_m2: number("trav_wind_area_m2")?,
        drag_coeff: number("trav_wind_cd")?,
    };
    let load = TravelLoad::new(
        number(TRAVEL_KEY)?,
        number("trav_wheel_radius_m")?,
        number("trav_gear_ratio")?,
        number("trav_efficiency")?,
        number("trav_rolling")?,
        number("trav_wind_n")?,
        number("trav_rotor_inertia")?,
    )?
    .with_wind(wind)?;
    let grip = Grip::new(number("trav_driven_share")?, number("trav_adhesion")?)?;
    let mut limits = Vec::new();
    for (which, kind, device, distance) in [
        (
            "forward",
            LimitKind::Upper,
            "trav_limit_fwd",
            "trav_limit_fwd_m",
        ),
        (
            "reverse",
            LimitKind::Lower,
            "trav_limit_rev",
            "trav_limit_rev_m",
        ),
    ] {
        match (text(device), text(distance)) {
            (None, None) => {}
            (Some(device_tag), Some(_)) => limits.push(LimitSwitch {
                binding: SensorBinding {
                    name: which.to_owned(),
                    device_tag,
                    actuate_at_mm: number(distance)? * 1000.0,
                },
                kind,
            }),
            _ => return Err(SpecError::LimitIncomplete(which)),
        }
    }
    let binding = TravelBinding {
        inputs,
        mains: text("trav_mains"),
        limits,
        brake_relay: text("trav_brake_relay"),
        brake_feed: text("trav_brake_feed"),
    };
    let mut notes: Vec<String> = report.notes.iter().map(|n| (*n).to_owned()).collect();
    if let Some(ratio) = report.assumed_breakdown_ratio {
        notes.push(format!(
            "The motor's breakdown torque was not on the plate: {ratio:.1} x rated is assumed."
        ));
    }
    let mut module = TravelModule::new(id, Drive::new(cfg), motor, load, binding);
    module.grip = grip;
    if text("trav_brake_torque_nm").is_some() {
        let switching = match text("trav_brake_switching").map(|t| t.to_ascii_lowercase()) {
            None => Switching::AcSide,
            Some(t) if t == "ac" => Switching::AcSide,
            Some(t) if t == "dc" => Switching::DcSide,
            Some(t) => {
                return Err(SpecError::NotANumber {
                    key: "trav_brake_switching",
                    text: t,
                });
            }
        };
        module = module.with_brake(
            SpringBrake::new(
                number("trav_brake_torque_nm")?,
                number("trav_brake_release_s")?,
                number("trav_brake_apply_ac_s")?,
                number("trav_brake_apply_dc_s")?,
                number("trav_brake_rise_s")?,
                switching,
            )?,
            grip,
        );
    } else if text("trav_brake_relay").is_some() || text("trav_brake_feed").is_some() {
        notes.push(
            "A brake relay or contactor is bound but no brake torque is entered: the axis has no brake."
                .to_owned(),
        );
    }
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
    ) -> Result<Option<(TravelModule, Vec<String>)>, SpecError> {
        travel_from_properties("=M1", &|k| p.get(k).cloned())
    }

    #[test]
    fn no_mass_means_no_travel_axis() {
        assert!(build(&props(&[])).unwrap().is_none());
    }

    #[test]
    fn defaults_make_a_generic_axis() {
        let p = props(&[(TRAVEL_KEY, "5000"), ("trav_in_fwd", "=K1")]);
        let (m, _) = build(&p).unwrap().unwrap();
        assert!((m.load.metres_per_rad() - 0.005).abs() < 1e-12);
        // No enable wired: three inputs, and one slot per input.
        assert_eq!(m.drive.cfg.macro_.inputs.len(), 3);
        assert_eq!(m.binding.inputs.len(), 3);
        assert_eq!(m.binding.inputs[0].as_deref(), Some("=K1"));
        assert_eq!(m.binding.inputs[1], None);
    }

    #[test]
    fn enable_input_is_kept_when_wired() {
        let p = props(&[(TRAVEL_KEY, "5000"), ("trav_in_enable", "=K9")]);
        let (m, _) = build(&p).unwrap().unwrap();
        assert_eq!(m.drive.cfg.macro_.inputs.len(), 4);
        assert_eq!(m.binding.inputs[3].as_deref(), Some("=K9"));
    }

    #[test]
    fn decimal_comma_and_bad_numbers() {
        let p = props(&[(TRAVEL_KEY, "5000"), ("trav_rolling", "0,02")]);
        assert!(build(&p).unwrap().is_some());
        let p = props(&[(TRAVEL_KEY, "5000"), ("trav_gear_ratio", "abc")]);
        assert!(matches!(build(&p), Err(SpecError::NotANumber { .. })));
        let p = props(&[(TRAVEL_KEY, "-5")]);
        assert!(matches!(build(&p), Err(SpecError::Value(_))));
    }

    #[test]
    fn a_comma_before_three_digits_is_refused_not_read_as_a_decimal() {
        for text in ["5,000", "1,500", "12,345.6"] {
            let p = props(&[(TRAVEL_KEY, text)]);
            assert!(
                matches!(build(&p), Err(SpecError::NotANumber { .. })),
                "{text}"
            );
        }
        let p = props(&[(TRAVEL_KEY, "5000,5")]);
        assert!(build(&p).unwrap().is_some());
    }

    #[test]
    fn zero_before_the_comma_is_a_decimal() {
        // An integer part of 0 cannot be a thousands group.
        for text in ["0,500", "0,5"] {
            let p = props(&[(TRAVEL_KEY, "5000"), ("trav_rolling", text)]);
            assert!(build(&p).is_ok(), "{text}");
        }
        assert_eq!(crate::spec::decimal("k", "-0,125".into()).unwrap(), -0.125);
        let p = props(&[(TRAVEL_KEY, "5000"), ("trav_rolling", "10,500")]);
        assert!(matches!(build(&p), Err(SpecError::NotANumber { .. })));
    }

    #[test]
    fn extreme_numbers_are_refused_before_the_run() {
        for (key, value) in [
            ("trav_gear_ratio", "1e-300"),
            ("trav_wheel_radius_m", "1e300"),
            ("trav_wind_n", "1e300"),
            ("trav_rolling", "1e300"),
            ("trav_efficiency", "1e-300"),
        ] {
            let p = props(&[(TRAVEL_KEY, "5000"), (key, value)]);
            assert!(matches!(build(&p), Err(SpecError::Value(_))), "{key}");
        }
    }

    #[test]
    fn a_limit_needs_device_and_distance() {
        let p = props(&[(TRAVEL_KEY, "5000"), ("trav_limit_fwd", "=S1")]);
        assert!(matches!(
            build(&p),
            Err(SpecError::LimitIncomplete("forward"))
        ));
        let p = props(&[
            (TRAVEL_KEY, "5000"),
            ("trav_limit_fwd", "=S1"),
            ("trav_limit_fwd_m", "12,5"),
        ]);
        let (m, _) = build(&p).unwrap().unwrap();
        assert_eq!(m.binding.limits[0].binding.actuate_at_mm, 12_500.0);
    }

    #[test]
    fn a_brake_torque_fits_a_brake_with_its_bindings() {
        let p = props(&[
            (TRAVEL_KEY, "5000"),
            ("trav_brake_torque_nm", "15"),
            ("trav_brake_switching", "DC"),
            ("trav_brake_relay", "=K-R2"),
            ("trav_brake_feed", "=K-BR"),
            ("trav_adhesion", "0,1"),
            ("trav_driven_share", "0,25"),
        ]);
        let (m, _) = build(&p).unwrap().unwrap();
        let brake = m.brake.expect("a brake is fitted");
        assert_eq!(brake.apply_delay_s(), 0.03);
        assert_eq!(m.binding.brake_relay.as_deref(), Some("=K-R2"));
        assert_eq!(m.binding.brake_feed.as_deref(), Some("=K-BR"));
        assert_eq!(m.grip.coeff, 0.1);
        assert_eq!(m.grip.share, 0.25);
    }

    #[test]
    fn no_brake_torque_means_no_brake_and_a_bound_contactor_is_said_to_be_unused() {
        let p = props(&[(TRAVEL_KEY, "5000"), ("trav_brake_relay", "=K-R2")]);
        let (m, notes) = build(&p).unwrap().unwrap();
        assert!(m.brake.is_none());
        assert!(notes.iter().any(|n| n.contains("no brake torque")));
    }

    #[test]
    fn wind_and_gust_come_from_properties_and_bad_ones_are_refused() {
        let p = props(&[
            (TRAVEL_KEY, "5000"),
            ("trav_wind_ms", "8"),
            ("trav_wind_area_m2", "40"),
            ("trav_gust_ms", "7"),
            ("trav_gust_start_s", "20"),
        ]);
        let (m, _) = build(&p).unwrap().unwrap();
        let w = m.load.wind();
        assert_eq!(w.speed_m_s, 8.0);
        assert_eq!(w.drag_coeff, 1.2);
        let g = w.gust.expect("a gust");
        assert_eq!((g.peak_m_s, g.start_s, g.rise_s), (7.0, 20.0, 2.0));
        let bad = |k: &str, v: &str| build(&props(&[(TRAVEL_KEY, "5000"), (k, v)])).is_err();
        assert!(bad("trav_wind_ms", "400"));
        assert!(bad("trav_wind_area_m2", "-1"));
        let p = props(&[
            (TRAVEL_KEY, "5000"),
            ("trav_brake_torque_nm", "15"),
            ("trav_brake_switching", "both"),
        ]);
        assert!(build(&p).is_err());
        assert!(bad("trav_driven_share", "0"));
        assert!(bad("trav_driven_share", "1.5"));
        assert!(bad("trav_adhesion", "0"));
        let p = props(&[
            (TRAVEL_KEY, "5000"),
            ("trav_gust_ms", "5"),
            ("trav_gust_rise_s", "x"),
        ]);
        assert!(build(&p).is_err());
        let p = props(&[
            (TRAVEL_KEY, "5000"),
            ("trav_gust_ms", "5"),
            ("trav_gust_rise_s", "0"),
        ]);
        assert!(build(&p).is_err());
    }

    #[test]
    fn keys_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for k in KEYS {
            assert!(seen.insert(k.key), "{}", k.key);
        }
    }
}
