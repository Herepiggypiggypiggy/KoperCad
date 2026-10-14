//! Junction styles and Tab cycling.

use novo_core::*;

#[test]
fn tab_cycles_junction_styles() {
    let mut doc = Document::default();
    let (junction, _) = doc
        .transact("Junction", |tx| {
            let page = tx.add_page("1")?;
            tx.add_junction(page, Point::new(10.0, 10.0))
        })
        .unwrap();
    let style = |doc: &Document| doc.project().junction(junction).unwrap().style;
    assert_eq!(style(&doc), JunctionStyle::Dot);

    let mut seen = vec![style(&doc)];
    for _ in 0..JunctionStyle::ALL.len() {
        doc.execute(Command::CycleJunctionStyle { junction })
            .unwrap();
        seen.push(style(&doc));
    }
    // Every style once, then back to the dot.
    assert_eq!(&seen[..9], JunctionStyle::ALL);
    assert_eq!(seen[9], JunctionStyle::Dot);

    doc.execute(Command::SetJunctionStyle {
        junction,
        style: JunctionStyle::TDown,
    })
    .unwrap();
    assert_eq!(
        JunctionStyle::TDown.sides(),
        [Direction::Left, Direction::Right, Direction::Down]
    );
    let json = doc.project().to_json();
    assert!(json.contains("\"style\": \"t_down\""));
    let loaded = Project::from_json(&json).unwrap();
    assert_eq!(
        loaded.junction(junction).unwrap().style,
        JunctionStyle::TDown
    );

    doc.undo().unwrap();
    assert_eq!(style(&doc), JunctionStyle::Dot);
    // Dots are not written, so older files and dots look the same.
    assert!(!doc.project().to_json().contains("\"style\""));
}
