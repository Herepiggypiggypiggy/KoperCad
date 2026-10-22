use novo_layout::enclosure::{Door, Enclosure};
use novo_layout::rules::Spec;
use novo_layout::surface_rules::*;
use novo_layout::surfaces::Surfaces;
use novo_layout::{Plate, Size};

fn spec(id: u64, kind: &str, w: f64, h: f64) -> Spec {
    Spec {
        id,
        size: Size::from_part(Some(w), Some(h)),
        sort_key: format!("{kind}{id:06}"),
        kind: kind.into(),
        gaps: Default::default(),
        loss_w: None,
        limit_c: None,
    }
}

fn surfaces(door: bool) -> Surfaces {
    let e = Enclosure {
        width: 600.0,
        height: 800.0,
        depth: 250.0,
        plate_setback: 20.0,
        door: door.then(Door::default),
        front_clearance: None,
        hinge_side_clearance: None,
    };
    let plate = Plate {
        width: 560.0,
        height: 760.0,
        margin: 10.0,
    };
    Surfaces::from_enclosure(&e, &plate, &[]).0
}

#[test]
fn rules_parse_and_the_longest_code_wins() {
    let (r, p) = SurfaceRules::parse("# panel\ndoor: H, S P\nplate: SQ\n");
    assert!(p.is_empty());
    assert_eq!(r.surface_for("H"), Some("door"));
    assert_eq!(r.surface_for("sb"), Some("door"));
    assert_eq!(r.surface_for("SQ"), Some("plate"));
    assert_eq!(r.surface_for("K"), None);
}

#[test]
fn unreadable_lines_are_sentences_with_their_number() {
    let (r, p) = SurfaceRules::parse("door: H\nnonsense\n: S\ndoor:\n");
    assert_eq!(r.rules.len(), 1);
    assert_eq!(p.len(), 3);
    assert!(p[0].contains("line 2") && p[2].contains("line 4"));
}

#[test]
fn devices_are_split_by_rule_and_unknown_surfaces_are_reported_once() {
    let (rules, _) = SurfaceRules::parse("door: H S\nroofx: P\n");
    let specs = [
        spec(1, "K", 45.0, 90.0),
        spec(2, "H", 22.0, 22.0),
        spec(3, "S", 22.0, 22.0),
        spec(4, "P", 48.0, 48.0),
        spec(5, "P", 48.0, 48.0),
    ];
    let s = split(&specs, &rules, &surfaces(true));
    assert_eq!(
        s.plate.iter().map(|x| x.id).collect::<Vec<_>>(),
        vec![1, 4, 5]
    );
    assert_eq!(s.others.len(), 1);
    assert_eq!(s.others[0].0, "door");
    assert_eq!(s.others[0].1.len(), 2);
    assert_eq!(s.problems.len(), 1);
    assert!(s.problems[0].contains("'roofx'"));
}

#[test]
fn without_a_door_the_door_devices_stay_on_the_plate_with_a_sentence() {
    let (rules, _) = SurfaceRules::parse("door: H\n");
    let s = split(&[spec(2, "H", 22.0, 22.0)], &rules, &surfaces(false));
    assert_eq!(s.plate.len(), 1);
    assert!(s.others.is_empty());
    assert_eq!(s.problems.len(), 1);
}

#[test]
fn the_door_plan_fills_the_door_without_rails_and_leaves_out_what_does_not_fit() {
    let specs = [
        spec(2, "H", 22.0, 22.0),
        spec(3, "S", 22.0, 22.0),
        spec(4, "P", 700.0, 22.0),
    ];
    let plan = plan_surface(&surfaces(true), "door", &specs).unwrap();
    assert_eq!(plan.placements.len(), 2);
    assert!(plan.rails.is_empty() && plan.placements.iter().all(|p| p.rail == 0));
    assert_eq!(plan.unplaced, vec![4]);
    assert!(plan.placements.iter().all(|p| p.x >= 10.0 && p.y >= 10.0));
    assert!(plan_surface(&surfaces(true), "nowhere", &specs).is_none());
}
