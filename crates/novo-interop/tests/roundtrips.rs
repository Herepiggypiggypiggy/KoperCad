//! Every sample file through import, export and import again, compared with
//! `compare_projects`. The differences printed here are what the format
//! cannot carry; a new one fails the test so it gets looked at.

use std::path::Path;

use novo_core::{Document, Project};
use novo_interop::aml::{AmlOptions, export_project, plan_import, read_aml};
use novo_interop::compare::compare_projects;
use novo_interop::eplan::pxf::{PxfOptions, PxfWriteOptions, plan_pxf, read_pxf_str, write_pxf};

fn fixture(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn from_pxf(text: &str) -> Project {
    let file = read_pxf_str(text).unwrap();
    let mut doc = Document::default();
    plan_pxf(doc.project(), &file, &PxfOptions::default())
        .apply(&mut doc)
        .unwrap();
    doc.project().clone()
}

fn from_aml(text: &str) -> Project {
    let plant = read_aml(text).unwrap();
    let mut doc = Document::default();
    plan_import(doc.project(), &plant).apply(&mut doc).unwrap();
    doc.project().clone()
}

fn lossy(before: &Project, after: &Project) -> Vec<String> {
    let c = compare_projects(before, after);
    let mut v: Vec<String> = c.report.lossy.iter().map(|l| l.what.clone()).collect();
    v.sort();
    eprintln!("{}", c.report.text());
    v
}

/// Through PXF: the file has no EPLAN symbol table, so drawings do not come
/// back; everything else must.
const PXF_LOSES: &[&str] = &[
    "drawn symbols missing or changed",
    "nets missing or changed",
];

#[test]
fn pxf_samples_survive_a_pxf_round_trip() {
    for name in ["pxf/synthetic-small.epj", "pxf/synthetic-layout.epj"] {
        let first = from_pxf(&fixture(name));
        let out = write_pxf(&first, &PxfWriteOptions::default());
        let second = from_pxf(&out.text);
        let got = lossy(&first, &second);
        for what in &got {
            assert!(PXF_LOSES.contains(&what.as_str()), "{name}: {what}");
        }
    }
}

#[test]
fn the_crane_samples_survive_a_pxf_round_trip() {
    for name in [
        "crane/output/custom/demo/v1/DEMO-1.epj",
        "crane/output/custom/demo/v2/DEMO-1.epj",
    ] {
        let first = from_pxf(&fixture(name));
        let out = write_pxf(&first, &PxfWriteOptions::default());
        let second = from_pxf(&out.text);
        for what in lossy(&first, &second) {
            assert!(PXF_LOSES.contains(&what.as_str()), "{name}: {what}");
        }
    }
}

#[test]
fn the_aml_sample_survives_an_aml_round_trip() {
    let first = from_aml(&fixture("aml/hoist-feeder.aml"));
    let out = export_project(&first, &AmlOptions::default());
    let second = from_aml(&out.text);
    let got = lossy(&first, &second);
    assert!(got.is_empty(), "{got:?}");
}

#[test]
fn the_pxf_samples_survive_an_aml_round_trip() {
    for name in [
        "pxf/synthetic-small.epj",
        "pxf/synthetic-layout.epj",
        "crane/output/custom/demo/v2/DEMO-1.epj",
    ] {
        let first = from_pxf(&fixture(name));
        let out = export_project(&first, &AmlOptions::default());
        let second = from_aml(&out.text);
        let got = lossy(&first, &second);
        assert!(got.is_empty(), "{name}: {got:?}");
    }
}
