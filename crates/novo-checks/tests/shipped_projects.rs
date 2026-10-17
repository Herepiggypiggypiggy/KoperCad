//! The shipped lesson and example projects must not light up the
//! EPLAN-style checks that every project runs by default.

use std::path::{Path, PathBuf};

use novo_checks::{ProblemKind, check};
use novo_core::Project;

fn json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            json_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "json") {
            out.push(path);
        }
    }
}

#[test]
fn shipped_projects_have_no_eplan_style_problems() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    json_files(&root.join("assets/lessons"), &mut files);
    json_files(&root.join("docs/examples"), &mut files);
    let watched = [
        ProblemKind::AmbiguousInterruption,
        ProblemKind::WireNumber,
        ProblemKind::UntaggedDevice,
        ProblemKind::UnnamedTerminal,
        ProblemKind::DuplicatePage,
        ProblemKind::DanglingJunction,
        ProblemKind::UnwiredTerminal,
        ProblemKind::StrayWireEnd,
        ProblemKind::ContactType,
        ProblemKind::ContactPosition,
        ProblemKind::ContactDrawnTwice,
        ProblemKind::VoltageClassMix,
        ProblemKind::ConductorContinuity,
        ProblemKind::MotorPhases,
        ProblemKind::CoilSupply,
        ProblemKind::PlugPinOneSided,
        ProblemKind::RelaySize,
        ProblemKind::CoilOnPowerFeed,
        ProblemKind::CableText,
        ProblemKind::NeutralUse,
        ProblemKind::TagRule,
        ProblemKind::DisplayedTag,
        ProblemKind::StructureCode,
    ];
    let mut loaded = 0;
    let mut bad = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Ok(project) = Project::from_json(&text) else {
            continue;
        };
        loaded += 1;
        for problem in check(&project) {
            if watched.contains(&problem.kind) {
                bad.push(format!("{}: {}", file.display(), problem.message));
            }
        }
    }
    assert!(loaded > 0, "no shipped project loaded");
    assert!(bad.is_empty(), "{loaded} loaded:\n{}", bad.join("\n"));
}
