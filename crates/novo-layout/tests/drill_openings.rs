use novo_layout::Plate;
use novo_layout::drill::{CutoutShape, Surface, drill_table};
use novo_layout::openings::Opening;
use novo_layout::state::CabinetLayout;

fn layout(openings: &[(&str, &str)]) -> CabinetLayout {
    let mut l = CabinetLayout::new(Plate {
        width: 600.0,
        height: 800.0,
        margin: 10.0,
    });
    l.openings = openings
        .iter()
        .map(|(n, t)| Opening::parse(n, t).unwrap())
        .collect();
    l
}

#[test]
fn openings_become_numbered_cutouts_on_their_surface() {
    let l = layout(&[
        ("gland", "plate round 25 300 780"),
        ("window", "door rect 80 40 300 200"),
        ("duct", "plate rect 100 30 150 770"),
    ]);
    let t = drill_table(&l, &[]);
    let (_, plate) = t.on(&Surface::Plate);
    assert_eq!(plate.len(), 2);
    assert_eq!(plate[0].shape, CutoutShape::Rect { w: 100.0, h: 30.0 });
    assert_eq!((plate[0].no, plate[0].x, plate[0].y), (1, 150.0, 770.0));
    assert_eq!(plate[1].shape, CutoutShape::Round { dia: 25.0 });
    assert_eq!(plate[1].source, "Opening gland");
    let (_, door) = t.on(&Surface::Door);
    assert_eq!(door.len(), 1);
    assert_eq!((door[0].no, door[0].x, door[0].y), (1, 300.0, 200.0));
    assert!(t.problems.is_empty());
}

#[test]
fn an_opening_on_a_wall_or_extra_plate_gets_its_own_rows() {
    let mut l = layout(&[
        ("side", "left round 20 50 60"),
        ("vent", "left rect 80 40 120 200"),
        ("gland", "plate2 round 25 30 40"),
    ]);
    l.surface_sizes.insert("left".into(), (300.0, 900.0));
    l.surface_sizes.insert("plate2".into(), (200.0, 200.0));
    let t = drill_table(&l, &[]);
    assert!(t.problems.is_empty(), "{:?}", t.problems);
    let (_, left) = t.on(&Surface::Named("left".into()));
    assert_eq!(left.len(), 2);
    assert_eq!((left[0].no, left[0].x, left[0].y), (1, 120.0, 200.0));
    assert_eq!(left[1].shape, CutoutShape::Round { dia: 20.0 });
    let (_, extra) = t.on(&Surface::Named("plate2".into()));
    assert_eq!(extra.len(), 1);
    let (docs, _) = novo_layout::plate_drawing::documents("t", &l, &[]);
    assert!(docs.len() >= 3, "{}", docs.len());
}

#[test]
fn an_opening_on_an_unknown_surface_is_reported_not_dropped() {
    let t = drill_table(&layout(&[("side", "left round 20 50 50")]), &[]);
    assert!(t.cutouts.is_empty());
    assert_eq!(t.problems.len(), 1);
    assert!(t.problems[0].contains("side") && t.problems[0].contains("left"));
}
