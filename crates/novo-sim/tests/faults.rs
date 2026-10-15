//! Fault scenarios end to end: waveform, protection timing, selectivity,
//! overload memory, reclosing.

use novo_sim::fault::{FaultCurrent, FaultKind};
use novo_sim::feeder::{Impedance, RadialNetwork};
use novo_sim::protection::{Band, ProtectiveDevice, TableCurve, ThermalMemory, TripModel};
use novo_sim::scenario::{Action, EventKind, Scenario, run};
use novo_sim::trace::Trace;

/// A C-characteristic miniature breaker as a printed curve (test data, the
/// shape of IEC 60898-1: thermal 1.45 In within the hour, magnetic 5..10 In).
fn c_breaker(rated: f64) -> TableCurve {
    TableCurve::new(
        rated,
        vec![
            (1.45, 60.0, 3600.0),
            (2.55, 1.0, 120.0),
            (5.0, 0.02, 10.0),
            (10.0, 0.005, 0.02),
            (50.0, 0.004, 0.01),
        ],
    )
}

/// gG fuse melting band, slow enough to be selective against a breaker.
fn gg_fuse(rated: f64) -> TableCurve {
    TableCurve::new(
        rated,
        vec![
            (1.6, 3600.0, 7200.0),
            (2.5, 100.0, 300.0),
            (5.0, 2.0, 8.0),
            (10.0, 0.2, 0.6),
            (50.0, 0.02, 0.06),
        ],
    )
}

/// Overload relay class 10: 7.2 In trips in 4 to 10 s (IEC 60947-4-1).
fn relay_class10(rated: f64) -> TableCurve {
    TableCurve::new(
        rated,
        vec![
            (1.2, 600.0, 7200.0),
            (1.5, 120.0, 480.0),
            (7.2, 4.0, 10.0),
            (20.0, 4.0, 10.0),
        ],
    )
}

fn network() -> RadialNetwork {
    // 400 V, 25 kA-class busbar behind a 630 kVA-ish transformer.
    RadialNetwork::new("busbar", 400.0, 50.0, Impedance::new(0.002, 0.009))
        .segment("mcc", "busbar", Impedance::new(0.004, 0.002), Some("Q1"))
        .segment("pump", "mcc", Impedance::new(0.020, 0.003), Some("F2"))
}

#[test]
fn the_asymmetrical_peak_follows_iec_60909() {
    for r_over_x in [0.05, 0.1, 0.3, 1.0] {
        let fc = FaultCurrent::new(10_000.0, r_over_x, 50.0);
        let theta = fc.worst_case_angle();
        let trace = fc.waveform("i", theta, 0.0, 0.1, 0.1, 400);
        let peak = trace.peak().unwrap();
        let expected = fc.peak();
        // kappa is a design formula; the true waveform is within 6 percent
        // and never above the real worst case by more than that.
        assert!(
            (peak - expected).abs() / expected < 0.06,
            "R/X {r_over_x}: waveform peak {peak:.0} vs ip {expected:.0}"
        );
    }
}

#[test]
fn a_symmetrical_start_has_no_dc_offset_and_the_rms_is_ik() {
    let fc = FaultCurrent::new(8_000.0, 0.2, 50.0);
    let theta = fc.phi(); // sin(theta - phi) = 0
    let trace = fc.waveform("i", theta, 0.0, 0.2, 0.2, 400);
    let rms = trace.rms_between(0.0, 0.2).unwrap();
    assert!((rms - 8_000.0).abs() < 40.0, "rms {rms}");
    assert!((trace.peak().unwrap() - 8_000.0 * 2f64.sqrt()).abs() < 20.0);
}

#[test]
fn the_arc_is_only_extinguished_at_a_current_zero() {
    let fc = FaultCurrent::new(10_000.0, 0.1, 50.0);
    let theta = fc.worst_case_angle();
    let t = fc.interrupt_at(0.0123, theta);
    assert!(t >= 0.0123);
    assert!(fc.instant(t, theta).abs() < 1.0, "i at interruption");
    // With a heavy DC offset the first zero comes late.
    let early = fc.interrupt_at(0.003, theta);
    assert!(early > 0.015 && early < 0.02, "{early}");
}

#[test]
fn let_through_is_the_area_under_i_squared() {
    let fc = FaultCurrent::new(5_000.0, 0.15, 50.0);
    let theta = fc.worst_case_angle();
    let trace = fc.waveform("i", theta, 0.0, 0.05, 0.05, 800);
    let numeric = trace.i2t_between(0.0, 0.05).unwrap();
    let direct = fc.let_through(theta, 0.05);
    assert!(
        (numeric - direct).abs() / direct < 0.01,
        "{numeric} vs {direct}"
    );
}

#[test]
fn trace_statistics() {
    let t = Trace::sampled("x", "V", 1.0, 0.001, |t| {
        (2.0 * std::f64::consts::PI * 5.0 * t).sin()
    });
    assert!((t.rms_between(0.0, 1.0).unwrap() - 0.5f64.sqrt()).abs() < 1e-3);
    assert_eq!(t.crossings(0.0).len(), 9, "interior zero crossings");
    let d = t.decimate(100);
    assert!(d.len() <= 120 && d.peak().unwrap() > 0.99, "peaks survive");
    assert_eq!(t.value_at(0.25).map(|v| (v * 1000.0).round()), Some(1000.0));
}

#[test]
fn table_curves_join_in_log_space_and_bands_pick_the_edge() {
    let c = c_breaker(16.0);
    assert_eq!(c.trip_time_range(10.0).0, f64::INFINITY, "below pickup");
    let (lo, hi) = c.trip_time_range(16.0 * 2.55);
    assert!((lo - 1.0).abs() < 1e-9 && (hi - 120.0).abs() < 1e-9);
    let fast = Band::Fastest.time(&c, 40.8);
    let slow = Band::Slowest.time(&c, 40.8);
    let mid = Band::Typical.time(&c, 40.8);
    assert!(fast < mid && mid < slow);
    assert!(
        (mid - (fast * slow).sqrt()).abs() < 1e-6,
        "geometric middle"
    );
}

#[test]
fn a_short_circuit_at_the_pump_trips_only_the_pump_breaker_when_the_fuse_is_slower() {
    let devices = [
        ProtectiveDevice::new("Q1", "-Q1", gg_fuse(100.0)).with_opening_time(0.0),
        ProtectiveDevice::new("F2", "-F2", c_breaker(16.0)),
    ];
    let s = Scenario::new("pump short", 1.0).at(
        0.1,
        Action::Fault {
            id: "SC1".into(),
            node: "pump".into(),
            kind: FaultKind::ThreePhase,
            fault_ohm: 0.0,
            inception_deg: None,
        },
    );
    let r = run(&network(), &devices, &s);
    let o = &r.faults[0];
    assert!(o.ik > 5_000.0 && o.ik < 15_000.0, "Ik {}", o.ik);
    assert!(o.peak > o.ik * 2f64.sqrt());
    assert_eq!(o.cleared_by.as_deref(), Some("F2"));
    assert_eq!(o.expected.as_deref(), Some("F2"));
    assert!(o.selective, "{:?}", o.tripped);
    let cleared = o.cleared_at.unwrap();
    assert!(cleared > 0.1 && cleared < 0.14, "cleared at {cleared}");
    assert!(o.let_through_i2t.unwrap() > 0.0);
    assert!(r.trip_time("Q1").is_none());
    // The fault waveform ends at a natural zero.
    let wave = r.traces.get("i fault SC1").unwrap();
    let (_, end) = wave.span().unwrap();
    assert!(end > cleared - 1e-9);
}

#[test]
fn two_equal_fast_breakers_in_series_are_not_selective() {
    let devices = [
        ProtectiveDevice::new("Q1", "-Q1", c_breaker(63.0)),
        ProtectiveDevice::new("F2", "-F2", c_breaker(16.0)),
    ];
    let s = Scenario::new("pump short", 1.0).at(
        0.0,
        Action::Fault {
            id: "SC1".into(),
            node: "pump".into(),
            kind: FaultKind::ThreePhase,
            fault_ohm: 0.0,
            inception_deg: None,
        },
    );
    let r = run(&network(), &devices, &s);
    let o = &r.faults[0];
    assert!(!o.selective, "both are in their magnetic region");
    assert_eq!(o.tripped.len(), 2, "both decide to trip: {:?}", o.tripped);
}

#[test]
fn a_motor_start_rides_through_cold_and_trips_the_relay_when_repeated() {
    let devices = [ProtectiveDevice::new("F2", "-F2", relay_class10(10.0)).with_opening_time(0.05)];
    let net = RadialNetwork::new("busbar", 400.0, 50.0, Impedance::new(0.002, 0.009)).segment(
        "pump",
        "busbar",
        Impedance::new(0.02, 0.003),
        Some("F2"),
    );
    // One 3 s start at 7 In: used about 3/6.3 = half the relay's life.
    let once = Scenario::new("one start", 60.0).motor_start(1.0, "pump", 70.0, 3.0, 8.0);
    let r = run(&net, &devices, &once);
    assert!(r.trip_time("F2").is_none(), "{:?}", r.events);
    let heat = r.peak_heat["F2"];
    assert!(heat > 0.3 && heat < 0.7, "heat {heat}");
    // Three starts 20 s apart: little cools in between, the third trips.
    let mut thrice = Scenario::new("three starts", 120.0);
    for k in 0..3 {
        thrice = thrice.motor_start(1.0 + 20.0 * f64::from(k), "pump", 70.0, 3.0, 0.0);
    }
    let r = run(&net, &devices, &thrice);
    let trip = r.trip_time("F2").expect("trips on the repeated start");
    assert!(
        trip > 41.0 && trip < 44.0,
        "trips during the third start: {trip}"
    );
    assert!(r.events.iter().any(|e| e.kind == EventKind::TripOverload));
}

#[test]
fn the_band_decides_nuisance_tripping_versus_protection() {
    let net = RadialNetwork::new("busbar", 400.0, 50.0, Impedance::new(0.002, 0.009)).segment(
        "pump",
        "busbar",
        Impedance::new(0.02, 0.003),
        Some("F2"),
    );
    let s = Scenario::new("start", 30.0).motor_start(0.0, "pump", 70.0, 6.0, 0.0);
    let run_with = |band| {
        let d = [ProtectiveDevice::new("F2", "-F2", relay_class10(10.0)).with_band(band)];
        run(&net, &d, &s).trip_time("F2")
    };
    assert!(
        run_with(Band::Fastest).is_some(),
        "4 s edge trips a 6 s start"
    );
    assert!(run_with(Band::Slowest).is_none(), "10 s edge rides through");
}

#[test]
fn an_open_phase_overloads_a_motor_by_root_three() {
    let net = RadialNetwork::new("busbar", 400.0, 50.0, Impedance::new(0.002, 0.009)).segment(
        "pump",
        "busbar",
        Impedance::new(0.02, 0.003),
        Some("F2"),
    );
    let d = [ProtectiveDevice::new("F2", "-F2", relay_class10(10.0))];
    // 9 A is under the 10 A relay; single phased it draws 15.6 A = 1.56 In.
    let ok = Scenario::new("healthy", 600.0).at(
        0.0,
        Action::Load {
            node: "pump".into(),
            amps: 9.0,
        },
    );
    assert!(run(&net, &d, &ok).trip_time("F2").is_none());
    let bad = ok.at(
        10.0,
        Action::Fault {
            id: "OP".into(),
            node: "pump".into(),
            kind: FaultKind::OpenPhase,
            fault_ohm: 0.0,
            inception_deg: None,
        },
    );
    let r = run(&net, &d, &bad);
    let trip = r.trip_time("F2").expect("single phasing trips the relay");
    assert!(trip > 10.0 + 60.0 && trip < 10.0 + 500.0, "{trip}");
}

#[test]
fn closing_a_tripped_breaker_onto_a_persisting_fault_trips_it_again() {
    let devices = [ProtectiveDevice::new("F2", "-F2", c_breaker(16.0))];
    let net = RadialNetwork::new("busbar", 400.0, 50.0, Impedance::new(0.002, 0.009)).segment(
        "pump",
        "busbar",
        Impedance::new(0.02, 0.003),
        Some("F2"),
    );
    let s = Scenario::new("reclose", 2.0)
        .at(
            0.1,
            Action::Fault {
                id: "SC1".into(),
                node: "pump".into(),
                kind: FaultKind::PhaseNeutral,
                fault_ohm: 0.0,
                inception_deg: None,
            },
        )
        .at(
            1.0,
            Action::Close {
                device: "F2".into(),
            },
        );
    let r = run(&net, &devices, &s);
    let trips: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.kind == EventKind::TripShortCircuit)
        .collect();
    assert_eq!(trips.len(), 2, "{:?}", r.events);
    assert!(trips[1].t >= 1.0 && trips[1].t < 1.05);
}

#[test]
fn the_lowest_fault_current_is_phase_neutral_at_the_far_end() {
    let net = network();
    let ik3 = net
        .prospective("pump", FaultKind::ThreePhase, 0.0)
        .unwrap()
        .ik;
    let ik2 = net
        .prospective("pump", FaultKind::PhasePhase, 0.0)
        .unwrap()
        .ik;
    let ik1 = net
        .prospective("pump", FaultKind::PhaseNeutral, 0.0)
        .unwrap()
        .ik;
    assert!((ik2 / ik3 - 3f64.sqrt() / 2.0).abs() < 1e-9);
    assert!(ik1 < ik2 && ik1 < ik3, "{ik1} {ik2} {ik3}");
    let arc = net
        .prospective("pump", FaultKind::ThreePhase, 0.05)
        .unwrap()
        .ik;
    assert!(arc < ik3, "fault resistance lowers the current");
}

#[test]
fn a_scenario_file_round_trips_and_checks_itself() {
    let s = Scenario::new("pump short", 1.0)
        .at(
            0.1,
            Action::Fault {
                id: "SC1".into(),
                node: "pump".into(),
                kind: FaultKind::PhaseEarth,
                fault_ohm: 0.01,
                inception_deg: Some(30.0),
            },
        )
        .motor_start(0.0, "pump", 70.0, 1.0, 8.0);
    let json = s.to_json();
    assert!(json.contains("\"do\": \"fault\"") && json.contains("phase_earth"));
    assert_eq!(Scenario::from_json(&json).unwrap(), s);
    assert!(s.problems().is_empty());
    let bad = Scenario::new("x", 1.0).at(5.0, Action::ClearFault { id: "nope".into() });
    assert_eq!(bad.problems().len(), 2);
    assert!(
        Scenario::from_json("{\"format\": 99, \"name\":\"x\",\"duration\":1,\"events\":[]}")
            .is_err()
    );
}

#[test]
fn the_timing_diagram_shows_the_sequence_and_exports() {
    let devices = [
        ProtectiveDevice::new("Q1", "-Q1", gg_fuse(100.0)).with_opening_time(0.0),
        ProtectiveDevice::new("F2", "-F2", c_breaker(16.0)),
    ];
    let s = Scenario::new("pump short", 0.5).at(
        0.1,
        Action::Fault {
            id: "SC1".into(),
            node: "pump".into(),
            kind: FaultKind::ThreePhase,
            fault_ohm: 0.0,
            inception_deg: None,
        },
    );
    let r = run(&network(), &devices, &s);
    let f2 = r.timing.channel("-F2").unwrap();
    assert_eq!(f2.level_at(0.05), novo_sim::timing::Level::High);
    assert_eq!(f2.level_at(0.5), novo_sim::timing::Level::Low);
    assert!(
        f2.edges
            .iter()
            .any(|e| e.1 == novo_sim::timing::Level::Alarm),
        "tripped but conducting"
    );
    let mut d = r.timing.clone();
    d.analog.push(r.traces.get("i fault SC1").unwrap().clone());
    let svg = d.to_svg(900.0, Some((0.09, 0.16)));
    assert!(svg.starts_with("<svg") && svg.contains("-F2") && svg.contains("<path"));
    let vcd = r.timing.to_vcd();
    assert!(vcd.contains("$var wire 2") && vcd.contains("#100000"));
    let csv = r.traces.to_csv();
    assert!(csv.starts_with("t_s,") && csv.lines().count() > 50);
}

#[test]
fn heat_cools_when_the_current_drops() {
    let d = ProtectiveDevice::new("F", "F", relay_class10(10.0));
    let mut m = ThermalMemory::default();
    m.advance(&d, 72.0, 3.0);
    let hot = m.heat;
    assert!(hot > 0.3);
    m.advance(&d, 1.0, 600.0);
    assert!((m.heat - hot * (-1.0f64).exp()).abs() < 1e-9);
    assert!(m.time_to_trip(&d, 72.0) < (1.0 - hot * 0.3) * 10.0);
}

#[test]
fn trip_models_are_object_safe() {
    let m: Box<dyn TripModel> = Box::new(c_breaker(16.0));
    assert!(m.clears(1000.0) <= 0.02);
    assert!(m.let_through_i2t(1000.0).is_none());
}

#[test]
fn the_example_study_runs_all_its_scenarios_and_round_trips() {
    let study = novo_sim::study::Study::example();
    let again = novo_sim::study::Study::from_json(&study.to_json()).unwrap();
    assert_eq!(again, study);
    let sc = study.run(0).unwrap();
    assert!(sc.faults[0].selective, "the pump breaker clears it alone");
    let low = study.run(1).unwrap();
    assert!(
        low.faults[0].cleared_at.is_some(),
        "magnetic trip is reached: {:?}",
        low.events
    );
    let start = study.run(2).unwrap();
    assert!(start.trip_time("F2").is_none() && start.trip_time("Q1").is_none());
    let phase = study.run(3).unwrap();
    assert!(
        phase.trip_time("K3").is_some() && phase.trip_time("F2").is_none(),
        "{:?}",
        phase.events
    );
    assert!(study.run(9).is_err());
}
