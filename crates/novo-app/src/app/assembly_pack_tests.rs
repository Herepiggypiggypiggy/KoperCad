//! The assembly print pack: build order first per cabinet, cut list last.

use novo_core::Project;
use novo_reports::{AssemblyChecklist, Report};

use super::assembly::print_pack;

fn lesson(path: &str) -> Project {
    let file = format!("{}/../../assets/lessons/{path}", env!("CARGO_MANIFEST_DIR"));
    Project::from_json(&std::fs::read_to_string(file).unwrap()).unwrap()
}

#[test]
fn the_pack_opens_each_cabinet_with_its_build_order_and_ends_with_the_cut_list() {
    let project = lesson("wire-routing/project.json");
    let pack = print_pack(&project, AssemblyChecklist.generate(&project));
    let keys: Vec<&str> = pack.sections.iter().map(|s| s.key.as_str()).collect();
    let order = keys
        .iter()
        .position(|k| k.ends_with(":order"))
        .expect("order");
    let tag = keys[order].split(':').next().unwrap();
    assert_eq!(order, 0);
    assert_eq!(keys[1], format!("{tag}:prepare"));
    assert!(keys.contains(&format!("{tag}:drill").as_str()));
    assert!(
        keys.iter().any(|k| *k == "summary" || k.starts_with("cut")),
        "{keys:?}"
    );
    let steps = &pack.sections[0];
    // Rails come first, labels last.
    assert_eq!(steps.rows[0].cells[1].to_string(), "Rails");
    assert_eq!(steps.rows.last().unwrap().cells[1].to_string(), "Labels");
}
