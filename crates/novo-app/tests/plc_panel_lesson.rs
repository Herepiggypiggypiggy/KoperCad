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
}
