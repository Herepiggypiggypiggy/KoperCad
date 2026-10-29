//! Loop checks for two closed-loop winches on one beam driven by a scripted circuit: winch A's
//! devices command the pair, every other input (STO channels, encoder signal, brake contactor)
//! is a device per winch, so a defect on winch B reaches only B and the pair reacts as
//! configured. The beam is 5000 kg on two 2 m apart ropes with the centre of gravity in the
//! middle, so each rope carries 24.5 kN.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use novo_drive::closed_loop::ClFault;
use novo_machine::cl_pair_loop::{ClPairFindingKind, ClPairLoop, ClPairModule};
use novo_machine::cl_winch_spec::closed_pair_from_properties;
use novo_machine::cosim::StepConfig;
use novo_machine::port::FakeCircuit;
use novo_machine::sync_winch::SyncFault;

const DT: f64 = 0.005;

#[derive(Clone, Copy)]
struct Cmd {
    up: bool,
    down: bool,
    fast: bool,
    /// STO channels per winch.
    sto: [(bool, bool); 2],
    encoder: [bool; 2],
    /// The brake contactor of a winch pulls in with the drive's relay contact.
    contactor_follows_relay: [bool; 2],
}

impl Default for Cmd {
    fn default() -> Self {
        Self {
            up: false,
            down: false,
            fast: false,
            sto: [(true, true); 2],
            encoder: [true; 2],
            contactor_follows_relay: [true; 2],
        }
    }
}

type Shared = Rc<RefCell<Cmd>>;

fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn module(extra: &[(&str, &str)]) -> ClPairModule {
    let mut a = vec![
        ("cw_mass_kg", "2500"),
        ("cw_pair_partner", "=M2"),
        ("cw_beam_mass_kg", "5000"),
        ("cw_beam_length_m", "2"),
        ("cw_in_up", "KU"),
        ("cw_in_down", "KD"),
        ("cw_in_fast", "KF"),
        ("cw_sto_a", "STOA1"),
        ("cw_sto_b", "STOA2"),
        ("cw_encoder", "ENCA"),
        ("cw_brake_relay", "KRA"),
        ("cw_brake_feed", "KBA"),
    ];
    a.extend_from_slice(extra);
    let b = [
        ("cw_mass_kg", "2500"),
        ("cw_sto_a", "STOB1"),
        ("cw_sto_b", "STOB2"),
        ("cw_encoder", "ENCB"),
        ("cw_brake_relay", "KRB"),
        ("cw_brake_feed", "KBB"),
    ];
    let (pa, pb) = (props(&a), props(&b));
    closed_pair_from_properties("=M1", &|k| pa.get(k).cloned(), "=M2", &|k| {
        pb.get(k).cloned()
    })
    .unwrap()
    .unwrap()
    .0
}

fn rig(extra: &[(&str, &str)]) -> (ClPairLoop, FakeCircuit, Shared) {
    let cmd: Shared = Rc::new(RefCell::new(Cmd::default()));
    let c = cmd.clone();
    let mut circuit = FakeCircuit::new().with_rule(move |s| {
        let cmd = *c.borrow();
        let relay = |tag: &str| s.contacts.get(tag).copied().unwrap_or(false);
        let table = [
            ("KU", cmd.up),
            ("KD", cmd.down),
            ("KF", cmd.fast),
            ("STOA1", cmd.sto[0].0),
            ("STOA2", cmd.sto[0].1),
            ("STOB1", cmd.sto[1].0),
            ("STOB2", cmd.sto[1].1),
            ("ENCA", cmd.encoder[0]),
            ("ENCB", cmd.encoder[1]),
            ("KBA", cmd.contactor_follows_relay[0] && relay("KRA")),
            ("KBB", cmd.contactor_follows_relay[1] && relay("KRB")),
        ];
        for (tag, on) in table {
            if on {
                s.energised.insert(tag.to_owned());
            } else {
                s.energised.remove(tag);
            }
        }
    });
    circuit.known.extend(
        [
            "KU", "KD", "KF", "STOA1", "STOA2", "STOB1", "STOB2", "ENCA", "ENCB", "KRA", "KRB",
            "KBA", "KBB",
        ]
        .map(str::to_owned),
    );
    let mut l = ClPairLoop::new(StepConfig::new(DT).unwrap()).unwrap();
    l.modules.push(module(extra));
    (l, circuit, cmd)
}

fn seconds(l: &mut ClPairLoop, c: &mut FakeCircuit, s: f64) {
    l.run(c, (s / DT).round() as usize);
}

fn has(m: &ClPairModule, f: impl Fn(&ClPairFindingKind) -> bool) -> bool {
    m.findings().iter().any(|x| f(&x.kind))
}

#[test]
fn up_lifts_both_ropes_at_the_step_speed_and_the_load_is_shared() {
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 6.0);
    let m = l.module("=M1").unwrap();
    let s = m.snapshot().unwrap();
    // 0.15 m/s for the last second or so: the beam is well above the start.
    assert!(s.height_m > 0.4, "height {}", s.height_m);
    assert!(s.tilt_deg.abs() < 0.2, "tilt {}", s.tilt_deg);
    assert!((s.share_a - 0.5).abs() < 0.05, "share {}", s.share_a);
    assert!(
        !has(m, |k| matches!(k, ClPairFindingKind::Trip { .. })),
        "{:?}",
        m.findings()
    );
}

#[test]
fn one_sto_channel_lost_on_b_trips_b_and_the_link_stops_a_too() {
    // The position limit is off so the stop is the partner link's, not the position error's.
    let (mut l, mut c, cmd) = rig(&[("cw_pair_max_error_m", "0")]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 3.0);
    cmd.borrow_mut().sto[1].0 = false;
    seconds(&mut l, &mut c, 3.0);
    let m = l.module("=M1").unwrap();
    assert!(
        has(m, |k| matches!(
            k,
            ClPairFindingKind::Trip {
                winch: 1,
                fault: ClFault::StoDiscrepancy
            }
        )),
        "{:?}",
        m.findings()
    );
    assert!(
        has(m, |k| matches!(
            k,
            ClPairFindingKind::Sync(SyncFault::PartnerFault)
        )),
        "{:?}",
        m.findings()
    );
    let s = m.snapshot().unwrap();
    assert!(
        s.winches[0].shaft_rpm.abs() < 1.0,
        "A still turns at {} rpm",
        s.winches[0].shaft_rpm
    );
}

#[test]
fn with_the_position_limit_on_the_pair_stops_before_the_trip_and_b_s_rope_is_overloaded() {
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 3.0);
    cmd.borrow_mut().sto[1].0 = false;
    seconds(&mut l, &mut c, 3.0);
    let m = l.module("=M1").unwrap();
    // B loses torque as soon as one channel is low, drops behind A and the position error
    // (100 mm by default) stops both before the discrepancy latch trips B.
    assert!(has(m, |k| matches!(
        k,
        ClPairFindingKind::Sync(SyncFault::PositionError)
    )));
    assert!(has(m, |k| matches!(
        k,
        ClPairFindingKind::Overload { winch: 1, .. }
    )));
}

#[test]
fn without_a_link_a_keeps_running_when_b_loses_sto() {
    let (mut l, mut c, cmd) = rig(&[
        ("cw_partner_fault", "keep running"),
        ("cw_pair_max_error_m", "0"),
    ]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 3.0);
    cmd.borrow_mut().sto[1] = (false, false);
    seconds(&mut l, &mut c, 1.0);
    let m = l.module("=M1").unwrap();
    assert!(has(m, |k| matches!(
        k,
        ClPairFindingKind::StoRequested { winch: 1 }
    )));
    let s = m.snapshot().unwrap();
    assert!(
        s.winches[0].shaft_rpm > 100.0,
        "A at {} rpm",
        s.winches[0].shaft_rpm
    );
    assert!(s.winches[1].shaft_rpm.abs() < 1.0);
    assert!(m.pair.sync_fault().is_none());
}

#[test]
fn a_lost_encoder_signal_on_b_trips_b_and_names_it() {
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 3.0);
    cmd.borrow_mut().encoder[1] = false;
    seconds(&mut l, &mut c, 2.0);
    let m = l.module("=M1").unwrap();
    assert!(has(m, |k| matches!(
        k,
        ClPairFindingKind::EncoderSignalLost { winch: 1 }
    )));
    assert!(has(m, |k| matches!(
        k,
        ClPairFindingKind::Trip { winch: 1, .. }
    )));
    assert!(!has(m, |k| matches!(
        k,
        ClPairFindingKind::EncoderSignalLost { winch: 0 }
    )));
}

#[test]
fn a_brake_contactor_of_b_that_does_not_pull_in_stops_the_pair() {
    let (mut l, mut c, cmd) = rig(&[]);
    cmd.borrow_mut().contactor_follows_relay[1] = false;
    cmd.borrow_mut().up = true;
    seconds(&mut l, &mut c, 6.0);
    let m = l.module("=M1").unwrap();
    let s = m.snapshot().unwrap();
    // B's brake never opens, so B cannot pay out and the beam cannot lift as a pair: the pair
    // is stopped or a drive tripped, and the beam is nowhere near the 0.9 m a free run gives.
    assert!(
        m.pair.sync_fault().is_some() || has(m, |k| matches!(k, ClPairFindingKind::Trip { .. })),
        "{:?}",
        m.findings()
    );
    assert!(s.height_m < 0.5, "height {}", s.height_m);
}

#[test]
fn winch_b_commands_are_not_read_and_the_bindings_say_so() {
    let (l, c, _) = rig(&[]);
    // Rebuild with B's own up device drawn.
    let mut m = module(&[]);
    m.bindings[1].up = Some("KU".into());
    let mut l2 = ClPairLoop::new(StepConfig::new(DT).unwrap()).unwrap();
    l2.modules.push(m);
    let problems = l2.check_bindings(&c);
    assert!(
        problems
            .iter()
            .any(|p| p.message.contains("read from winch A only")),
        "{problems:?}"
    );
    // A pair with everything drawn has no missing-device problem.
    assert!(
        !l.check_bindings(&c)
            .iter()
            .any(|p| p.message.contains("is not in the circuit"))
    );
}

#[test]
fn the_run_is_deterministic() {
    let run = || {
        let (mut l, mut c, cmd) = rig(&[]);
        cmd.borrow_mut().up = true;
        seconds(&mut l, &mut c, 2.0);
        cmd.borrow_mut().sto[1].0 = false;
        seconds(&mut l, &mut c, 2.0);
        let m = l.module("=M1").unwrap();
        (m.snapshot().unwrap().height_m, m.findings().to_vec())
    };
    assert_eq!(run(), run());
}
