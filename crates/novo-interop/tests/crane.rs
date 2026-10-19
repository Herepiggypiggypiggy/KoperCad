//! Crane configurator import on a synthetic output folder
//! (`tests/fixtures/crane`), and, when `KOPERCAD_CRANE_EPJ` points at a real
//! project export, on that.

use std::path::{Path, PathBuf};

use novo_core::{Document, Project};
use novo_interop::eplan::crane::{locate, plan_crane};
use novo_interop::eplan::pxf::PxfOptions;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/crane/output")
}

#[test]
fn the_newest_version_is_read_and_the_plan_says_what_it_took() {
    let (plan, src, summary) =
        plan_crane(&Project::new("x"), &fixture(), &PxfOptions::default()).unwrap();
    assert_eq!(src.version, Some(2));
    assert_eq!(src.other_versions, [1]);
    assert_eq!(summary.verdict, "GOED MET OPMERKINGEN");
    let r = plan.report();
    assert_eq!(r.count_of("pages (empty)"), 1, "{}", r.text());
    assert_eq!(
        r.lossy
            .iter()
            .find(|l| l.what == "generated report pages")
            .map(|l| l.count),
        Some(1),
        "the parts list page is left to KoperCAD's own report: {}",
        r.text()
    );
    // A cable drawn without a cable text still gets its core, by reference.
    assert_eq!(r.count_of("cables"), 2, "{}", r.text());
    assert_eq!(r.count_of("cable cores"), 2, "{}", r.text());
    assert!(
        !r.lossy
            .iter()
            .any(|l| l.what == "cable cores without a cable"),
        "{}",
        r.text()
    );
    let text = r.text();
    for want in [
        "version 2",
        "Order data from the configurator: project DEMO-1, customer Demo Customer",
        "3 devices checked, 1 renamed",
        "Exact (as the file writes it)",
        "Inferred",
    ] {
        assert!(text.contains(want), "missing {want:?} in\n{text}");
    }
}

#[test]
fn applying_names_the_project_and_is_one_undo_step() {
    let (plan, _, _) = plan_crane(&Project::new("x"), &fixture(), &PxfOptions::default()).unwrap();
    let mut doc = Document::new(Project::new("x"));
    plan.apply(&mut doc).unwrap();
    let p = doc.project();
    assert_eq!(p.info().name, "DEMO-1 Demo slewing crane");
    assert_eq!(p.pages().count(), 1);
    assert_eq!(p.cables().count(), 2);
    assert!(doc.undo().unwrap().is_some(), "one undo step");
    assert_eq!(doc.project().pages().count(), 0);
}

#[test]
fn an_older_version_can_be_chosen_by_folder() {
    let src = locate(&fixture().join("custom/demo/v1")).unwrap();
    assert_eq!(src.version, Some(1));
}

#[test]
fn a_real_project_imports_with_its_cables_and_terminals() {
    let Some(path) = std::env::var_os("KOPERCAD_CRANE_EPJ").map(PathBuf::from) else {
        return;
    };
    let (plan, _, _) = plan_crane(&Project::new("x"), &path, &PxfOptions::default()).unwrap();
    let r = plan.report();
    assert!(r.count_of("devices") > 0, "{}", r.text());
    assert!(r.count_of("terminals") > 0, "{}", r.text());
    assert!(r.count_of("cables") > 0, "{}", r.text());
}
