//! Wiring ducts cannot pass through each other. Every sample and lesson
//! project that ships under `assets/` must have its ducts clear of each
//! other, the Issues list must report ducts that do cross, and Place all must
//! lay ducts that meet in a T (one ends against the side of the other).

use std::path::{Path, PathBuf};

use novo_checks::ProblemKind;
use novo_core::{Project, Structure};
use novo_layout::model::load;

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

fn crossing_ducts(project: &Project) -> Vec<(u64, u64)> {
    let mut found = Vec::new();
    for s in project.all::<Structure>() {
        let Some(layout) = load(project, s.id) else {
            continue;
        };
        for o in layout.report().duct_overlaps {
            if let novo_layout::DuctHit::Duct { id, .. } = o.hit {
                found.push((o.duct, id));
            }
        }
    }
    found
}

#[test]
fn no_shipped_project_has_ducts_over_each_other() {
    let mut files = Vec::new();
    project_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"),
        &mut files,
    );
    let mut scanned = 0;
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        // Only project files; other JSON under assets is not one.
        let Ok(project) = Project::from_json(&text) else {
            continue;
        };
        scanned += 1;
        let crossing = crossing_ducts(&project);
        assert!(
            crossing.is_empty(),
            "{}: ducts over each other {crossing:?}",
            file.display()
        );
    }
    assert!(scanned >= 10, "only {scanned} projects scanned");
}

#[test]
fn ducts_that_cross_are_an_issue_and_a_t_is_not() {
    use novo_core::{DeviceTag, MountingPlate, RailKind};
    let build = |second_x: f64| {
        let mut p = Project::new("t");
        p.transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1")?, "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 600.0,
                    height: 600.0,
                    margin: 10.0,
                }),
            )?;
            // A vertical trunk 30 mm wide at x 100 ...
            let trunk = tx.add_rail(cab, 100.0, 20.0, 500.0, RailKind::Duct)?;
            tx.update::<novo_core::Rail>(trunk, |r| r.vertical = true)?;
            tx.set_rail_size(trunk, 500.0, 30.0, 60.0)?;
            // ... and a horizontal duct starting at `second_x`.
            let row = tx.add_rail(cab, second_x, 200.0, 400.0, RailKind::Duct)?;
            tx.set_rail_size(row, 400.0, 30.0, 60.0)?;
            Ok(())
        })
        .unwrap();
        p
    };
    // Starting on the trunk's centre line: it runs into the trunk.
    let crossing = build(100.0);
    assert_eq!(crossing_ducts(&crossing).len(), 1);
    assert!(
        novo_checks::check(&crossing)
            .iter()
            .any(|p| p.kind == ProblemKind::DuctOverlap && p.message.contains("crosses duct"))
    );
    // Ending against the trunk's side (x 115): a T junction, clear.
    let tee = build(115.0);
    assert!(crossing_ducts(&tee).is_empty());
    assert!(
        !novo_checks::check(&tee)
            .iter()
            .any(|p| p.kind == ProblemKind::DuctOverlap)
    );
}
