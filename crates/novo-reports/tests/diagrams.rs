//! Terminal and cable diagrams, generated sheets, the project-identity
//! guard and Excel's cell limit.

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use novo_core::{DeviceTag, Document, Page, PageKind, Project};
use novo_reports::{
    Cell, Column, LiveReports, ReportOutput, Section, sheet_name, sheet_names, xlsx_bytes,
};
use novo_tests::motor_starter;

fn starter() -> Document {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    doc
}

fn rows(output: &ReportOutput, section: &str) -> Vec<Vec<String>> {
    output.section(section).expect("section").text_rows()
}

fn strings(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|r| r.iter().map(|s| (*s).to_owned()).collect())
        .collect()
}

#[test]
fn terminal_diagram_has_one_section_per_strip() {
    let doc = starter();
    let terminals = LiveReports::standard()
        .get(doc.project(), "terminals")
        .expect("built in")
        .clone();
    assert_eq!(terminals.sections.len(), 1);
    assert_eq!(terminals.sections[0].title, "=A1+CAB-X1");
    assert_eq!(
        rows(&terminals, "=A1+CAB-X1"),
        strings(&[
            &[
                "=A1+CAB-F2:2",
                "1",
                "feed-through",
                "=A1+FLD-M1:U1",
                "=A1-W1 BK",
                "",
                "1",
                "1 clamp",
                ""
            ],
            &[
                "=A1+CAB-F2:4",
                "2",
                "feed-through",
                "=A1+FLD-M1:V1",
                "=A1-W1 BN",
                "",
                "1",
                "1 clamp",
                ""
            ],
            &[
                "=A1+CAB-F2:6",
                "3",
                "feed-through",
                "=A1+FLD-M1:W1",
                "=A1-W1 GY",
                "",
                "1",
                "1 clamp",
                ""
            ],
            &[
                "PE",
                "PE",
                "PE",
                "=A1+FLD-M1:PE",
                "=A1-W1 GNYE",
                "",
                "1",
                "1 clamp",
                ""
            ],
        ])
    );
}

#[test]
fn cable_diagram_has_an_overview_and_one_section_per_cable() {
    let doc = starter();
    let cables = LiveReports::standard()
        .get(doc.project(), "cables")
        .expect("built in")
        .clone();
    assert_eq!(
        rows(&cables, "overview"),
        strings(&[&["=A1-W1", "4G1.5", "12", "+CAB", "+FLD", "4", "4", "0"]])
    );
    let w1 = cables.section("=A1-W1").expect("cable section");
    assert_eq!(w1.title, "=A1-W1 · 4G1.5 · +CAB → +FLD · 12 m");
    assert_eq!(
        w1.text_rows(),
        strings(&[
            &[
                "BK",
                "1.5",
                "",
                "L1",
                "+CAB",
                "=A1+CAB-X1:1",
                "+FLD",
                "=A1+FLD-M1:U1",
                ""
            ],
            &[
                "BN",
                "1.5",
                "",
                "L2",
                "+CAB",
                "=A1+CAB-X1:2",
                "+FLD",
                "=A1+FLD-M1:V1",
                ""
            ],
            &[
                "GY",
                "1.5",
                "",
                "L3",
                "+CAB",
                "=A1+CAB-X1:3",
                "+FLD",
                "=A1+FLD-M1:W1",
                ""
            ],
            &[
                "GNYE",
                "1.5",
                "",
                "PE",
                "+CAB",
                "=A1+CAB-X1:PE",
                "+FLD",
                "=A1+FLD-M1:PE",
                ""
            ],
        ])
    );
    // Core rows point at the cable and the wire they carry.
    assert!(w1.rows.iter().all(|r| r.sources.len() == 2));
}

#[test]
fn generated_pages_print_as_named_sheets() {
    assert_eq!(sheet_name("20", 0, 1), "20");
    assert_eq!(sheet_name("20", 0, 3), "20a");
    assert_eq!(sheet_name("20", 2, 3), "20c");
    assert_eq!(sheet_name("20", 25, 30), "20z");
    assert_eq!(sheet_name("20", 26, 30), "20aa");
    assert_eq!(sheet_name("20", 27, 30), "20ab");
    assert_eq!(sheet_name("20", 52, 60), "20ba");

    let mut doc = starter();
    let (pages, _) = doc
        .transact("Report pages", |tx| {
            let generated = |generator: &str| PageKind::Generated {
                generator: generator.into(),
            };
            let devices = tx.add_page("20")?;
            tx.update::<Page>(devices, |p| {
                p.title = "Devices".into();
                p.kind = generated("devices");
            })?;
            let unknown = tx.add_page("21")?;
            tx.update::<Page>(unknown, |p| p.kind = generated("no-such-report"))?;
            // Enough devices for three A3 sheets.
            for i in 1..=80 {
                tx.add_device(DeviceTag::parse(&format!("=A9-K{i}"))?, "Relay")?;
            }
            Ok((devices, unknown))
        })
        .expect("pages");
    let (devices, unknown) = pages;
    let mut reports = LiveReports::standard();

    let sheets = reports.sheets_of_page(doc.project(), devices);
    let names: Vec<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["20a", "20b", "20c"]);
    assert!(
        sheets
            .iter()
            .all(|s| s.report == "devices" && s.page == devices)
    );
    // Sheet names sort between the page and the next one, and never
    // contain a dot, so they cannot read as a page.column reference.
    let mut sorted = vec!["21", "20b", "20", "20c", "20a"];
    sorted.sort_by(|a, b| novo_core::natural_cmp(a, b));
    assert_eq!(sorted, ["20", "20a", "20b", "20c", "21"]);
    assert!(names.iter().all(|n| !n.contains('.')));

    // An unregistered generator prints nothing rather than failing.
    assert!(reports.sheets_of_page(doc.project(), unknown).is_empty());
    // Schematic pages are not generated pages.
    let main = doc.project().pages_in_order()[0].id;
    assert!(reports.sheets_of_page(doc.project(), main).is_empty());

    let all = reports.generated_sheets(doc.project());
    assert_eq!(all, sheets);
    let layout = reports
        .layout_sheet(doc.project(), &sheets[1])
        .expect("layout");
    let (w, h) = doc.project().page(devices).expect("page").format.size_mm();
    assert_eq!((layout.width, layout.height), (w, h));
    assert!(layout.texts.iter().any(|t| t.text == "page 2 / 3"));
}

#[test]
fn sheet_names_never_repeat_another_page() {
    let free = |_: &str| false;
    assert_eq!(sheet_names("20", 1, free), ["20"]);
    assert_eq!(sheet_names("20", 2, free), ["20a", "20b"]);
    // A single sheet is the page's own name, whatever else exists.
    assert_eq!(sheet_names("20", 1, |n| n == "20"), ["20"]);
    // One clash moves the whole family to the next free separator.
    assert_eq!(
        sheet_names("20", 3, |n| n == "20b"),
        ["20-a", "20-b", "20-c"]
    );
    assert_eq!(
        sheet_names("20", 2, |n| ["20a", "20-b"].contains(&n)),
        ["20/a", "20/b"]
    );
    // Every separator taken: still distinct, free names.
    let taken = ["20a", "20-a", "20/a", "20_a", "20a'"];
    assert_eq!(
        sheet_names("20", 2, |n| taken.contains(&n)),
        ["20a''", "20b"]
    );
}

#[test]
fn generated_sheets_step_around_a_real_sub_page() {
    let mut doc = starter();
    let (devices, _) = doc
        .transact("Report page", |tx| {
            let devices = tx.add_page("20")?;
            tx.update::<Page>(devices, |p| {
                p.kind = PageKind::Generated {
                    generator: "devices".into(),
                }
            })?;
            // A hand-drawn sub-page that happens to share a sheet's name.
            tx.add_page("20b")?;
            for i in 1..=80 {
                tx.add_device(DeviceTag::parse(&format!("=A9-K{i}"))?, "Relay")?;
            }
            Ok(devices)
        })
        .expect("pages");
    let mut reports = LiveReports::standard();
    let names: Vec<String> = reports
        .sheets_of_page(doc.project(), devices)
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, ["20-a", "20-b", "20-c"]);
    let pages: Vec<&str> = doc.project().pages().map(|p| p.name.as_str()).collect();
    assert!(names.iter().all(|n| !pages.contains(&n.as_str())));

    // Renaming the sub-page away gives the dotted names back.
    let sub = doc
        .project()
        .pages()
        .find(|p| p.name == "20b")
        .expect("sub-page")
        .id;
    doc.transact("Rename", |tx| {
        tx.update::<Page>(sub, |p| p.name = "19.1".into())
    })
    .expect("rename");
    let names: Vec<String> = reports
        .sheets_of_page(doc.project(), devices)
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, ["20a", "20b", "20c"]);
}

#[test]
fn a_replaced_project_is_not_served_stale_output() {
    let a = starter();
    let mut reports = LiveReports::standard();
    let bom = reports.get(a.project(), "bom").expect("bom").clone();
    assert!(!bom.sections[0].rows.is_empty());

    // Another project at the same revision, opened without `clear()`.
    let mut b = Project::new("Other");
    for _ in 0..a.project().revision() {
        b.transact("Device", |tx| {
            let n = tx.project().devices().count() + 1;
            tx.add_device(DeviceTag::parse(&format!("-K{n}"))?, "")
        })
        .expect("device");
    }
    assert_eq!(b.revision(), a.project().revision());
    assert!(!reports.is_current(&b, "bom"));
    let other = reports.get(&b, "bom").expect("bom");
    assert!(other.sections[0].rows.is_empty());
}

#[test]
fn oversized_cells_are_cut_to_excels_limit() {
    let long = "x".repeat(40_000);
    let mut section = Section::new("s", "", vec![Column::new("text", "Text", 1.0)]);
    section.push(vec![Cell::Text(long)], vec![]);
    let output = ReportOutput {
        report: "long".into(),
        title: "Long".into(),
        revision: 0,
        sections: vec![section],
    };
    let bytes = xlsx_bytes(&[&output]).expect("export is never blocked");
    let mut book: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).expect("opens");
    let range = book.worksheet_range("Long").expect("sheet");
    let Some(Data::String(text)) = range.get((1, 0)) else {
        panic!("text cell");
    };
    assert_eq!(text.chars().count(), 32_767);
    assert!(text.ends_with('…'));
}
