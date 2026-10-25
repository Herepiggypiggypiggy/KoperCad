//! The cooling what-if: conditions change the answer, the fixes are ranked by
//! what the model says they win, and the numbers reproduce by hand.

mod common;

use common::*;
use novo_core::{Command, DeviceTag, MountingPlate, Project, RailKind, StructureId, Value};
use novo_layout::enclosure::Enclosure;
use novo_layout::model::set_enclosure;
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};
use novo_thermal::placement_project::rows;
use novo_thermal::sweep;
use novo_thermal::whatif::{self, Conditions, Control, Fix, Rails, Settings};
use novo_thermal::*;

/// Three rails 350 mm apart. A 150 W drive sits at the left of the lowest
/// rail with a cool 45 C-limit device on the rail above it; the right of the
/// lowest rail is free.
fn cabinet() -> (Project, StructureId, u64, u64, Vec<u64>) {
    let mut p = Project::new("placement");
    let (mut hot, mut cool, mut rails) = (0, 0, Vec::new());
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 1100.0,
                    margin: 10.0,
                }),
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 900.0,
                    height: 1200.0,
                    depth: 300.0,
                    plate_setback: 0.0,
                    door: None,
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            for row in 0..3 {
                let id = tx.add_rail(
                    cab,
                    20.0,
                    150.0 + 350.0 * f64::from(row),
                    760.0,
                    RailKind::DinRail,
                )?;
                rails.push(id.raw());
            }
            let put = |tx: &mut novo_core::Tx<'_>,
                       tag: &str,
                       x: f64,
                       row: usize,
                       w,
                       h,
                       loss,
                       limit: Option<f64>| {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                for (k, v) in [
                    ("width_mm", w),
                    ("height_mm", h),
                    ("depth_mm", 80.0),
                    ("power_loss_w", loss),
                ] {
                    tx.set_property(d, k, Some(Value::Float(v)))?;
                }
                if let Some(l) = limit {
                    tx.set_property(d, "max_ambient_c", Some(Value::Float(l)))?;
                }
                let rail = novo_core::RailId::from(rails[row]);
                tx.execute(Command::PlaceDevice {
                    device: d,
                    structure: cab,
                    x,
                    y: 150.0 + 350.0 * row as f64 - h / 2.0,
                    rotated: false,
                    rail: Some((rail, x - 20.0)),
                })?;
                Ok::<_, novo_core::ModelError>(d.raw())
            };
            hot = put(tx, "=A1+CAB1-T1", 30.0, 2, 120.0, 220.0, 150.0, Some(40.0))?;
            cool = put(tx, "=A1+CAB1-K1", 40.0, 1, 50.0, 90.0, 4.0, Some(45.0))?;
            Ok(cab)
        })
        .unwrap();
    (p, cab, hot, cool, rails)
}

fn hot_cabinet() -> Scenario {
    let mut vfd = device(1, 300.0, 700.0, 120.0, 200.0, 165.0);
    vfd.height_mm = Some(150.0);
    vfd.rated_max_ambient_c = Some(40.0);
    let mut above = device(2, 300.0, 450.0, 100.0, 100.0, 25.0);
    above.rated_max_ambient_c = Some(45.0);
    let mut plc = device(3, 500.0, 500.0, 120.0, 150.0, 20.0);
    plc.rated_max_ambient_c = Some(55.0);
    let mut sc = scenario(vec![vfd, above, plc]);
    sc.ambient_c = 35.0;
    sc
}

fn run(sc: &Scenario, c: &Conditions) -> whatif::WhatIf {
    whatif::run(sc, c, None, &Settings::quick(), &Control::default()).unwrap()
}

#[test]
fn a_cabinet_with_margin_has_nothing_to_fix() {
    let sc = scenario(vec![device(1, 300.0, 300.0, 100.0, 100.0, 5.0)]);
    let w = run(&sc, &Conditions::as_entered(&sc));
    assert!(w.choices.is_empty());
    assert!(w.notes.iter().any(|n| n.contains("nothing to fix")));
    assert!(w.notes.iter().any(|n| n.contains("Model estimate")));
}

#[test]
fn a_hotter_day_and_a_lighter_duty_move_the_baseline_the_right_way() {
    let sc = hot_cabinet();
    let hot = run(
        &sc,
        &Conditions {
            ambient_c: 45.0,
            duty: 1.0,
        },
    );
    let normal = run(&sc, &Conditions::as_entered(&sc));
    let light = run(
        &sc,
        &Conditions {
            ambient_c: 35.0,
            duty: 0.4,
        },
    );
    let m = |w: &whatif::WhatIf| w.baseline.worst_margin_k.unwrap();
    assert!(m(&hot) < m(&normal) - 5.0, "{} {}", m(&hot), m(&normal));
    assert!(m(&light) > m(&normal) + 2.0, "{} {}", m(&light), m(&normal));
    assert!(light.notes.iter().any(|n| n.contains("Duty 40 %")));
}

#[test]
fn fixes_are_ranked_and_the_numbers_reproduce_by_hand() {
    let sc = hot_cabinet();
    let w = run(&sc, &Conditions::as_entered(&sc));
    assert!(w.over_before() > 0 || w.baseline.worst_margin_k.unwrap() < 5.0);
    assert!(w.choices.len() >= 2, "{:?}", w.choices.len());
    for (i, c) in w.choices.iter().enumerate() {
        assert_eq!(c.rank, i + 1);
        assert!(c.gain_k >= 0.3 || c.solves);
    }
    // Best first: nobody before a fix has more devices over.
    for pair in w.choices.windows(2) {
        assert!(pair[0].reading.over <= pair[1].reading.over);
    }
    // Both a passive and a powered pair are offered.
    let kinds: Vec<usize> = w
        .choices
        .iter()
        .filter_map(|c| c.fix.candidate().map(|k| k.pairing.fans()))
        .collect();
    assert!(
        kinds.contains(&0) && kinds.iter().any(|f| *f > 0),
        "{kinds:?}"
    );
    // The first openings fix, added by hand, gives the reported margin.
    let c = w
        .choices
        .iter()
        .find(|c| matches!(c.fix, Fix::Openings { .. }))
        .unwrap();
    let mut by_hand = c.fix.candidate().unwrap().apply(&sc);
    by_hand.params.cell_mm = sweep::FINE_CELL_MM;
    let r = simulate_steady_with(
        &by_hand,
        Options {
            bands: false,
            advice: false,
        },
    )
    .unwrap();
    let margin = r
        .devices
        .iter()
        .filter_map(|d| d.margin_k)
        .fold(f64::MAX, f64::min);
    assert!(
        (margin - c.reading.worst_margin_k.unwrap()).abs() < 1.5,
        "{margin} vs {:?}",
        c.reading.worst_margin_k
    );
}

#[test]
fn a_bigger_enclosure_is_tried_but_cannot_be_applied_by_the_program() {
    let sc = hot_cabinet();
    let w = run(&sc, &Conditions::as_entered(&sc));
    let big = w
        .choices
        .iter()
        .find(|c| matches!(c.fix, Fix::BiggerEnclosure { .. }));
    if let Some(big) = big {
        assert!(!big.fix.applicable());
        let Fix::BiggerEnclosure { from, to } = &big.fix else {
            unreachable!()
        };
        assert!(to.width_mm > from.width_mm && to.height_mm > from.height_mm);
        assert_eq!(to.width_mm % 100.0, 0.0);
        assert_eq!(big.fix.effort(), 5);
    }
    // It is left out when growth is 1.
    let mut s = Settings::quick();
    s.enclosure_growth = 1.0;
    let w = whatif::run(
        &sc,
        &Conditions::as_entered(&sc),
        None,
        &s,
        &Control::default(),
    )
    .unwrap();
    assert!(
        !w.choices
            .iter()
            .any(|c| matches!(c.fix, Fix::BiggerEnclosure { .. }))
    );
}

#[test]
fn without_rails_the_layout_fix_is_left_out_and_says_so() {
    let sc = hot_cabinet();
    let w = run(&sc, &Conditions::as_entered(&sc));
    assert!(
        !w.choices
            .iter()
            .any(|c| matches!(c.fix, Fix::Layout { .. } | Fix::LayoutAndOpenings { .. }))
    );
    assert!(w.notes.iter().any(|n| n.contains("no layout fix")));
}

#[test]
fn a_stopped_run_is_marked_partial() {
    let sc = hot_cabinet();
    let control = Control::default();
    control.stop();
    let w = whatif::run(
        &sc,
        &Conditions::as_entered(&sc),
        None,
        &Settings::quick(),
        &control,
    )
    .unwrap();
    assert!(w.partial);
}

#[test]
fn moving_devices_along_rails_is_offered_first_and_cheapest() {
    let (p, cab, hot, _cool, _rails) = cabinet();
    let b = scenario_from_project(&p, cab, &BridgeOptions::default()).unwrap();
    let r = rows(&p, cab, &b.scenario).unwrap();
    let rails = Rails {
        rows: &r.rows,
        row_of: &r.row_of,
    };
    let sc = b.scenario;
    let w = whatif::run(
        &sc,
        &Conditions::as_entered(&sc),
        Some(rails),
        &Settings::quick(),
        &Control::default(),
    )
    .unwrap();
    let layout = w
        .choices
        .iter()
        .find(|c| matches!(c.fix, Fix::Layout { .. }));
    let layout =
        layout.unwrap_or_else(|| panic!("no layout fix: {:?} {:?}", w.notes, w.choices.len()));
    assert!(layout.fix.applicable());
    assert!(layout.fix.moves().iter().any(|m| m.id == hot));
    assert!(layout.gain_k > 0.0);
    assert_eq!(layout.fix.effort(), 1);
}

#[test]
fn a_real_run_names_the_devices_beside_the_stream_of_each_fan_fix_and_none_for_a_fix_without_fans()
{
    let sc = hot_cabinet();
    let w = run(&sc, &Conditions::as_entered(&sc));
    let mut fan_fixes = 0;
    for c in &w.choices {
        match c.fix.candidate() {
            Some(k) if k.pairing.fans() > 0 && matches!(c.fix, Fix::Openings { .. }) => {
                fan_fixes += 1;
                // The list is the rule applied by hand to the cabinet as it is.
                assert_eq!(c.beside_stream, whatif::beside_stream(&sc, k));
                assert!(c.beside_stream.iter().all(|id| w.device_ids.contains(id)));
            }
            Some(k) if k.pairing.fans() == 0 => assert!(c.beside_stream.is_empty()),
            None => assert!(c.beside_stream.is_empty()),
            _ => {}
        }
    }
    assert!(fan_fixes > 0, "no fan fix offered");
}
