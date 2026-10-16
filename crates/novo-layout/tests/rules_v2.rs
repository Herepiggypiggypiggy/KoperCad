//! Rules-based placement, second step: power and signal classes, wiring
//! ducts, explanations, the rule check and the cut list for rails and ducts.

use novo_layout::check::{Problem, Side, check, duct_rect};
use novo_layout::plan::{Plan, PlannedDuct};
use novo_layout::rules::{
    Class, GapSource, Gaps, RuleSet, Spec, UnplacedWhy, Zone, place_all_explained, place_all_rules,
};
use novo_layout::stock::{Stock, optimise, rail_pieces};
use novo_layout::{Placement, Plate, Size, collisions, outside, rail_overflows};

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

fn plate() -> Plate {
    Plate {
        width: 500.0,
        height: 900.0,
        margin: 10.0,
    }
}

fn placements(plan: &Plan, specs: &[Spec]) -> Vec<Placement> {
    plan.placements
        .iter()
        .map(|p| Placement {
            id: p.id,
            x: p.x,
            y: p.y,
            rotated: false,
            size: specs.iter().find(|s| s.id == p.id).unwrap().size,
            clearance: None,
            margins: novo_layout::Margins::default(),
            rail: Some(p.rail),
        })
        .collect()
}

/// The ducts between the rows, without the trunk that joins them.
fn row_ducts(plan: &Plan) -> Vec<&PlannedDuct> {
    plan.ducts.iter().filter(|d| !d.vertical).collect()
}

fn at(plan: &Plan, id: u64) -> (f64, f64) {
    let p = plan.placements.iter().find(|p| p.id == id).unwrap();
    (p.x, p.y)
}

#[test]
fn a_power_row_over_a_signal_row_gets_two_ducts_kept_apart() {
    let specs = vec![spec(1, "Q", 18.0, 90.0), spec(2, "K", 45.0, 90.0)];
    let rules = RuleSet::standard();
    let plan = place_all_rules(&plate(), &specs, &rules, 1, 0.0);
    let (_, qy) = at(&plan, 1);
    let (_, ky) = at(&plan, 2);
    assert_eq!(row_ducts(&plan).len(), 2);
    let power = plan.ducts.iter().find(|d| d.class == Class::Power).unwrap();
    let signal = plan
        .ducts
        .iter()
        .find(|d| d.class == Class::Signal)
        .unwrap();
    // The power duct hugs the power row, the signal duct the signal row.
    assert!(power.y < signal.y);
    let (pr, sr) = (duct_rect(power), duct_rect(signal));
    assert!(pr.y >= qy + 90.0 && sr.bottom() <= ky);
    assert!(sr.y - pr.bottom() >= rules.separation - 1e-9);
    assert!(
        check(&placements(&plan, &specs), &plan.ducts, &specs, &rules).is_empty(),
        "the planner's own output passes the check"
    );
}

#[test]
fn rows_of_one_class_share_a_duct() {
    // A and K are both signal rows on their own rails.
    let specs = vec![spec(1, "A", 100.0, 90.0), spec(2, "K", 45.0, 90.0)];
    let plan = place_all_rules(&plate(), &specs, &RuleSet::standard(), 1, 0.0);
    assert_eq!(row_ducts(&plan).len(), 1);
    let d = row_ducts(&plan)[0];
    assert_eq!(d.class, Class::Signal);
    let (_, y1) = at(&plan, 1);
    let (_, y2) = at(&plan, 2);
    // Centred in the space between the two rows.
    assert!((d.y - ((y1 + 90.0 + y2) / 2.0)).abs() < 1e-9);
    assert_eq!((d.width, d.height), (25.0, 60.0));
}

#[test]
fn separation_off_gives_one_duct_and_the_normal_row_gap() {
    let specs = vec![spec(1, "Q", 18.0, 90.0), spec(2, "K", 45.0, 90.0)];
    let mut rules = RuleSet::standard();
    rules.separation = 0.0;
    let plan = place_all_rules(&plate(), &specs, &rules, 1, 0.0);
    let (_, qy) = at(&plan, 1);
    let (_, ky) = at(&plan, 2);
    assert_eq!(ky - (qy + 90.0), rules.row_gap);
    assert_eq!(row_ducts(&plan).len(), 1);
}

#[test]
fn a_small_plate_gives_up_the_separation_before_leaving_a_device_off() {
    let small = Plate {
        width: 300.0,
        height: 400.0,
        margin: 10.0,
    };
    let specs = vec![
        spec(1, "Q", 18.0, 90.0),
        spec(2, "K", 45.0, 90.0),
        spec(3, "A", 100.0, 90.0),
    ];
    let rules = RuleSet::standard();
    let (plan, why) = place_all_explained(&small, &specs, &rules, 1, 0.0);
    assert!(plan.unplaced.is_empty(), "{:?}", plan.unplaced);
    assert!(why.separation_relaxed);
    // And the check says what was given up.
    let v = check(&placements(&plan, &specs), &plan.ducts, &specs, &rules);
    assert!(v.is_empty() || v.iter().all(|v| v.rule_name() == "separation"));
    // A roomy plate keeps the separation.
    let (_, why) = place_all_explained(&plate(), &specs, &rules, 1, 0.0);
    assert!(!why.separation_relaxed);
}

#[test]
fn a_shared_rail_never_mixes_power_and_signal() {
    let mut rules = RuleSet::standard();
    // Make K share with Q, F, G: the classes still keep the rails apart.
    rules.rules[1].rail = novo_layout::rules::RailUse::Shared;
    rules.rules[0].codes.push("K".into());
    let specs = vec![spec(1, "Q", 18.0, 90.0), spec(2, "A", 45.0, 90.0)];
    let plan = place_all_rules(&plate(), &specs, &rules, 1, 0.0);
    let rails: Vec<u64> = plan.placements.iter().map(|p| p.rail).collect();
    assert_ne!(rails[0], rails[1]);
}

#[test]
fn every_placement_says_which_rule_put_it_there() {
    let mut specs = vec![
        spec(1, "Q", 18.0, 90.0),
        spec(2, "K", 45.0, 90.0),
        spec(3, "K", 45.0, 90.0),
        spec(4, "T", 80.0, 150.0),
    ];
    specs[2].loss_w = Some(40.0);
    specs[1].gaps.right = Some(25.0);
    let rules = RuleSet::standard();
    let (plan, why) = place_all_explained(&plate(), &specs, &rules, 1, 0.0);
    assert_eq!(why.decisions.len(), plan.placements.len());
    let q = why.decision(1).unwrap();
    assert_eq!(q.zone, Zone::Top);
    assert_eq!(q.class, Class::Power);
    assert_eq!(
        rules.rule_text(q.rule.unwrap()),
        "Q F G zone=top rail=shared class=power"
    );
    // The hot K (40 W against a 25 W limit) went to the hot zone; it is
    // still on its own K rail, after the cooler K.
    let hot = why.decision(3).unwrap();
    assert!(hot.hot);
    assert_eq!(hot.zone, Zone::Middle);
    // The cool K asked for 25 mm on its right; the hot K asks for 20 on its left.
    let cool = why.decision(2).unwrap();
    assert_eq!(cool.gap_source, GapSource::Start);
    assert_eq!((hot.gap_before, hot.gap_source), (25.0, GapSource::Part));
    // The drive T has its rule's room above and below.
    let t = why.decision(4).unwrap();
    assert_eq!(t.zone, Zone::Bottom);
    let note = why.rows.iter().find(|r| Some(r.rail) == t.rail).unwrap();
    assert!(note.space_above.unwrap() >= 80.0);
    // The sentence names the rule.
    let text = why.describe(3, &rules);
    assert!(
        text.contains("zone=middle") && text.contains("hot"),
        "{text}"
    );
}

#[test]
fn what_does_not_fit_is_named_with_its_reason() {
    let specs = vec![spec(1, "K", 600.0, 90.0), spec(2, "K", 45.0, 90.0)];
    let small = Plate {
        width: 200.0,
        height: 200.0,
        margin: 10.0,
    };
    let rules = RuleSet::standard();
    let (plan, why) = place_all_explained(&small, &specs, &rules, 1, 0.0);
    assert_eq!(plan.unplaced, vec![1]);
    assert_eq!(why.unplaced, vec![(1, UnplacedWhy::TooLarge)]);
    assert!(why.describe(1, &rules).starts_with("Not placed"));
}

#[test]
fn the_rule_text_round_trips_with_the_new_settings() {
    let mut rules = RuleSet::standard();
    rules.separation = 55.0;
    rules.duct_width = 40.0;
    rules.duct_height = 60.0;
    rules.hot_gap = 15.0;
    let back = RuleSet::parse(&rules.to_text()).unwrap();
    assert_eq!(back, rules);
    assert!(RuleSet::parse("duct 0 0").is_err());
    assert!(RuleSet::parse("rule K class=loud").is_err());
    // An old rule file (no separation, no classes) still reads, with the
    // separation off.
    let old =
        RuleSet::parse("gap 10\nrow_gap 40\nrule Q F zone=top\nrule * zone=middle\n").unwrap();
    assert_eq!(old.separation, 0.0);
    assert!(old.rules.iter().all(|r| r.class == Class::Mixed));
}

// ---- the check -------------------------------------------------------------

fn put(id: u64, x: f64, y: f64, w: f64, h: f64) -> Placement {
    Placement {
        id,
        x,
        y,
        rotated: false,
        size: Size::from_part(Some(w), Some(h)),
        clearance: None,
        margins: novo_layout::Margins::default(),
        rail: None,
    }
}

#[test]
fn a_relay_above_a_breaker_breaks_the_zone_rule_and_names_it() {
    let specs = vec![spec(1, "Q", 18.0, 90.0), spec(2, "K", 45.0, 90.0)];
    let rules = RuleSet::standard();
    let v = check(
        &[
            put(1, 20.0, 400.0, 18.0, 90.0),
            put(2, 20.0, 50.0, 45.0, 90.0),
        ],
        &[],
        &specs,
        &rules,
    );
    assert_eq!(v.len(), 1);
    let Problem::WrongZone {
        device,
        wants,
        against,
        hot,
    } = v[0].problem
    else {
        panic!("{:?}", v[0]);
    };
    assert_eq!((device, wants, against, hot), (1, Zone::Top, 2, false));
    assert_eq!(
        rules.rule_text(v[0].rule.unwrap()),
        "Q F G zone=top rail=shared class=power"
    );
    let text = v[0].describe(&rules, &|id| format!("-D{id}"));
    assert!(
        text.contains("-D1") && text.contains("top zone") && text.contains("-D2"),
        "{text}"
    );
}

#[test]
fn a_hot_device_below_the_terminals_breaks_the_hot_rule() {
    // A 60 W relay is hot (limit 25 W), so the hot rule puts it in the
    // middle zone, above the terminals in the bottom zone.
    let mut specs = vec![spec(1, "K", 45.0, 90.0), spec(3, "X", 6.0, 60.0)];
    specs[0].loss_w = Some(60.0);
    let rules = RuleSet::standard();
    let v = check(
        &[
            put(1, 20.0, 600.0, 45.0, 90.0),
            put(3, 300.0, 200.0, 6.0, 60.0),
        ],
        &[],
        &specs,
        &rules,
    );
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].rule_name(), "hot");
    assert!(matches!(
        v[0].problem,
        Problem::WrongZone {
            device: 1,
            wants: Zone::Middle,
            against: 3,
            hot: true
        }
    ));
    let text = v[0].describe(&rules, &|id| format!("-D{id}"));
    assert!(text.contains("it is hot"), "{text}");
}

#[test]
fn a_drive_without_its_room_names_the_side_the_amount_and_the_neighbour() {
    let mut specs = vec![spec(1, "T", 80.0, 150.0), spec(2, "K", 45.0, 90.0)];
    specs[0].gaps.above = Some(100.0);
    let rules = RuleSet::standard();
    // K is 60 mm above the drive; the part wants 100.
    let v = check(
        &[
            put(1, 20.0, 400.0, 80.0, 150.0),
            put(2, 30.0, 250.0, 45.0, 90.0),
        ],
        &[],
        &specs,
        &rules,
    );
    let c = v
        .iter()
        .find_map(|v| match v.problem {
            Problem::Clearance {
                device,
                side,
                need,
                actual,
                neighbour,
                source,
            } => Some((device, side, need, actual, neighbour, source)),
            _ => None,
        })
        .expect("a clearance violation");
    assert_eq!(c, (1, Side::Above, 100.0, 60.0, 2, GapSource::Part));
    // Move K up and the violation goes.
    let v = check(
        &[
            put(1, 20.0, 400.0, 80.0, 150.0),
            put(2, 30.0, 150.0, 45.0, 90.0),
        ],
        &[],
        &specs,
        &rules,
    );
    assert!(
        v.iter()
            .all(|v| !matches!(v.problem, Problem::Clearance { .. }))
    );
}

#[test]
fn neighbours_beside_a_hot_device_need_the_hot_gap() {
    let mut specs = vec![spec(1, "K", 45.0, 90.0), spec(2, "K", 45.0, 90.0)];
    specs[0].loss_w = Some(60.0);
    let rules = RuleSet::standard();
    let near = [
        put(1, 20.0, 100.0, 45.0, 90.0),
        put(2, 75.0, 100.0, 45.0, 90.0),
    ];
    let v = check(&near, &[], &specs, &rules);
    // 10 mm apart, the hot K wants 20 on each side.
    assert!(v.iter().any(|v| matches!(
        v.problem,
        Problem::Clearance { device: 1, side: Side::Right, need, actual, source: GapSource::Hot, .. }
            if need == 20.0 && actual == 10.0
    )));
}

#[test]
fn power_and_signal_ducts_side_by_side_must_keep_their_distance() {
    let duct = |y: f64, class: Class| PlannedDuct {
        x: 10.0,
        y,
        length: 400.0,
        vertical: false,
        width: 25.0,
        height: 60.0,
        class,
    };
    let rules = RuleSet::standard();
    let ducts = [duct(100.0, Class::Power), duct(150.0, Class::Signal)];
    // Centres 50 apart, 25 wide: 25 mm between the edges, 40 required.
    let v = check(&[], &ducts, &[], &rules);
    assert_eq!(v.len(), 1);
    match v[0].problem {
        Problem::Separation {
            power,
            signal,
            need,
            actual,
        } => {
            assert_eq!((power, signal, need, actual), (0, 1, 40.0, 25.0));
        }
        ref p => panic!("{p:?}"),
    }
    let far = [duct(100.0, Class::Power), duct(200.0, Class::Signal)];
    assert!(check(&[], &far, &[], &rules).is_empty());
    // Two ducts of the same class may touch.
    let same = [duct(100.0, Class::Signal), duct(125.0, Class::Signal)];
    assert!(check(&[], &same, &[], &rules).is_empty());
}

// ---- properties ------------------------------------------------------------

/// A small deterministic generator, so a failure names its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

#[test]
fn whatever_the_devices_a_plan_obeys_its_rules() {
    let kinds = ["Q", "F", "G", "A", "K", "K", "T", "U", "X", "X", "M", "?"];
    for seed in 1..=400u64 {
        let mut rng = Rng(seed * 2_654_435_761 + 1);
        let n = rng.range(1, 24);
        let specs: Vec<Spec> = (1..=n)
            .map(|id| {
                let kind = kinds[rng.range(0, kinds.len() as u64 - 1) as usize];
                let mut s = spec(
                    id,
                    kind,
                    rng.range(6, 120) as f64,
                    rng.range(40, 160) as f64,
                );
                if rng.range(0, 5) == 0 {
                    s.loss_w = Some(rng.range(5, 80) as f64);
                }
                if rng.range(0, 7) == 0 {
                    s.gaps.above = Some(rng.range(20, 120) as f64);
                    s.gaps.below = Some(rng.range(20, 120) as f64);
                }
                if rng.range(0, 7) == 0 {
                    s.gaps.right = Some(rng.range(5, 40) as f64);
                    s.gaps.left = Some(rng.range(5, 40) as f64);
                }
                s
            })
            .collect();
        let p = Plate {
            width: rng.range(300, 800) as f64,
            height: rng.range(500, 2000) as f64,
            margin: 10.0,
        };
        let rules = RuleSet::standard();
        let (plan, why) = place_all_explained(&p, &specs, &rules, 1, 0.0);
        let ctx = format!("seed {seed}");
        // Every device is either placed or reported, once.
        assert_eq!(
            plan.placements.len() + plan.unplaced.len(),
            specs.len(),
            "{ctx}"
        );
        assert_eq!(why.decisions.len(), plan.placements.len(), "{ctx}");
        assert_eq!(why.unplaced.len(), plan.unplaced.len(), "{ctx}");
        let placed = placements(&plan, &specs);
        assert!(collisions(&placed).is_empty(), "{ctx}");
        assert!(outside(&p, &placed).is_empty(), "{ctx}");
        assert!(rail_overflows(&plan.rails, &placed).is_empty(), "{ctx}");
        // The ducts stay on the plate.
        for d in &plan.ducts {
            assert!(
                p.usable().contains(&duct_rect(d)) || duct_rect(d).h <= p.usable().h,
                "{ctx}"
            );
        }
        // The check finds nothing wrong with the planner's own output; with
        // the separation given up, only separation is allowed to be wrong.
        let placed_specs: Vec<Spec> = specs
            .iter()
            .filter(|s| plan.placements.iter().any(|p| p.id == s.id))
            .cloned()
            .collect();
        let v = check(&placed, &plan.ducts, &placed_specs, &rules);
        for v in &v {
            assert!(
                why.separation_relaxed && v.rule_name() == "separation",
                "{ctx}: {:?}",
                v
            );
        }
        // Rows never mix power with signal when the separation holds.
        if !why.separation_relaxed {
            for rail in &plan.rails {
                let classes: Vec<Class> = why
                    .decisions
                    .iter()
                    .filter(|d| d.rail == Some(rail.id))
                    .map(|d| d.class)
                    .collect();
                assert!(classes.windows(2).all(|w| !w[0].conflicts(w[1])), "{ctx}");
            }
        }
    }
}

// ---- the rail and duct cut list --------------------------------------------

#[test]
fn a_plan_becomes_a_cut_list_with_rails_trimmed_to_save_bars() {
    let specs: Vec<Spec> = (1..=30)
        .map(|id| spec(id, if id % 3 == 0 { "K" } else { "Q" }, 45.0, 90.0))
        .collect();
    let plate = Plate {
        width: 1010.0,
        height: 1600.0,
        margin: 5.0,
    };
    let plan = place_all_rules(&plate, &specs, &RuleSet::standard(), 1, 0.0);
    let placed = placements(&plan, &specs);
    let pieces = rail_pieces(&plan.rails, &placed);
    assert_eq!(pieces.len(), plan.rails.len());
    let cut = optimise(&pieces, Stock::default());
    // Full-length rails (1000 mm each) would need one bar per two rails;
    // the cut plan never needs more bars than that.
    let full: Vec<_> = plan
        .rails
        .iter()
        .map(|r| novo_layout::stock::Piece {
            id: r.id,
            min: r.length,
            max: r.length,
        })
        .collect();
    assert!(cut.bars_needed() <= optimise(&full, Stock::default()).bars_needed());
    assert!(cut.bars_needed() >= cut.lower_bound);
    // Every rail still reaches its devices.
    for (id, len) in &cut.lengths {
        let piece = pieces.iter().find(|p| p.id == *id).unwrap();
        assert!(*len >= piece.min - 1e-9 && *len <= piece.max + 1e-9);
    }
}

#[test]
fn a_trunk_duct_joins_every_duct_between_rows() {
    let specs = vec![
        spec(1, "Q", 18.0, 90.0),
        spec(2, "K", 45.0, 90.0),
        spec(3, "A", 100.0, 90.0),
        spec(4, "X", 60.0, 40.0),
    ];
    let rules = RuleSet::standard();
    let (plan, why) = place_all_explained(&plate(), &specs, &rules, 1, 0.0);
    assert!(why.trunk && !why.trunk_dropped);
    let trunks: Vec<&PlannedDuct> = plan.ducts.iter().filter(|d| d.vertical).collect();
    assert_eq!(trunks.len(), 1);
    let trunk = trunks[0];
    assert_eq!(trunk.class, Class::Mixed);
    assert_eq!(
        (trunk.width, trunk.height),
        (rules.duct_width, rules.duct_height)
    );
    let tr = duct_rect(trunk);
    let ducts = row_ducts(&plan);
    assert!(ducts.len() >= 2);
    for d in ducts {
        // Each duct starts on the trunk's centre line and lies within its span.
        assert_eq!(d.x, trunk.x);
        assert!(d.y >= trunk.y && d.y <= trunk.y + trunk.length);
        assert!(duct_rect(d).x <= tr.right());
    }
    // The trunk spans every row and stays on the plate.
    for p in &plan.placements {
        let s = specs.iter().find(|s| s.id == p.id).unwrap();
        assert!(p.y >= trunk.y - 1e-9 && p.y + s.size.height <= trunk.y + trunk.length + 1e-9);
    }
    assert!(tr.x >= plate().usable().x && tr.bottom() <= plate().usable().y + plate().usable().h);
    // No device or rail runs into it, and the plan still passes the check.
    for p in &plan.placements {
        assert!(p.x >= tr.right(), "device {} starts inside the trunk", p.id);
    }
    for r in &plan.rails {
        assert!(r.x >= tr.right());
    }
    assert!(check(&placements(&plan, &specs), &plan.ducts, &specs, &rules).is_empty());
}

#[test]
fn one_row_needs_no_trunk() {
    let specs = vec![spec(1, "K", 45.0, 90.0), spec(2, "K", 45.0, 90.0)];
    let (plan, why) = place_all_explained(&plate(), &specs, &RuleSet::standard(), 1, 0.0);
    assert_eq!(plan.rails.len(), 1);
    assert!(plan.ducts.is_empty() && !why.trunk);
}

#[test]
fn the_trunk_is_dropped_when_the_narrower_plate_would_lose_a_device() {
    // Two rows that fill the width exactly: the strip for a trunk would push
    // a device off, so the plan without a trunk wins and says so.
    let plate = Plate {
        width: 220.0,
        height: 400.0,
        margin: 10.0,
    };
    let specs = vec![spec(1, "Q", 200.0, 60.0), spec(2, "K", 200.0, 60.0)];
    let (plan, why) = place_all_explained(&plate, &specs, &RuleSet::standard(), 1, 0.0);
    assert!(plan.unplaced.is_empty());
    assert!(why.trunk_dropped && !why.trunk);
    assert!(plan.ducts.iter().all(|d| !d.vertical));
}
