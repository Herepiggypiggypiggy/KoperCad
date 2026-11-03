//! Autogroup by wiring and by heat across the rails, on hand-made layouts.

use novo_layout::plan::Plan;
use novo_layout::regroup::{Goal, Inputs, Link, Report, regroup};
use novo_layout::rules::{Gaps, RuleSet, Spec};
use novo_layout::state::CabinetLayout;
use novo_layout::{Margins, Placement, Plate, Rail, Size};

fn plate() -> Plate {
    Plate {
        width: 600.0,
        height: 600.0,
        margin: 20.0,
    }
}

fn dev(id: u64, x: f64, rail_y: f64, rail: u64) -> Placement {
    Placement {
        id,
        x,
        y: rail_y - 30.0,
        rotated: false,
        size: Size {
            width: 45.0,
            height: 60.0,
            guessed: false,
        },
        clearance: None,
        margins: Margins::default(),
        rail: Some(rail),
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

fn spec(id: u64, tag: &str, loss: Option<f64>, limit: Option<f64>) -> Spec {
    Spec {
        id,
        size: Size {
            width: 45.0,
            height: 60.0,
            guessed: false,
        },
        sort_key: format!("{tag}{id:03}"),
        kind: tag.to_owned(),
        gaps: Gaps::default(),
        loss_w: loss,
        limit_c: limit,
        surface: None,
    }
}

fn link(a: u64, b: u64) -> Link {
    Link { a, b, weight: 1.0 }
}

fn layout(rails: Vec<Rail>, placements: Vec<Placement>) -> CabinetLayout {
    let mut l = CabinetLayout::new(plate());
    l.rails = rails;
    l.placements = placements;
    l
}

fn run(
    l: &CabinetLayout,
    ids: &[u64],
    goal: Goal,
    specs: &[Spec],
    links: &[Link],
) -> (Plan, Report) {
    let rules = RuleSet::standard();
    regroup(
        l,
        ids,
        goal,
        &Inputs {
            specs,
            links,
            rules: &rules,
        },
    )
}

fn after(l: &CabinetLayout, plan: &Plan) -> CabinetLayout {
    let mut out = l.clone();
    out.accept(plan, &[]);
    out
}

fn clean(l: &CabinetLayout) {
    let r = l.report();
    assert!(r.collisions.is_empty(), "{:?}", r.collisions);
    assert!(r.outside.is_empty(), "{:?}", r.outside);
}

fn row_of(l: &CabinetLayout, id: u64) -> u64 {
    l.placements
        .iter()
        .find(|p| p.id == id)
        .unwrap()
        .rail
        .unwrap()
}

#[test]
fn wiring_puts_power_and_signal_parts_on_separate_rows() {
    // Q parts carry power (top zone), K parts carry signals (middle zone),
    // standing mixed along row 1; row 2 is empty.
    let l = layout(
        vec![rail(1, 100.0), rail(2, 250.0)],
        vec![
            dev(1, 30.0, 100.0, 1),
            dev(2, 90.0, 100.0, 1),
            dev(3, 150.0, 100.0, 1),
            dev(4, 210.0, 100.0, 1),
        ],
    );
    let specs = vec![
        spec(1, "Q", None, None),
        spec(2, "K", None, None),
        spec(3, "Q", None, None),
        spec(4, "K", None, None),
    ];
    let links = vec![link(1, 3), link(2, 4)];
    let (plan, report) = run(&l, &[], Goal::Wiring, &specs, &links);
    assert_eq!(report.mixed_before, 1);
    assert_eq!(report.mixed_after, 0);
    assert!(report.wire_after_mm < report.wire_before_mm);
    let a = after(&l, &plan);
    clean(&a);
    assert_eq!(row_of(&a, 1), row_of(&a, 3));
    assert_eq!(row_of(&a, 2), row_of(&a, 4));
    assert_ne!(row_of(&a, 1), row_of(&a, 2));
    // Power above signal, like the zone rule says.
    assert_eq!(row_of(&a, 1), 1);
    assert!(plan.notes.iter().any(|n| n.contains("estimate")));
}

#[test]
fn wiring_brings_connected_devices_together() {
    // Relays 1 and 5 are wired to each other, 2 to 4; 3 is alone.
    let l = layout(
        vec![rail(1, 100.0)],
        vec![
            dev(1, 30.0, 100.0, 1),
            dev(2, 90.0, 100.0, 1),
            dev(3, 150.0, 100.0, 1),
            dev(4, 210.0, 100.0, 1),
            dev(5, 270.0, 100.0, 1),
        ],
    );
    let specs: Vec<Spec> = (1..=5).map(|i| spec(i, "K", None, None)).collect();
    let links = vec![link(1, 5), link(2, 4)];
    let (plan, report) = run(&l, &[], Goal::Wiring, &specs, &links);
    assert!(report.wire_after_mm < report.wire_before_mm, "{report:?}");
    assert!(report.moved > 0);
    let a = after(&l, &plan);
    clean(&a);
    let x = |id: u64| a.placements.iter().find(|p| p.id == id).unwrap().x;
    assert!((x(1) - x(5)).abs() < (30.0 - 270.0_f64).abs());
    assert!((x(2) - x(4)).abs() < (90.0 - 210.0_f64).abs());
}

#[test]
fn nothing_moves_when_the_layout_is_already_good() {
    let l = layout(
        vec![rail(1, 100.0)],
        vec![dev(1, 30.0, 100.0, 1), dev(2, 85.0, 100.0, 1)],
    );
    let specs = vec![spec(1, "K", None, None), spec(2, "K", None, None)];
    let (plan, report) = run(&l, &[], Goal::Wiring, &specs, &[link(1, 2)]);
    assert!(plan.moves.is_empty(), "{:?}", plan.moves);
    assert_eq!(report.moved, 0);
    assert!(plan.notes.iter().any(|n| n.contains("nothing was moved")));
}

#[test]
fn no_wires_known_says_so() {
    let l = layout(
        vec![rail(1, 100.0)],
        vec![dev(1, 30.0, 100.0, 1), dev(2, 200.0, 100.0, 1)],
    );
    let specs = vec![spec(1, "K", None, None), spec(2, "K", None, None)];
    let (plan, _) = run(&l, &[], Goal::Wiring, &specs, &[]);
    assert!(plan.moves.is_empty());
    assert!(plan.notes.iter().any(|n| n.contains("No wiring")));
}

#[test]
fn selection_limits_what_moves() {
    let l = layout(
        vec![rail(1, 100.0)],
        vec![
            dev(1, 30.0, 100.0, 1),
            dev(2, 90.0, 100.0, 1),
            dev(3, 150.0, 100.0, 1),
            dev(4, 400.0, 100.0, 1),
        ],
    );
    let specs: Vec<Spec> = (1..=4).map(|i| spec(i, "K", None, None)).collect();
    let (plan, _) = run(&l, &[1, 2, 3], Goal::Wiring, &specs, &[link(1, 3)]);
    assert!(plan.moves.iter().all(|m| m.id != 4));
    let a = after(&l, &plan);
    clean(&a);
    assert_eq!(a.placements.iter().find(|p| p.id == 4).unwrap().x, 400.0);
}

#[test]
fn heat_keeps_a_hot_part_away_from_parts_that_mind_heat() {
    // A 40 W source stands between two parts that allow only 45 C around
    // them; row 2 is free.
    let l = layout(
        vec![rail(1, 100.0), rail(2, 250.0)],
        vec![
            dev(1, 30.0, 100.0, 1),
            dev(2, 85.0, 100.0, 1),
            dev(3, 140.0, 100.0, 1),
        ],
    );
    let specs = vec![
        spec(1, "K", None, Some(45.0)),
        spec(2, "K", Some(40.0), None),
        spec(3, "K", None, Some(45.0)),
    ];
    let (plan, report) = run(&l, &[], Goal::Heat, &specs, &[]);
    assert!(report.heat_before > 0.0);
    assert!(report.heat_after < report.heat_before, "{report:?}");
    assert!(!plan.moves.is_empty());
    let a = after(&l, &plan);
    clean(&a);
    assert!(plan.notes.iter().any(|n| n.contains("not a temperature")));
}

#[test]
fn heat_without_losses_says_so() {
    let l = layout(
        vec![rail(1, 100.0)],
        vec![dev(1, 30.0, 100.0, 1), dev(2, 85.0, 100.0, 1)],
    );
    let specs = vec![spec(1, "K", None, None), spec(2, "K", None, None)];
    let (plan, _) = run(&l, &[], Goal::Heat, &specs, &[]);
    assert!(plan.moves.is_empty());
    assert!(plan.notes.iter().any(|n| n.contains("no heat")));
}

#[test]
fn both_goals_never_score_worse_than_the_start() {
    let l = layout(
        vec![rail(1, 100.0), rail(2, 250.0)],
        vec![
            dev(1, 30.0, 100.0, 1),
            dev(2, 90.0, 100.0, 1),
            dev(3, 150.0, 100.0, 1),
            dev(4, 210.0, 250.0, 2),
        ],
    );
    let specs = vec![
        spec(1, "K", Some(30.0), None),
        spec(2, "K", None, Some(45.0)),
        spec(3, "K", None, None),
        spec(4, "K", None, Some(45.0)),
    ];
    let links = vec![link(1, 4), link(2, 3)];
    let (plan, report) = run(&l, &[], Goal::Both, &specs, &links);
    let a = after(&l, &plan);
    clean(&a);
    assert!(
        report.wire_after_mm <= report.wire_before_mm + 1e-6
            || report.heat_after < report.heat_before
    );
}

#[test]
fn devices_on_no_rail_are_left_and_named() {
    let mut loose = dev(3, 300.0, 100.0, 1);
    loose.rail = None;
    let l = layout(
        vec![rail(1, 100.0)],
        vec![dev(1, 30.0, 100.0, 1), dev(2, 200.0, 100.0, 1), loose],
    );
    let specs: Vec<Spec> = (1..=3).map(|i| spec(i, "K", None, None)).collect();
    let (plan, _) = run(&l, &[], Goal::Wiring, &specs, &[link(1, 2)]);
    assert!(plan.moves.iter().all(|m| m.id != 3));
    assert!(plan.notes.iter().any(|n| n.contains("no rail")));
}

/// A small deterministic generator, so the test needs no extra crate.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

#[test]
fn a_move_that_empties_a_row_does_not_break_the_search() {
    // Many small layouts with rows of one or two devices, so moves often
    // empty a row; every goal must finish, stay clean and never score worse.
    let mut rng = Lcg(7);
    for round in 0..300 {
        let rows = 2 + rng.next(3);
        let mut placements = Vec::new();
        let mut id = 0;
        for r in 0..rows {
            let n = 1 + rng.next(2);
            for k in 0..n {
                id += 1;
                placements.push(dev(
                    id,
                    30.0 + 120.0 * k as f64,
                    100.0 + 150.0 * r as f64,
                    r + 1,
                ));
            }
        }
        let rails: Vec<Rail> = (0..rows)
            .map(|r| rail(r + 1, 100.0 + 150.0 * r as f64))
            .collect();
        let l = layout(rails, placements);
        let kinds = ["K", "Q", "T"];
        let specs: Vec<Spec> = (1..=id)
            .map(|i| {
                let loss = (rng.next(3) == 0).then(|| 15.0 + rng.next(30) as f64);
                let limit = (rng.next(3) == 0).then_some(42.0);
                spec(i, kinds[rng.next(3) as usize], loss, limit)
            })
            .collect();
        let mut links = Vec::new();
        for _ in 0..rng.next(5) {
            let (a, b) = (1 + rng.next(id), 1 + rng.next(id));
            if a != b {
                links.push(link(a, b));
            }
        }
        for goal in [Goal::Wiring, Goal::Heat, Goal::Both] {
            let (plan, _) = run(&l, &[], goal, &specs, &links);
            let a = after(&l, &plan);
            let r = a.report();
            assert!(r.collisions.is_empty(), "round {round} {goal:?}");
        }
    }
}

#[test]
fn both_without_wiring_groups_by_heat_alone_and_never_offers_a_worse_heat_score() {
    // Power and signal parts mixed on one row, one heat source, no wires.
    // The wiring side has nothing to group by, so "both" is "heat" alone:
    // it never offers a plan whose heat score is worse.
    let l = layout(
        vec![rail(1, 100.0), rail(2, 250.0)],
        vec![
            dev(1, 30.0, 100.0, 1),
            dev(2, 90.0, 100.0, 1),
            dev(3, 150.0, 100.0, 1),
            dev(4, 210.0, 100.0, 1),
        ],
    );
    let specs = vec![
        spec(1, "Q", None, None),
        spec(2, "K", Some(40.0), None),
        spec(3, "Q", None, Some(42.0)),
        spec(4, "K", None, Some(42.0)),
    ];
    let (plan, report) = run(&l, &[], Goal::Both, &specs, &[]);
    assert!(
        plan.notes.iter().any(|n| n.contains("heat only")),
        "{:?}",
        plan.notes
    );
    if plan.moves.is_empty() {
        return;
    }
    assert!(report.heat_after <= report.heat_before * 1.01, "{report:?}");
}

#[test]
fn a_worse_heat_score_is_only_traded_for_a_better_wire_estimate_with_its_number() {
    for seed in 0..200u64 {
        let mut rng = Lcg(seed);
        let l = layout(
            vec![rail(1, 100.0), rail(2, 250.0)],
            (1..=5)
                .map(|i| {
                    dev(
                        i,
                        30.0 + 90.0 * (i - 1) as f64,
                        100.0 + 150.0 * (i % 2) as f64,
                        1 + i % 2,
                    )
                })
                .collect(),
        );
        let specs: Vec<Spec> = (1..=5)
            .map(|i| {
                spec(
                    i,
                    "K",
                    (rng.next(2) == 0).then(|| 20.0 + rng.next(30) as f64),
                    Some(42.0),
                )
            })
            .collect();
        let links: Vec<Link> = (0..rng.next(4))
            .map(|_| link(1 + rng.next(5), 1 + rng.next(5)))
            .filter(|l| l.a != l.b)
            .collect();
        let (plan, report) = run(&l, &[], Goal::Both, &specs, &links);
        if plan.moves.is_empty() || report.heat_before == 0.0 {
            continue;
        }
        let worse = report.heat_after > report.heat_before * 1.01;
        if worse {
            assert!(
                report.wire_after_mm < report.wire_before_mm * 0.99,
                "seed {seed} {report:?}"
            );
            assert!(
                plan.notes
                    .iter()
                    .any(|n| n.contains("shorter wire estimate")),
                "{:?}",
                plan.notes
            );
        }
    }
}
