//! The protection study read from a real project: the Gate 1 motor starter
//! (main fuses, contactor, overload relay).

use novo_checks::protection::coordination::Rule;
use novo_checks::protection::devices::MakerTables;
use novo_checks::protection::model::{DeviceType, SUPPLY_SHORT_CIRCUIT_KA, Study};
use novo_checks::protection::selectivity::Settings;
use novo_core::{Device, Document, Value};
use novo_tests::motor_starter;

fn set(doc: &mut Document, device: novo_core::DeviceId, props: &[(&str, Value)]) {
    doc.transact("set properties", |tx| {
        tx.update::<Device>(device, |d| {
            for (k, v) in props {
                d.properties.insert((*k).to_string(), v.clone());
            }
        })
    })
    .unwrap();
}

#[test]
fn the_starter_reads_as_fuse_then_relay_through_the_contactor() {
    let mut doc = Document::default();
    let ms = motor_starter(&mut doc).unwrap();
    set(
        &mut doc,
        ms.f1,
        &[
            ("rated_current_a", Value::Float(25.0)),
            ("breaking_capacity_ka", Value::Float(100.0)),
        ],
    );
    set(
        &mut doc,
        ms.f2,
        &[
            ("rated_current_a", Value::Float(6.3)),
            ("setting_a", Value::Float(5.0)),
            ("trip_class", Value::Text("10".into())),
        ],
    );
    set(&mut doc, ms.k1, &[("sccr_ka", Value::Float(10.0))]);
    doc.transact("supply", |tx| {
        tx.set_info(|i| {
            i.properties
                .insert(SUPPLY_SHORT_CIRCUIT_KA.into(), Value::Float(10.0));
        })
    })
    .unwrap();
    let project = doc.project();
    let study = Study::read(project, &project.netlist());
    let tags: Vec<&str> = study.devices.iter().map(|d| d.tag.as_str()).collect();
    assert_eq!(tags, ["=A1+CAB-F1", "=A1+CAB-F2"], "{tags:?}");
    assert_eq!(study.devices[0].kind, DeviceType::Fuse);
    assert_eq!(study.devices[1].kind, DeviceType::OverloadRelay);
    // F1 feeds F2 through K1.
    assert_eq!(study.devices[0].downstream, vec![1]);
    assert_eq!(study.devices[0].passes.len(), 1);
    assert_eq!(study.devices[0].passes[0].1, Some(10.0));
    assert_eq!(study.chains(), vec![vec![0, 1]]);
    assert_eq!(study.supply_ka, Some(10.0));
    // The relay says what was assumed; the fuse its melting energy.
    assert!(
        study.devices[1].assumed.is_empty(),
        "{:?}",
        study.devices[1].assumed
    );
    assert!(
        study.devices[0]
            .assumed
            .iter()
            .any(|a| a.contains("estimated"))
    );
    // No finding on a 100 kA fuse at 10 kA.
    let report = study.report(&MakerTables::default(), &Settings::default());
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.rule != Rule::BreakingCapacity),
        "{:?}",
        report.findings
    );
    assert_eq!(report.pairs.len(), 1);
    // SCCR: the fuse holder has no marked rating (10 kA default), the
    // contactor is marked 10 kA, the relay defaults to 5 kA: 5 kA.
    let sccr = study.sccr("Gate 1");
    assert_eq!(sccr.panel_sccr_ka, 5.0, "{:?}", sccr.rows);
    assert_eq!(
        sccr.verdict,
        novo_checks::protection::sccr::Verdict::Insufficient
    );
}

#[test]
fn devices_without_a_rating_are_left_out() {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let project = doc.project();
    let study = Study::read(project, &project.netlist());
    assert!(study.devices.is_empty());
    assert_eq!(study.supply_ka, None);
}

#[test]
fn assumptions_are_listed() {
    let mut doc = Document::default();
    let ms = motor_starter(&mut doc).unwrap();
    set(&mut doc, ms.f2, &[("rated_current_a", Value::Float(6.3))]);
    let project = doc.project();
    let study = Study::read(project, &project.netlist());
    let relay = &study.devices[0];
    assert!(
        !relay
            .assumed
            .iter()
            .any(|a| a.contains("breaking capacity"))
    );
    assert!(relay.assumed.iter().any(|a| a.contains("class 10")));
    assert!(relay.assumed.iter().any(|a| a.contains("setting")));
}

#[test]
fn a_limiter_ahead_raises_the_rating_only_when_its_table_and_the_withstand_are_stated() {
    use novo_checks::protection::model::{
        LET_THROUGH, WITHSTAND_I2T, WITHSTAND_PEAK_KA, format_let_through, parse_let_through,
    };
    use novo_checks::protection::sccr::Source;
    let mut doc = Document::default();
    let ms = motor_starter(&mut doc).unwrap();
    set(
        &mut doc,
        ms.f1,
        &[
            ("rated_current_a", Value::Float(25.0)),
            ("sccr_ka", Value::Float(100.0)),
        ],
    );
    set(
        &mut doc,
        ms.f2,
        &[
            ("rated_current_a", Value::Float(6.3)),
            ("trip_class", Value::Text("10".into())),
        ],
    );
    set(&mut doc, ms.k1, &[("sccr_ka", Value::Float(100.0))]);
    let read = |doc: &Document| {
        let p = doc.project();
        Study::read(p, &p.netlist()).sccr("Gate 1")
    };
    // Nothing stated: the relay's default 5 kA decides, as before.
    assert_eq!(read(&doc).panel_sccr_ka, 5.0);
    // A table on the fuse alone is not enough: the relay's withstand is unknown.
    set(
        &mut doc,
        ms.f1,
        &[(
            LET_THROUGH,
            Value::Text("10000:5000:5e4; 100000:20000:4e5".into()),
        )],
    );
    assert_eq!(read(&doc).panel_sccr_ka, 5.0);
    // With the relay's withstand the fuse's let-through holds it up.
    set(
        &mut doc,
        ms.f2,
        &[
            (WITHSTAND_PEAK_KA, Value::Float(25.0)),
            (WITHSTAND_I2T, Value::Float(5.0e5)),
        ],
    );
    let r = read(&doc);
    let relay = r.rows.iter().find(|row| row.name.ends_with("F2")).unwrap();
    assert_eq!(relay.source, Source::CurrentLimiting);
    assert_eq!(relay.effective_ka, 100.0);
    assert_eq!(r.panel_sccr_ka, 100.0);
    // A relay that withstands less than the let-through gets no credit.
    set(&mut doc, ms.f2, &[(WITHSTAND_PEAK_KA, Value::Float(2.0))]);
    assert_eq!(read(&doc).panel_sccr_ka, 5.0);
    // The table text round-trips and bad text is refused.
    let t = parse_let_through("1000:800:5000; 6000:1500:20000").unwrap();
    assert_eq!(parse_let_through(&format_let_through(&t)).unwrap(), t);
    assert!(parse_let_through("1000:800").is_err());
    assert!(parse_let_through("6000:1:1; 1000:1:1").is_err());
    assert!(parse_let_through("a:b:c").is_err());
}
