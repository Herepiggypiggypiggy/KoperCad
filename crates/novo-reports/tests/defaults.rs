//! Which generated pages a project gets (G2-1).

use novo_core::Document;
use novo_reports::{
    GeneratedPageSpec, LiveReports, add_generated_pages, default_generated_pages,
    default_generated_pages_with,
};
use novo_tests::motor_starter;

fn names(specs: &[GeneratedPageSpec]) -> Vec<(&str, &str)> {
    specs
        .iter()
        .map(|s| (s.name.as_str(), s.generator.as_str()))
        .collect()
}

#[test]
fn the_starter_gets_every_report_after_its_schematic_pages() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let specs = default_generated_pages(doc.project());
    assert_eq!(
        names(&specs),
        [
            ("0", "toc"),
            ("10", "bom"),
            ("20", "devices"),
            ("30", "wires"),
            ("40", "terminals"),
            ("50", "cables"),
        ]
    );
    assert_eq!(specs[1].title, "Bill of materials");

    doc.transact("Generated pages", |tx| add_generated_pages(tx, &specs))
        .expect("pages");
    // The contents list opens the project; the sections follow it.
    let order: Vec<&str> = doc
        .project()
        .pages_in_order()
        .into_iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(order, ["0", "1", "2", "10", "20", "30", "40", "50"]);
    // Every page has sheets, and the starter's strip is on 40.
    let mut reports = LiveReports::standard();
    let sheets = reports.generated_sheets(doc.project());
    assert!(sheets.iter().any(|s| s.name == "40"), "{sheets:?}");
    for name in ["0", "10", "20", "30", "50"] {
        assert!(
            sheets.iter().any(|s| s.name.starts_with(name)),
            "{name}: {sheets:?}"
        );
    }
    // Asked again (on open), nothing is added twice.
    assert!(default_generated_pages(doc.project()).is_empty());
}

#[test]
fn an_empty_project_gets_only_the_contents_list() {
    let doc = Document::default();
    let specs = default_generated_pages(doc.project());
    assert_eq!(names(&specs), [("0", "toc")]);
}

#[test]
fn kinds_can_be_switched_off_and_new_ones_go_after_the_last_section() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let specs = default_generated_pages_with(doc.project(), &|id| id == "bom");
    assert_eq!(names(&specs), [("10", "bom")]);
    doc.transact("BOM page", |tx| add_generated_pages(tx, &specs))
        .expect("page");
    // Later, the wire list is switched on: it goes after section 10.
    let specs = default_generated_pages_with(doc.project(), &|id| id == "wires");
    assert_eq!(names(&specs), [("20", "wires")]);
}

#[test]
fn sections_start_above_every_page_number_in_use() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    doc.transact("Pages", |tx| {
        tx.add_page("0")?;
        tx.add_page("37a")?;
        Ok(())
    })
    .expect("pages");
    let specs = default_generated_pages(doc.project());
    // "0" is taken, so the contents list gets the first free section.
    assert_eq!(specs[0].name, "40");
    assert!(specs.iter().all(|s| s.name != "0"));
    assert!(specs.iter().any(|s| s.generator == "toc"));
    let generated = specs
        .iter()
        .all(|s| s.name.parse::<u32>().is_ok_and(|n| n >= 40 && n % 10 == 0));
    assert!(generated, "{specs:?}");
}
