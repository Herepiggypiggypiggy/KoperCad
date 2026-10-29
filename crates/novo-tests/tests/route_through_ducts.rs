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

/// The footprints of the placed devices, `(x0, y0, x1, y1)`.
fn device_bodies(project: &Project) -> Vec<(f64, f64, f64, f64)> {
    project
        .all::<novo_core::CabinetPlacement>()
        .filter_map(|p| {
            let size = novo_layout::model::device_size(project, project.device(p.device)?);
            let (w, h) = if p.rotated {
                (size.height, size.width)
            } else {
                (size.width, size.height)
            };
            Some((p.x, p.y, p.x + w, p.y + h))
        })
        .collect()
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
        let bodies = device_bodies(&project);
        for (id, r) in route_project(&project).routes {
            if r.kind != Kind::Ducted {
                continue;
            }
            wires += 1;
            let n = r.points.len();
            let at = |i: usize| (r.points[i].x, r.points[i].y);
            // Between the stubs from the two terminals the wire lies in a
            // duct, or along the channel of a DIN rail.
            let first = (0..n).find(|&i| in_a_duct(&rects, at(i).0, at(i).1));
            let last = (0..n).rev().find(|&i| in_a_duct(&rects, at(i).0, at(i).1));
            if r.rails.is_empty() {
                let (first, last) = (
                    first
                        .unwrap_or_else(|| panic!("{}: wire {id} touches no duct", file.display())),
                    last.unwrap(),
                );
                for i in first..=last {
                    assert!(
                        in_a_duct(&rects, at(i).0, at(i).1),
                        "{}: wire {id} point {i} ({}, {}) lies outside the ducts",
                        file.display(),
                        r.points[i].x,
                        r.points[i].y
                    );
                }
            }
            // No stretch of an unblocked wire passes through a device,
            // other than the first and last, which leave their own.
            if r.blocked {
                continue;
            }
            for i in 0..n - 1 {
                if i == 0 || i + 2 == n {
                    continue;
                }
                let (a, b) = (at(i), at(i + 1));
                for (x0, y0, x1, y1) in &bodies {
                    let cuts = a.0.max(b.0) > x0 + 1.0
                        && a.0.min(b.0) < x1 - 1.0
                        && a.1.max(b.1) > y0 + 1.0
                        && a.1.min(b.1) < y1 - 1.0;
                    assert!(
                        !cuts,
                        "{}: wire {id} segment {i} {a:?} to {b:?} passes through a device",
                        file.display()
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
