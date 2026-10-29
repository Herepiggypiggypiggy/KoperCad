//! Kitting against the Gate 1 motor starter in a cabinet.

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, StructureId};
use novo_reports::kitting::cutting::Source;
use novo_reports::kitting::{KitSettings, Phase, kit_plan};
use novo_reports::{Kitting, Report, ReportOutput};
use novo_reports::{WireDefaults, apply_wire_defaults, bench_jobs};
use novo_tests::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let handles = motor_starter(&mut doc).expect("fixture builds");
    (doc, handles)
}

fn cabinet(doc: &mut Document, h: &MotorStarter, tag: &str, rails: &[f64]) -> StructureId {
    doc.transact("Cabinet", |tx| {
        let cab = tx.add_structure(DeviceTag::parse(tag)?, "Cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 1200.0,
                height: 800.0,
                margin: 10.0,
            }),
        )?;
        let mut first = None;
        for (i, len) in rails.iter().enumerate() {
            let r = tx.add_rail(cab, 50.0, 100.0 + 120.0 * i as f64, *len, RailKind::DinRail)?;
            first.get_or_insert(r);
        }
        let duct = tx.add_rail(cab, 50.0, 700.0, 900.0, RailKind::Duct)?;
        tx.set_rail_size(duct, 900.0, 50.0, 60.0)?;
        if let Some(r1) = first {
            tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, Some((r1, 50.0)))?;
            tx.place_device(
                h.k1,
                cab,
                Point::new(200.0, 100.0),
                false,
                Some((r1, 150.0)),
            )?;
        }
        Ok(cab)
    })
    .expect("layout")
    .0
}

fn with_settings(doc: &mut Document, f: impl FnOnce(&mut KitSettings)) -> KitSettings {
    let mut s = KitSettings::of(doc.project());
    f(&mut s);
    doc.transact("Kitting settings", |tx| KitSettings::store(tx, &s))
        .expect("stored");
    s
}

fn wired(doc: &mut Document) {
    doc.transact("Sizes", |tx| {
        apply_wire_defaults(
            tx,
            &WireDefaults {
                size_mm2: Some(1.5),
                colour: Some("BK".into()),
                fill_colours: true,
            },
        )
    })
    .expect("defaults");
}

fn row_with(out: &ReportOutput, key: &str, text: &str) -> Vec<String> {
    out.section(key)
        .unwrap_or_else(|| panic!("no section {key}"))
        .text_rows()
        .into_iter()
        .find(|r| r.iter().any(|c| c.contains(text)))
        .unwrap_or_else(|| panic!("no row with {text} in {key}"))
}

#[test]
fn rails_of_three_lengths_share_bars_and_the_offcut_is_named() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0, 600.0, 700.0]);
    let plan = kit_plan(doc.project(), &KitSettings::default());
    let din = plan.saw.iter().find(|s| s.key == "din").expect("rails");
    // 1000 + 700 + two cuts = 1702, 600 alone: two bars, never three.
    assert_eq!(din.plan.bars_bought(), 2);
    assert!(din.plan.proven_optimal() || din.plan.bought_mm == 4000.0);
    let bar = &din.bars[0];
    assert_eq!(bar.cuts.len(), 2);
    // 2000 - 1000 - 2 - 700 - 2 for the cut that frees the offcut.
    assert_eq!(bar.left, 296.0);
    let out = Kitting.generate(doc.project());
    let second = row_with(&out, "saw-din", "+CAB R3");
    assert_eq!(second[1], "Offcut of the cut above");
    assert_eq!(second[4], "998");
    let pick = row_with(&out, "pick-prepare", "DIN rail bar");
    assert_eq!(pick[2], "2");
    assert_eq!(pick[5], "+CAB R1, +CAB R2, +CAB R3");
    // Bars the saw plan buys are not "from stock": the pick line says to buy.
    assert!(pick[4].contains("to buy"), "{pick:?}");
}

#[test]
fn the_notes_explain_why_the_pick_list_numbers_skip_the_drill_step() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0]);
    let out = Kitting.generate(doc.project());
    let notes = out.sections.iter().find(|s| s.key == "notes").unwrap();
    assert!(notes.rows.iter().any(|r| {
        r.cells
            .iter()
            .any(|c| c.to_string().contains("step 2, Drill, has no pick list"))
    }));
}

#[test]
fn rails_and_ducts_of_two_cabinets_are_cut_from_the_same_bars() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+A", &[1200.0]);
    let (mut doc2, h2) = starter();
    let _ = (&mut doc2, h2);
    doc.transact("Second", |tx| {
        let cab = tx.add_structure(DeviceTag::parse("+B")?, "Cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 800.0,
                height: 600.0,
                margin: 10.0,
            }),
        )?;
        tx.add_rail(cab, 50.0, 100.0, 700.0, RailKind::DinRail)?;
        Ok(())
    })
    .expect("second cabinet");
    let plan = kit_plan(doc.project(), &KitSettings::default());
    let din = plan.saw.iter().find(|s| s.key == "din").unwrap();
    // 1200 + 700 + 2 = 1902 fits one bar: one bar for two cabinets.
    assert_eq!(din.plan.bars_bought(), 1);
    let labels: Vec<&str> = din.bars[0].cuts.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["+A R1", "+B R1"]);
}

#[test]
fn a_held_offcut_serves_a_rail_and_saves_a_bar() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0, 600.0, 700.0]);
    with_settings(&mut doc, |s| s.on_hand = "rail 700\n".into());
    let plan = kit_plan(doc.project(), &KitSettings::of(doc.project()));
    let din = plan.saw.iter().find(|s| s.key == "din").unwrap();
    assert_eq!(din.plan.bars_bought(), 1, "1000 + 600 share a bar");
    let held = din
        .bars
        .iter()
        .find(|b| matches!(b.source, Source::Held(_)))
        .unwrap();
    assert_eq!(held.cuts[0].label, "+CAB R3");
    let out = Kitting.generate(doc.project());
    let pick = row_with(&out, "pick-prepare", "offcut");
    assert_eq!(pick[4], "Offcut bin");
    assert_eq!(pick[5], "+CAB R3");
}

#[test]
fn a_held_duct_of_the_right_section_only() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0]);
    // Wrong section: the 900 mm duct still needs a bar.
    with_settings(&mut doc, |s| s.on_hand = "duct 25x40 1500\n".into());
    let plan = kit_plan(doc.project(), &KitSettings::of(doc.project()));
    let duct = plan.saw.iter().find(|s| s.key.starts_with("duct")).unwrap();
    assert_eq!(duct.plan.bars_bought(), 1);
    with_settings(&mut doc, |s| s.on_hand = "duct 50x60 1500\n".into());
    let plan = kit_plan(doc.project(), &KitSettings::of(doc.project()));
    let duct = plan.saw.iter().find(|s| s.key.starts_with("duct")).unwrap();
    assert_eq!(duct.plan.bars_bought(), 0);
}

#[test]
fn a_short_load_is_bought_in_the_short_bar() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[400.0]);
    with_settings(&mut doc, |s| s.rail_stock_mm = vec![1000.0, 2000.0]);
    let plan = kit_plan(doc.project(), &KitSettings::of(doc.project()));
    let din = plan.saw.iter().find(|s| s.key == "din").unwrap();
    assert_eq!(din.plan.bought, vec![(1000.0, 1)]);
    let out = Kitting.generate(doc.project());
    assert_eq!(row_with(&out, "shopping", "DIN rail 1000 mm bar")[3], "1");
}

#[test]
fn wires_are_cut_from_reels_with_the_allowance_held_back() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0]);
    wired(&mut doc);
    // Small reels so that several are needed: 3 m with 5 % held back (the
    // three longest wires leave their terminals facing away from the only
    // duct and come back, about 2.6 m each).
    with_settings(&mut doc, |s| s.reel_m = 3.0);
    let jobs = bench_jobs(doc.project());
    let hookup: Vec<_> = jobs.iter().filter(|j| j.cable.is_none()).collect();
    assert!(hookup.len() > 5);
    let plan = kit_plan(doc.project(), &KitSettings::of(doc.project()));
    // One group per size and colour: control wires, neutral, earth.
    assert!(plan.reels.len() >= 2);
    let group = plan
        .reels
        .iter()
        .find(|g| g.label == "1.5 mm² BK")
        .expect("control wires");
    let same: Vec<_> = hookup.iter().filter(|j| j.colour == "BK").collect();
    assert_eq!(group.wires, same.len());
    let total: f64 = same.iter().map(|j| j.cut_mm).sum::<f64>() / 1000.0;
    assert!((group.metres - total).abs() < 1e-6);
    // Never more on a reel than 3 m less the allowance, and no reel bought too many.
    for r in &group.reels {
        assert!(r.used_m * 1.05 <= 3.0 + 1e-9, "reel holds {} m", r.used_m);
    }
    assert!(group.new_reels as f64 >= (total * 1.05 / 3.0).ceil() - 1e-9);
    assert!(group.new_reels as f64 <= (total * 1.05 / 3.0).ceil() + 3.0);
    // The wires of the other colours are on their own reels.
    assert!(
        plan.reels
            .iter()
            .filter(|g| g.label != group.label)
            .all(|g| g.new_reels >= 1)
    );
    // Wires that do not fit: one wire longer than the reel is named.
    with_settings(&mut doc, |s| s.reel_m = 0.2);
    let plan = kit_plan(doc.project(), &KitSettings::of(doc.project()));
    assert!(
        plan.problems
            .iter()
            .any(|p| p.contains("longer than a 0.2 m reel"))
    );
}

#[test]
fn a_part_reel_on_hand_is_used_first() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0]);
    wired(&mut doc);
    let find = |p: &novo_reports::KitPlan| {
        p.reels
            .iter()
            .find(|g| g.label == "1.5 mm² BK")
            .cloned()
            .unwrap()
    };
    let before = kit_plan(doc.project(), &KitSettings::of(doc.project()));
    assert_eq!(find(&before).new_reels, 1);
    with_settings(&mut doc, |s| s.on_hand = "wire 1.5 BK 90 m\n".into());
    let after = kit_plan(doc.project(), &KitSettings::of(doc.project()));
    assert_eq!(find(&after).new_reels, 0);
    assert!(matches!(find(&after).reels[0].source, Source::Held(_)));
    let out = Kitting.generate(doc.project());
    assert!(row_with(&out, "pick-wire", "Part reel")[1].contains("90 m"));
}

#[test]
fn ferrules_are_counted_with_spares_and_bought_in_packs() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0]);
    wired(&mut doc);
    let out = Kitting.generate(doc.project());
    let pick = row_with(&out, "pick-wire", "Ferrule A1.5-10");
    let qty: f64 = pick[2].parse().unwrap();
    assert!(qty > 0.0);
    let shop = row_with(&out, "shopping", "Ferrule A1.5-10");
    assert_eq!(shop[6], "100", "a box of 100 for {qty} pieces");
    // With a box on hand nothing is bought.
    with_settings(&mut doc, |s| s.on_hand = "have A1.5-10 x500\n".into());
    let out = Kitting.generate(doc.project());
    let shop = row_with(&out, "shopping", "Ferrule A1.5-10");
    assert_eq!(shop[5], "500");
    assert_eq!(shop[6], "0");
}

#[test]
fn the_shopping_list_merges_a_stock_article_into_the_bom_line() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0]);
    let out = Kitting.generate(doc.project());
    let before = row_with(&out, "shopping", "Phoenix Contact 3044076");
    assert_eq!(before[3], "3");
    with_settings(&mut doc, |s| {
        s.articles = "Wiring duct 50 x 60 2000 mm bar = 3044076\n".into();
        s.on_hand = "have 3044076 x2\n".into();
    });
    let out = Kitting.generate(doc.project());
    let shopping = out.section("shopping").unwrap();
    assert!(
        shopping
            .text_rows()
            .iter()
            .all(|r| !r[1].starts_with("Wiring duct")),
        "the duct bar is part of the BOM line now"
    );
    let merged = row_with(&out, "shopping", "Phoenix Contact 3044076");
    // 3 from the BOM + 1 duct bar, 2 on hand.
    assert_eq!(
        (merged[3].as_str(), merged[5].as_str(), merged[6].as_str()),
        ("4", "2", "2")
    );
}

#[test]
fn settings_are_stored_in_the_project_and_defaults_leave_it_clean() {
    let (mut doc, _) = starter();
    assert_eq!(KitSettings::of(doc.project()), KitSettings::default());
    let s = with_settings(&mut doc, |s| s.kerf_mm = 3.0);
    assert_eq!(KitSettings::of(doc.project()), s);
    doc.undo().expect("undo");
    assert_eq!(KitSettings::of(doc.project()), KitSettings::default());
    // A setting that is out of range reads as the defaults.
    let bad = KitSettings {
        reel_m: -4.0,
        ..KitSettings::default()
    };
    assert!(!bad.is_valid());
}

#[test]
fn bad_settings_and_bad_on_hand_lines_are_reported_not_ignored() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0]);
    with_settings(&mut doc, |s| s.on_hand = "bolt 3\nrail 700\n".into());
    let plan = kit_plan(doc.project(), &KitSettings::of(doc.project()));
    assert!(plan.problems.iter().any(|p| p.contains("line 1")));
    // The good line still counted.
    assert!(
        plan.saw[0]
            .bars
            .iter()
            .any(|b| matches!(b.source, Source::Held(_)))
            || plan.saw[0].plan.unused_held.len() == 1
    );
}

#[test]
fn the_report_has_a_pick_list_per_phase_and_no_phase_without_rows() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0]);
    let out = Kitting.generate(doc.project());
    let keys: Vec<&str> = out.sections.iter().map(|s| s.key.as_str()).collect();
    for want in [
        "pick-prepare",
        "pick-mount",
        "pick-wire",
        "saw-din",
        "shopping",
        "notes",
    ] {
        assert!(keys.contains(&want), "{keys:?} lacks {want}");
    }
    for s in &out.sections {
        assert!(!s.rows.is_empty(), "{} is empty", s.key);
    }
    // Picks come in phase order.
    let plan = kit_plan(doc.project(), &KitSettings::default());
    let phases: Vec<Phase> = plan.picks.iter().map(|p| p.phase).collect();
    let mut sorted = phases.clone();
    sorted.sort();
    assert_eq!(phases, sorted);
    // The same project gives the same report.
    assert_eq!(out, Kitting.generate(doc.project()));
}

#[test]
fn a_project_without_a_cabinet_still_lists_what_to_buy() {
    let (doc, _) = starter();
    let plan = kit_plan(doc.project(), &KitSettings::default());
    assert!(plan.saw.is_empty());
    assert!(plan.picks_in(Phase::Prepare).next().is_none());
    assert!(!plan.shopping.is_empty(), "the BOM parts");
}

#[test]
fn what_makes_the_list_incomplete_is_printed_with_the_shopping_list() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h, "+CAB", &[1000.0]);
    let out = Kitting.generate(doc.project());
    let rows = out.section("shopping").unwrap().text_rows();
    let notes: Vec<&Vec<String>> = rows.iter().filter(|r| r[0] == "Note").collect();
    assert!(
        notes.iter().any(|r| r[1].contains("no cross-section")),
        "{notes:?}"
    );
}
