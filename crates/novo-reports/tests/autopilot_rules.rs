//! The project's wire rules steer the Autopilot, and the plan groups by
//! protective device.

use std::collections::BTreeSet;

use novo_checks::calc::tables::TableSet;
use novo_core::{Document, Value};
use novo_reports::autopilot::{Basis, Change, groups, plan_for, rule_breaks};
use novo_reports::wire_rules::{self, ColourClass, Preset, WireRules};
use novo_tests::motor_starter;

fn doc() -> Document {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).expect("fixture builds");
    doc.transact("ratings", |tx| {
        tx.update::<novo_core::Device>(h.f1, |d| {
            d.properties
                .insert("rated_current_a".into(), Value::Float(16.0));
        })
    })
    .expect("rating");
    doc
}

fn set(doc: &mut Document, key: &str, value: Value) {
    doc.transact("rule", |tx| {
        novo_checks::standards::set_info_property(tx, key, Some(value))
    })
    .expect("set");
}

fn colours(doc: &Document) -> BTreeSet<String> {
    plan_for(doc.project(), &TableSet::builtin())
        .proposals
        .iter()
        .filter_map(|p| match &p.change {
            Change::Colour(c) => Some(c.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_project_without_rules_follows_iec_60204_1() {
    let d = doc();
    assert_eq!(WireRules::of(d.project()).preset, Preset::Iec60204);
    assert!(!colours(&d).is_empty());
}

#[test]
fn a_house_colour_replaces_the_standard_and_the_reason_names_both() {
    let mut d = doc();
    let before = plan_for(d.project(), &TableSet::builtin());
    let class = before
        .proposals
        .iter()
        .find(|p| matches!(&p.change, Change::Colour(c) if c == "BK"))
        .expect("a black power wire in the fixture");
    let id = class.id;
    set(&mut d, wire_rules::PRESET, Value::Text("house".into()));
    set(
        &mut d,
        &ColourClass::AcPower.key(),
        Value::Text("brown".into()), // not a code: ignored
    );
    assert_eq!(
        colours(&d),
        colours(&doc()),
        "an unknown code changes nothing"
    );
    set(
        &mut d,
        &ColourClass::AcPower.key(),
        Value::Text("bn".into()),
    );
    let after = plan_for(d.project(), &TableSet::builtin());
    let row = after
        .proposals
        .iter()
        .find(|p| p.id == id)
        .expect("same row");
    assert_eq!(row.change, Change::Colour("BN".into()));
    assert!(row.reason.contains("house rule"), "{}", row.reason);
    assert!(
        row.reason.contains("IEC 60204-1 would give BK"),
        "{}",
        row.reason
    );
}

#[test]
fn the_iec_preset_ignores_stored_house_values() {
    let mut d = doc();
    set(
        &mut d,
        &ColourClass::AcPower.key(),
        Value::Text("BN".into()),
    );
    assert_eq!(colours(&d), colours(&doc()));
}

#[test]
fn a_house_minimum_raises_small_sections_and_says_so() {
    let mut d = doc();
    set(&mut d, wire_rules::PRESET, Value::Text("house".into()));
    set(&mut d, wire_rules::MIN_CONTROL_MM2, Value::Float(1.5));
    let plan = plan_for(d.project(), &TableSet::builtin());
    let sizes: Vec<_> = plan
        .proposals
        .iter()
        .filter_map(|p| match p.change {
            Change::Size(s) => Some((s, &p.reason)),
            _ => None,
        })
        .collect();
    assert!(!sizes.is_empty());
    assert!(sizes.iter().all(|(s, _)| *s >= 1.5 - 1e-9), "{sizes:?}");
    assert!(sizes.iter().any(|(_, r)| r.contains("house minimum")));
}

#[test]
fn the_plan_is_grouped_by_protective_device_and_covers_every_row_once() {
    let d = doc();
    let plan = plan_for(d.project(), &TableSet::builtin());
    let gs = groups(d.project(), &plan);
    assert!(gs.len() >= 2, "{gs:?}");
    let mut all: Vec<usize> = gs.iter().flat_map(|g| g.ids.clone()).collect();
    all.sort();
    let want: Vec<usize> = plan.proposals.iter().map(|p| p.id).collect();
    assert_eq!(all, want);
    assert!(
        gs.iter()
            .any(|g| g.title.starts_with("-F") && g.title.contains(" A)")),
        "{gs:?}"
    );
    assert!(gs.iter().all(|g| !g.summary.is_empty()));
}

#[test]
fn protective_earth_stays_green_yellow_whatever_the_project_stores() {
    let mut d = doc();
    set(&mut d, wire_rules::PRESET, Value::Text("house".into()));
    set(
        &mut d,
        &ColourClass::ProtectiveEarth.key(),
        Value::Text("BK".into()),
    );
    set(&mut d, &ColourClass::Pen.key(), Value::Text("BK".into()));
    let rules = WireRules::of(d.project());
    assert_eq!(rules.house_colour(ColourClass::ProtectiveEarth), None);
    assert_eq!(rules.house_colour(ColourClass::Pen), None);
    assert!(colours(&d).contains("GNYE"));
}

#[test]
fn an_absurd_minimum_section_is_ignored_as_unset() {
    let mut d = doc();
    set(&mut d, wire_rules::PRESET, Value::Text("house".into()));
    for bad in [1000.0, 1e9, 0.01, 5.0] {
        set(&mut d, wire_rules::MIN_POWER_MM2, Value::Float(bad));
        set(&mut d, wire_rules::MIN_CONTROL_MM2, Value::Float(bad));
        let rules = WireRules::of(d.project());
        assert_eq!(rules.min_mm2(false), None, "{bad}");
        assert_eq!(rules.min_mm2(true), None, "{bad}");
    }
    set(&mut d, wire_rules::MIN_POWER_MM2, Value::Float(2.5));
    assert_eq!(WireRules::of(d.project()).min_mm2(false), Some(2.5));
}

fn accept_all(doc: &mut Document) {
    let plan = plan_for(doc.project(), &TableSet::builtin());
    doc.transact("Autopilot", |tx| plan.apply(tx, None))
        .expect("apply");
}

#[test]
fn wires_that_break_the_house_rules_are_listed_and_the_standard_is_not_checked() {
    let mut d = doc();
    assert!(rule_breaks(d.project()).is_empty(), "nothing set yet");
    accept_all(&mut d);
    assert!(rule_breaks(d.project()).is_empty(), "IEC states no rules");

    set(&mut d, wire_rules::PRESET, Value::Text("house".into()));
    assert!(
        rule_breaks(d.project()).is_empty(),
        "a house set without entries breaks nothing"
    );
    set(
        &mut d,
        &ColourClass::AcPower.key(),
        Value::Text("RD".into()),
    );
    set(&mut d, wire_rules::MIN_POWER_MM2, Value::Float(4.0));
    let breaks = rule_breaks(d.project());
    assert!(
        breaks
            .iter()
            .any(|b| b.message.contains("house rule gives RD")),
        "{breaks:?}"
    );
    assert!(
        breaks
            .iter()
            .any(|b| b.message.contains("under the house minimum of 4 mm²")),
        "{breaks:?}"
    );
}

#[test]
fn a_wire_whose_class_was_a_guess_is_not_reported() {
    let mut d = doc();
    let before = plan_for(d.project(), &TableSet::builtin());
    let guessed: Vec<_> = before
        .proposals
        .iter()
        .filter(|p| matches!(p.change, Change::Colour(_)) && p.basis == Basis::Assumed)
        .flat_map(|p| p.wires.iter().copied())
        .collect();
    accept_all(&mut d);
    set(&mut d, wire_rules::PRESET, Value::Text("house".into()));
    for class in ColourClass::EDITABLE {
        set(&mut d, &class.key(), Value::Text("VT".into()));
    }
    let breaks = rule_breaks(d.project());
    assert!(!breaks.is_empty(), "sure classes are still reported");
    for w in guessed {
        assert!(
            !breaks
                .iter()
                .any(|b| b.wire == w && b.message.contains("house rule gives")),
            "wire {w:?} has a guessed class"
        );
    }
}
