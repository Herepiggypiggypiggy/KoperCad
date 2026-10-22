//! Golden cases for the rule engine, built by hand: every expected number is
//! worked out in the comment beside it.

use novo_emc::*;

fn run(id: u64, class: EmcClass, path: &[(f64, f64)], ducts: &[u64]) -> Run {
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
        path: path.to_vec(),
        ducts: ducts.to_vec(),
        diameter_mm: 3.0,
        drive: None,
        leaves_cabinet: false,
    }
}

/// A motor run in a properly shielded cable, so only the segregation rules apply to it.
fn motor(id: u64, path: &[(f64, f64)], ducts: &[u64]) -> Run {
    let mut r = run(id, EmcClass::Motor, path, ducts);
    r.cable = Some(100 + id);
    r.shield = Some(full());
    r.drive = Some(5);
    r
}

fn duct(id: u64, a: (f64, f64), b: (f64, f64), width: f64) -> Duct {
    Duct {
        id,
        a,
        b,
        width_mm: width,
        divider: false,
    }
}

fn input(runs: Vec<Run>, ducts: Vec<Duct>) -> Input {
    Input {
        runs,
        ducts,
        devices: Vec::new(),
        plates: vec![Plate {
            id: 1,
            tag: "+CAB".into(),
            finish: None,
        }],
        params: Params::default(),
    }
}

fn full() -> Shield {
    Shield {
        a: Termination::Full360,
        b: Termination::Full360,
    }
}

#[test]
fn motor_and_analog_in_one_duct_are_a_rule_finding_with_a_worked_score() {
    // One duct 0..600 mm. Motor-analog need 300 mm; both in the same duct, so
    // distance 0: s = 1; length 600 mm so lf = 0.6; w = sqrt(1.0 * 1.0) = 1.
    // victim = 100 * 1 * (0.25 + 0.75) * (0.3 + 0.7 * 0.6) = 72, aggressor 36.
    let d = duct(7, (0.0, 50.0), (600.0, 50.0), 60.0);
    let m = motor(1, &[(0.0, 50.0), (600.0, 50.0)], &[7]);
    let a = run(2, EmcClass::Analog, &[(0.0, 50.0), (600.0, 50.0)], &[7]);
    let report = analyse(&input(vec![m, a], vec![d]));
    let f: Vec<_> = report.of_rule("EMC-01").collect();
    assert_eq!(f.len(), 1, "{:#?}", report.findings);
    assert_eq!(f[0].level, Level::Fact);
    assert_eq!(
        f[0].points,
        vec![(Subject::Run(2), 72), (Subject::Run(1), 36)]
    );
    assert_eq!(report.score_of(Subject::Run(2)).unwrap().points, 72);
    assert_eq!(report.score_of(Subject::Run(2)).unwrap().band, Band::High);
    assert_eq!(report.score_of(Subject::Run(1)).unwrap().band, Band::Medium);
    // The induced voltage is shown as an estimate with its assumption.
    let v = f[0]
        .metrics
        .iter()
        .find(|m| m.label.starts_with("Induced"))
        .unwrap();
    assert!(v.value.contains("10 A/us"), "{}", v.value);
    // The reasons of a score add up to the score.
    let s = report.score_of(Subject::Run(2)).unwrap();
    assert_eq!(s.reasons.iter().map(|r| r.points).sum::<u32>(), s.points);
}

#[test]
fn two_ducts_apart_use_the_edge_distance() {
    // Ducts 60 mm wide, centre lines 100 mm apart: edge distance 40 mm.
    // Power-control need 100 mm, so 500 mm of parallel run is short by 60 %.
    let d1 = duct(1, (0.0, 0.0), (500.0, 0.0), 60.0);
    let d2 = duct(2, (0.0, 100.0), (500.0, 100.0), 60.0);
    let p = run(1, EmcClass::Power, &[(0.0, 0.0), (500.0, 0.0)], &[1]);
    let c = run(2, EmcClass::Control, &[(0.0, 100.0), (500.0, 100.0)], &[2]);
    let report = analyse(&input(vec![p, c], vec![d1, d2]));
    let f: Vec<_> = report.of_rule("EMC-02").collect();
    assert_eq!(f.len(), 1, "{:#?}", report.findings);
    assert_eq!(f[0].level, Level::Index);
    let length = f[0]
        .metrics
        .iter()
        .find(|m| m.label == "Parallel length")
        .unwrap();
    assert_eq!(length.value, "500 mm");
    let dist = f[0]
        .metrics
        .iter()
        .find(|m| m.label == "Closest distance")
        .unwrap();
    assert_eq!(dist.value, "40 mm");
    // w = sqrt(0.7 * 0.4) = 0.529; s = (100 - 40) / 100 = 0.6; lf = 0.5:
    // 100 * 0.529 * (0.25 + 0.45) * (0.3 + 0.35) = 24.1 -> 24, info (< 25).
    assert_eq!(f[0].points[0], (Subject::Run(2), 24));
    assert_eq!(f[0].severity, Severity::Info);
}

#[test]
fn far_enough_short_or_crossing_runs_are_quiet() {
    let d1 = duct(1, (0.0, 0.0), (500.0, 0.0), 60.0);
    let d2 = duct(2, (0.0, 300.0), (500.0, 300.0), 60.0);
    // 240 mm edge distance, power-control needs 100.
    let far = analyse(&input(
        vec![
            run(1, EmcClass::Power, &[(0.0, 0.0), (500.0, 0.0)], &[1]),
            run(2, EmcClass::Control, &[(0.0, 300.0), (500.0, 300.0)], &[2]),
        ],
        vec![d1.clone(), d2],
    ));
    assert!(far.findings.is_empty(), "{:#?}", far.findings);
    // Only 200 mm side by side: under the 300 mm reporting limit.
    let short = analyse(&input(
        vec![
            motor(1, &[(0.0, 0.0), (200.0, 0.0)], &[]),
            run(2, EmcClass::Analog, &[(0.0, 20.0), (200.0, 20.0)], &[]),
        ],
        vec![],
    ));
    assert!(short.findings.is_empty(), "{:#?}", short.findings);
    // A crossing at 90 degrees has no parallel stretch.
    let cross = analyse(&input(
        vec![
            motor(1, &[(0.0, 100.0), (600.0, 100.0)], &[]),
            run(2, EmcClass::Analog, &[(300.0, 0.0), (300.0, 600.0)], &[]),
        ],
        vec![],
    ));
    assert!(cross.findings.is_empty(), "{:#?}", cross.findings);
    let _ = d1;
}

#[test]
fn a_full_shield_and_a_twisted_pair_buy_distance_back() {
    // Motor-analog need 300. Free wires 3 mm thick lying 100 mm apart centre to
    // centre: 97 mm edge to edge, so an unshielded pair is flagged.
    let mut m = motor(1, &[(0.0, 0.0), (600.0, 0.0)], &[]);
    m.shield = None;
    let mut a = run(2, EmcClass::Analog, &[(0.0, 100.0), (600.0, 100.0)], &[]);
    let n = |m: &Run, a: &Run| {
        analyse(&input(vec![m.clone(), a.clone()], vec![]))
            .of_rule("EMC-02")
            .count()
    };
    assert_eq!(n(&m, &a), 1);
    // Victim shielded all round at both ends: 300 x 0.25 = 75 mm needed.
    a.shield = Some(full());
    assert_eq!(n(&m, &a), 0);
    // A shield with unknown ends earns nothing.
    a.shield = Some(Shield {
        a: Termination::Unknown,
        b: Termination::Unknown,
    });
    assert_eq!(n(&m, &a), 1);
    // Twisting alone: 300 x 0.8 = 240 still flagged at 97 mm.
    a.shield = None;
    a.twisted = true;
    assert_eq!(n(&m, &a), 1);
    // Both: 300 x 0.25 x 0.8 = 60 mm, met.
    a.shield = Some(full());
    assert_eq!(n(&m, &a), 0);
}

#[test]
fn a_metal_divider_counts_as_a_gap() {
    let mut d = duct(7, (0.0, 0.0), (600.0, 0.0), 60.0);
    let m = motor(1, &[(0.0, 0.0), (600.0, 0.0)], &[7]);
    let a = run(2, EmcClass::Analog, &[(0.0, 0.0), (600.0, 0.0)], &[7]);
    assert_eq!(
        analyse(&input(vec![m.clone(), a.clone()], vec![d.clone()]))
            .of_rule("EMC-01")
            .count(),
        1
    );
    d.divider = true;
    // With the divider the pair is judged as 50 mm apart: an EMC-02 index, not a shared duct.
    let r = analyse(&input(vec![m, a], vec![d]));
    assert_eq!(r.of_rule("EMC-01").count(), 0);
    assert_eq!(r.of_rule("EMC-02").count(), 1, "{:#?}", r.findings);
}

#[test]
fn classes_in_one_cable_are_flagged_and_safety_with_power_is_an_error() {
    let mut p = run(1, EmcClass::Power, &[], &[]);
    let mut s = run(2, EmcClass::Safety, &[], &[]);
    p.cable = Some(9);
    s.cable = Some(9);
    let r = analyse(&input(vec![p.clone(), s], vec![]));
    let f: Vec<_> = r.of_rule("EMC-01").collect();
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].severity, Severity::Error);
    assert!(f[0].subjects.contains(&Subject::Cable(9)));
    let mut c = run(3, EmcClass::Control, &[], &[]);
    c.cable = Some(9);
    let r = analyse(&input(vec![p, c], vec![]));
    assert_eq!(
        r.of_rule("EMC-01").next().unwrap().severity,
        Severity::Warning
    );
}

#[test]
fn motor_cable_rules() {
    let mut m = run(1, EmcClass::Motor, &[], &[]);
    m.drive = Some(5);
    // A bare wire, then a cable without a shield, then each shield state.
    let r = analyse(&input(vec![m.clone()], vec![]));
    assert_eq!(r.of_rule("EMC-04").next().unwrap().fixes, vec![]);
    m.cable = Some(3);
    let r = analyse(&input(vec![m.clone()], vec![]));
    assert_eq!(
        r.of_rule("EMC-04").next().unwrap().fixes,
        vec![Fix::AddShield { cable: 3 }]
    );
    m.shield = Some(full());
    assert_eq!(
        analyse(&input(vec![m.clone()], vec![]))
            .of_rule("EMC-04")
            .count(),
        0
    );
    m.shield = Some(Shield {
        a: Termination::Full360,
        b: Termination::Open,
    });
    let f = analyse(&input(vec![m.clone()], vec![]));
    let f = f.of_rule("EMC-04").next().unwrap().clone();
    assert!(f.message.contains("not connected"), "{}", f.message);
    assert_eq!(f.fixes, vec![Fix::Terminate360 { cable: 3 }]);
    m.shield = Some(Shield {
        a: Termination::Unknown,
        b: Termination::Full360,
    });
    assert_eq!(
        analyse(&input(vec![m], vec![]))
            .of_rule("EMC-04")
            .next()
            .unwrap()
            .severity,
        Severity::Info
    );
}

#[test]
fn pigtail_impedance_is_the_rule_of_thumb() {
    // 80 mm at 1 nH/mm = 80 nH; 2 pi * 10 MHz * 80 nH = 5.03 ohm.
    let z = geometry::pigtail_ohm(80.0, 10.0e6);
    assert!((z - 5.0265).abs() < 1e-3, "{z}");
    let mut m = run(1, EmcClass::Analog, &[], &[]);
    m.cable = Some(3);
    m.shield = Some(Shield {
        a: Termination::Pigtail(80.0),
        b: Termination::Full360,
    });
    let r = analyse(&input(vec![m.clone()], vec![]));
    let f = r.of_rule("EMC-05").next().unwrap();
    assert_eq!(f.level, Level::Estimate);
    assert!(
        f.metrics.iter().any(|m| m.value.starts_with("5.0 ohm")),
        "{:?}",
        f.metrics
    );
    m.shield = Some(Shield {
        a: Termination::Pigtail(40.0),
        b: Termination::Full360,
    });
    assert_eq!(
        analyse(&input(vec![m], vec![])).of_rule("EMC-05").count(),
        0
    );
}

#[test]
fn mutual_inductance_matches_the_image_method_closed_form() {
    // mu0/(4 pi) ln(1 + 4 h^2 / d^2) with h = 15 mm, d = 3 mm: 1e-7 * ln(101).
    let m = geometry::mutual_per_m(15.0, 3.0, 3.0);
    assert!((m - 1.0e-7 * 101f64.ln()).abs() < 1e-12, "{m}");
    // Falls with distance.
    assert!(geometry::mutual_per_m(15.0, 30.0, 3.0) < m);
    assert!(geometry::mutual_per_m(15.0, 100.0, 3.0) < geometry::mutual_per_m(15.0, 30.0, 3.0));
}

#[test]
fn loop_area_is_the_enclosed_polygon() {
    let go = [(0.0, 0.0), (200.0, 0.0)];
    let back = [(0.0, 100.0), (200.0, 100.0)];
    // 200 x 100 mm = 20000 mm2 = 200 cm2, whichever way the return is drawn.
    assert!((geometry::loop_area_cm2(&go, &back) - 200.0).abs() < 1e-9);
    let back_rev = [(200.0, 100.0), (0.0, 100.0)];
    assert!((geometry::loop_area_cm2(&go, &back_rev) - 200.0).abs() < 1e-9);
    let mut a = run(1, EmcClass::Analog, &go, &[]);
    let mut b = run(2, EmcClass::Analog, &back, &[]);
    a.from = Some(1);
    a.to = Some(2);
    b.from = Some(2);
    b.to = Some(1);
    let r = analyse(&input(vec![a.clone(), b.clone()], vec![]));
    let f = r.of_rule("EMC-09").next().unwrap();
    assert_eq!(f.level, Level::Index);
    assert_eq!(f.metrics[0].value, "200 cm2");
    // The same pair in one cable is a twisted bundle: not judged.
    a.cable = Some(4);
    b.cable = Some(4);
    assert_eq!(
        analyse(&input(vec![a, b], vec![]))
            .of_rule("EMC-09")
            .count(),
        0
    );
}

fn dev(id: u64, tag: &str, role: Role, rect: (f64, f64, f64, f64)) -> Device {
    Device {
        id,
        tag: tag.into(),
        role,
        plate: Some(1),
        rect: Some(rect),
        filter: FilterState::Unknown,
        filter_for: None,
        bonded: None,
    }
}

#[test]
fn drive_filter_rules() {
    let mut i = input(vec![], vec![]);
    i.devices = vec![dev(1, "-T1", Role::Drive, (0.0, 0.0, 100.0, 200.0))];
    let r = analyse(&i);
    let f = r.of_rule("EMC-07").next().unwrap();
    assert_eq!(f.fixes, vec![Fix::AddFilter { drive: 1 }]);
    assert_eq!(r.score_of(Subject::Device(1)).unwrap().points, 30);
    // A filter 150 mm away is fine, 400 mm away is flagged.
    i.devices
        .push(dev(2, "-Z1", Role::Filter, (250.0, 0.0, 80.0, 100.0)));
    assert_eq!(analyse(&i).of_rule("EMC-07").count(), 0);
    i.devices[1].rect = Some((500.0, 0.0, 80.0, 100.0));
    let r = analyse(&i);
    let f = r.of_rule("EMC-07").next().unwrap();
    assert!(f.message.contains("400 mm"), "{}", f.message);
    // Built in, or stated: nothing to find.
    i.devices.truncate(1);
    i.devices[0].filter = FilterState::Built;
    assert_eq!(analyse(&i).of_rule("EMC-07").count(), 0);
}

#[test]
fn painted_plate_and_close_sensitive_device() {
    let mut i = input(vec![], vec![]);
    i.devices = vec![
        dev(1, "-T1", Role::Drive, (0.0, 0.0, 100.0, 200.0)),
        dev(2, "-A1", Role::Sensitive, (150.0, 0.0, 100.0, 100.0)),
    ];
    i.devices[0].filter = FilterState::Built;
    // 50 mm gap against 100 mm: flagged; a plate with no stated finish is not.
    let r = analyse(&i);
    assert_eq!(r.of_rule("EMC-17").count(), 1);
    assert_eq!(r.of_rule("EMC-11").count(), 0);
    assert_eq!(
        r.rules.iter().find(|s| s.rule == "EMC-11").unwrap().status,
        Status::NotEvaluated
    );
    i.plates[0].finish = Some(Finish::Painted);
    let r = analyse(&i);
    assert_eq!(r.of_rule("EMC-11").count(), 1);
    assert_eq!(
        r.rules.iter().find(|s| s.rule == "EMC-11").unwrap().status,
        Status::Finding
    );
    assert_eq!(r.score_of(Subject::Plate(1)).unwrap().points, 20);
    i.plates[0].finish = Some(Finish::Conductive);
    assert_eq!(analyse(&i).of_rule("EMC-11").count(), 0);
}

#[test]
fn an_unshielded_signal_leaving_the_cabinet_is_flagged() {
    let mut a = run(1, EmcClass::Fieldbus, &[], &[]);
    a.leaves_cabinet = true;
    a.cable = Some(2);
    let r = analyse(&input(vec![a.clone()], vec![]));
    assert_eq!(
        r.of_rule("EMC-14").next().unwrap().fixes,
        vec![Fix::AddShield { cable: 2 }]
    );
    a.shield = Some(full());
    assert_eq!(
        analyse(&input(vec![a], vec![])).of_rule("EMC-14").count(),
        0
    );
}

#[test]
fn scores_cap_at_100_and_nets_take_the_worst_run() {
    let m = motor(1, &[(0.0, 0.0), (1000.0, 0.0)], &[]);
    let mut runs = vec![m];
    for k in 0..5u64 {
        runs.push(run(
            10 + k,
            EmcClass::Analog,
            &[
                (0.0, 10.0 * (k + 1) as f64),
                (1000.0, 10.0 * (k + 1) as f64),
            ],
            &[],
        ));
    }
    let r = analyse(&input(runs, vec![]));
    let motor = r.score_of(Subject::Run(1)).unwrap();
    assert!(motor.points <= 100);
    assert!(motor.points > 0);
    let net1 = r.nets.iter().find(|n| n.0 == 1).unwrap();
    assert_eq!(net1.1.points, motor.points);
    // A run with nothing against it reads "No findings".
    let q = analyse(&input(vec![run(1, EmcClass::Control, &[], &[])], vec![]));
    assert_eq!(q.score_of(Subject::Run(1)).unwrap().band, Band::None);
}

#[test]
fn the_catalogue_has_unique_ids_and_every_rule_reports_a_status() {
    let ids: std::collections::BTreeSet<_> = RULES.iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), RULES.len());
    let r = analyse(&input(vec![], vec![]));
    for rule in RULES {
        let s = r.rules.iter().find(|s| s.rule == rule.id);
        assert!(s.is_some(), "{} has no status", rule.id);
    }
    assert!(NOT_CHECKED.iter().any(|t| t.contains("Compliance")));
}

#[test]
fn the_picture_carries_ribbons_loops_zones_and_the_wording_rule() {
    let d = duct(7, (0.0, 50.0), (600.0, 50.0), 60.0);
    let m = motor(1, &[(0.0, 50.0), (600.0, 50.0)], &[7]);
    let a = run(2, EmcClass::Analog, &[(0.0, 50.0), (600.0, 50.0)], &[7]);
    let mut i = input(vec![m, a], vec![d]);
    i.devices = vec![dev(1, "-T1", Role::Drive, (0.0, 0.0, 100.0, 200.0))];
    let report = analyse(&i);
    let pic = picture(&i, &report);
    assert_eq!(pic.note, "Design aid, not a test result.");
    let pair = pic.pairs.iter().find(|p| p.rule == "EMC-01").unwrap();
    assert_eq!((pair.a, pair.b), (2, 1));
    assert_eq!(pair.ribbons.len(), 1);
    assert_eq!(pair.ribbons[0].a, ((0.0, 50.0), (600.0, 50.0)));
    assert_eq!(pair.ribbons[0].length_mm, 600.0);
    assert_eq!(pic.runs.iter().find(|r| r.id == 2).unwrap().score, 72);
    assert_eq!(pic.zones.len(), 1);
    assert!(pic.assumptions.iter().all(|a| !a.user_set));
    assert!(pic.not_checked.iter().any(|t| t.contains("Compliance")));
    // The estimate carries its band.
    let hot = pic.hotspots.iter().find(|h| h.rule == "EMC-01").unwrap();
    assert_eq!(hot.band_hz, Some((0.0, 30.0e6)));
    assert!(hot.source.contains("not verified"));
    assert!(RULES.iter().all(|r| r.verify));
}

#[test]
fn a_loop_finding_carries_its_polygon_and_no_estimate_is_given_above_the_band() {
    let go = [(0.0, 0.0), (200.0, 0.0)];
    let back = [(0.0, 100.0), (200.0, 100.0)];
    let a = run(1, EmcClass::Analog, &go, &[]);
    let mut b = run(2, EmcClass::Analog, &back, &[]);
    b.from = Some(2);
    b.to = Some(1);
    let i = input(vec![a, b], vec![]);
    let report = analyse(&i);
    let pic = picture(&i, &report);
    assert_eq!(pic.loops.len(), 1);
    assert!((pic.loops[0].area_cm2 - 200.0).abs() < 1e-9);
    // 100 MHz is above the band: no pigtail number at all.
    let mut c = run(3, EmcClass::Analog, &[], &[]);
    c.cable = Some(9);
    c.shield = Some(Shield {
        a: Termination::Pigtail(80.0),
        b: Termination::Full360,
    });
    let mut j = input(vec![c], vec![]);
    assert_eq!(analyse(&j).of_rule("EMC-05").count(), 1);
    j.params.frequency_hz = 100.0e6;
    let r = analyse(&j);
    assert_eq!(r.of_rule("EMC-05").count(), 0);
    assert_eq!(
        r.rules.iter().find(|s| s.rule == "EMC-05").unwrap().status,
        Status::NotEvaluated
    );
}
