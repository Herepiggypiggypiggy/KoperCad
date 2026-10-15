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
                "",
                "1",
                "feed-through",
                "",
                "",
                "=A1+FLD-M1:U1",
                "=A1-W1 BK",
                "",
                "1",
                "1.1",
                "1 clamp",
                ""
            ],
            &[
                "=A1+CAB-F2:4",
                "",
                "2",
                "feed-through",
                "",
                "",
                "=A1+FLD-M1:V1",
                "=A1-W1 BN",
                "",
                "1",
                "1.2",
                "1 clamp",
                ""
            ],
            &[
                "=A1+CAB-F2:6",
                "",
                "3",
                "feed-through",
                "",
                "",
                "=A1+FLD-M1:W1",
                "=A1-W1 GY",
                "",
                "1",
                "1.2",
                "1 clamp",
                ""
            ],
            &[
                "PE",
                "",
                "PE",
                "PE",
                "",
                "",
                "=A1+FLD-M1:PE",
                "=A1-W1 GNYE",
                "",
                "1",
                "1.2",
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
    assert_eq!(sheet_name("20", 22, 30), "20z");
    assert_eq!(sheet_name("20", 23, 30), "20aa");

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

    // The contents list names every sheet with what it shows; the page of
    // an unknown report is listed as itself.
    let toc = reports.get(doc.project(), "toc").expect("toc");
    let listed: Vec<[String; 2]> = toc.sections[0]
        .text_rows()
        .into_iter()
        .map(|r| [r[0].clone(), r[1].clone()])
        .collect();
    let expect = |n: &str, t: &str| [n.to_owned(), t.to_owned()];
    assert_eq!(
        listed,
        [
            expect("1", "Main circuit"),
            expect("2", "Control circuit"),
            expect("20a", "Devices"),
            expect("20b", "Devices"),
            expect("20c", "Devices"),
            expect("21", ""),
        ]
    );
}

#[test]
fn the_contents_list_names_what_each_sheet_shows() {
    let mut doc = starter();
    doc.transact("Terminal page", |tx| {
        let page = tx.add_page("30")?;
        tx.update::<Page>(page, |p| {
            p.title = "Terminals".into();
            p.kind = PageKind::Generated {
                generator: "terminals".into(),
            };
        })
    })
    .expect("page");
    let toc = LiveReports::standard()
        .get(doc.project(), "toc")
        .expect("toc")
        .clone();
    let last = toc.sections[0].text_rows().pop().expect("rows");
    assert_eq!(last[0], "30");
    assert_eq!(last[1], "Terminals · =A1+CAB-X1");
    assert_eq!(last[4], "Generated: terminals");
}

#[test]
fn sheet_names_never_repeat_another_page() {
    let free = |_: &str| false;
    assert_eq!(sheet_names("20", 1, free), ["20"]);
    assert_eq!(sheet_names("20", 2, free), ["20a", "20b"]);
    // A single sheet is the page's own name, whatever else exists.
    assert_eq!(sheet_names("20", 1, |n| n == "20"), ["20"]);
    // A page the user named like a sheet keeps its name; the sheet gets marks.
    assert_eq!(sheet_names("20", 3, |n| n == "20b"), ["20a", "20b'", "20c"]);
    let taken = ["20a", "20a'"];
    assert_eq!(
        sheet_names("20", 2, |n| taken.contains(&n)),
        ["20a''", "20b"]
    );
}

#[test]
fn sheet_letters_skip_i_l_and_o() {
    let names: Vec<String> = (0..30).map(|i| sheet_name("20", i, 30)).collect();
    assert_eq!(
        names[..10],
        [
            "20a", "20b", "20c", "20d", "20e", "20f", "20g", "20h", "20j", "20k"
        ]
    );
    assert_eq!(names[10..14], ["20m", "20n", "20p", "20q"]);
    assert_eq!(names[22], "20z");
    assert_eq!(names[23], "20aa");
    assert_eq!(names[24], "20ab");
    assert!(names.iter().all(|n| !n[2..].contains(['i', 'l', 'o', '.'])));
    // 23 + 23 * 23 names with up to two letters.
    assert_eq!(sheet_name("20", 23 + 23 * 23 - 1, 1000), "20zz");
    assert_eq!(sheet_name("20", 23 + 23 * 23, 1000), "20aaa");
}

#[test]
fn a_page_named_like_a_sheet_keeps_its_name() {
    let mut doc = starter();
    let (devices, _) = doc
        .transact("Report page", |tx| {
            let devices = tx.add_page("20")?;
            tx.update::<Page>(devices, |p| {
                p.kind = PageKind::Generated {
                    generator: "devices".into(),
                }
            })?;
            // A hand-drawn page that happens to share a sheet's name.
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
    assert_eq!(names, ["20a", "20b'", "20c"]);
    let pages: Vec<&str> = doc.project().pages().map(|p| p.name.as_str()).collect();
    assert!(names.iter().all(|n| !pages.contains(&n.as_str())));

    // Renaming the page away gives the plain names back.
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

#[test]
fn the_contents_list_splits_group_and_panel_and_lists_panels() {
    use novo_core::{Command, DeviceTag};
    let mut doc = starter();
    for (name, structure) in [("5", "=10D41+K1"), ("6", "=11D21+K1")] {
        doc.execute(Command::AddPage {
            name: name.into(),
            title: String::new(),
            kind: Default::default(),
            format: Default::default(),
            structure: DeviceTag::parse(structure).unwrap(),
        })
        .unwrap();
    }
    let toc = LiveReports::standard()
        .get(doc.project(), "toc")
        .expect("toc")
        .clone();
    let rows = toc.sections[0].text_rows();
    let row = |name: &str| rows.iter().find(|r| r[0] == name).unwrap().clone();
    assert_eq!(
        (row("5")[2].as_str(), row("5")[3].as_str()),
        ("=10D41", "+K1")
    );
    // One panel, two groups, two pages.
    let panels = toc.sections[1].text_rows();
    assert_eq!(panels, [["+K1", "=10D41, =11D21", "2"]]);
}
