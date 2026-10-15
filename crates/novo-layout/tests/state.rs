use novo_layout::plan::Candidate;
use novo_layout::state::CabinetLayout;
use novo_layout::{Plate, Size};

fn layout() -> CabinetLayout {
    CabinetLayout::new(Plate {
        width: 400.0,
        height: 400.0,
        margin: 10.0,
    })
}

fn cand(id: u64, w: f64, h: f64) -> Candidate {
    Candidate {
        id,
        size: Size::from_part(Some(w), Some(h)),
        sort_key: "K".into(),
    }
}

#[test]
fn drop_snaps_to_the_grid_and_moving_again_does_not_duplicate() {
    let mut l = layout();
    let size = Size::from_part(Some(45.0), Some(90.0));
    l.drop_device(1, size, 22.0, 33.0);
    assert_eq!((l.placements[0].x, l.placements[0].y), (20.0, 35.0));
    l.drop_device(1, size, 100.0, 100.0);
    assert_eq!(l.placements.len(), 1);
}

#[test]
fn overlap_and_edges_are_reported() {
    let mut l = layout();
    let size = Size::from_part(Some(45.0), Some(90.0));
    l.drop_device(1, size, 50.0, 50.0);
    l.drop_device(2, size, 60.0, 50.0);
    l.drop_device(3, size, 0.0, 0.0);
    let r = l.report();
    assert_eq!(r.collisions, vec![(1, 2)]);
    assert_eq!(r.outside, vec![3]);
    assert!(!r.is_clean());
}

#[test]
fn guessed_sizes_are_listed() {
    let mut l = layout();
    l.drop_device(1, Size::from_part(None, None), 50.0, 50.0);
    assert_eq!(l.report().guessed, vec![1]);
}

#[test]
fn drawer_shrinks_as_devices_are_placed_and_rotate_remove_work() {
    let mut l = layout();
    let all = vec![cand(1, 45.0, 90.0), cand(2, 45.0, 90.0)];
    assert_eq!(l.unplaced(&all).len(), 2);
    l.drop_device(1, all[0].size, 50.0, 50.0);
    assert_eq!(
        l.unplaced(&all).iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![2]
    );
    l.rotate(1);
    assert!(l.placements[0].rotated);
    l.remove(1);
    assert_eq!(l.unplaced(&all).len(), 2);
}

#[test]
fn place_all_preview_then_accept_places_the_rest() {
    let mut l = layout();
    let all: Vec<_> = (1..=6).map(|i| cand(i, 45.0, 90.0)).collect();
    l.drop_device(1, all[0].size, 20.0, 300.0);
    let plan = l.preview(&all);
    assert_eq!(plan.placements.len(), 5);
    l.accept(&plan, &all);
    assert_eq!(l.placements.len(), 6);
    assert!(l.unplaced(&all).is_empty());
    assert!(l.report().rail_overflows.is_empty());
}
