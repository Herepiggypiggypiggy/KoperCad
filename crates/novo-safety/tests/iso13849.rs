use novo_safety::*;

fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

fn sub(cat: Category, years: f64, dc: f64, ccf: u32) -> Subsystem {
    Subsystem {
        name: "t".into(),
        category: cat,
        channel1: vec![Component::new("c", years)],
        channel2: vec![],
        dc_avg_percent: dc,
        ccf_score: ccf,
    }
}

#[test]
fn category_b_matches_table_k1_anchors() {
    // Table K.1 category B: 3 years 3.80E-05, 100 years 1.14E-06.
    let r = sub(Category::B, 3.0, 0.0, 0).calculate();
    assert!(rel(r.pfhd, 3.80e-5) < 0.01, "got {}", r.pfhd);
    let r = sub(Category::B, 100.0, 0.0, 0).calculate();
    assert!(rel(r.pfhd, 1.14e-6) < 0.01, "got {}", r.pfhd);
    assert_eq!(
        r.pl,
        PerformanceLevel::B.min(PerformanceLevel::from_pfhd(r.pfhd))
    );
}

#[test]
fn category_4_high_high_is_about_2_47e_8() {
    let r = sub(Category::C4, 100.0, 99.0, 70).calculate();
    assert!(
        rel(r.pfhd, 2.47e-8) < 0.15,
        "error {:.1}% got {}",
        rel(r.pfhd, 2.47e-8) * 100.0,
        r.pfhd
    );
    assert_eq!(r.pl, PerformanceLevel::E);
}

#[test]
fn pl_bands() {
    assert_eq!(PerformanceLevel::from_pfhd(5e-8), PerformanceLevel::E);
    assert_eq!(PerformanceLevel::from_pfhd(2.9e-7), PerformanceLevel::D);
    assert_eq!(PerformanceLevel::from_pfhd(1.7e-6), PerformanceLevel::C);
    assert_eq!(PerformanceLevel::from_pfhd(5e-6), PerformanceLevel::B);
    assert_eq!(PerformanceLevel::from_pfhd(2e-5), PerformanceLevel::A);
}

#[test]
fn risk_graph_corners() {
    use {Frequency::*, Probability::*, Severity::*};
    assert_eq!(required_pl(S1, F1, P1), PerformanceLevel::A);
    assert_eq!(required_pl(S2, F2, P2), PerformanceLevel::E);
    assert_eq!(required_pl(S2, F1, P1), PerformanceLevel::C);
    assert_eq!(required_pl(S2, F2, P1), PerformanceLevel::D);
}

#[test]
fn b10d_to_mttfd() {
    // 220 days x 16 h at 10 s cycle = 1_267_200 operations per year.
    let nop = operations_per_year(220.0, 16.0, 10.0);
    assert!((nop - 1_267_200.0).abs() < 1e-6);
    let m = mttfd_from_b10d(2_000_000.0, nop);
    assert!((m - 15.78).abs() < 0.01, "got {m}");
}

#[test]
fn channel_mttfd_and_dc_avg() {
    let parts = [Component::new("a", 20.0), Component::new("b", 20.0)];
    assert!((channel_mttfd(&parts) - 10.0).abs() < 1e-9);
    // Equal MTTFd parts: plain average of the DC values.
    assert!((dc_avg(&[(20.0, 99.0), (20.0, 60.0)]) - 79.5).abs() < 1e-9);
    // The weaker part dominates.
    assert!(dc_avg(&[(5.0, 0.0), (100.0, 99.0)]) < 20.0);
}

#[test]
fn symmetrisation_two_channels() {
    let s = Subsystem {
        name: "x".into(),
        category: Category::C3,
        channel1: vec![Component::new("a", 30.0)],
        channel2: vec![Component::new("b", 100.0)],
        dc_avg_percent: 90.0,
        ccf_score: 65,
    };
    let r = s.calculate();
    // 2/3 (30 + 100 - 1/(1/30+1/100)) = 2/3 (130 - 23.08) = 71.28
    assert!(
        (r.mttfd_years - 71.28).abs() < 0.01,
        "got {}",
        r.mttfd_years
    );
}

#[test]
fn category_rules_become_problems() {
    let r = sub(Category::C3, 50.0, 90.0, 50).calculate();
    assert!(r.problems.iter().any(|p| p.message.contains("CCF")));
    assert_eq!(r.pl, PerformanceLevel::None);
    let r = sub(Category::C4, 50.0, 90.0, 70).calculate();
    assert!(r.problems.iter().any(|p| p.message.contains("category 4")));
    let r = sub(Category::B, 2.0, 0.0, 0).calculate();
    assert!(
        r.problems
            .iter()
            .any(|p| p.message.contains("below 3 years"))
    );
}

#[test]
fn category_caps_pl() {
    // Category 1 with very high MTTFd cannot exceed PL c.
    let r = sub(Category::C1, 100.0, 0.0, 0).calculate();
    assert!(r.pl <= PerformanceLevel::C);
    let r = sub(Category::B, 100.0, 0.0, 0).calculate();
    assert!(r.pl <= PerformanceLevel::B);
}

#[test]
fn mttfd_is_capped_at_100_years() {
    let r = sub(Category::B, 400.0, 0.0, 0).calculate();
    assert_eq!(r.mttfd_years, 100.0);
}

#[test]
fn better_diagnostics_never_worsen_pfhd() {
    for cat in [Category::C2, Category::C3] {
        let lo = sub(cat, 50.0, 60.0, 70).calculate().pfhd;
        let mid = sub(cat, 50.0, 90.0, 70).calculate().pfhd;
        let hi = sub(cat, 50.0, 99.0, 70).calculate().pfhd;
        assert!(lo > mid && mid > hi, "{cat:?}: {lo} {mid} {hi}");
    }
}

#[test]
fn series_adds_pfhd_and_limits_pl() {
    let sensor = sub(Category::C3, 100.0, 99.0, 70).calculate();
    let logic = sub(Category::C4, 100.0, 99.0, 70).calculate();
    let actuator = sub(Category::C3, 100.0, 90.0, 70).calculate();
    let c = combine_series(&[sensor.clone(), logic.clone(), actuator.clone()]);
    assert!(rel(c.pfhd, sensor.pfhd + logic.pfhd + actuator.pfhd) < 1e-12);
    let weak = sub(Category::B, 100.0, 0.0, 0).calculate();
    assert!(combine_series(&[logic, weak]).pl <= PerformanceLevel::B);
}

#[test]
fn parallel_beats_either_alone() {
    let a = sub(Category::C1, 100.0, 0.0, 0).calculate();
    let b = sub(Category::C1, 100.0, 0.0, 0).calculate();
    let c = combine_parallel(&a, &b, 0.05);
    assert!(c.pfhd < a.pfhd);
}
