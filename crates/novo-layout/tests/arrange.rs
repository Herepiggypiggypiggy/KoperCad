//! The arrange tools: autodistance, autogroup, autoplace on rail, auto rails
//! and auto ducts, on hand-made layouts.

use std::collections::BTreeMap;

use novo_layout::arrange::{self, Spacing};
use novo_layout::plan::Candidate;
use novo_layout::state::CabinetLayout;
use novo_layout::{Margins, Placement, Plate, Rail, Size};

fn plate() -> Plate {
    Plate {
        width: 600.0,
        height: 600.0,
        margin: 20.0,
    }
}

fn dev(id: u64, x: f64, y: f64, w: f64, h: f64, rail: Option<u64>) -> Placement {
    Placement {
        id,
        x,
        y,
        rotated: false,
        size: Size {
            width: w,
            height: h,
            guessed: false,
        },
        clearance: None,
        margins: Margins::default(),
        rail,
    }
}

fn rail(id: u64, y: f64) -> Rail {
    Rail {
        id,
        x: 20.0,
        y,
        length: 500.0,
        vertical: false,
    }
}

fn layout(rails: Vec<Rail>, placements: Vec<Placement>) -> CabinetLayout {
    let mut l = CabinetLayout::new(plate());
    l.rails = rails;
    l.placements = placements;
    l
}

/// Applies the moves of a plan to a layout, like the preview does.
fn after(l: &CabinetLayout, plan: &novo_layout::plan::Plan) -> CabinetLayout {
    let mut out = l.clone();
    out.accept(plan, &[]);
    out
}

fn clean(l: &CabinetLayout) {
    let r = l.report();
    assert!(r.collisions.is_empty(), "{:?}", r.collisions);
    assert!(r.too_close.is_empty(), "{:?}", r.too_close);
    assert!(r.outside.is_empty(), "{:?}", r.outside);
}

#[test]
fn tight_closes_gaps_to_the_default_ten_millimetres() {
    let l = layout(
        vec![rail(1, 100.0)],
        vec![
            dev(1, 30.0, 70.0, 45.0, 60.0, Some(1)),
            dev(2, 120.0, 70.0, 45.0, 60.0, Some(1)),
            dev(3, 300.0, 70.0, 45.0, 60.0, Some(1)),
        ],
    );
    let plan = arrange::distribute(&l, &[], Spacing::Tight);
    let a = after(&l, &plan);
    let x: Vec<f64> = a.placements.iter().map(|p| p.x).collect();
    assert_eq!(x, vec![30.0, 85.0, 140.0]);
    clean(&a);
    assert_eq!(plan.moves.len(), 2, "the first device stays");
}

#[test]
fn even_spreads_between_the_first_and_the_last() {
    let l = layout(
        vec![rail(1, 100.0)],
        vec![
            dev(1, 30.0, 70.0, 40.0, 60.0, Some(1)),
            dev(2, 80.0, 70.0, 40.0, 60.0, Some(1)),
            dev(3, 90.0, 70.0, 40.0, 60.0, Some(1)),
            dev(4, 400.0, 70.0, 40.0, 60.0, Some(1)),
        ],
    );
    let plan = arrange::distribute(&l, &[], Spacing::Even);
    let a = after(&l, &plan);
    let x: Vec<f64> = a.placements.iter().map(|p| p.x).collect();
    // 400 - 30 - 3*40 = 250 mm for 3 gaps: 83 mm each, rounded down.
    assert_eq!(x, vec![30.0, 153.0, 276.0, 400.0]);
    clean(&a);
}

#[test]
fn even_says_so_when_the_gaps_do_not_fit() {
    let l = layout(
        vec![rail(1, 100.0)],
        vec![
            dev(1, 30.0, 70.0, 40.0, 60.0, Some(1)),
            dev(2, 72.0, 70.0, 40.0, 60.0, Some(1)),
            dev(3, 114.0, 70.0, 40.0, 60.0, Some(1)),
        ],
    );
    let plan = arrange::distribute(&l, &[], Spacing::Even);
    assert!(plan.moves.is_empty());
    assert_eq!(plan.notes.len(), 1);
}

#[test]
fn a_selection_limits_what_moves() {
    let l = layout(
        vec![rail(1, 100.0), rail(2, 300.0)],
        vec![
            dev(1, 30.0, 70.0, 40.0, 60.0, Some(1)),
            dev(2, 150.0, 70.0, 40.0, 60.0, Some(1)),
            dev(3, 30.0, 270.0, 40.0, 60.0, Some(2)),
            dev(4, 150.0, 270.0, 40.0, 60.0, Some(2)),
        ],
    );
    let plan = arrange::distribute(&l, &[3, 4], Spacing::Tight);
    assert!(plan.moves.iter().all(|m| m.id == 4));
}

#[test]
fn a_neighbour_that_is_not_selected_blocks_the_move() {
    // Closing up 1 and 3 would put 3 on top of 2, which stays.
    let l = layout(
        vec![rail(1, 100.0)],
        vec![
            dev(1, 30.0, 70.0, 40.0, 60.0, Some(1)),
            dev(2, 100.0, 70.0, 40.0, 60.0, Some(1)),
            dev(3, 200.0, 70.0, 40.0, 60.0, Some(1)),
        ],
    );
    let plan = arrange::distribute(&l, &[1, 3], Spacing::Tight);
    assert!(plan.moves.is_empty());
    assert_eq!(plan.notes.len(), 1);
}

#[test]
fn parts_that_ask_for_more_room_get_it() {
    let mut wide = dev(2, 120.0, 70.0, 45.0, 60.0, Some(1));
    wide.margins.left = Some(25.0);
    let l = layout(
        vec![rail(1, 100.0)],
        vec![dev(1, 30.0, 70.0, 45.0, 60.0, Some(1)), wide],
    );
    let plan = arrange::distribute(&l, &[], Spacing::Tight);
    let a = after(&l, &plan);
    assert_eq!(a.placements[1].x, 100.0, "30 + 45 + 25");
    clean(&a);
}

#[test]
fn group_puts_kinds_together_with_a_wider_gap_between() {
    let l = layout(
        vec![rail(1, 100.0)],
        vec![
            dev(1, 30.0, 70.0, 40.0, 60.0, Some(1)),
            dev(2, 90.0, 70.0, 40.0, 60.0, Some(1)),
            dev(3, 150.0, 70.0, 40.0, 60.0, Some(1)),
            dev(4, 210.0, 70.0, 40.0, 60.0, Some(1)),
        ],
    );
    let keys: BTreeMap<u64, String> = [
        (1, "K000002"),
        (2, "F000001"),
        (3, "K000001"),
        (4, "F000002"),
    ]
    .into_iter()
    .map(|(i, k)| (i, k.to_owned()))
    .collect();
    let plan = arrange::group(&l, &[], &keys);
    let a = after(&l, &plan);
    let order = |id: u64| a.placements.iter().find(|p| p.id == id).unwrap().x;
    assert!(order(2) < order(4) && order(4) < order(3) && order(3) < order(1));
    // F, F, then K, K: 10 mm inside a group, 20 mm between the groups.
    assert_eq!(order(4) - (order(2) + 40.0), 10.0);
    assert_eq!(order(3) - (order(4) + 40.0), 20.0);
    clean(&a);
}

#[test]
fn onto_rails_picks_the_nearest_rail_and_the_free_place() {
    let l = layout(
        vec![rail(1, 100.0), rail(2, 300.0)],
        vec![
            dev(1, 30.0, 70.0, 40.0, 60.0, Some(1)),
            dev(2, 30.0, 270.0, 40.0, 60.0, Some(2)),
            // Loose, close to the lower rail and above the first device.
            dev(3, 32.0, 230.0, 40.0, 60.0, None),
        ],
    );
    let plan = arrange::onto_rails(&l, &[], &[]);
    assert_eq!(plan.moves.len(), 1);
    let m = &plan.moves[0];
    assert_eq!((m.id, m.rail), (3, 2));
    assert_eq!(m.y, 270.0, "centred on the rail line at 300 mm");
    assert_eq!(m.x, 80.0, "after device 2 and its 10 mm gap");
    clean(&after(&l, &plan));
}

#[test]
fn onto_rails_names_a_device_that_fits_nowhere() {
    let mut l = layout(
        vec![Rail {
            id: 1,
            x: 20.0,
            y: 100.0,
            length: 100.0,
            vertical: false,
        }],
        vec![
            dev(1, 20.0, 70.0, 45.0, 60.0, Some(1)),
            dev(2, 70.0, 70.0, 45.0, 60.0, Some(1)),
            dev(3, 300.0, 300.0, 45.0, 60.0, None),
        ],
    );
    l.grid = 0.0;
    let plan = arrange::onto_rails(&l, &[], &[]);
    assert!(plan.moves.is_empty());
    assert_eq!(plan.unplaced, vec![3]);
    assert!(!plan.notes.is_empty());
}

#[test]
fn onto_rails_places_an_unplaced_device_when_asked() {
    let l = layout(
        vec![rail(1, 100.0)],
        vec![dev(1, 30.0, 70.0, 40.0, 60.0, Some(1))],
    );
    let unplaced = [Candidate {
        id: 9,
        size: Size {
            width: 40.0,
            height: 60.0,
            guessed: false,
        },
        sort_key: "K000001".into(),
    }];
    let plan = arrange::onto_rails(&l, &[9], &unplaced);
    assert_eq!(plan.moves.len(), 1);
    assert_eq!(plan.moves[0].id, 9);
    let a = {
        let mut a = l.clone();
        a.accept(&plan, &[unplaced[0].clone()]);
        a
    };
    assert_eq!(a.placements.len(), 2);
    clean(&a);
}

#[test]
fn make_rails_gives_each_row_a_rail_through_its_middle() {
    let l = layout(
        vec![],
        vec![
            dev(1, 40.0, 100.0, 45.0, 80.0, None),
            dev(2, 100.0, 104.0, 45.0, 72.0, None),
            dev(3, 40.0, 300.0, 45.0, 60.0, None),
        ],
    );
    let plan = arrange::make_rails(&l, &[]);
    assert_eq!(plan.rails.len(), 2);
    assert_eq!(plan.rails[0].y, 140.0);
    assert_eq!(plan.rails[0].x, 30.0, "10 mm before the first device");
    assert_eq!(plan.rails[0].length % 5.0, 0.0);
    let a = after(&l, &plan);
    assert!(a.placements.iter().all(|p| p.rail.is_some()));
    assert!(a.report().off_centre.is_empty());
    assert!(a.report().rail_overflows.is_empty());
    clean(&a);
}

#[test]
fn make_rails_leaves_a_row_over_an_existing_rail_alone() {
    let l = layout(
        vec![rail(1, 140.0)],
        vec![dev(1, 300.0, 110.0, 45.0, 60.0, None)],
    );
    let plan = arrange::make_rails(&l, &[]);
    assert!(plan.rails.is_empty() && plan.moves.is_empty());
    assert_eq!(plan.notes.len(), 1);
}

#[test]
fn add_ducts_fills_the_band_between_two_rows() {
    let l = layout(
        vec![rail(1, 100.0), rail(2, 300.0)],
        vec![
            dev(1, 30.0, 70.0, 40.0, 60.0, Some(1)),
            dev(2, 30.0, 270.0, 40.0, 60.0, Some(2)),
        ],
    );
    let plan = arrange::add_ducts(&l);
    assert_eq!(plan.ducts.len(), 1);
    assert_eq!(plan.ducts[0].y, 200.0, "between 130 and 270");
    let a = after(&l, &plan);
    assert_eq!(a.ducts.len(), 1);
    assert!(a.report().duct_overlaps.is_empty());
}

#[test]
fn add_ducts_says_when_the_rows_are_too_close_or_have_a_duct() {
    let close = layout(
        vec![rail(1, 100.0), rail(2, 200.0)],
        vec![
            dev(1, 30.0, 70.0, 40.0, 60.0, Some(1)),
            dev(2, 30.0, 170.0, 40.0, 60.0, Some(2)),
        ],
    );
    let plan = arrange::add_ducts(&close);
    assert!(plan.ducts.is_empty());
    assert_eq!(plan.notes.len(), 1);

    let mut once = layout(
        vec![rail(1, 100.0), rail(2, 300.0)],
        vec![
            dev(1, 30.0, 70.0, 40.0, 60.0, Some(1)),
            dev(2, 30.0, 270.0, 40.0, 60.0, Some(2)),
        ],
    );
    let first = arrange::add_ducts(&once);
    once.accept(&first, &[]);
    let again = arrange::add_ducts(&once);
    assert!(again.ducts.is_empty(), "running it twice adds nothing");
}
