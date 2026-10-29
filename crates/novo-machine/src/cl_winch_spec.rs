//! A closed-loop (encoder) winch described by plain device properties (lane L2c, "Closed-loop
//! winch").
//!
//! The motor's property sheet holds the numbers as `cw_*` properties; this module reads them (a
//! lookup closure keeps `novo-machine` free of the project model), checks them and builds the
//! drive, the brake, the hoist and the bindings to the drawing. A motor with `cw_mass_kg` set is
//! a closed-loop winch. Nothing here is a manufacturer's data: every default is a generic value
//! for a 30 kW hoist motor behind a generic encoder drive, labelled as such in the UI, and a
//! drive's real parameter set never appears in a drawing.

use novo_drive::brake::{SpringBrake, Switching};
use novo_drive::closed_loop::{ClosedLoopConfig, ClosedLoopWinch, PreTorque, check_winch};
use novo_drive::drive::MotorNameplate;
use novo_drive::mechanics::Hoist;

use crate::bindings::SensorBinding;
use crate::cl_pair_loop::ClPairModule;
use crate::cl_winch_loop::{ClWinchBinding, ClWinchModule};
use crate::hoist_loop::{LimitKind, LimitSwitch};
use crate::spec::{SpecError, SpecKey};
use crate::sync_winch::{Beam, PartnerAction, Rope, Supervision, SyncMode, WinchPair};

/// Property key that marks a motor as a closed-loop winch.
pub const CLOSED_WINCH_KEY: &str = "cw_mass_kg";

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

/// The rows, in the order the property sheet shows them. `cw_mass_kg` comes first and has no
/// default: setting it turns the motor into a closed-loop winch.
pub const KEYS: &[SpecKey] = &[
    num(CLOSED_WINCH_KEY, "Hook load (payload and block)", "kg", ""),
    num("cw_drum_radius_m", "Drum radius", "m", "0.2"),
    num("cw_gear_ratio", "Gearbox ratio (motor to drum)", "", "31.5"),
    num("cw_falls", "Rope falls", "", "1"),
    num("cw_efficiency", "Efficiency per direction", "", "0.9"),
    num("cw_motor_kw", "Motor power", "kW", "30"),
    num("cw_motor_v", "Motor voltage", "V", "400"),
    num("cw_motor_a", "Motor current", "A", "56"),
    num("cw_motor_hz", "Motor frequency", "Hz", "50"),
    num("cw_motor_rpm", "Motor rated speed", "rpm", "1480"),
    num("cw_motor_pf", "Motor power factor", "", "0.85"),
    num("cw_rotor_inertia", "Rotor inertia", "kg m\u{b2}", "0.5"),
    num(
        "cw_current_limit_a",
        "Drive current limit (blank: 1.5 x motor)",
        "A",
        "",
    ),
    num("cw_accel_s", "Acceleration time (to rated speed)", "s", "3"),
    num(
        "cw_decel_s",
        "Deceleration time (from rated speed)",
        "s",
        "3",
    ),
    num("cw_speed1_m_s", "Hook speed, step 1", "m/s", "0.15"),
    num("cw_speed2_m_s", "Hook speed, step 2", "m/s", "0.8"),
    num(
        "cw_resistor_ohm",
        "Braking resistor (0: none fitted)",
        "\u{3a9}",
        "40",
    ),
    tag(
        "cw_pre_torque",
        "Torque before the brake opens (none, fixed, load cell, memory)",
    ),
    num(
        "cw_pre_torque_nm",
        "Fixed torque before the brake opens",
        "N m",
        "",
    ),
    num(
        "cw_encoder_ppr",
        "Encoder pulses per revolution",
        "",
        "1024",
    ),
    num(
        "cw_release_wait_s",
        "Wait after the brake relay (no brake feedback)",
        "s",
        "0.3",
    ),
    num(
        "cw_close_hold_s",
        "Zero speed held after the relay opens",
        "s",
        "0.5",
    ),
    num("cw_brake_torque_nm", "Brake torque", "N m", "400"),
    num(
        "cw_brake_release_delay_s",
        "Brake opening delay",
        "s",
        "0.15",
    ),
    num("cw_brake_apply_s", "Brake apply delay", "s", "0.2"),
    num(
        "cw_brake_rise_s",
        "Brake torque rise after applying",
        "s",
        "0.02",
    ),
    tag("cw_brake_switching", "Brake switched on (AC or DC)"),
    tag("cw_in_up", "Up command device"),
    tag("cw_in_down", "Down command device"),
    tag("cw_in_fast", "Speed step 2 input device"),
    tag(
        "cw_sto_a",
        "STO channel 1 device (energised: torque allowed)",
    ),
    tag(
        "cw_sto_b",
        "STO channel 2 device (energised: torque allowed)",
    ),
    tag(
        "cw_encoder",
        "Encoder signal device (energised: signal present)",
    ),
    tag(
        "cw_encoder_b",
        "Encoder channel B device (energised: present)",
    ),
    tag("cw_mains", "Main contactor device"),
    tag("cw_brake_relay", "Drive brake relay contact device"),
    tag("cw_brake_feed", "Brake contactor / rectifier device"),
    tag("cw_brake_feedback", "Brake feedback device (blank: none)"),
    tag("cw_reset", "Fault reset input device"),
    tag("cw_limit_up", "Upper limit switch device"),
    num(
        "cw_limit_up_m",
        "Upper limit height (m above the hook start)",
        "m",
        "",
    ),
    tag("cw_limit_down", "Lower limit switch device"),
    num(
        "cw_limit_down_m",
        "Lower limit height (m above the hook start)",
        "m",
        "",
    ),
];

/// Property key that makes a closed-loop winch the first of a pair: the tag of the second
/// winch's motor.
pub const PAIR_PARTNER_KEY: &str = "cw_pair_partner";

/// The rows of a pair, shown on winch A's motor after the winch rows.
pub const PAIR_KEYS: &[SpecKey] = &[
    tag(
        PAIR_PARTNER_KEY,
        "Second winch on the same beam (motor tag)",
    ),
    num(
        "cw_beam_mass_kg",
        "Beam and load mass (blank: both hook loads)",
        "kg",
        "",
    ),
    num("cw_beam_length_m", "Distance between the ropes", "m", "4"),
    num(
        "cw_beam_cog_m",
        "Centre of gravity, from the middle towards winch A",
        "m",
        "0",
    ),
    tag("cw_beam_guided", "Beam guided, tilt locked (yes, no)"),
    num(
        "cw_rope_k_n_m",
        "Rope stiffness at the hook",
        "N/m",
        "2000000",
    ),
    num("cw_rope_damping", "Rope damping ratio (0 to 2)", "", "0.3"),
    num(
        "cw_rope_swl_n",
        "Rope rating (blank: 0.6 x beam weight)",
        "N",
        "",
    ),
    tag(
        "cw_sync_mode",
        "Synchronisation (independent, position lock, torque follower)",
    ),
    num("cw_sync_gain", "Position lock gain", "1/s", "20"),
    num(
        "cw_sync_limit_m_s",
        "Position lock correction limit",
        "m/s",
        "0.05",
    ),
    num("cw_sync_share", "Torque follower share of A", "", "1"),
    num("cw_sync_margin", "Torque follower speed margin", "", "0.15"),
    num(
        "cw_pair_max_error_m",
        "Stop both above this position difference (0: off)",
        "m",
        "0.1",
    ),
    tag(
        "cw_partner_fault",
        "When one drive faults (stop both, keep running)",
    ),
];

/// The tag of the second winch's motor, if this motor is the first of a pair.
pub fn pair_partner(get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    get(PAIR_PARTNER_KEY)
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// Two closed-loop winches on one beam. `get_a` reads the properties of the first motor (the
/// pair, the beam, the ropes and the synchronisation are entered there), `get_b` those of the
/// second. Returns `Ok(None)` when the first motor names no partner. Each winch is built from its
/// own rows with the hook load replaced by its share of the beam (the beam is integrated by the
/// pair); the notes are the design findings of both and the assumptions made.
pub fn closed_pair_from_properties(
    id_a: &str,
    get_a: &dyn Fn(&str) -> Option<String>,
    id_b: &str,
    get_b: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<(ClPairModule, Vec<String>)>, SpecError> {
    if pair_partner(get_a).is_none() {
        return Ok(None);
    }
    let text = |key: &'static str| {
        get_a(key)
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    let number = |key: &'static str| -> Result<f64, SpecError> {
        let default = PAIR_KEYS
            .iter()
            .find(|k| k.key == key)
            .map_or("", |k| k.default);
        let raw = text(key).unwrap_or_else(|| default.to_owned());
        crate::spec::decimal(key, raw)
    };
    // The hook loads entered on the motors, for the default beam mass.
    let load = |get: &dyn Fn(&str) -> Option<String>| -> f64 {
        get(CLOSED_WINCH_KEY)
            .and_then(|t| crate::spec::decimal(CLOSED_WINCH_KEY, t).ok())
            .unwrap_or(0.0)
    };
    let beam_mass = match text("cw_beam_mass_kg") {
        Some(_) => number("cw_beam_mass_kg")?,
        None => load(get_a) + load(get_b),
    };
    let length = number("cw_beam_length_m")?;
    let cog = number("cw_beam_cog_m")?;
    let guided = match text("cw_beam_guided")
        .map(|t| t.to_ascii_lowercase())
        .as_deref()
    {
        None | Some("no") => false,
        Some("yes") => true,
        Some(other) => {
            return Err(SpecError::UnknownWord {
                key: "cw_beam_guided",
                text: other.to_owned(),
            });
        }
    };
    let beam = Beam::new(beam_mass, length, cog, guided)?;
    let rope = Rope::with_stiffness(number("cw_rope_k_n_m")?, number("cw_rope_damping")?)?;
    let swl_default = text("cw_rope_swl_n").is_none();
    let swl = if swl_default {
        0.6 * beam_mass * novo_drive::GRAVITY
    } else {
        number("cw_rope_swl_n")?
    };
    let mode = match text("cw_sync_mode")
        .map(|t| t.to_ascii_lowercase())
        .as_deref()
    {
        None | Some("position lock") => SyncMode::PositionLock {
            gain_per_s: number("cw_sync_gain")?,
            limit_m_s: number("cw_sync_limit_m_s")?,
        },
        Some("independent") => SyncMode::Independent,
        Some("torque follower") => SyncMode::TorqueFollower {
            share: number("cw_sync_share")?,
            margin: number("cw_sync_margin")?,
        },
        Some(other) => {
            return Err(SpecError::UnknownWord {
                key: "cw_sync_mode",
                text: other.to_owned(),
            });
        }
    };
    let mut supervision = Supervision::new(swl);
    supervision.max_position_error_m = number("cw_pair_max_error_m")?;
    supervision.on_partner_fault = match text("cw_partner_fault")
        .map(|t| t.to_ascii_lowercase())
        .as_deref()
    {
        None | Some("stop both") => PartnerAction::StopBoth,
        Some("keep running") => PartnerAction::KeepRunning,
        Some(other) => {
            return Err(SpecError::UnknownWord {
                key: "cw_partner_fault",
                text: other.to_owned(),
            });
        }
    };

    // Each winch carries its share of the beam: A the side the centre of gravity is nearer to.
    let (fa, fb) = beam.static_forces_n();
    let weight = beam_mass * novo_drive::GRAVITY;
    let shares = [fa / weight, fb / weight];
    let mut notes = Vec::new();
    let mut parts = Vec::new();
    for (i, (id, get)) in [(id_a, get_a), (id_b, get_b)].into_iter().enumerate() {
        let mass = beam_mass * shares[i];
        let with_mass = move |key: &str| -> Option<String> {
            if key == CLOSED_WINCH_KEY {
                Some(mass.to_string())
            } else {
                get(key)
            }
        };
        let (module, found) =
            closed_winch_from_properties(id, &with_mass)?.ok_or(SpecError::NotANumber {
                key: CLOSED_WINCH_KEY,
                text: String::new(),
            })?;
        let who = if i == 0 { "A" } else { "B" };
        notes.extend(found.into_iter().map(|n| format!("Winch {who}: {n}")));
        parts.push(module);
    }
    notes.push(format!(
        "Each winch is checked and loaded with its share of the beam: {:.0} % for A, {:.0} % for B.",
        shares[0] * 100.0,
        shares[1] * 100.0
    ));
    if swl_default {
        notes.push(
            "No rope rating entered: 0.6 x the beam weight is used for the overload flag."
                .to_owned(),
        );
    }
    let mut it = parts.into_iter();
    let (ma, mb) = (it.next().unwrap(), it.next().unwrap());
    let speeds = ma.speeds_m_s;
    let rebuilt =
        |m: &ClWinchModule| -> Result<novo_drive::closed_loop::ClosedLoopWinch, SpecError> {
            // The beam is integrated by the pair, so the shaft carries the rotor only.
            let rotor = m.winch.rotor_inertia_kg_m2();
            Ok(
                novo_drive::closed_loop::ClosedLoopWinch::with_shaft_inertia(
                    m.winch.drive.cfg.clone(),
                    *m.winch.hoist(),
                    *m.winch.brake(),
                    rotor,
                    rotor.max(1e-3),
                )?,
            )
        };
    let pair = WinchPair::new(
        rebuilt(&ma)?,
        rebuilt(&mb)?,
        [rope; 2],
        beam,
        mode,
        supervision,
    );
    let module = ClPairModule::new(
        [id_a.to_owned(), id_b.to_owned()],
        pair,
        speeds,
        [ma.binding, mb.binding],
    )?;
    Ok(Some((module, notes)))
}

/// A closed-loop winch from properties. `get(key)` returns the property text if set. Returns
/// `Ok(None)` when the motor is no closed-loop winch (`cw_mass_kg` not set). The strings are the
/// design findings of the numbers and the assumptions made (what to show next to the run).
pub fn closed_winch_from_properties(
    id: &str,
    get: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<(ClWinchModule, Vec<String>)>, SpecError> {
    let text = |key: &'static str| {
        get(key)
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    if text(CLOSED_WINCH_KEY).is_none() {
        return Ok(None);
    }
    let number = |key: &'static str| -> Result<f64, SpecError> {
        let default = KEYS.iter().find(|k| k.key == key).map_or("", |k| k.default);
        let raw = text(key).unwrap_or_else(|| default.to_owned());
        crate::spec::decimal(key, raw)
    };
    let np = MotorNameplate {
        voltage_v: number("cw_motor_v")?,
        current_a: number("cw_motor_a")?,
        frequency_hz: number("cw_motor_hz")?,
        rpm: number("cw_motor_rpm")?,
        power_kw: number("cw_motor_kw")?,
        power_factor: number("cw_motor_pf")?,
    };
    let mut cfg = ClosedLoopConfig::generic_hoist(np);
    if text("cw_current_limit_a").is_some() {
        cfg.current_limit_a = number("cw_current_limit_a")?;
    }
    let accel_s = number("cw_accel_s")?;
    let decel_s = number("cw_decel_s")?;
    if !(accel_s > 0.0 && decel_s > 0.0) {
        return Err(SpecError::Value(novo_drive::InputError::NotPositive {
            field: "acceleration or deceleration time",
            value: accel_s.min(decel_s),
        }));
    }
    cfg.accel_rpm_s = np.rpm / accel_s;
    cfg.decel_rpm_s = np.rpm / decel_s;
    let ohm = number("cw_resistor_ohm")?;
    cfg.bus.resistor_ohm = (ohm > 0.0).then_some(ohm);
    cfg.pre_torque = match text("cw_pre_torque")
        .map(|t| t.to_ascii_lowercase())
        .as_deref()
    {
        None | Some("load cell") | Some("loadcell") => PreTorque::LoadCell,
        Some("none") => PreTorque::None,
        Some("memory") => PreTorque::Memory,
        Some("fixed") => {
            if text("cw_pre_torque_nm").is_none() {
                return Err(SpecError::NotANumber {
                    key: "cw_pre_torque_nm",
                    text: String::new(),
                });
            }
            PreTorque::Fixed(number("cw_pre_torque_nm")?)
        }
        Some(other) => {
            return Err(SpecError::UnknownWord {
                key: "cw_pre_torque",
                text: other.to_owned(),
            });
        }
    };
    let ppr = number("cw_encoder_ppr")?;
    if ppr.fract() != 0.0 || !(1.0..=1.0e6).contains(&ppr) {
        return Err(SpecError::NotWhole {
            key: "cw_encoder_ppr",
            value: ppr,
        });
    }
    cfg.encoder_ppr = ppr as u32;
    cfg.release_wait_s = number("cw_release_wait_s")?;
    cfg.brake_close_wait_s = number("cw_close_hold_s")?;
    cfg.brake_feedback = text("cw_brake_feedback").is_some();

    let switching = match text("cw_brake_switching").map(|t| t.to_ascii_lowercase()) {
        None => Switching::DcSide,
        Some(t) if t == "dc" => Switching::DcSide,
        Some(t) if t == "ac" => Switching::AcSide,
        Some(t) => {
            return Err(SpecError::UnknownWord {
                key: "cw_brake_switching",
                text: t,
            });
        }
    };
    let apply = number("cw_brake_apply_s")?;
    let brake = SpringBrake::new(
        number("cw_brake_torque_nm")?,
        number("cw_brake_release_delay_s")?,
        apply,
        apply,
        number("cw_brake_rise_s")?,
        switching,
    )?;
    let falls = number("cw_falls")?;
    if falls.fract() != 0.0 || !(1.0..=64.0).contains(&falls) {
        return Err(SpecError::NotWhole {
            key: "cw_falls",
            value: falls,
        });
    }
    let hoist = Hoist::new(
        number(CLOSED_WINCH_KEY)?,
        number("cw_drum_radius_m")?,
        number("cw_gear_ratio")?,
        falls as u32,
        number("cw_efficiency")?,
    )?;
    let speeds = [number("cw_speed1_m_s")?, number("cw_speed2_m_s")?];
    if !speeds.iter().all(|v| v.is_finite() && *v > 0.0) {
        return Err(SpecError::Value(novo_drive::InputError::NotPositive {
            field: "hook speed",
            value: speeds[0].min(speeds[1]),
        }));
    }

    let mut limits = Vec::new();
    for (which, kind, device, height) in [
        ("upper", LimitKind::Upper, "cw_limit_up", "cw_limit_up_m"),
        (
            "lower",
            LimitKind::Lower,
            "cw_limit_down",
            "cw_limit_down_m",
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
    let binding = ClWinchBinding {
        up: text("cw_in_up"),
        down: text("cw_in_down"),
        fast: text("cw_in_fast"),
        sto: [text("cw_sto_a"), text("cw_sto_b")],
        encoder: text("cw_encoder"),
        encoder_b: text("cw_encoder_b"),
        mains: text("cw_mains"),
        brake_relay: text("cw_brake_relay"),
        brake_feed: text("cw_brake_feed"),
        brake_feedback: text("cw_brake_feedback"),
        reset: text("cw_reset"),
        limits,
    };

    let findings = check_winch(&cfg, &hoist, &brake);
    let mut notes: Vec<String> = findings
        .iter()
        .map(|f| format!("{:?}: {}", f.severity, f.text))
        .collect();
    if cfg.bus.resistor_ohm.is_none() {
        notes.push(
            "No braking resistor: lowering a load returns energy to the DC bus and the drive \
             trips on over-voltage."
                .to_owned(),
        );
    }
    let rotor_inertia = number("cw_rotor_inertia")?;
    let winch = ClosedLoopWinch::new(cfg, hoist, brake, rotor_inertia)?;
    let module = ClWinchModule::new(id, winch, speeds, binding)?;
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
    ) -> Result<Option<(ClWinchModule, Vec<String>)>, SpecError> {
        closed_winch_from_properties("=M1", &|k| p.get(k).cloned())
    }

    #[test]
    fn no_mass_means_no_closed_loop_winch() {
        assert!(build(&props(&[])).unwrap().is_none());
    }

    #[test]
    fn defaults_make_a_generic_load_cell_winch() {
        let (m, _) = build(&props(&[(CLOSED_WINCH_KEY, "2500"), ("cw_in_up", "=K1")]))
            .unwrap()
            .unwrap();
        assert_eq!(m.binding.up.as_deref(), Some("=K1"));
        assert_eq!(m.winch.drive.cfg.pre_torque, PreTorque::LoadCell);
        assert_eq!(m.winch.drive.cfg.encoder_ppr, 1024);
        assert!(!m.winch.drive.cfg.brake_feedback);
        assert!((m.winch.drive.cfg.current_limit_a - 84.0).abs() < 1e-9);
    }

    #[test]
    fn a_brake_feedback_device_wires_the_feedback_into_the_drive() {
        let (m, _) = build(&props(&[
            (CLOSED_WINCH_KEY, "2500"),
            ("cw_brake_feedback", "=K9"),
        ]))
        .unwrap()
        .unwrap();
        assert!(m.winch.drive.cfg.brake_feedback);
    }

    #[test]
    fn a_fixed_pre_torque_needs_its_number() {
        let bad = build(&props(&[
            (CLOSED_WINCH_KEY, "2500"),
            ("cw_pre_torque", "fixed"),
        ]))
        .err()
        .map(|e| e.to_string())
        .unwrap();
        assert!(bad.contains("cw_pre_torque_nm"));
        let (m, _) = build(&props(&[
            (CLOSED_WINCH_KEY, "2500"),
            ("cw_pre_torque", "Fixed"),
            ("cw_pre_torque_nm", "150"),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(m.winch.drive.cfg.pre_torque, PreTorque::Fixed(150.0));
    }

    #[test]
    fn notes_carry_the_design_findings() {
        let (_, notes) = build(&props(&[
            (CLOSED_WINCH_KEY, "2500"),
            ("cw_pre_torque", "none"),
            ("cw_brake_torque_nm", "100"),
        ]))
        .unwrap()
        .unwrap();
        assert!(
            notes.iter().any(|n| n.contains("cannot hold the load")),
            "{notes:?}"
        );
    }

    #[test]
    fn bad_values_are_named() {
        let bad = |extra: &[(&str, &str)]| {
            let mut p = vec![(CLOSED_WINCH_KEY, "2500")];
            p.extend_from_slice(extra);
            build(&props(&p)).err().map(|e| e.to_string()).unwrap()
        };
        assert!(bad(&[("cw_falls", "1.5")]).contains("cw_falls"));
        assert!(bad(&[("cw_brake_switching", "both")]).contains("cw_brake_switching"));
        assert!(bad(&[("cw_pre_torque", "magic")]).contains("cw_pre_torque"));
        assert!(bad(&[("cw_encoder_ppr", "0")]).contains("cw_encoder_ppr"));
        assert!(bad(&[("cw_limit_up", "=S1")]).contains("upper limit"));
        assert!(bad(&[(CLOSED_WINCH_KEY, "2,500")]).contains("cw_mass_kg"));
        assert!(bad(&[("cw_accel_s", "0")]).contains("greater than zero"));
    }

    #[test]
    fn keys_are_unique_and_prefixed() {
        let mut seen = std::collections::BTreeSet::new();
        for k in KEYS {
            assert!(seen.insert(k.key), "{}", k.key);
            assert!(k.key.starts_with("cw_"));
        }
        assert_eq!(KEYS[0].key, CLOSED_WINCH_KEY);
    }

    fn pair(
        a: &[(&str, &str)],
        b: &[(&str, &str)],
    ) -> Result<Option<(ClPairModule, Vec<String>)>, SpecError> {
        let (pa, pb) = (props(a), props(b));
        closed_pair_from_properties("=M1", &|k| pa.get(k).cloned(), "=M2", &|k| {
            pb.get(k).cloned()
        })
    }

    #[test]
    fn a_motor_without_a_partner_is_no_pair() {
        assert!(pair(&[(CLOSED_WINCH_KEY, "2500")], &[]).unwrap().is_none());
    }

    #[test]
    fn the_beam_mass_defaults_to_both_hook_loads_and_the_cog_moves_the_share() {
        let (m, notes) = pair(
            &[
                (CLOSED_WINCH_KEY, "2000"),
                (PAIR_PARTNER_KEY, "=M2"),
                ("cw_beam_cog_m", "0.5"),
            ],
            &[(CLOSED_WINCH_KEY, "3000")],
        )
        .unwrap()
        .unwrap();
        assert!((m.pair.beam.mass_kg - 5000.0).abs() < 1e-9);
        // 4 m between the ropes, cog 0.5 m towards A: A carries 62.5 %.
        assert!(
            notes
                .iter()
                .any(|n| n.contains("62 % for A") || n.contains("63 % for A")),
            "{notes:?}"
        );
        assert!(notes.iter().any(|n| n.contains("No rope rating")));
    }

    #[test]
    fn bad_pair_values_are_named() {
        let bad = |extra: &[(&str, &str)]| {
            let mut a = vec![(CLOSED_WINCH_KEY, "2500"), (PAIR_PARTNER_KEY, "=M2")];
            a.extend_from_slice(extra);
            pair(&a, &[]).err().map(|e| e.to_string()).unwrap()
        };
        assert!(bad(&[("cw_sync_mode", "magic")]).contains("cw_sync_mode"));
        assert!(bad(&[("cw_beam_guided", "maybe")]).contains("cw_beam_guided"));
        assert!(bad(&[("cw_partner_fault", "ignore")]).contains("cw_partner_fault"));
        assert!(bad(&[("cw_beam_cog_m", "9")]).contains("cog_offset_m"));
    }

    #[test]
    fn pair_keys_are_unique_and_do_not_clash_with_the_winch_rows() {
        let mut seen: std::collections::BTreeSet<&str> = KEYS.iter().map(|k| k.key).collect();
        for k in PAIR_KEYS {
            assert!(seen.insert(k.key), "{}", k.key);
            assert!(k.key.starts_with("cw_"));
        }
    }
}
