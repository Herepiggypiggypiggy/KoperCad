//! Heat-aware Place all on the made-up control cabinet of
//! `novo_tests::heat_cabinet`, judged by the 2D thermal model (planning
//! values, not validated against measurements).

use novo_core::{Command, Document, Rail, StructureId};
use novo_layout::model::{apply_plan, load};
use novo_layout::rules::{RuleSet, model::specs, place_all_explained};
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};
use novo_thermal::{Options, simulate_steady_with};

fn cabinet(doc: &Document) -> StructureId {
    doc.project()
        .all::<novo_core::Structure>()
        .find(|s| s.enclosure)
        .expect("a cabinet")
        .id
}

fn place(doc: &mut Document, cab: StructureId, rules: &RuleSet) -> novo_layout::rules::Explanation {
    let layout = load(doc.project(), cab).expect("plate");
    let (plan, why) = place_all_explained(&layout.plate, &specs(doc.project(), cab), rules, 1, 0.0);
    assert!(plan.unplaced.is_empty(), "every device fits");
    doc.transact("place", |tx| apply_plan(tx, cab, &plan))
        .expect("apply");
    why
}

/// Devices over their limit and the tightest margin, K.
fn margins(doc: &Document, cab: StructureId) -> (usize, f64) {
    let bridge = scenario_from_project(doc.project(), cab, &BridgeOptions::default()).unwrap();
    let r = simulate_steady_with(
        &bridge.scenario,
        Options {
            bands: false,
            advice: false,
        },
    )
    .unwrap();
    let m: Vec<f64> = r.devices.iter().filter_map(|d| d.margin_k).collect();
    (
        m.iter().filter(|m| **m < 0.0).count(),
        m.iter().copied().fold(f64::INFINITY, f64::min),
    )
}

fn clear(doc: &mut Document, cab: StructureId) {
    let rails: Vec<_> = doc
        .project()
        .all::<Rail>()
        .filter(|r| r.structure == cab)
        .map(|r| r.id)
        .collect();
    let placed: Vec<u64> = load(doc.project(), cab)
        .unwrap()
        .placements
        .iter()
        .map(|p| p.id)
        .collect();
    doc.transact("clear", |tx| {
        for id in placed {
            novo_layout::model::unplace_device(tx, id)?;
        }
        for rail in rails {
            tx.execute(Command::DeleteRail { rail })?;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn heat_aware_place_all_beats_the_rules_alone_in_the_2d_model() {
    let mut doc = novo_tests::heat_cabinet::heat_cabinet();
    let cab = cabinet(&doc);
    let mut off = RuleSet::standard();
    off.heat = None;
    place(&mut doc, cab, &off);
    let (over_rules, tight_rules) = margins(&doc, cab);

    clear(&mut doc, cab);
    let why = place(&mut doc, cab, &RuleSet::standard());
    let (over_heat, tight_heat) = margins(&doc, cab);

    let summary = why.heat.expect("the heat step ran");
    assert!(!summary.unchanged && summary.gain() > 0.3, "{summary:?}");
    assert!(
        over_rules >= 5,
        "the rules alone leave devices over: {over_rules}"
    );
    assert!(
        over_heat < over_rules && over_heat == 0,
        "over limit: {over_rules} with the rules, {over_heat} heat-aware"
    );
    assert!(
        tight_heat > tight_rules + 3.0,
        "tightest margin {tight_rules:.1} K to {tight_heat:.1} K"
    );
    eprintln!(
        "rules only: {over_rules} over, tightest {tight_rules:+.1} K; heat-aware: {over_heat} over, {tight_heat:+.1} K"
    );
}

#[test]
fn heat_aware_place_all_is_deterministic_and_keeps_every_device() {
    let doc = novo_tests::heat_cabinet::heat_cabinet();
    let cab = cabinet(&doc);
    let layout = load(doc.project(), cab).unwrap();
    let s = specs(doc.project(), cab);
    let rules = RuleSet::standard();
    let a = place_all_explained(&layout.plate, &s, &rules, 1, 0.0);
    let b = place_all_explained(&layout.plate, &s, &rules, 1, 0.0);
    assert_eq!(a.0, b.0);
    assert_eq!(a.0.placements.len(), s.len());
    // Turned off, nothing about the plan or the explanation mentions heat.
    let mut off = rules.clone();
    off.heat = None;
    let plain = place_all_explained(&layout.plate, &s, &off, 1, 0.0);
    assert!(plain.1.heat.is_none());
    assert!(plain.1.decisions.iter().all(|d| d.heat.is_none()));
}
