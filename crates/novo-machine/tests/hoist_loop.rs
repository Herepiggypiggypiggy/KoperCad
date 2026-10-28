//! Loop checks for lane L5a with a scripted circuit. Expected values are worked out on paper
//! in the comments.

use std::cell::Cell;
use std::rc::Rc;

use novo_drive::brake::{SpringBrake, Switching};
use novo_drive::hoist_unit::{BrakeFeed, TwoSpeedHoistUnit};
use novo_drive::mechanics::Hoist;
use novo_drive::motor::{Direction, PoleChangeMotor, Winding, WindingSel};
use novo_drive::sequencer::{Sequencer, SequencerConfig};
use novo_machine::bindings::SensorBinding;
use novo_machine::cosim::StepConfig;
use novo_machine::hoist_loop::{HoistBinding, HoistLoop, HoistModule, LimitKind, LimitSwitch};
use novo_machine::port::{FakeCircuit, MotorFeed};

const G: f64 = novo_drive::GRAVITY;

fn motor() -> PoleChangeMotor {
    // 12/2 pole hoist motor: fine 0.8 kW 480 of 500 rpm, coarse 6 kW 2880 of 3000 rpm.
    PoleChangeMotor::new(
        Winding::new(12, 50.0, 800.0, 480.0, 2.0).unwrap(),
        Winding::new(2, 50.0, 6000.0, 2880.0, 2.2).unwrap(),
        0.05,
    )
    .unwrap()
}

/// 100 kg, r 0.1 m, i 40 (0.0025 m per rad), lossless; brake 60 N m, 20 ms delays.
fn unit(feed: BrakeFeed) -> TwoSpeedHoistUnit {
    let seq = Sequencer::new(SequencerConfig {
        changeover_dead_time_s: 0.1,
        fine_first_s: 0.5,
        reversal_dead_time_s: 0.2,
    })
    .unwrap();
    let brake = SpringBrake::new(60.0, 0.02, 0.02, 0.02, 0.0, Switching::DcSide).unwrap();
    let hoist = Hoist::new(100.0, 0.1, 40.0, 1, 1.0).unwrap();
    TwoSpeedHoistUnit::new(motor(), hoist, seq, brake, feed).unwrap()
}

fn module(id: &str, motor_tag: &str, feed: BrakeFeed, limits: Vec<LimitSwitch>) -> HoistModule {
    HoistModule::new(
        id,
        unit(feed),
        HoistBinding {
            motor: motor_tag.into(),
            brake_feed: Some("=K-BRK".into()),
            mains: Some("=K-MAIN".into()),
            limits,
        },
    )
}

fn upper_limit(at_mm: f64) -> LimitSwitch {
    LimitSwitch {
        binding: SensorBinding {
            name: "upper".into(),
            device_tag: "=S-UP".into(),
            actuate_at_mm: at_mm,
        },
        kind: LimitKind::Upper,
    }
}

fn fed(w: WindingSel) -> MotorFeed {
    MotorFeed::Fed {
        winding: w,
        direction: Direction::Positive,
    }
}

fn the_loop(h: Vec<HoistModule>) -> HoistLoop {
    let mut l = HoistLoop::new(StepConfig { dt_s: 0.005 }).unwrap();
    l.hoists = h;
    l
}

#[test]
fn fine_raise_settles_at_the_closed_form_slip_speed() {
    // Load torque m g k = 100 * 9.80665 * 0.0025 = 2.4517 N m on the fine winding. The loop
    // must reach the speed the closed-form slip gives for that torque.
    let m = motor();
    let omega = m
        .winding(WindingSel::Fine)
        .steady_omega_rad_s(Direction::Positive, 100.0 * G * 0.0025)
        .unwrap();
    let want = omega * 0.0025;
    let mut c = FakeCircuit::new();
    c.energised.insert("=K-MAIN".into());
    c.feeds.insert("=M1".into(), fed(WindingSel::Fine));
    let mut l = the_loop(vec![module("h", "=M1", BrakeFeed::MotorTerminals, vec![])]);
    l.run(&mut c, 600); // 3 s
    let h = l.hoist("h").unwrap();
    let got = h.snapshot().unwrap().hook_speed_m_s;
    assert!((got - want).abs() < 0.01 * want, "{got} vs {want}");
    assert!(h.height_m() > 0.2);
}

#[test]
fn coarse_only_upper_limit_stops_coarse_and_fine_still_runs() {
    // Upper limit at 1.0 m in the coarse chain only. Step 2 starts coarse; once the limit
    // contact opens, the circuit drops coarse, fine still feeds and the hook keeps rising.
    let step = Rc::new(Cell::new(2u8));
    let s = step.clone();
    let mut c = FakeCircuit::new().with_rule(move |st| {
        st.energised.insert("=K-MAIN".into());
        let coarse_ok = st.contacts.get("=S-UP").copied().unwrap_or(true);
        let f = match s.get() {
            2 if coarse_ok => fed(WindingSel::Coarse),
            1 | 2 => fed(WindingSel::Fine),
            _ => MotorFeed::Off,
        };
        st.feeds.insert("=M1".into(), f);
    });
    let mut l = the_loop(vec![module(
        "h",
        "=M1",
        BrakeFeed::MotorTerminals,
        vec![upper_limit(1000.0)],
    )]);
    let mut crossed_at = None;
    let mut winding_after = None;
    for tick in 0..4000 {
        l.step(&mut c);
        let h = l.hoist("h").unwrap();
        let w = h.snapshot().unwrap().energised.map(|e| e.winding);
        if crossed_at.is_none() && h.height_m() >= 1.0 {
            crossed_at = Some(tick);
        }
        if crossed_at.is_some_and(|t0| tick == t0 + 1) {
            winding_after = w;
        }
    }
    assert!(crossed_at.is_some(), "never reached the limit");
    // The contact opens on the tick after the crossing: one sample of overshoot at most.
    assert_eq!(winding_after, Some(WindingSel::Fine));
    let h = l.hoist("h").unwrap();
    assert!(
        h.height_m() > 1.05,
        "fine must keep running: {}",
        h.height_m()
    );
    assert!(!c.contacts["=S-UP"]);
}

#[test]
fn without_the_nc_contact_in_the_drawing_the_limit_does_not_stop_coarse() {
    // Same circuit, but the limit is not in the chain (rule ignores the contact): coarse
    // runs through the limit height. This is the "remove the contact and the check fails" case.
    let mut c = FakeCircuit::new().with_rule(|st| {
        st.energised.insert("=K-MAIN".into());
        st.feeds.insert("=M1".into(), fed(WindingSel::Coarse));
    });
    let mut l = the_loop(vec![module(
        "h",
        "=M1",
        BrakeFeed::MotorTerminals,
        vec![upper_limit(1000.0)],
    )]);
    l.run(&mut c, 600);
    let h = l.hoist("h").unwrap();
    assert!(h.height_m() > 1.0);
    assert_eq!(
        h.snapshot().unwrap().energised.unwrap().winding,
        WindingSel::Coarse
    );
}

#[test]
fn main_contactor_off_drops_the_motor_and_the_brake_holds() {
    let mut c = FakeCircuit::new();
    c.energised.insert("=K-MAIN".into());
    c.feeds.insert("=M1".into(), fed(WindingSel::Fine));
    let mut l = the_loop(vec![module("h", "=M1", BrakeFeed::MotorTerminals, vec![])]);
    l.run(&mut c, 200);
    c.energised.clear(); // E-stop opens the main contactor
    l.run(&mut c, 400);
    let snap = *l.hoist("h").unwrap().snapshot().unwrap();
    assert!(snap.energised.is_none());
    assert!(snap.hook_speed_m_s.abs() < 1e-3, "{}", snap.hook_speed_m_s);
    // Holding at last: the height no longer changes.
    let h0 = l.hoist("h").unwrap().height_m();
    l.run(&mut c, 200);
    assert!((l.hoist("h").unwrap().height_m() - h0).abs() < 1e-4);
}

#[test]
fn separate_brake_feed_not_energised_holds_the_load_against_the_motor() {
    // Brake 60 N m against a fine winding that gives about 15.9 N m at rated: it must not move.
    let mut c = FakeCircuit::new();
    c.energised.insert("=K-MAIN".into());
    c.feeds.insert("=M1".into(), fed(WindingSel::Fine));
    let mut l = the_loop(vec![module("h", "=M1", BrakeFeed::Separate, vec![])]);
    l.run(&mut c, 400);
    assert!(l.hoist("h").unwrap().height_m().abs() < 1e-3);
    c.energised.insert("=K-BRK".into()); // brake rectifier fed: now it lifts
    l.run(&mut c, 400);
    assert!(l.hoist("h").unwrap().height_m() > 0.1);
}

#[test]
fn swapped_bindings_give_swapped_motion() {
    // Two identical hoists; only =M1 is fed. Bound as given, A rises and B stays; with the
    // motor tags swapped, B rises and A stays.
    for (tag_a, tag_b, a_moves) in [("=M1", "=M2", true), ("=M2", "=M1", false)] {
        let mut c = FakeCircuit::new();
        c.energised.insert("=K-MAIN".into());
        c.feeds.insert("=M1".into(), fed(WindingSel::Fine));
        let mut l = the_loop(vec![
            module("A", tag_a, BrakeFeed::MotorTerminals, vec![]),
            module("B", tag_b, BrakeFeed::MotorTerminals, vec![]),
        ]);
        l.run(&mut c, 400);
        let (a, b) = (
            l.hoist("A").unwrap().height_m(),
            l.hoist("B").unwrap().height_m(),
        );
        let (moved, still) = if a_moves { (a, b) } else { (b, a) };
        assert!(moved > 0.1, "{moved}");
        assert!(still.abs() < 1e-9, "{still}");
    }
}

#[test]
fn ten_hoists_run_twice_to_identical_bits() {
    fn run() -> Vec<u64> {
        let mut c = FakeCircuit::new();
        c.energised.insert("=K-MAIN".into());
        let mut hoists = Vec::new();
        for i in 0..10 {
            let tag = format!("=M{i}");
            if i % 2 == 0 {
                c.feeds.insert(tag.clone(), fed(WindingSel::Coarse));
            }
            hoists.push(module(
                &format!("h{i}"),
                &tag,
                BrakeFeed::MotorTerminals,
                vec![],
            ));
        }
        let mut l = the_loop(hoists);
        l.run(&mut c, 1000);
        l.hoists.iter().map(|h| h.height_m().to_bits()).collect()
    }
    let first = run();
    assert_eq!(first, run());
    assert!(first.iter().any(|b| f64::from_bits(*b) > 1.0));
}

#[test]
fn bindings_to_unknown_devices_are_reported() {
    let mut c = FakeCircuit::new();
    c.energised.insert("=K-MAIN".into());
    c.known.insert("=K-BRK".into());
    c.feeds.insert("=M1".into(), MotorFeed::Off);
    let ok = module("ok", "=M1", BrakeFeed::MotorTerminals, vec![]);
    let mut bad = module(
        "bad",
        "=NOPE",
        BrakeFeed::Separate,
        vec![upper_limit(1000.0)],
    );
    bad.binding.brake_feed = None;
    let l = the_loop(vec![ok, bad]);
    let found = l.check_bindings(&c);
    let texts: Vec<_> = found.iter().map(|f| f.message.as_str()).collect();
    assert!(found.iter().all(|f| f.module == "bad"), "{texts:?}");
    assert!(texts.iter().any(|t| t.contains("motor =NOPE")), "{texts:?}");
    assert!(
        texts.iter().any(|t| t.contains("no brake feed")),
        "{texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.contains("limit switch =S-UP")),
        "{texts:?}"
    );
}

#[test]
fn the_loop_refuses_a_bad_step() {
    for bad in [0.0, -1.0, 1e9, f64::NAN] {
        assert!(HoistLoop::new(StepConfig { dt_s: bad }).is_err(), "{bad}");
    }
}

#[test]
fn winding_follows_the_circuit_not_the_sequencer() {
    let mut c = FakeCircuit::new();
    c.energised.insert("=K-MAIN".into());
    c.feeds.insert("=M1".into(), fed(WindingSel::Coarse));
    let mut l = the_loop(vec![module("h", "=M1", BrakeFeed::MotorTerminals, vec![])]);
    l.run(&mut c, 3);
    assert_eq!(
        l.hoist("h").unwrap().unit.winding(),
        Some(WindingSel::Coarse)
    );
}
