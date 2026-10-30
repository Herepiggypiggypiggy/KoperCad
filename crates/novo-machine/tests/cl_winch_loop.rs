//! Loop checks for the closed-loop winch driven by a scripted circuit: the operator inputs, the
//! two STO channels, the encoder signal device and the brake contactor are devices of the
//! drawing, so a defect injected on any of them reaches the winch. Expected values are worked
//! out on paper in the comments.

use std::cell::RefCell;
use std::rc::Rc;

use novo_drive::closed_loop::ClFault;
use novo_machine::cl_winch_loop::{ClFindingKind, ClWinchLoop, ClWinchModule};
use novo_machine::cl_winch_spec::{CLOSED_WINCH_KEY, closed_winch_from_properties};
use novo_machine::cosim::StepConfig;
use novo_machine::port::{CircuitPort, FakeCircuit};

const DT: f64 = 0.005;

/// What the operator and the defects do; the scripted circuit reads it every tick.
#[derive(Clone, Copy)]
struct Cmd {
    up: bool,
    down: bool,
    fast: bool,
    sto_a: bool,
    sto_b: bool,
    encoder: bool,
    encoder_b: bool,
    load_cell: bool,
    /// The brake contactor pulls in when the drive's relay contact is closed.
    contactor_follows_relay: bool,
    /// A welded brake contactor: the coil is fed whatever the relay says.
    contactor_welded: bool,
}

impl Default for Cmd {
    fn default() -> Self {
        Self {
            up: false,
            down: false,
            fast: false,
            sto_a: true,
            sto_b: true,
            encoder: true,
            encoder_b: true,
            load_cell: true,
            contactor_follows_relay: true,
            contactor_welded: false,
        }
    }
}

type Shared = Rc<RefCell<Cmd>>;

/// 2500 kg on a 0.2 m drum, ratio 31.5: 0.2 / 31.5 = 6.349 mm of hook per rad of motor shaft,
/// so 0.15 m/s is 23.6 rad/s = 225.6 rpm. Weight torque 2500 x 9.80665 x 0.006349 = 155.7 N m
/// (80 % of the 193.6 N m rated torque of the 30 kW motor).
fn winch(extra: &[(&str, &str)]) -> ClWinchModule {
    let mut p = vec![
        (CLOSED_WINCH_KEY, "2500"),
        ("cw_in_up", "KU"),
        ("cw_in_down", "KD"),
        ("cw_in_fast", "KF"),
        ("cw_sto_a", "STOA"),
        ("cw_sto_b", "STOB"),
        ("cw_encoder", "ENC"),
        ("cw_encoder_b", "ENCB"),
        ("cw_brake_relay", "KR"),
        ("cw_brake_feed", "KB"),
    ];
    p.extend_from_slice(extra);
    let map: std::collections::BTreeMap<String, String> = p
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    closed_winch_from_properties("=M1", &|k| map.get(k).cloned())
        .unwrap()
        .unwrap()
        .0
}

fn rig(extra: &[(&str, &str)]) -> (ClWinchLoop, FakeCircuit, Shared) {
    let cmd: Shared = Rc::new(RefCell::new(Cmd::default()));
    let c = cmd.clone();
    let circuit = FakeCircuit::new().with_rule(move |s| {
        let cmd = *c.borrow();
        let relay_closed = s.contacts.get("KR").copied().unwrap_or(false);
        for (tag, on) in [
            ("KU", cmd.up),
            ("KD", cmd.down),
            ("KF", cmd.fast),
            ("STOA", cmd.sto_a),
            ("STOB", cmd.sto_b),
            ("ENC", cmd.encoder),
            ("ENCB", cmd.encoder_b),
            ("LC", cmd.load_cell),
            (
                "KB",
                cmd.contactor_welded || (cmd.contactor_follows_relay && relay_closed),
            ),
        ] {
            if on {
                s.energised.insert(tag.to_owned());
            } else {
                s.energised.remove(tag);
            }
        }
    });
    let mut circuit = circuit;
    circuit.known.extend(
        [
            "KU", "KD", "KF", "STOA", "STOB", "ENC", "ENCB", "LC", "OV", "KR", "KB",
        ]
        .map(str::to_owned),
    );
    let mut l = ClWinchLoop::new(StepConfig::new(DT).unwrap()).unwrap();
    l.modules.push(winch(extra));
    (l, circuit, cmd)
}

fn seconds(l: &mut ClWinchLoop, c: &mut FakeCircuit, s: f64) {
    l.run(c, (s / DT).round() as usize);
}

fn has(m: &ClWinchModule, f: impl Fn(&ClFindingKind) -> bool) -> bool {
    m.findings().iter().any(|x| f(&x.kind))
}

#[test]
fn up_command_proves_torque_opens_the_brake_and_lifts_at_the_step_speed() {
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 6.0);
    let m = l.module("=M1").unwrap();
    let s = m.snapshot().unwrap();
    // Step 1 is 0.15 m/s; the encoder drive holds it within the speed resolution.
    assert!(
        (s.hook_speed_m_s - 0.15).abs() < 0.01,
        "{}",
        s.hook_speed_m_s
    );
    assert!(s.hook_height_m > 0.4, "{}", s.hook_height_m);
    // The brake opened only after the drive built its torque: nothing sank.
    assert!(!has(m, |k| matches!(
        k,
        ClFindingKind::StartRollback { .. }
    )));
    assert!(
        m.findings()
            .iter()
            .all(|f| !matches!(f.kind, ClFindingKind::Trip(_)))
    );
}

#[test]
fn step_two_is_faster_and_down_reverses() {
    let (mut l, mut c, cmd) = rig(&[]);
    *cmd.borrow_mut() = Cmd {
        up: true,
        fast: true,
        ..Cmd::default()
    };
    seconds(&mut l, &mut c, 7.0);
    let v = l.module("=M1").unwrap().snapshot().unwrap().hook_speed_m_s;
    assert!((v - 0.8).abs() < 0.03, "{v}");
    *cmd.borrow_mut() = Cmd {
        down: true,
        ..Cmd::default()
    };
    seconds(&mut l, &mut c, 12.0);
    let m = l.module("=M1").unwrap();
    assert!(m.snapshot().unwrap().hook_speed_m_s < -0.1);
    assert!(
        m.findings()
            .iter()
            .all(|f| !matches!(f.kind, ClFindingKind::Trip(_))),
        "{:?}",
        m.findings()
    );
}

#[test]
fn stopping_holds_the_load_until_the_brake_has_it() {
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 5.0);
    cmd.borrow_mut().up = false;
    seconds(&mut l, &mut c, 4.0);
    let m = l.module("=M1").unwrap();
    let s = m.snapshot().unwrap();
    assert!(s.shaft_rpm.abs() < 0.01);
    assert!(!has(m, |k| matches!(k, ClFindingKind::BrakeSlip { .. })));
    // The hook carries on a little while the ramp comes down: a few centimetres, never metres.
    let travel = m
        .findings()
        .iter()
        .find_map(|f| match f.kind {
            ClFindingKind::StopTravel { metres } => Some(metres),
            _ => None,
        })
        .unwrap_or(0.0);
    assert!((0.0..0.5).contains(&travel), "{travel}");
}

#[test]
fn a_brake_contactor_that_does_not_pull_in_trips_the_feedback_and_the_hook_stays() {
    let (mut l, mut c, cmd) = rig(&[("cw_brake_feedback", "KB")]);
    *cmd.borrow_mut() = Cmd {
        up: true,
        contactor_follows_relay: false,
        ..Cmd::default()
    };
    seconds(&mut l, &mut c, 4.0);
    let m = l.module("=M1").unwrap();
    assert!(has(m, |k| matches!(
        k,
        ClFindingKind::Trip(ClFault::BrakeFeedback)
    )));
    assert!(m.height_m().abs() < 0.002, "{}", m.height_m());
}

#[test]
fn a_welded_brake_contactor_lets_the_load_run_with_the_drive_off() {
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().contactor_welded = true;
    seconds(&mut l, &mut c, 3.0);
    let m = l.module("=M1").unwrap();
    assert!(m.height_m() < -0.5, "{}", m.height_m());
    assert!(has(m, |k| matches!(k, ClFindingKind::BrakeSlip { .. })));
}

#[test]
fn one_sto_channel_lost_trips_after_the_discrepancy_time_and_both_lost_stop_at_once() {
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 3.0);
    // Channel 1 loses its wire. Nothing happens until the discrepancy time (1 s) is over.
    cmd.borrow_mut().sto_a = false;
    seconds(&mut l, &mut c, 0.5);
    let m = l.module("=M1").unwrap();
    assert!(!has(m, |k| matches!(k, ClFindingKind::Trip(_))));
    seconds(&mut l, &mut c, 1.0);
    let m = l.module("=M1").unwrap();
    assert!(has(m, |k| matches!(
        k,
        ClFindingKind::Trip(ClFault::StoDiscrepancy)
    )));

    // Both channels low on a fresh run: STO at once, the brake closes, the load stays held.
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 3.0);
    {
        let mut k = cmd.borrow_mut();
        k.sto_a = false;
        k.sto_b = false;
    }
    seconds(&mut l, &mut c, 2.0);
    let m = l.module("=M1").unwrap();
    assert!(has(m, |k| matches!(k, ClFindingKind::StoRequested)));
    assert!(m.snapshot().unwrap().shaft_rpm.abs() < 0.5);
    assert!(!has(m, |k| matches!(k, ClFindingKind::Trip(_))));
}

#[test]
fn a_lost_encoder_signal_trips_the_drive_and_the_brake_catches_the_load() {
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 3.0);
    cmd.borrow_mut().encoder = false;
    seconds(&mut l, &mut c, 3.0);
    let m = l.module("=M1").unwrap();
    assert!(has(m, |k| matches!(k, ClFindingKind::EncoderSignalLost)));
    assert!(
        m.findings()
            .iter()
            .any(|f| matches!(f.kind, ClFindingKind::Trip(_)))
    );
    assert!(m.snapshot().unwrap().shaft_rpm.abs() < 0.5);
    // The signal comes back and the trip is reset through the drive's reset edge.
    cmd.borrow_mut().encoder = true;
    seconds(&mut l, &mut c, 0.5);
    assert!(has(l.module("=M1").unwrap(), |k| matches!(
        k,
        ClFindingKind::EncoderSignalBack
    )));
}

#[test]
fn a_flapping_encoder_signal_keeps_a_capped_list_and_counts_the_rest() {
    use novo_machine::cl_winch_loop::MAX_FINDINGS;
    let (mut l, mut c, cmd) = rig(&[]);
    // Every other tick the signal toggles: well over MAX_FINDINGS changes in 10 s.
    for i in 0..2000 {
        cmd.borrow_mut().encoder = i % 2 == 0;
        l.run(&mut c, 1);
    }
    let m = l.module("=M1").unwrap();
    assert_eq!(m.findings().len(), MAX_FINDINGS);
    assert!(m.findings_dropped() > 0);
}

#[test]
fn a_lost_encoder_channel_is_found_and_lowering_trips_the_drive() {
    let (mut l, mut c, cmd) = rig(&[]);
    // Lifting: the counts of a one-channel encoder still run upward, the drive does not notice.
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 3.0);
    cmd.borrow_mut().encoder_b = false;
    seconds(&mut l, &mut c, 2.0);
    {
        let m = l.module("=M1").unwrap();
        assert!(has(m, |k| matches!(k, ClFindingKind::EncoderChannelLost)));
        assert!(
            !has(m, |k| matches!(k, ClFindingKind::Trip(_))),
            "{:?}",
            m.findings()
        );
    }
    // Lowering: the shaft turns down but the counts say up, so the speed loop runs away from
    // the reference and the drive trips on its own plausibility or deviation check.
    {
        let mut c2 = cmd.borrow_mut();
        c2.up = false;
        c2.down = true;
    }
    seconds(&mut l, &mut c, 4.0);
    let m = l.module("=M1").unwrap();
    assert!(
        has(m, |k| matches!(k, ClFindingKind::Trip(_))),
        "{:?}",
        m.findings()
    );
    assert!(!has(m, |k| matches!(k, ClFindingKind::EncoderSignalLost)));
}

/// Ground 0.3 m below the start, 2500 kg, rope 2e6 N/m. Lowering lands the hook and the drum pays
/// the rest out as slack (reported at 5 cm, and as "loose" past half a metre). Hoisting again takes
/// the slack up and picks the hook up once, with the snatch estimated by m g + v sqrt(k m): the
/// numbers are 24516 N plus v x 70711 N s/m.
#[test]
fn a_hook_lowered_onto_the_ground_goes_slack_and_snatches_when_lifted() {
    let (mut l, mut c, cmd) = rig(&[("cw_ground_m", "-0.3")]);
    cmd.borrow_mut().down = true;
    seconds(&mut l, &mut c, 10.0);
    let m = &l.modules[0];
    assert!((m.height_m() + 0.3).abs() < 1e-9, "{}", m.height_m());
    assert!(has(m, |k| matches!(k, ClFindingKind::HookLanded { .. })));
    assert!(has(m, |k| matches!(k, ClFindingKind::SlackRope { .. })));
    assert!(has(m, |k| matches!(k, ClFindingKind::RopeLoose { .. })));
    assert!(m.snapshot().unwrap().slack_m > 0.5);
    {
        let mut k = cmd.borrow_mut();
        k.down = false;
        k.up = true;
    }
    seconds(&mut l, &mut c, 12.0);
    let m = &l.modules[0];
    let picked: Vec<_> = m
        .findings()
        .iter()
        .filter_map(|f| match f.kind {
            ClFindingKind::RopePickedUp { m_s, peak_n } => Some((m_s, peak_n)),
            _ => None,
        })
        .collect();
    assert_eq!(picked.len(), 1, "{picked:?}");
    let (m_s, peak) = picked[0];
    assert!(m_s > 0.05, "{m_s}");
    let expect = 2500.0 * 9.80665 + m_s * (2.0e6_f64 * 2500.0).sqrt();
    assert!((peak - expect).abs() < 1e-6, "{peak} vs {expect}");
    assert!(m.height_m() > -0.3 + 0.05, "{}", m.height_m());
}

#[test]
fn without_a_ground_level_the_rope_never_goes_slack() {
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().down = true;
    seconds(&mut l, &mut c, 8.0);
    let m = &l.modules[0];
    assert!(m.height_m() < -0.5, "{}", m.height_m());
    assert!(!has(m, |k| matches!(
        k,
        ClFindingKind::SlackRope { .. } | ClFindingKind::HookLanded { .. }
    )));
}

/// 2500 kg on a rated load of 2000 kg is 125 % of it (weight 24516 N against 19613 N), above a
/// 110 % cut-out: lifting is inhibited from the first tick, lowering is not.
#[test]
fn a_load_above_the_cutout_inhibits_lifting_but_not_lowering() {
    let (mut l, mut c, cmd) = rig(&[("cw_swl_kg", "2000"), ("cw_overload_out", "OV")]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 4.0);
    let m = &l.modules[0];
    assert!(m.height_m().abs() < 0.001, "{}", m.height_m());
    assert!(m.lifting_inhibited());
    assert!(c.contacts.get("OV").copied().unwrap_or(false));
    assert!(has(m, |k| matches!(k, ClFindingKind::OverRated { .. })));
    let pct = m
        .findings()
        .iter()
        .find_map(|f| match f.kind {
            ClFindingKind::OverloadCutOut { pct } => Some(pct),
            _ => None,
        })
        .expect("the cut-out is a finding");
    assert!((pct - 125.0).abs() < 0.5, "{pct}");
    {
        let mut k = cmd.borrow_mut();
        k.up = false;
        k.down = true;
    }
    seconds(&mut l, &mut c, 6.0);
    assert!(
        l.modules[0].height_m() < -0.3,
        "{}",
        l.modules[0].height_m()
    );
}

/// 2500 kg on 2400 kg rated is 104 %: over the rating (a finding) but under a 110 % cut-out, so
/// the winch still lifts.
#[test]
fn a_load_between_the_rating_and_the_cutout_is_reported_but_lifts() {
    let (mut l, mut c, cmd) = rig(&[("cw_swl_kg", "2400")]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 6.0);
    let m = &l.modules[0];
    assert!(m.height_m() > 0.3, "{}", m.height_m());
    assert!(has(m, |k| matches!(k, ClFindingKind::OverRated { .. })));
    assert!(!has(m, |k| matches!(
        k,
        ClFindingKind::OverloadCutOut { .. }
    )));
}

/// 2500 kg on 3000 kg rated: 83.3 % at constant speed. A lost load cell inhibits lifting (the
/// load is unknown) and says so; the signal coming back releases it.
#[test]
fn a_lost_load_cell_inhibits_lifting_until_it_is_back() {
    let (mut l, mut c, cmd) = rig(&[("cw_swl_kg", "3000"), ("cw_load_cell", "LC")]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 6.0);
    let pct = l.modules[0].load_pct().unwrap();
    assert!((pct - 83.33).abs() < 1.0, "{pct}");
    let h = l.modules[0].height_m();
    assert!(h > 0.3, "{h}");
    cmd.borrow_mut().load_cell = false;
    seconds(&mut l, &mut c, 6.0);
    let m = &l.modules[0];
    assert!(has(m, |k| matches!(k, ClFindingKind::LoadCellLost)));
    assert!(m.lifting_inhibited());
    assert!(m.height_m() < h + 0.5, "it kept lifting: {}", m.height_m());
    cmd.borrow_mut().load_cell = true;
    seconds(&mut l, &mut c, 6.0);
    let m = &l.modules[0];
    assert!(has(m, |k| matches!(k, ClFindingKind::LoadCellBack)));
    assert!(!m.lifting_inhibited());
    assert!(m.height_m() > h + 0.5, "{}", m.height_m());
}

#[test]
fn without_a_rated_load_nothing_is_monitored() {
    let (mut l, mut c, cmd) = rig(&[("cw_load_cell", "LC")]);
    cmd.borrow_mut().load_cell = false;
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 4.0);
    let m = &l.modules[0];
    assert!(m.load_pct().is_none());
    assert!(!m.lifting_inhibited());
    assert!(m.height_m() > 0.1);
}

#[test]
fn binding_problems_are_named() {
    let (l, c, _) = rig(&[]);
    assert!(
        l.check_bindings(&c)
            .iter()
            .all(|f| !f.message.contains("not in the circuit"))
    );
    let mut bare = ClWinchLoop::new(StepConfig::new(DT).unwrap()).unwrap();
    bare.modules.push(winch(&[("cw_in_up", "NOPE")]));
    let f = bare.check_bindings(&FakeCircuit::new());
    assert!(
        f.iter()
            .any(|x| x.message.contains("NOPE is not in the circuit"))
    );
    let mut none = ClWinchLoop::new(StepConfig::new(DT).unwrap()).unwrap();
    let m = closed_winch_from_properties("=M2", &|k| {
        (k == CLOSED_WINCH_KEY).then(|| "1000".to_owned())
    })
    .unwrap()
    .unwrap()
    .0;
    none.modules.push(m);
    let f = none.check_bindings(&FakeCircuit::new());
    let text: String = f
        .iter()
        .map(|x| x.message.as_str())
        .collect::<Vec<_>>()
        .join("|");
    assert!(text.contains("cannot be commanded"), "{text}");
    assert!(text.contains("STO"), "{text}");
    assert!(text.contains("encoder"), "{text}");
}

#[test]
fn the_run_is_deterministic() {
    let run = || {
        let (mut l, mut c, cmd) = rig(&[]);
        cmd.borrow_mut().up = true;
        seconds(&mut l, &mut c, 4.0);
        l.module("=M1").unwrap().snapshot().unwrap().hook_height_m
    };
    assert_eq!(run(), run());
}

#[allow(dead_code)]
fn _port_is_used(c: &impl CircuitPort) -> bool {
    c.knows_device("x")
}
