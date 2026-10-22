//! The PLC panel lesson's finished sample (`assets/lessons/plc-panel/finished.json`): every part is
//! on the plate and mounted on a rail through its centre, and the layout has no problem.

use std::path::PathBuf;

use novo_core::Project;
use novo_layout::model::load;

fn finished() -> Project {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/lessons/plc-panel/finished.json");
    Project::from_json(&std::fs::read_to_string(path).expect("finished.json")).expect("loads")
}

#[test]
fn the_finished_panel_has_every_part_on_a_rail_and_no_problem() {
    let project = finished();
    let cab = project
        .structures()
        .find(|s| s.enclosure)
        .expect("cabinet")
        .id;
    let layout = load(&project, cab).expect("plate");
    let on = layout.on_plate();
    assert_eq!(on.len(), 15, "all fifteen parts are placed");
    assert_eq!(layout.rails.len(), 5);
    for p in &on {
        let rail = p
            .rail
            .unwrap_or_else(|| panic!("device {} is not on a rail", p.id));
        let r = layout.rails.iter().find(|r| r.id == rail).expect("rail");
        let off = novo_layout::mount::off_the_line(p, r).expect("on the rail's line");
        assert!(
            off.abs() < 0.5,
            "device {} is {off} mm off its rail line",
            p.id
        );
    }
    let report = layout.report();
    assert!(report.collisions.is_empty() && report.outside.is_empty());
    assert!(report.too_close.is_empty() && report.rail_overflows.is_empty());
    assert!(report.duct_overlaps.is_empty() && report.guessed.is_empty());

    // The lesson text names the rows: pin them, so the text and the sample cannot drift apart.
    let mut rows: std::collections::BTreeMap<u64, Vec<String>> = Default::default();
    for p in &on {
        let tag = project
            .devices()
            .find(|d| d.id.raw() == p.id)
            .map(|d| d.tag.to_string())
            .expect("device");
        let short = tag.rsplit('-').next().unwrap().to_string();
        rows.entry(p.rail.unwrap()).or_default().push(short);
    }
    let mut rows: Vec<Vec<String>> = rows
        .into_values()
        .map(|mut r| {
            r.sort();
            r
        })
        .collect();
    rows.sort();
    let want: Vec<Vec<&str>> = vec![
        vec!["A1", "A2", "A3"],
        vec!["F1", "F2", "F3", "G1", "Q1"],
        vec!["K1", "K2", "K3"],
        vec!["U1"],
        vec!["X1", "X2", "X3"],
    ];
    assert_eq!(rows, want, "the rails the lesson text describes");
}
