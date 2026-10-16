//! A duct over a rail or a device is a problem, and Place all keeps clear of
//! the ducts that are already on the plate.

use novo_layout::plan::Plan;
use novo_layout::rules::{Gaps, RuleSet, Spec, place_all_around, place_all_explained};
use novo_layout::state::CabinetLayout;
use novo_layout::{Duct, DuctHit, Margins, Placement, Plate, Rail, Rect, Size, duct_overlaps};

fn spec(id: u64, kind: &str, w: f64, h: f64) -> Spec {
    Spec {
        id,
        size: Size::from_part(Some(w), Some(h)),
        sort_key: format!("{kind}{id:06}"),
        kind: kind.into(),
        gaps: Gaps::default(),
        loss_w: None,
    }
}

fn placed(id: u64, x: f64, y: f64, w: f64, h: f64) -> Placement {
    Placement {
        id,
        x,
        y,
        rotated: false,
        size: Size::from_part(Some(w), Some(h)),
        clearance: None,
        margins: Margins::default(),
        rail: None,
    }
}

fn vertical_duct(id: u64, x: f64, y: f64, length: f64) -> Duct {
    Duct {
        id,
        x,
        y,
        length,
        width: 30.0,
        vertical: true,
    }
}

fn rail(id: u64, x: f64, y: f64, length: f64) -> Rail {
    Rail {
        id,
        x,
        y,
        length,
        vertical: false,
    }
}

#[test]
fn a_rail_running_under_a_duct_is_found_with_the_length_under_it() {
    // The duct covers x 285..315; the rail runs x 20..580 at y 100.
    let ducts = [vertical_duct(7, 300.0, 20.0, 700.0)];
    let found = duct_overlaps(&ducts, &[rail(1, 20.0, 100.0, 560.0)], &[]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].duct, 7);
    match found[0].hit {
        DuctHit::Rail { id, by } => {
            assert_eq!(id, 1);
            assert!((by - 30.0).abs() < 1e-9, "{by}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_rail_beside_a_duct_or_ending_at_its_edge_is_clear() {
    let ducts = [vertical_duct(7, 300.0, 20.0, 700.0)];
    // Ends exactly at the duct's left edge (x 285).
    assert!(duct_overlaps(&ducts, &[rail(1, 20.0, 100.0, 265.0)], &[]).is_empty());
    // Starts at its right edge.
    assert!(duct_overlaps(&ducts, &[rail(2, 315.0, 100.0, 200.0)], &[]).is_empty());
    // Below the duct's end.
    assert!(duct_overlaps(&ducts, &[rail(3, 20.0, 750.0, 560.0)], &[]).is_empty());
}

#[test]
fn a_device_partly_under_a_duct_is_found_with_how_far() {
    let ducts = [vertical_duct(7, 300.0, 20.0, 700.0)];
    // x 270..320 against the duct's 285..315: 30 across, 90 along.
    let found = duct_overlaps(&ducts, &[], &[placed(4, 270.0, 100.0, 50.0, 90.0)]);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].hit,
        DuctHit::Device {
            id: 4,
            across: 30.0,
            along: 90.0
        }
    );
    // Touching the edge is clear.
    assert!(duct_overlaps(&ducts, &[], &[placed(5, 315.0, 100.0, 50.0, 90.0)]).is_empty());
}

#[test]
fn a_horizontal_duct_reports_across_and_along_the_other_way() {
    let duct = Duct {
        id: 9,
        x: 20.0,
        y: 200.0,
        length: 400.0,
        width: 30.0,
        vertical: false,
    };
    // y 190..250 against the duct's 185..215: 25 across, 40 along.
    let found = duct_overlaps(&[duct], &[], &[placed(4, 100.0, 190.0, 40.0, 60.0)]);
    assert_eq!(
        found[0].hit,
        DuctHit::Device {
            id: 4,
            across: 25.0,
            along: 40.0
        }
    );
}

#[test]
fn the_layout_report_counts_duct_overlaps_and_is_not_clean() {
    let mut layout = CabinetLayout::new(Plate {
        width: 600.0,
        height: 800.0,
        margin: 10.0,
    });
    layout.ducts = vec![vertical_duct(7, 300.0, 20.0, 700.0)];
    layout.rails = vec![rail(1, 20.0, 100.0, 560.0)];
    layout.placements = vec![placed(4, 270.0, 120.0, 50.0, 90.0)];
    let report = layout.report();
    assert_eq!(report.duct_overlaps.len(), 2);
    assert!(!report.is_clean());
    layout.ducts.clear();
    assert!(layout.report().duct_overlaps.is_empty());
}

fn clear_of_ducts(plan: &Plan, specs: &[Spec], ducts: &[Duct]) {
    let placements: Vec<Placement> = plan
        .placements
        .iter()
        .map(|p| Placement {
            rail: Some(p.rail),
            ..placed(
                p.id,
                p.x,
                p.y,
                specs.iter().find(|s| s.id == p.id).unwrap().size.width,
                specs.iter().find(|s| s.id == p.id).unwrap().size.height,
            )
        })
        .collect();
    let found = duct_overlaps(ducts, &plan.rails, &placements);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn place_all_keeps_devices_and_rails_off_an_existing_vertical_duct() {
    // The QA case: a 600 x 800 plate with a vertical duct down the middle.
    let plate = Plate {
        width: 600.0,
        height: 800.0,
        margin: 10.0,
    };
    let duct = vertical_duct(7, 300.0, 10.0, 780.0);
    let specs = vec![
        spec(1, "K", 45.0, 90.0),
        spec(2, "K", 45.0, 90.0),
        spec(3, "Q", 54.0, 90.0),
        spec(4, "F", 18.0, 90.0),
    ];
    let (plan, why) = place_all_around(
        &plate,
        &specs,
        &RuleSet::standard(),
        1,
        0.0,
        &[duct.footprint()],
    );
    assert!(plan.unplaced.is_empty(), "{:?}", plan.unplaced);
    assert_eq!(plan.placements.len(), 4);
    clear_of_ducts(&plan, &specs, std::slice::from_ref(&duct));
    // Every placed device has its reason, and rail ids are unique.
    assert!(why.decision(1).is_some());
    let mut ids: Vec<u64> = plan.rails.iter().map(|r| r.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), plan.rails.len());
    // Without the duct the same devices fit; with it nothing straddles it.
    for r in &plan.rails {
        assert!(
            r.x + r.length <= 285.0 + 1e-9 || r.x >= 315.0 - 1e-9,
            "{r:?}"
        );
    }
}

#[test]
fn what_one_side_of_the_duct_cannot_hold_goes_to_the_other() {
    let plate = Plate {
        width: 600.0,
        height: 300.0,
        margin: 10.0,
    };
    // Left of the duct (x 10..285) holds two 120 mm devices side by side,
    // not three; the right side takes the rest.
    let duct = vertical_duct(7, 300.0, 10.0, 280.0);
    let specs: Vec<Spec> = (1..=5).map(|i| spec(i, "K", 120.0, 250.0)).collect();
    let (plan, why) = place_all_around(
        &plate,
        &specs,
        &RuleSet::standard(),
        1,
        0.0,
        &[duct.footprint()],
    );
    assert_eq!(plan.placements.len() + plan.unplaced.len(), 5);
    assert!(plan.placements.len() >= 4, "{:?}", plan.unplaced);
    clear_of_ducts(&plan, &specs, std::slice::from_ref(&duct));
    for id in &plan.unplaced {
        assert!(why.unplaced.iter().any(|(u, _)| u == id));
    }
}

#[test]
fn a_duct_across_the_whole_plate_leaves_nothing_to_place_on() {
    let plate = Plate {
        width: 100.0,
        height: 100.0,
        margin: 10.0,
    };
    let wall = Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    };
    let specs = vec![spec(1, "K", 20.0, 20.0)];
    let (plan, why) = place_all_around(&plate, &specs, &RuleSet::standard(), 1, 0.0, &[wall]);
    assert!(plan.placements.is_empty());
    assert_eq!(plan.unplaced, vec![1]);
    assert_eq!(why.unplaced.len(), 1);
}

#[test]
fn with_no_ducts_the_plan_is_the_one_place_all_makes() {
    let plate = Plate {
        width: 500.0,
        height: 900.0,
        margin: 10.0,
    };
    let specs = vec![
        spec(1, "Q", 18.0, 90.0),
        spec(2, "K", 45.0, 90.0),
        spec(3, "A", 100.0, 90.0),
    ];
    let rules = RuleSet::standard();
    assert_eq!(
        place_all_around(&plate, &specs, &rules, 3, 40.0, &[]),
        place_all_explained(&plate, &specs, &rules, 3, 40.0)
    );
}

#[test]
fn the_ducts_place_all_plans_do_not_trip_the_check() {
    let plate = Plate {
        width: 500.0,
        height: 900.0,
        margin: 10.0,
    };
    let specs = vec![
        spec(1, "Q", 18.0, 90.0),
        spec(2, "K", 45.0, 90.0),
        spec(3, "A", 100.0, 90.0),
        spec(4, "X", 60.0, 40.0),
    ];
    let (plan, _) = place_all_explained(&plate, &specs, &RuleSet::standard(), 1, 0.0);
    let ducts: Vec<Duct> = plan
        .ducts
        .iter()
        .enumerate()
        .map(|(i, d)| Duct {
            id: 100 + i as u64,
            x: d.x,
            y: d.y,
            length: d.length,
            width: d.width,
            vertical: d.vertical,
        })
        .collect();
    assert!(!ducts.is_empty());
    clear_of_ducts(&plan, &specs, &ducts);
}
