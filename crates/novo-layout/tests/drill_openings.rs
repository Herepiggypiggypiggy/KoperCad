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
    let (_, plate) = t.on(Surface::Plate);
    assert_eq!(plate.len(), 2);
    assert_eq!(plate[0].shape, CutoutShape::Rect { w: 100.0, h: 30.0 });
    assert_eq!((plate[0].no, plate[0].x, plate[0].y), (1, 150.0, 770.0));
    assert_eq!(plate[1].shape, CutoutShape::Round { dia: 25.0 });
    assert_eq!(plate[1].source, "Opening gland");
    let (_, door) = t.on(Surface::Door);
    assert_eq!(door.len(), 1);
    assert_eq!((door[0].no, door[0].x, door[0].y), (1, 300.0, 200.0));
    assert!(t.problems.is_empty());
}

#[test]
fn an_opening_on_another_surface_is_reported_not_dropped() {
    let t = drill_table(&layout(&[("side", "left round 20 50 50")]), &[]);
    assert!(t.cutouts.is_empty());
    assert_eq!(t.problems.len(), 1);
    assert!(t.problems[0].contains("side") && t.problems[0].contains("left"));
}
