//! Every shipped project that has a cabinet: each wire the router finds a way
//! for runs inside the ducts between its two terminals; the ones it cannot
//! are reported as free air (listed in Issues), never passed off as routed.

use std::path::{Path, PathBuf};

use novo_core::{Project, Rail, RailKind};
use novo_routing::Kind;
use novo_routing::model::route_project;

fn project_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            project_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "json") {
            out.push(path);
        }
    }
}

fn in_a_duct(rects: &[(f64, f64, f64, f64)], x: f64, y: f64) -> bool {
    rects
        .iter()
        .any(|r| x >= r.0 - 0.5 && x <= r.2 + 0.5 && y >= r.1 - 0.5 && y <= r.3 + 0.5)
}

#[test]
fn ducted_wires_of_every_shipped_cabinet_lie_in_the_ducts() {
    let mut files = Vec::new();
    project_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"),
        &mut files,
    );
    let (mut cabinets, mut wires) = (0, 0);
    for file in files {
        let Ok(project) = Project::from_json(&std::fs::read_to_string(&file).unwrap()) else {
            continue;
        };
        let rects: Vec<(f64, f64, f64, f64)> = project
            .all::<Rail>()
            .filter(|r| r.kind == RailKind::Duct)
            .map(|r| {
                let h = r.width / 2.0;
                if r.vertical {
                    (r.x - h, r.y, r.x + h, r.y + r.length)
                } else {
                    (r.x, r.y - h, r.x + r.length, r.y + h)
                }
            })
            .collect();
        if rects.is_empty() {
            continue;
        }
        cabinets += 1;
        for (id, r) in route_project(&project).routes {
            if r.kind != Kind::Ducted {
                continue;
            }
            wires += 1;
            let n = r.points.len();
            for (i, p) in r.points.iter().enumerate() {
                // The first two and last two points are the terminal and
                // its stub corner.
                if i >= 2 && i + 2 < n {
                    assert!(
                        in_a_duct(&rects, p.x, p.y),
                        "{}: wire {id} point {i} ({}, {}) lies outside the ducts",
                        file.display(),
                        p.x,
                        p.y
                    );
                }
            }
        }
    }
    assert!(
        cabinets >= 3 && wires >= 10,
        "{cabinets} cabinets, {wires} wires"
    );
}
