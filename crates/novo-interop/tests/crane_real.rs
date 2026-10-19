//! Opt-in checks on a real crane configurator export. They run only when
//! `KOPERCAD_CRANE_EPJ` names a project export (`.epj`); otherwise each test
//! returns at once and passes, so CI and anyone without the file stay green.
//! The file is the user's own and never goes into the repository.

use std::io::BufReader;
use std::path::PathBuf;

use novo_interop::eplan::pxf::{PageClass, read_pxf};

fn real_file() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("KOPERCAD_CRANE_EPJ")?);
    p.is_file().then_some(p)
}

#[test]
fn the_reader_scopes_library_graphics_away_from_pages() {
    let Some(path) = real_file() else { return };
    let f = read_pxf(BufReader::new(std::fs::File::open(path).unwrap())).unwrap();
    let schematic = f
        .pages
        .iter()
        .filter(|p| p.class() == PageClass::Schematic)
        .count();
    eprintln!(
        "pages {} schematic {} devices {} cores {} ips {} nodes {} texts {} structures {} library {} symbols {} functions {}",
        f.pages.len(),
        schematic,
        f.devices.len(),
        f.cores.len(),
        f.interruptions.len(),
        f.nodes.len(),
        f.texts.len(),
        f.structures.len(),
        f.library_objects.values().sum::<usize>(),
        f.library.symbol_count(),
        f.library.function_count()
    );
    eprintln!("project {:?}", f.project);
    for p in f.pages.iter().take(40) {
        eprintln!(
            "{} {:?} sym {} nodes {} cells {}",
            p.label(),
            p.class(),
            p.symbols,
            p.nodes,
            p.report_cells
        );
    }
}

/// The drawn pages of a real project: nets agree with EPLAN's rules.
#[test]
fn the_drawn_pages_keep_the_nets_of_the_real_project() {
    let Some(path) = real_file() else { return };
    let (plan, _, _) = novo_interop::eplan::crane::plan_crane(
        &novo_core::Project::new("x"),
        &path,
        &novo_interop::eplan::pxf::PxfOptions::default(),
    )
    .unwrap();
    let r = plan.report();
    assert!(r.count_of("symbols drawn") > 100, "{}", r.text());
    assert!(r.count_of("wires") > 100, "{}", r.text());
    let mut doc = novo_core::Document::new(novo_core::Project::new("x"));
    let (_, _, check) = plan.apply_checked(&mut doc).unwrap().unwrap();
    let c = check.expect("pages are drawn");
    assert!(c.compared > 500, "{c:?}");
    assert!(
        c.import_only.is_empty(),
        "a net EPLAN's rules give is split in KoperCAD: {:?}",
        c.import_only
    );
    // Only same-name terminals and names used more than twice may join more.
    assert!(c.project_only.len() < 10, "{:?}", c.project_only);
}

/// Lines, rectangles, arcs and location boxes of the real project reach the
/// pages as background drawings.
#[test]
fn the_real_project_keeps_its_lines_and_location_boxes() {
    let Some(path) = real_file() else { return };
    let (plan, _, _) = novo_interop::eplan::crane::plan_crane(
        &novo_core::Project::new("x"),
        &path,
        &novo_interop::eplan::pxf::PxfOptions::default(),
    )
    .unwrap();
    let r = plan.report();
    eprintln!("{}", r.text());
    let mut doc = novo_core::Document::new(novo_core::Project::new("x"));
    plan.apply(&mut doc).unwrap();
    let with_bg = doc
        .project()
        .pages()
        .filter(|p| novo_interop::underlay::stored_text(&p.properties).is_some())
        .count();
    assert!(with_bg >= 15, "{with_bg} pages with a background drawing");
}

/// The mounting plate sheets of the full export become cabinets.
#[test]
fn the_real_project_layout_sheets_become_cabinets() {
    let Some(path) = real_file() else { return };
    let (plan, _, _) = novo_interop::eplan::crane::plan_crane(
        &novo_core::Project::new("x"),
        &path,
        &novo_interop::eplan::pxf::PxfOptions::default(),
    )
    .unwrap();
    let r = plan.report();
    let cabinets: Vec<_> = r
        .text()
        .lines()
        .filter(|l| l.contains("Cabinet +"))
        .map(str::to_owned)
        .collect();
    eprintln!("{}", cabinets.join("\n"));
    let mut doc = novo_core::Document::new(novo_core::Project::new("x"));
    plan.apply(&mut doc).unwrap();
    let placed = doc.project().all::<novo_core::CabinetPlacement>().count();
    eprintln!("placements {placed}");
    if path.to_string_lossy().contains("full") {
        assert!(!cabinets.is_empty(), "no mounting plate sheet found");
        assert!(placed > 10);
    }
}
