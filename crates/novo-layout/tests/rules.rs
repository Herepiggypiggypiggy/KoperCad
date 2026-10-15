use novo_core::{DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::plan::{Candidate, Plan, ROW_GAP_MM};
use novo_layout::rules::{
    Gaps, HotRule, KindRule, RailUse, RuleSet, Spec, Zone, kind_of, place_all_rules,
};
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
        width: 420.0,
        height: 700.0,
        margin: 10.0,
    }
}

fn at(plan: &Plan, id: u64) -> (f64, f64, u64) {
    let p = plan.placements.iter().find(|p| p.id == id).unwrap();
    (p.x, p.y, p.rail)
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
            rail: Some(p.rail),
        })
        .collect()
}

#[test]
fn the_standard_rules_put_protection_on_top_and_terminals_at_the_bottom() {
    let specs = vec![
        spec(1, "X", 6.0, 60.0),
        spec(2, "K", 45.0, 90.0),
        spec(3, "Q", 18.0, 90.0),
        spec(4, "A", 100.0, 120.0),
        spec(5, "F", 18.0, 90.0),
    ];
    let p = plate();
    let plan = place_all_rules(&p, &specs, &RuleSet::standard(), 1, 0.0);
    assert!(plan.unplaced.is_empty());
    let usable = p.usable();
    // Q and F share the top rail, Q left of F as the rule lists them.
    let (qx, qy, qr) = at(&plan, 3);
    let (fx, fy, fr) = at(&plan, 5);
    assert_eq!((qy, qr), (usable.y, 1));
    assert_eq!((fy, fr), (usable.y, 1));
    assert!(qx < fx);
    // The controller row comes before the relay row (rule order), both
    // between the protection and the terminals.
    let (_, ay, _) = at(&plan, 4);
    let (_, ky, _) = at(&plan, 2);
    assert!(qy + 90.0 + ROW_GAP_MM <= ay && ay < ky);
    // The terminals sit against the bottom edge.
    let (_, xy, _) = at(&plan, 1);
    assert_eq!(xy + 60.0, usable.y + usable.h);
    // Rails are numbered top to bottom.
    let ys: Vec<f64> = plan.rails.iter().map(|r| r.y).collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(
        plan.rails.iter().map(|r| r.id).collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
}

#[test]
fn an_own_rail_kind_never_shares_a_rail_and_a_shared_kind_does() {
    let specs = vec![
        spec(1, "K", 45.0, 90.0),
        spec(2, "A", 45.0, 90.0),
        spec(3, "Q", 45.0, 90.0),
        spec(4, "F", 45.0, 90.0),
    ];
    let plan = place_all_rules(&plate(), &specs, &RuleSet::standard(), 1, 0.0);
    assert_ne!(at(&plan, 1).2, at(&plan, 2).2);
    assert_eq!(at(&plan, 3).2, at(&plan, 4).2);
}

#[test]
fn unknown_kinds_follow_the_star_rule_and_the_longest_code_wins() {
    let mut rules = RuleSet::standard();
    rules.rules.push(KindRule {
        codes: vec!["QA".into()],
        zone: Zone::Bottom,
        rail: RailUse::Own,
        gap: None,
        above: None,
        below: None,
    });
    let specs = vec![
        spec(1, "QA", 45.0, 90.0),
        spec(2, "Q", 45.0, 90.0),
        spec(3, "ZZ", 45.0, 90.0),
    ];
    let p = plate();
    let plan = place_all_rules(&p, &specs, &rules, 1, 0.0);
    // QA is the bottom rule, plain Q is on top, ZZ falls to the middle.
    let (_, qa, _) = at(&plan, 1);
    let (_, q, _) = at(&plan, 2);
    let (_, zz, _) = at(&plan, 3);
    assert!(q < zz && zz < qa);
    assert_eq!(qa + 90.0, p.usable().y + p.usable().h);
}

#[test]
fn a_hot_device_goes_to_the_hot_zone_whatever_its_kind() {
    let mut rules = RuleSet::standard();
    rules.hot = Some(HotRule {
        min_loss_w: 40.0,
        zone: Zone::Bottom,
    });
    let mut psu = spec(1, "G", 60.0, 120.0);
    psu.loss_w = Some(55.0);
    let mut cool = spec(2, "G", 60.0, 120.0);
    cool.loss_w = Some(5.0);
    let unknown = spec(3, "G", 60.0, 120.0);
    let p = plate();
    let plan = place_all_rules(&p, &[psu, cool, unknown], &rules, 1, 0.0);
    let usable = p.usable();
    assert_eq!(at(&plan, 1).1 + 120.0, usable.y + usable.h);
    // Below the threshold or with no loss data stays where its kind says.
    assert_eq!(at(&plan, 2).1, usable.y);
    assert_eq!(at(&plan, 3).1, usable.y);
}

#[test]
fn part_gaps_widen_the_space_between_neighbours() {
    let mut a = spec(1, "K", 45.0, 90.0);
    a.gaps.right = Some(30.0);
    let b = spec(2, "K", 45.0, 90.0);
    let mut c = spec(3, "K", 45.0, 90.0);
    c.gaps.left = Some(5.0);
    let plan = place_all_rules(&plate(), &[a, b, c], &RuleSet::standard(), 1, 0.0);
    let (ax, ..) = at(&plan, 1);
    let (bx, ..) = at(&plan, 2);
    let (cx, ..) = at(&plan, 3);
    assert_eq!(bx - (ax + 45.0), 30.0);
    // b asks for the default on its right and c for 5 on its left: the
    // larger, the default, rules.
    assert_eq!(cx - (bx + 45.0), 10.0);
}

#[test]
fn rule_gaps_and_vertical_needs_apply() {
    let mut rules = RuleSet::standard();
    rules.rules[1].gap = Some(20.0);
    rules.rules[1].below = Some(90.0);
    let mut specs = vec![spec(1, "K", 45.0, 90.0), spec(2, "K", 45.0, 90.0)];
    // Make the second K too wide to share the first one's rail.
    specs[1].size = Size::from_part(Some(380.0), Some(90.0));
    let plan = place_all_rules(&plate(), &specs, &rules, 1, 0.0);
    let (_, y1, r1) = at(&plan, 1);
    let (_, y2, r2) = at(&plan, 2);
    assert_ne!(r1, r2);
    // The row's own 90 mm below-need beats the 40 mm row gap.
    assert_eq!(y2 - (y1 + 90.0), 90.0);

    let mut same = vec![spec(1, "K", 45.0, 90.0), spec(2, "K", 45.0, 90.0)];
    same[0].gaps.right = Some(0.0);
    let plan = place_all_rules(&plate(), &same, &rules, 1, 0.0);
    let (x1, ..) = at(&plan, 1);
    let (x2, ..) = at(&plan, 2);
    // The rule's gap of 20 applies where the part says nothing.
    assert_eq!(x2 - (x1 + 45.0), 20.0);
}

#[test]
fn a_mixed_plan_has_no_collisions_overflows_or_off_plate_devices() {
    let kinds = [
        "Q", "F", "G", "A", "K", "K", "K", "K", "K", "T", "U", "X", "X", "X", "X",
    ];
    let specs: Vec<Spec> = kinds
        .iter()
        .enumerate()
        .map(|(i, k)| spec(i as u64 + 1, k, if *k == "X" { 6.0 } else { 45.0 }, 70.0))
        .collect();
    let p = plate();
    let plan = place_all_rules(&p, &specs, &RuleSet::standard(), 1, 0.0);
    assert!(plan.unplaced.is_empty());
    assert_eq!(plan.placements.len(), specs.len());
    let ps = placements(&plan, &specs);
    assert!(collisions(&ps).is_empty());
    assert!(outside(&p, &ps).is_empty());
    assert!(rail_overflows(&plan.rails, &ps).is_empty());
}

#[test]
fn a_plate_that_is_too_small_keeps_the_bottom_rows_and_reports_the_rest() {
    let p = Plate {
        width: 220.0,
        height: 330.0,
        margin: 10.0,
    };
    let mut specs: Vec<Spec> = (1..=6).map(|i| spec(i, "K", 45.0, 90.0)).collect();
    specs.extend((7..=9).map(|i| spec(i, "X", 6.0, 60.0)));
    let plan = place_all_rules(&p, &specs, &RuleSet::standard(), 1, 0.0);
    // Every terminal is placed; some relays are not; nothing disappears.
    assert!((7..=9).all(|id| plan.placements.iter().any(|q| q.id == id)));
    assert!(!plan.unplaced.is_empty());
    let mut all: Vec<u64> = plan
        .placements
        .iter()
        .map(|q| q.id)
        .chain(plan.unplaced.iter().copied())
        .collect();
    all.sort();
    assert_eq!(all, (1..=9).collect::<Vec<_>>());
    let ps = placements(&plan, &specs);
    assert!(collisions(&ps).is_empty());
    assert!(outside(&p, &ps).is_empty());
}

#[test]
fn several_bottom_rows_under_top_rows_never_leave_the_plate() {
    // One device per row (300 of 400 mm usable width): a protection row on
    // top, then bottom rows that do not all fit. The gap under the top row
    // counts for every bottom row, not only the first.
    let p = Plate {
        width: 420.0,
        height: 400.0,
        margin: 10.0,
    };
    let mut specs = vec![spec(1, "Q", 18.0, 90.0)];
    specs.extend((2..=5).map(|i| spec(i, "X", 300.0, 60.0)));
    let plan = place_all_rules(&p, &specs, &RuleSet::standard(), 1, 0.0);
    let ps = placements(&plan, &specs);
    assert!(!plan.unplaced.is_empty());
    assert!(plan.placements.iter().any(|q| q.id == 1));
    assert!(outside(&p, &ps).is_empty());
    assert!(collisions(&ps).is_empty());
    let limit = p.usable().y + p.usable().h;
    for q in &plan.placements {
        let h = specs.iter().find(|s| s.id == q.id).unwrap().size.height;
        assert!(q.y + h <= limit + 1e-9, "id {} ends at {}", q.id, q.y + h);
    }
}

#[test]
fn a_device_wider_than_the_plate_is_unplaced() {
    let specs = vec![spec(1, "K", 500.0, 90.0), spec(2, "K", 45.0, 90.0)];
    let plan = place_all_rules(&plate(), &specs, &RuleSet::standard(), 1, 0.0);
    assert_eq!(plan.unplaced, vec![1]);
    assert_eq!(plan.placements.len(), 1);
}

#[test]
fn the_plan_can_sit_below_devices_already_placed() {
    let specs = vec![spec(1, "Q", 45.0, 90.0), spec(2, "X", 6.0, 60.0)];
    let plan = place_all_rules(&plate(), &specs, &RuleSet::standard(), 7, 200.0);
    assert_eq!(at(&plan, 1).1, 200.0);
    assert_eq!(at(&plan, 1).2, 7);
    assert_eq!(plan.rails[0].id, 7);
}

#[test]
fn the_same_input_gives_the_same_plan() {
    let specs: Vec<Spec> = (1..=20)
        .map(|i| spec(i, ["Q", "K", "X", "A"][i as usize % 4], 30.0, 80.0))
        .collect();
    let rules = RuleSet::standard();
    let a = place_all_rules(&plate(), &specs, &rules, 1, 0.0);
    let b = place_all_rules(&plate(), &specs, &rules, 1, 0.0);
    assert_eq!(a, b);
}

#[test]
fn rules_round_trip_through_text() {
    let std = RuleSet::standard();
    assert_eq!(RuleSet::parse(&std.to_text()).unwrap(), std);
    let mut set = RuleSet::standard();
    set.gap = 7.5;
    set.row_gap = 55.0;
    set.hot = Some(HotRule {
        min_loss_w: 25.0,
        zone: Zone::Top,
    });
    set.rules[1].gap = Some(12.0);
    set.rules[1].above = Some(30.0);
    assert_eq!(RuleSet::parse(&set.to_text()).unwrap(), set);
}

#[test]
fn rule_text_accepts_comments_commas_and_any_case() {
    let text = "# my rules\nGAP 8,5   # tighter\nrow_gap 50\nhot 30 low\n\n\
                rule q f zone=HIGH rail=Shared above=20\nrule * zone=mid\n";
    let set = RuleSet::parse(text).unwrap();
    assert_eq!(set.gap, 8.5);
    assert_eq!(set.row_gap, 50.0);
    assert_eq!(
        set.hot,
        Some(HotRule {
            min_loss_w: 30.0,
            zone: Zone::Bottom
        })
    );
    assert_eq!(set.rules[0].codes, ["Q", "F"]);
    assert_eq!(set.rules[0].zone, Zone::Top);
    assert_eq!(set.rules[0].rail, RailUse::Shared);
    assert_eq!(set.rules[0].above, Some(20.0));
    assert_eq!(set.rules[1].zone, Zone::Middle);
}

#[test]
fn bad_rule_lines_name_their_line_number() {
    let err = |text: &str| RuleSet::parse(text).unwrap_err();
    let e = err("gap 10\nrule Q zone=sideways\n");
    assert_eq!(e.line, 2);
    assert!(e.message.contains("sideways"), "{e}");
    assert_eq!(err("gap -3").line, 1);
    assert_eq!(err("gap ten").line, 1);
    assert_eq!(err("hot 30").line, 1);
    assert_eq!(err("rule zone=top").line, 1);
    assert_eq!(err("rule Q colour=red").line, 1);
    assert_eq!(err("paint it black").line, 1);
    assert_eq!(err("\n\nrule Q rail=maybe").line, 3);
}

#[test]
fn an_empty_rule_file_has_no_rules_and_everything_falls_in_the_middle() {
    let set = RuleSet::parse("").unwrap();
    assert!(set.rules.is_empty());
    let specs = vec![spec(1, "X", 6.0, 60.0), spec(2, "Q", 18.0, 90.0)];
    let p = plate();
    let plan = place_all_rules(&p, &specs, &set, 1, 0.0);
    // Both are shared in the middle, so they sit on one rail from the top.
    assert_eq!(at(&plan, 1).2, at(&plan, 2).2);
    assert_eq!(at(&plan, 1).1, p.usable().y);
}

#[test]
fn candidates_become_specs_with_their_kind() {
    let c = Candidate {
        id: 4,
        size: Size::from_part(Some(45.0), Some(90.0)),
        sort_key: "QA000012".into(),
    };
    assert_eq!(kind_of(&c.sort_key), "QA");
    let s = Spec::from_candidate(&c);
    assert_eq!(s.kind, "QA");
    assert_eq!(s.gaps, Gaps::default());
    assert_eq!(s.loss_w, None);
}

fn project() -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "VFD")?;
            tx.set_property(part, "width_mm", Some(Value::Float(70.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(180.0)))?;
            tx.set_property(part, "clearance_above_mm", Some(Value::Int(100)))?;
            tx.set_property(part, "clearance_below_mm", Some(Value::Int(100)))?;
            tx.set_property(part, "power_loss_w", Some(Value::Float(80.0)))?;
            tx.set_property(part, "load_factor", Some(Value::Float(0.5)))?;
            let plain = tx.add_part("ACME", "RELAY")?;
            tx.set_property(plain, "width_mm", Some(Value::Float(15.0)))?;
            tx.set_property(plain, "height_mm", Some(Value::Float(80.0)))?;
            for (tag, part) in [
                ("=A1+CAB1-U1", part),
                ("=A1+CAB1-K1", plain),
                ("=A1+CAB1-K2", plain),
                ("=A1+FIELD-M1", part),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
            }
            tx.execute(novo_core::Command::SetPlateSize {
                structure: cab,
                plate: Some(MountingPlate {
                    width: 420.0,
                    height: 700.0,
                    margin: 10.0,
                }),
            })?;
            Ok(cab)
        })
        .unwrap();
    (p, cab)
}

#[test]
fn specs_read_kind_part_gaps_and_loss_from_the_project() {
    let (mut p, cab) = project();
    let specs = novo_layout::rules::model::specs(&p, cab);
    assert_eq!(specs.len(), 3); // not the FIELD motor
    let u1 = specs.iter().find(|s| s.kind == "U").unwrap();
    assert_eq!(u1.gaps.above, Some(100.0));
    assert_eq!(u1.gaps.left, None);
    assert_eq!(u1.loss_w, Some(40.0));
    assert_eq!(u1.size.width, 70.0);
    let k = specs.iter().find(|s| s.kind == "K").unwrap();
    assert_eq!(k.gaps, Gaps::default());
    assert_eq!(k.loss_w, None);

    // A device's own value beats its part's; a negative value counts as missing.
    let id = novo_core::AnyId::Device(novo_core::DeviceId::from(u1.id));
    p.transact("override", |tx| {
        tx.set_property(id, "clearance_above_mm", Some(Value::Int(60)))?;
        tx.set_property(id, "clearance_below_mm", Some(Value::Float(-1.0)))
    })
    .unwrap();
    let u1 = novo_layout::rules::model::specs(&p, cab)
        .into_iter()
        .find(|s| s.kind == "U")
        .unwrap();
    assert_eq!(u1.gaps.above, Some(60.0));
    assert_eq!(u1.gaps.below, Some(100.0));
}

#[test]
fn a_project_plan_applies_in_one_undo_step_and_stays_clean() {
    use novo_layout::model::{apply_plan, load};
    let (mut p, cab) = project();
    let specs = novo_layout::rules::model::specs(&p, cab);
    let plate = {
        let layout = load(&p, cab).unwrap();
        layout.plate.clone()
    };
    let plan = place_all_rules(&plate, &specs, &RuleSet::standard(), 1, 0.0);
    assert!(plan.unplaced.is_empty());
    let (_, applied) = p
        .transact("place", |tx| apply_plan(tx, cab, &plan))
        .unwrap();
    let layout = load(&p, cab).unwrap();
    assert_eq!(layout.placements.len(), 3);
    assert!(layout.report().is_clean());
    p.apply(&applied.inverse()).unwrap();
    assert!(load(&p, cab).unwrap().placements.is_empty());
}
