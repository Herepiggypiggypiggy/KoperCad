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
