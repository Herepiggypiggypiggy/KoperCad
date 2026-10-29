//! Loop checks for the hoist families (lane L2d) with a scripted circuit.

use novo_drive::hoist_unit::HoistFaults;
use novo_drive::motor::{Direction, WindingSel};
use novo_drive::supply::Phase;
use novo_machine::cosim::StepConfig;
use novo_machine::hoist_loop::{HoistLoop, HoistModule};
use novo_machine::port::{FakeCircuit, MotorFeed};
use novo_machine::spec;
use std::collections::BTreeMap;

fn module(extra: &[(&str, &str)]) -> HoistModule {
    let mut p: BTreeMap<String, String> = BTreeMap::new();
    p.insert("mech_mass_kg".into(), "100".into());
    p.insert("mech_drum_radius_m".into(), "0.1".into());
    p.insert("mech_gear_ratio".into(), "40".into());
    p.insert("mech_efficiency".into(), "1".into());
    p.insert("mech_main_contactor".into(), "=K-MAIN".into());
    for (k, v) in extra {
        p.insert((*k).into(), (*v).into());
    }
    spec::hoist_from_properties("h", "=M1", &|k| p.get(k).cloned())
        .unwrap()
        .unwrap()
}

fn the_loop(h: HoistModule) -> HoistLoop {
    let mut l = HoistLoop::new(StepConfig { dt_s: 0.002 }).unwrap();
    l.hoists = vec![h];
    l
}

fn up(w: WindingSel) -> MotorFeed {
    MotorFeed::Fed {
        winding: w,
        direction: Direction::Positive,
    }
}

#[test]
fn a_conical_hoist_lifts_when_the_drawing_feeds_the_motor() {
    let mut c = FakeCircuit::new();
    c.energised.insert("=K-MAIN".into());
    c.feeds.insert("=M1".into(), up(WindingSel::Fine));
    let mut l = the_loop(module(&[("mech_brake_kind", "conical")]));
    l.run(&mut c, 1500);
    let h = l.hoist("h").unwrap();
    assert!(h.height_m() > 0.1, "{}", h.height_m());
    // Drop the contactor: no field, the brake falls shut and holds.
    c.feeds.insert("=M1".into(), MotorFeed::Off);
    l.run(&mut c, 1500);
    let s = *l.hoist("h").unwrap().snapshot().unwrap();
    assert!(s.hook_speed_m_s.abs() < 1e-3);
}

#[test]
fn a_hum_in_the_drawing_is_two_phases_for_the_hoist() {
    let mut c = FakeCircuit::new();
    c.energised.insert("=K-MAIN".into());
    c.feeds.insert("=M1".into(), MotorFeed::Off);
    c.humming.insert("=M1".into());
    c.known.insert("=M1".into());
    let mut l = the_loop(module(&[("mech_brake_kind", "conical")]));
    l.run(&mut c, 1500);
    let s = *l.hoist("h").unwrap().snapshot().unwrap();
    assert!(s.conditions.phase_lost);
    // A conical brake does not lift on two phases from standstill.
    assert!(l.hoist("h").unwrap().height_m().abs() < 1e-6);
    assert!(s.conditions.fed_brake_closed);
}

#[test]
fn injected_faults_reach_the_engine_through_the_module() {
    let mut c = FakeCircuit::new();
    c.energised.insert("=K-MAIN".into());
    c.feeds.insert("=M1".into(), up(WindingSel::Fine));
    let mut m = module(&[]);
    m.unit
        .set_faults(HoistFaults {
            lost_phase: Some(Phase::L1),
            ..HoistFaults::NONE
        })
        .unwrap();
    let mut l = the_loop(m);
    l.run(&mut c, 1000);
    // The bridge rectifier sits on L1-L2: losing L1 leaves the coil dead and the brake shut.
    let s = *l.hoist("h").unwrap().snapshot().unwrap();
    assert!(s.conditions.phase_lost);
    assert!(l.hoist("h").unwrap().height_m().abs() < 1e-6);
}

#[test]
fn the_fine_lift_motor_is_fed_from_its_own_device() {
    let mut c = FakeCircuit::new();
    c.energised.insert("=K-MAIN".into());
    c.feeds.insert("=M1".into(), MotorFeed::Off);
    c.feeds.insert("=M2".into(), up(WindingSel::Fine));
    let mut l = the_loop(module(&[
        ("mech_brake_kind", "conical"),
        ("mech_fine_lift_motor", "=M2"),
    ]));
    assert!(
        l.check_bindings(&c).is_empty(),
        "{:?}",
        l.check_bindings(&c)
    );
    l.run(&mut c, 3000);
    let h = l.hoist("h").unwrap();
    let s = h.snapshot().unwrap();
    assert!(s.fine_engaged);
    assert!(h.height_m() > 0.1, "{}", h.height_m());
    assert!(s.hook_speed_m_s < 0.06);
}

#[test]
fn a_fine_lift_motor_without_its_device_is_a_binding_finding() {
    let c = FakeCircuit::new();
    let mut l = the_loop(module(&[("mech_fine_lift_motor", "=M2")]));
    let found = l.check_bindings(&c);
    assert!(
        found
            .iter()
            .any(|f| f.message.contains("fine-lift motor =M2"))
    );
    // And the reverse: fine-lift data in the unit but no device bound.
    l.hoists[0].binding.fine_motor = None;
    let found = l.check_bindings(&c);
    assert!(
        found
            .iter()
            .any(|f| f.message.contains("no fine-lift motor device"))
    );
}

fn coarse_fed() -> MotorFeed {
    up(WindingSel::Coarse)
}

#[test]
fn a_motor_stalled_against_a_closed_brake_draws_the_locked_current_and_trips_the_relay() {
    // Supply at 70 %: the conical brake never lifts (pull-in 0.8), the coarse winding is fed.
    // Locked-rotor current at 70 % is 0.7 of the rated-voltage figure; the relay (class 10,
    // setting at the rated current) trips at a few times its setting, and its contact drops
    // the feed in the circuit.
    let mut c = FakeCircuit::new().with_rule(|st| {
        st.energised.insert("=K-MAIN".into());
        let ok = st.contacts.get("=F-OL").copied().unwrap_or(true);
        st.feeds
            .insert("=M1".into(), if ok { coarse_fed() } else { MotorFeed::Off });
    });
    c.known.insert("=F-OL".into());
    let mut m = module(&[
        ("mech_brake_kind", "conical"),
        ("mech_overload_amps", "12"),
        ("mech_overload_contact", "=F-OL"),
    ]);
    m.unit
        .set_faults(novo_drive::hoist_unit::HoistFaults {
            voltage_ratio: 0.7,
            ..novo_drive::hoist_unit::HoistFaults::NONE
        })
        .unwrap();
    let locked_at_70 = m
        .unit
        .electrics()
        .unwrap()
        .coarse
        .locked_rotor_current_a(0.7)
        .unwrap();
    let mut l = the_loop(m);
    let mut tripped_at = None;
    for tick in 0..(120.0 / 0.002) as usize {
        l.step(&mut c);
        let s = *l.hoist("h").unwrap().snapshot().unwrap();
        if tick == 100 {
            assert!((s.motor_current_a - locked_at_70).abs() < 0.02 * locked_at_70);
            assert!(s.conditions.low_voltage && !s.conditions.overload_tripped);
        }
        if tripped_at.is_none() && s.conditions.overload_tripped {
            tripped_at = Some(tick as f64 * 0.002);
        }
    }
    let t = tripped_at.expect("the relay never tripped");
    // About 4 times the setting: trips in well under a minute but slower than class time.
    assert!(t > 10.0 && t < 60.0, "{t}");
    assert!(!c.contacts["=F-OL"]);
    // The feed is gone and the winding starts to cool.
    let s = *l.hoist("h").unwrap().snapshot().unwrap();
    assert!(s.energised.is_none() && s.motor_current_a == 0.0);
    assert!(s.winding_rise_k > 0.0);
    // Reset closes the contact again, the relay still holds its heat.
    l.hoists[0].unit.reset_overload();
    assert!(!l.hoists[0].unit.overload_tripped());
}

#[test]
fn a_hoist_running_on_two_phases_draws_more_current_and_heats_faster_than_on_three() {
    let run = |lost: Option<Phase>| {
        let mut c = FakeCircuit::new();
        c.energised.insert("=K-MAIN".into());
        c.feeds.insert("=M1".into(), up(WindingSel::Coarse));
        let mut l = the_loop(module(&[("mech_brake_kind", "conical")]));
        // Get it running on three phases first, then lose the phase.
        l.run(&mut c, (3.0 / 0.002) as usize);
        l.hoists[0]
            .unit
            .set_faults(novo_drive::hoist_unit::HoistFaults {
                lost_phase: lost,
                ..novo_drive::hoist_unit::HoistFaults::NONE
            })
            .unwrap();
        l.run(&mut c, (20.0 / 0.002) as usize);
        *l.hoist("h").unwrap().snapshot().unwrap()
    };
    let three = run(None);
    let two = run(Some(Phase::L2));
    assert!(!three.conditions.two_phase_overcurrent);
    assert!(two.conditions.two_phase_overcurrent, "{two:?}");
    assert!(two.motor_current_a > 1.5 * three.motor_current_a);
    assert!(two.winding_rise_k > three.winding_rise_k);
}

#[test]
fn the_fine_lift_motor_has_its_own_current_and_heat() {
    let mut c = FakeCircuit::new();
    c.energised.insert("=K-MAIN".into());
    c.feeds.insert("=M1".into(), MotorFeed::Off);
    c.feeds.insert("=M2".into(), up(WindingSel::Fine));
    let mut l = the_loop(module(&[
        ("mech_brake_kind", "conical"),
        ("mech_fine_lift_motor", "=M2"),
    ]));
    l.run(&mut c, (10.0 / 0.002) as usize);
    let s = *l.hoist("h").unwrap().snapshot().unwrap();
    assert!(s.fine_current_a > 0.5, "{}", s.fine_current_a);
    assert!(s.fine_winding_rise_k > 0.0);
    // The main motor was not fed: no current, no heat.
    assert_eq!(s.motor_current_a, 0.0);
    assert!(s.winding_rise_k < 1.0);
}

#[test]
fn an_overload_contact_that_is_not_in_the_drawing_is_a_binding_finding() {
    let c = FakeCircuit::new();
    let l = the_loop(module(&[("mech_overload_contact", "=F-OL")]));
    let found = l.check_bindings(&c);
    assert!(
        found
            .iter()
            .any(|f| f.message.contains("overload relay contact =F-OL"))
    );
}
