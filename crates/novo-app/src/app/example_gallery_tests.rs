//! Every gallery example opens clean, has a cabinet without overlapping
//! ducts, a heat result inside the device limits, a parts list without
//! gaps and drilling output.

use novo_core::{DeviceTag, Document, PageKind, StructureId};
use novo_layout::model::load;
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};
use novo_thermal::{Options, simulate_steady_with};

use super::example_gallery::{Entry, GALLERY};
use super::gate1_tests::Drive;

fn built(entry: &Entry) -> Document {
    let mut doc = Document::default();
    (entry.build)(&mut doc).unwrap_or_else(|e| panic!("{}: {e}", entry.name));
    doc
}

fn cabinet(doc: &Document, entry: &Entry) -> StructureId {
    let tag = DeviceTag::parse(entry.cabinet).unwrap();
    doc.project()
        .structures()
        .find(|s| s.tag == tag)
        .unwrap_or_else(|| panic!("{}: no cabinet {}", entry.name, entry.cabinet))
        .id
}

#[test]
fn every_example_opens_without_a_warning_or_an_error() {
    for entry in GALLERY {
        let doc = built(entry);
        assert_eq!(doc.project().info().name, entry.name);
        let bad: Vec<_> = novo_checks::check(doc.project())
            .into_iter()
            .filter(|p| p.severity != novo_checks::Severity::Info)
            .map(|p| format!("{:?}: {}", p.severity, p.message))
            .collect();
        assert!(bad.is_empty(), "{}: {bad:#?}", entry.name);
    }
}

#[test]
fn every_example_has_pages_cross_references_and_terminals() {
    for entry in GALLERY {
        let doc = built(entry);
        let p = doc.project();
        let drawn = p
            .pages()
            .filter(|p| !matches!(p.kind, PageKind::Generated { .. }))
            .count();
        assert!(drawn >= 4, "{}: {drawn} pages", entry.name);
        assert!(p.devices().count() >= 25, "{}", entry.name);
        assert!(p.terminals().count() >= 12, "{}", entry.name);
        assert!(p.cables().count() >= 3, "{}", entry.name);
    }
}

#[test]
fn every_example_cabinet_is_laid_out_without_overlapping_ducts() {
    for entry in GALLERY {
        let doc = built(entry);
        let cab = cabinet(&doc, entry);
        let layout = load(doc.project(), cab).expect("a plate");
        assert!(!layout.ducts.is_empty(), "{}: no ducts", entry.name);
        assert!(!layout.rails.is_empty(), "{}: no rails", entry.name);
        let overlaps = novo_layout::duct_overlaps(&layout.ducts, &layout.rails, &layout.placements);
        assert!(overlaps.is_empty(), "{}: {overlaps:?}", entry.name);
        // Every device of the cabinet has its place on the plate.
        let tag = DeviceTag::parse(entry.cabinet).unwrap();
        let placed: std::collections::BTreeSet<u64> =
            layout.placements.iter().map(|p| p.id).collect();
        for d in doc.project().devices() {
            if d.tag.location == tag.location {
                assert!(
                    placed.contains(&d.id.raw()),
                    "{}: {} is not placed",
                    entry.name,
                    d.tag
                );
            }
        }
        // Drilling output exists: the rails alone have mounting holes.
        let drill = novo_layout::drill::drill_table(&layout, &[]);
        assert!(!drill.summary().is_empty(), "{}: no drilling", entry.name);
    }
}

#[test]
fn every_example_has_a_heat_result_inside_the_device_limits() {
    for entry in GALLERY {
        let doc = built(entry);
        let cab = cabinet(&doc, entry);
        let bridge = scenario_from_project(doc.project(), cab, &BridgeOptions::default())
            .unwrap_or_else(|e| panic!("{}: {e:?}", entry.name));
        let result = simulate_steady_with(
            &bridge.scenario,
            Options {
                bands: false,
                advice: false,
            },
        )
        .unwrap();
        let margins: Vec<f64> = result.devices.iter().filter_map(|d| d.margin_k).collect();
        assert!(!margins.is_empty(), "{}: no device has a limit", entry.name);
        let tightest = margins.iter().copied().fold(f64::INFINITY, f64::min);
        assert!(
            tightest >= 0.0,
            "{}: tightest margin {tightest:.1} K",
            entry.name
        );
    }
}

#[test]
fn every_example_has_a_parts_list_without_gaps() {
    use novo_reports::Report;
    for entry in GALLERY {
        let doc = built(entry);
        let out = novo_reports::BillOfMaterials.generate(doc.project());
        let sections: Vec<_> = out
            .sections
            .iter()
            .map(|s| (s.key.clone(), s.rows.len()))
            .collect();
        let parts = sections
            .iter()
            .find(|(id, _)| id == "parts")
            .map_or(0, |s| s.1);
        assert!(parts >= 10, "{}: {sections:?}", entry.name);
        assert!(
            sections.iter().all(|(id, _)| id != "without_parts"),
            "{}: {sections:?}",
            entry.name
        );
    }
}

#[test]
fn every_example_opens_from_its_command_with_generated_pages() {
    for entry in GALLERY {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        assert!(d.app.is_enabled(entry.action), "{}", entry.name);
        d.app.run(&ctx, entry.action);
        d.frame();
        let project = d.doc().project();
        assert_eq!(project.info().name, entry.name);
        assert!(
            project
                .pages()
                .any(|p| matches!(p.kind, PageKind::Generated { .. })),
            "{}: no generated pages",
            entry.name
        );
        assert!(d.app.current_page().is_some());
        let bad: Vec<_> = novo_checks::check(project)
            .into_iter()
            .filter(|p| p.severity != novo_checks::Severity::Info)
            .map(|p| p.message)
            .collect();
        assert!(bad.is_empty(), "{}: {bad:#?}", entry.name);
        // Starting from the palette finds it too.
        assert_eq!(
            crate::actions::Action::from_id(entry.action.id()),
            Some(entry.action)
        );
    }
}

/// `EXAMPLE_PDF_DIR=/some/dir cargo test -p novo-app --lib example_gallery_pdf`
/// writes every example's drawn pages as a PDF, for looking at them.
#[test]
fn example_gallery_pdf() {
    let Some(dir) = std::env::var_os("EXAMPLE_PDF_DIR") else {
        return;
    };
    for (i, entry) in GALLERY.iter().enumerate() {
        let doc = built(entry);
        let pdf = novo_pdf::export_pdf(doc.project(), &novo_pdf::PdfOptions::default()).unwrap();
        std::fs::write(
            std::path::Path::new(&dir).join(format!("example-{i}.pdf")),
            pdf,
        )
        .unwrap();
    }
}
