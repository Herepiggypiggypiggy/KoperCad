//! Magnetic field rules EMC-19 and EMC-20 on plates built by hand. The
//! expected numbers are worked out in the comments beside each case.

use novo_emc::*;

fn run(id: u64, class: EmcClass, y: f64) -> Run {
    Run {
        id,
        name: format!("W{id}"),
        class,
        origin: Origin::Derived,
        class_reason: String::new(),
        cable: None,
        shield: None,
        twisted: false,
        net: Some(id as u32),
        from: Some(1),
        to: Some(2),
        plate: Some(1),
        path: vec![(0.0, y), (600.0, y)],
        ducts: Vec::new(),
        diameter_mm: 3.0,
        drive: None,
        leaves_cabinet: false,
    }
}

fn plc(y: f64) -> Device {
    Device {
        id: 9,
        tag: "-A1".into(),
        role: Role::Sensitive,
        plate: Some(1),
        rect: Some((250.0, y, 60.0, 40.0)),
        filter: FilterState::Unknown,
        filter_for: None,
        wired: true,
        bonded: None,
        height_mm: None,
        on_door: false,
    }
}

fn input(runs: Vec<Run>, devices: Vec<Device>) -> Input {
    Input {
        runs,
        ducts: Vec::new(),
        devices,
        plates: vec![Plate {
            id: 1,
            tag: "+CAB".into(),
            finish: None,
            door_bonded: None,
        }],
        params: Params::default(),
    }
}

fn current(run: u64, amps: f64, phase: f64) -> Current {
    Current {
        run,
        amps,
        phase_deg: phase,
        source: "test".into(),
        declared: true,
        phase_known: true,
    }
}

fn setup(currents: Vec<Current>) -> FieldSetup {
    FieldSetup {
        currents,
        params: FieldParams::default(),
    }
}

#[test]
fn a_lone_wire_beside_a_plc_is_a_finding_with_the_closed_form_field() {
    // 16 A in one wire at y = 100; the PLC's nearest edge at y = 150, the
    // slice 30 mm up and the wire 15 mm up: closest distance sqrt(50^2 + 15^2)
    // = 52.2 mm. A long wire gives mu0 I / (2 pi d) = 2e-7 * 16 / 0.0522 = 61 uT;
    // 600 mm is long enough to be within a few percent of that.
    let i = input(vec![run(1, EmcClass::Motor, 100.0)], vec![plc(150.0)]);
    let report = analyse_with_field(&i, Some(&setup(vec![current(1, 16.0, 0.0)])));
    let f: Vec<_> = report.of_rule("EMC-19").collect();
    assert_eq!(f.len(), 1, "{:#?}", report.findings);
    assert_eq!(f[0].level, Level::Estimate);
    assert_eq!(f[0].severity, Severity::Warning);
    let field = report.field.as_ref().expect("summary");
    let b = field.devices[0].b_ut;
    let closed = 2.0e-7 * 16.0 / 0.0522 * 1.0e6;
    assert!((b - closed).abs() / closed < 0.05, "{b} against {closed}");
    assert!(f[0].message.contains("not a measurement"));
    assert!(matches!(f[0].fixes[0], Fix::MoveDevice { device: 9, .. }));
    // The device is scored for it.
    assert!(report.score_of(Subject::Device(9)).unwrap().points > 0);
}

#[test]
fn three_phases_together_make_a_fraction_of_the_field() {
    let lone = {
        let i = input(vec![run(1, EmcClass::Motor, 100.0)], vec![plc(150.0)]);
        let r = analyse_with_field(&i, Some(&setup(vec![current(1, 16.0, 0.0)])));
        r.field.unwrap().devices[0].b_ut
    };
    let bundle = {
        let runs = vec![
            run(1, EmcClass::Motor, 100.0),
            run(2, EmcClass::Motor, 105.0),
            run(3, EmcClass::Motor, 110.0),
        ];
        let c = vec![
            current(1, 16.0, 0.0),
            current(2, 16.0, -120.0),
            current(3, 16.0, 120.0),
        ];
        let r = analyse_with_field(&input(runs, vec![plc(150.0)]), Some(&setup(c)));
        r.field.unwrap().devices[0].b_ut
    };
    // Spacing 5 mm against about 45 mm: the set falls by roughly s/d.
    assert!(bundle < 0.25 * lone, "{bundle} against {lone}");
}

#[test]
fn a_distant_plc_has_no_warning_and_the_rule_says_so() {
    let i = input(vec![run(1, EmcClass::Motor, 100.0)], vec![plc(380.0)]);
    let report = analyse_with_field(&i, Some(&setup(vec![current(1, 16.0, 0.0)])));
    // 16 A at 280 mm is 11.4 uT for a long wire; a 600 mm wire seen from its
    // middle gives 300/sqrt(300^2 + 280^2) = 0.73 of that, 8.3 uT: under the
    // planning value, over a third of it, so an information line, no warning.
    let b = report.field.as_ref().unwrap().devices[0].b_ut;
    assert!((b - 8.3).abs() < 0.3, "{b}");
    let f: Vec<_> = report.of_rule("EMC-19").collect();
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].severity, Severity::Info);
    // 2 A is far under: nothing to say, and the rule reports that.
    let quiet = analyse_with_field(&i, Some(&setup(vec![current(1, 2.0, 0.0)])));
    assert_eq!(quiet.of_rule("EMC-19").count(), 0);
    let st = quiet.rules.iter().find(|r| r.rule == "EMC-19").unwrap();
    assert_eq!(st.status, Status::NoFinding);
}

#[test]
fn without_currents_the_rules_are_not_evaluated_rather_than_clean() {
    let i = input(
        vec![
            run(1, EmcClass::Motor, 100.0),
            run(2, EmcClass::Analog, 150.0),
        ],
        vec![plc(150.0)],
    );
    for report in [analyse_with_field(&i, Some(&setup(vec![]))), analyse(&i)] {
        for id in ["EMC-19", "EMC-20"] {
            let st = report.rules.iter().find(|r| r.rule == id).unwrap();
            assert_eq!(st.status, Status::NotEvaluated, "{id}");
        }
        assert!(report.findings.iter().all(|f| f.rule != "EMC-19"));
    }
}

#[test]
fn a_signal_wire_beside_a_motor_wire_picks_up_the_mutual_inductance_times_the_step() {
    // Analog wire 50 mm from a 16 A motor wire, 600 mm parallel, 15 mm over
    // the plate, return through the plate. Per metre the mutual inductance is
    // mu0/(4 pi) ln(1 + 4 h^2 / d^2) = 1e-7 ln(1.36) = 30.7 nH/m, so about 18 nH
    // over 0.6 m (a little less at the ends); at 10 A/us that is about 0.18 V.
    // The step uses the plate as a conducting plane.
    let i = input(
        vec![
            run(1, EmcClass::Motor, 100.0),
            run(2, EmcClass::Analog, 150.0),
        ],
        vec![],
    );
    let report = analyse_with_field(&i, Some(&setup(vec![current(1, 16.0, 0.0)])));
    let f: Vec<_> = report.of_rule("EMC-20").collect();
    assert_eq!(f.len(), 1, "{:#?}", report.findings);
    let v = &report.field.as_ref().unwrap().victims[0];
    assert!(
        v.v_edge_mv > 140.0 && v.v_edge_mv < 200.0,
        "{} mV",
        v.v_edge_mv
    );
    // At 50 Hz the plate is free space: the flux per metre is
    // mu0/(2 pi) ln(sqrt(d^2 + h^2)/d) = 2e-7 * 0.0148 = 3.0 nWb/m/A, so over
    // 0.6 m and 16 A about 28 nWb and 2 pi 50 * 28e-9 = 0.009 V... per the
    // numbers: 0.0259 mV after the finite length (a little less).
    assert!(
        v.v_line_mv > 0.018 && v.v_line_mv < 0.030,
        "{} mV",
        v.v_line_mv
    );
    assert_eq!(f[0].severity, Severity::Warning);
    assert!(f[0].subjects.contains(&Subject::Run(1)));
    assert!(!f[0].ribbons.is_empty());
    // Twisting is offered, and moving apart.
    assert!(
        f[0].fixes
            .iter()
            .any(|x| matches!(x, Fix::SeparateRuns { to_mm, .. } if *to_mm == 0.0))
    );
}

#[test]
fn twisting_the_pair_and_moving_away_both_help() {
    let base = |analog_y: f64, twisted: bool| {
        let mut a = run(2, EmcClass::Analog, analog_y);
        a.twisted = twisted;
        let i = input(vec![run(1, EmcClass::Motor, 100.0), a], vec![]);
        let r = analyse_with_field(&i, Some(&setup(vec![current(1, 16.0, 0.0)])));
        r.field.unwrap().victims[0].v_edge_mv
    };
    let plain = base(150.0, false);
    assert!(base(150.0, true) < 0.11 * plain);
    assert!(base(300.0, false) < 0.4 * plain);
}

#[test]
fn a_drawn_return_makes_a_smaller_loop_than_the_plate() {
    // Two analog cores of one cable, 4 mm apart, against one core with the
    // plate as its return: the pair encloses far less flux.
    let mut a = run(2, EmcClass::Analog, 150.0);
    let mut b = run(3, EmcClass::Analog, 154.0);
    a.cable = Some(7);
    b.cable = Some(7);
    let paired = {
        let i = input(vec![run(1, EmcClass::Motor, 100.0), a, b], vec![]);
        let r = analyse_with_field(&i, Some(&setup(vec![current(1, 16.0, 0.0)])));
        let v = r.field.unwrap().victims;
        assert_eq!(v.len(), 1, "one row per pair");
        assert!(v[0].paired);
        v[0].v_edge_mv
    };
    let lone = {
        let i = input(
            vec![
                run(1, EmcClass::Motor, 100.0),
                run(2, EmcClass::Analog, 150.0),
            ],
            vec![],
        );
        let r = analyse_with_field(&i, Some(&setup(vec![current(1, 16.0, 0.0)])));
        r.field.unwrap().victims[0].v_edge_mv
    };
    assert!(paired < 0.2 * lone, "{paired} against {lone}");
}

#[test]
fn the_map_peaks_over_the_wire_and_falls_away() {
    let i = input(vec![run(1, EmcClass::Motor, 200.0)], vec![]);
    let s = setup(vec![current(1, 16.0, 0.0)]);
    let m = field::map(&i, &s, 1, (600.0, 400.0)).expect("a wire carries current");
    assert_eq!(m.b_ut.len(), m.nx * m.ny);
    assert!((m.peak_at.1 - 200.0).abs() <= m.cell_mm, "{:?}", m.peak_at);
    let at = |x: f64, y: f64| m.b_ut[(y / m.cell_mm) as usize * m.nx + (x / m.cell_mm) as usize];
    assert!(at(300.0, 205.0) > 3.0 * at(300.0, 380.0));
    // 16 A at 15 mm (wire 15 mm up, slice 30 mm): 2e-7 * 16 / 0.015 = 213 uT.
    assert!(m.peak_ut > 150.0 && m.peak_ut < 230.0, "{}", m.peak_ut);
    // No current on the plate, no map.
    assert!(field::map(&i, &setup(vec![]), 1, (600.0, 400.0)).is_none());
    assert!(field::map(&i, &s, 2, (600.0, 400.0)).is_none());
}

#[test]
fn the_plate_image_changes_the_numbers_only_when_asked() {
    let i = input(vec![run(1, EmcClass::Motor, 100.0)], vec![plc(150.0)]);
    let mut s = setup(vec![current(1, 16.0, 0.0)]);
    let free = analyse_with_field(&i, Some(&s)).field.unwrap().devices[0].b_ut;
    s.params.plate_image = Some(true);
    let imaged = analyse_with_field(&i, Some(&s)).field.unwrap().devices[0].b_ut;
    assert!(
        imaged < free,
        "the image of a go wire opposes it at 30 mm up"
    );
    s.params.plate_image = None;
    s.params.hz = 5000.0;
    assert!(s.params.image(), "from 1 kHz the plate is a plane");
}

#[test]
fn the_assumptions_name_the_settings_of_the_estimate() {
    let i = input(vec![run(1, EmcClass::Motor, 100.0)], vec![]);
    let r = analyse_with_field(&i, Some(&setup(vec![current(1, 16.0, 0.0)])));
    for p in ["emc.field_hz", "emc.field_limit_ut", "emc.plate_image"] {
        assert!(r.assumptions.iter().any(|a| a.property == p), "{p}");
    }
}

#[test]
fn wires_on_one_line_are_laid_side_by_side_not_on_top_of_each_other() {
    // The router gives the three motor wires of a duct one centre line. Laid
    // out one diameter (3 mm) apart they must give the same field as three
    // wires drawn 3 mm apart; stacked they would cancel to nothing.
    let phases = [0.0, -120.0, 120.0];
    let stacked: Vec<Run> = (1..=3).map(|k| run(k, EmcClass::Motor, 100.0)).collect();
    let c: Vec<Current> = (1..=3)
        .map(|k| current(k, 16.0, phases[(k - 1) as usize]))
        .collect();
    let a = analyse_with_field(&input(stacked, vec![plc(150.0)]), Some(&setup(c.clone())));
    let apart: Vec<Run> = (1..=3)
        .map(|k| run(k, EmcClass::Motor, 97.0 + 3.0 * (k - 1) as f64))
        .collect();
    let b = analyse_with_field(&input(apart, vec![plc(150.0)]), Some(&setup(c)));
    let (fa, fb) = (
        a.field.unwrap().devices[0].b_ut,
        b.field.unwrap().devices[0].b_ut,
    );
    assert!(fa > 0.1, "stacked wires must not cancel exactly: {fa}");
    assert!((fa - fb).abs() / fb < 1e-6, "{fa} against {fb}");
}
