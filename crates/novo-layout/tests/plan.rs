use novo_layout::plan::{Candidate, ROW_GAP_MM, place_all};
use novo_layout::{
    Placement, Plate, Size, clearance_violations, collisions, outside, rail_overflows,
};

fn cand(id: u64, key: &str, w: f64, h: f64) -> Candidate {
    Candidate {
        id,
        size: Size::from_part(Some(w), Some(h)),
        sort_key: key.into(),
    }
}

fn placements(plan: &novo_layout::plan::Plan, cands: &[Candidate]) -> Vec<Placement> {
    plan.placements
        .iter()
        .map(|p| Placement {
            id: p.id,
            x: p.x,
            y: p.y,
            rotated: false,
            size: cands.iter().find(|c| c.id == p.id).unwrap().size,
            clearance: None,
            rail: Some(p.rail),
        })
        .collect()
}

#[test]
fn fills_a_row_then_starts_a_new_rail() {
    let plate = Plate {
        width: 220.0,
        height: 400.0,
        margin: 10.0,
    };
    let cands: Vec<_> = (1..=5).map(|i| cand(i, "K", 45.0, 90.0)).collect();
    let plan = place_all(&plate, &cands, 1);
    // usable width 200: 4 x 45 + 3 x 10 = 210 > 200, so 3 per row.
    assert_eq!(plan.rails.len(), 2);
    assert_eq!(plan.placements.iter().filter(|p| p.rail == 1).count(), 3);
    assert_eq!(plan.placements[3].y, plan.rails[0].y + 90.0 + ROW_GAP_MM);
    assert!(plan.unplaced.is_empty());
}

#[test]
fn the_plan_has_no_collisions_clearance_or_overflow_problems() {
    let plate = Plate {
        width: 400.0,
        height: 500.0,
        margin: 10.0,
    };
    let cands: Vec<_> = (1..=12).map(|i| cand(i, "K", 45.0, 90.0)).collect();
    let plan = place_all(&plate, &cands, 1);
    let ps = placements(&plan, &cands);
    assert!(collisions(&ps).is_empty());
    assert!(clearance_violations(&ps).is_empty());
    assert!(outside(&plate, &ps).is_empty());
    assert!(rail_overflows(&plan.rails, &ps).is_empty());
}

#[test]
fn groups_by_sort_key_and_reports_what_does_not_fit() {
    let plate = Plate {
        width: 120.0,
        height: 120.0,
        margin: 10.0,
    };
    let cands = vec![
        cand(1, "Q", 45.0, 90.0),
        cand(2, "F", 45.0, 90.0),
        cand(3, "K", 300.0, 20.0),
        cand(4, "K", 45.0, 90.0),
    ];
    let plan = place_all(&plate, &cands, 5);
    assert_eq!(plan.placements[0].id, 2); // F first
    assert_eq!(plan.placements[0].rail, 5);
    // 3 is too wide; 4 shares the first row (45 + 10 + 45 = 100); 1 would need
    // a second row, which does not fit.
    assert_eq!(
        plan.placements.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![2, 4]
    );
    assert_eq!(plan.unplaced, vec![3, 1]);
}
