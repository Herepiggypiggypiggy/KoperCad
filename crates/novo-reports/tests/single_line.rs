//! The single-line diagram report: the schedule, the drawn sheets and how a
//! wide distribution is split over sheets.

use novo_core::{Document, Page, PageId, PageKind, Project};
use novo_reports::single_line::{self, SingleLineDiagram};
use novo_reports::{
    LiveReports, PageSetup, Report, default_generated_pages, generated_page_spec, standard_reports,
};
use novo_tests::{BigSpec, big_project, distribution_board, distribution_board_wide};

fn page(doc: &mut Document) -> PageId {
    doc.transact("Single-line page", |tx| {
        let page = tx.add_page("30")?;
        tx.update::<Page>(page, |p| {
            p.title = "Single-line diagram".into();
            p.kind = PageKind::Generated {
                generator: SingleLineDiagram::ID.into(),
            };
        })?;
        Ok(page)
    })
    .unwrap()
    .0
}

fn board() -> Document {
    let mut doc = Document::default();
    distribution_board(&mut doc).unwrap();
    doc
}

fn texts(layout: &novo_reports::PageLayout) -> Vec<&str> {
    layout.texts.iter().map(|t| t.text.as_str()).collect()
}

#[test]
fn the_report_is_registered_but_not_added_to_every_project() {
    assert!(standard_reports().iter().any(|r| r.id() == "singleline"));
    assert!(!SingleLineDiagram.proposed_by_default());
    let doc = board();
    assert!(
        default_generated_pages(doc.project())
            .iter()
            .all(|s| s.generator != "singleline")
    );
    // By hand it is one command away, with the next free section number.
    let spec = generated_page_spec(doc.project(), "singleline").unwrap();
    assert_eq!(spec.title, "Single-line diagram");
    assert_eq!(spec.name, "10");
}

#[test]
fn a_project_without_distribution_has_nothing_to_list() {
    let doc = Document::default();
    let out = SingleLineDiagram.generate(doc.project());
    assert_eq!(out.row_count(), 0);
    assert!(generated_page_spec(doc.project(), "singleline").is_err());
    let setup = PageSetup::default();
    assert!(single_line::sheet_layout(doc.project(), 0, &setup).is_none());
}

#[test]
fn the_schedule_lists_every_box_with_its_figures() {
    let doc = board();
    let out = SingleLineDiagram.generate(doc.project());
    assert_eq!(out.sections.len(), 1);
    let section = &out.sections[0];
    // The supply and eight devices.
    assert_eq!(section.rows.len(), 9);
    let col = |key: &str| section.column(key).unwrap();
    let row = |tag: &str| {
        section
            .rows
            .iter()
            .find(|r| r.cells[col("tag")].to_string() == tag)
            .unwrap_or_else(|| panic!("{tag} not listed"))
    };
    let m1 = row("=A1+FLD-M1");
    assert_eq!(m1.cells[col("type")].to_string(), "Motor");
    assert_eq!(m1.cells[col("ib")].to_string(), "12");
    assert_eq!(m1.cells[col("section")].to_string(), "2.5");
    assert_eq!(m1.cells[col("length")].to_string(), "30");
    assert_eq!(m1.cells[col("check")].to_string(), "ok");
    // Numbers are rounded for the page, not printed with sixteen digits.
    let du = m1.cells[col("du_total")].to_string();
    assert!(du.len() <= 5, "{du}");
    let q2 = row("=A1+CAB-Q2");
    assert_eq!(q2.cells[col("in")].to_string(), "16");
    assert_eq!(q2.cells[col("curve")].to_string(), ""); // no curve stated
    // The ib basis column says what a calculated current rests on.
    let m2 = row("=A1+FLD-M2");
    assert!(m2.cells[col("ib_basis")].to_string().contains("typical"));
    // Rows point at their device and the wires of their run.
    assert!(!m1.sources.is_empty());
}

#[test]
fn a_failing_run_is_named_in_the_schedule() {
    let mut doc = Document::default();
    let b = distribution_board(&mut doc).unwrap();
    doc.transact("Overload", |tx| {
        tx.set_property(b.m1, "rated_current_a", Some(40.0.into()))
    })
    .unwrap();
    let out = SingleLineDiagram.generate(doc.project());
    let s = &out.sections[0];
    let row = s
        .rows
        .iter()
        .find(|r| r.cells[0].to_string() == "=A1+FLD-M1")
        .unwrap();
    assert_eq!(row.cells[s.column("check").unwrap()].to_string(), "FAIL");
    assert!(
        row.cells[s.column("why").unwrap()]
            .to_string()
            .contains("above the cable rating")
    );
}

#[test]
fn a_generated_page_prints_the_drawing_on_its_first_sheet() {
    let mut doc = board();
    let id = page(&mut doc);
    let mut live = LiveReports::standard();
    let sheets = live.sheets_of_page(doc.project(), id);
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].name, "30");
    let layout = live.layout_sheet(doc.project(), &sheets[0]).unwrap();
    let all = texts(&layout);
    assert!(all.contains(&"Single-line diagram"));
    for tag in [
        "-Q1  Breaker",
        "-Q2  Breaker",
        "-M1  Motor",
        "-U1  Drive",
        "-E1  Heater",
    ] {
        assert!(all.contains(&tag), "{tag} missing from {all:?}");
    }
    assert!(all.iter().any(|t| t.starts_with("Ib 12 A")));
    assert!(all.iter().any(|t| t.contains("2.5 mm²")));
    assert!(all.iter().any(|t| t.contains("30 m")));
    assert!(all.iter().any(|t| t.starts_with("ΔU ")));
    // The legend says what the marks mean and that the tables are planning values.
    assert!(all.iter().any(|t| t.contains("~ calculated")));
    assert!(all.iter().any(|t| t.contains("planning values")));
}

#[test]
fn every_figure_is_inside_the_usable_area_of_the_sheet() {
    let setup = PageSetup::default();
    let (x0, y0, w, h) = setup.table_area();
    let mut docs = vec![board()];
    let mut wide = Document::default();
    distribution_board_wide(&mut wide, 12).unwrap();
    docs.push(wide);
    let mut big = Document::default();
    big_project(&mut big, BigSpec::new(30)).unwrap();
    docs.push(big);
    for doc in &docs {
        let layouts = single_line::layouts(&single_line::build(doc.project()), &setup);
        assert!(!layouts.is_empty());
        for layout in &layouts {
            let eps = 0.01;
            for l in &layout.lines {
                for (x, y) in [l.from, l.to] {
                    assert!(
                        x >= x0 - eps && x <= x0 + w + eps && y >= y0 - eps && y <= y0 + h + eps,
                        "line end ({x}, {y}) outside ({x0}, {y0}, {w}, {h})"
                    );
                }
            }
            for t in &layout.texts {
                assert!(t.x >= x0 - eps && t.x + t.width <= x0 + w + eps, "{t:?}");
                // The heading sits above the table area.
                assert!(
                    t.y >= setup.margins.top - eps && t.y + t.height <= y0 + h + eps,
                    "{t:?}"
                );
            }
        }
    }
}

#[test]
fn a_wide_distribution_is_split_over_sheets_and_the_wide_branch_is_repeated() {
    let mut doc = Document::default();
    distribution_board_wide(&mut doc, 10).unwrap();
    let sl = single_line::build(doc.project());
    let setup = PageSetup::default();
    // Q1 has 13 branches: wider than a sheet, so it is split below Q1.
    let sheets = single_line::layouts(&sl, &setup);
    assert!(sheets.len() >= 2, "{} sheets", sheets.len());
    assert_eq!(single_line::sheet_count(&sl), sheets.len());
    let later = texts(&sheets[1]);
    assert!(later.contains(&"-Q1  Breaker"), "{later:?}");
    assert!(later.iter().any(|t| t.contains("continued")));
    // Every box is on exactly one sheet as a box of its own (not repeated).
    let out = SingleLineDiagram.generate(doc.project());
    let listed: Vec<String> = out
        .sections
        .iter()
        .flat_map(|s| s.rows.iter().map(|r| r.cells[0].to_string()))
        .collect();
    let mut unique = listed.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        listed.len(),
        unique.len(),
        "a box is listed twice: {listed:?}"
    );
    assert_eq!(listed.len(), sl.nodes.len());
    // One section per sheet.
    assert_eq!(out.sections.len(), sheets.len());
}

#[test]
fn twenty_feeders_make_two_sheets_named_like_any_generated_page() {
    let mut doc = Document::default();
    big_project(&mut doc, BigSpec::new(20)).unwrap();
    let id = page(&mut doc);
    let mut live = LiveReports::standard();
    let sheets = live.sheets_of_page(doc.project(), id);
    assert!(sheets.len() >= 2);
    let names: Vec<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(&names[..2], ["30a", "30b"]);
    for s in &sheets {
        assert!(live.layout_sheet(doc.project(), s).is_some());
    }
}

#[test]
fn the_drawing_changes_with_the_project_and_not_otherwise() {
    let mut doc = board();
    let a = single_line::layouts(&single_line::build(doc.project()), &PageSetup::default());
    let b = single_line::layouts(&single_line::build(doc.project()), &PageSetup::default());
    assert_eq!(a, b);
    let m1 = distribution_board_m1(doc.project());
    doc.transact("Nameplate", |tx| {
        tx.set_property(m1, "rated_current_a", Some(13.0.into()))
    })
    .unwrap();
    let c = single_line::layouts(&single_line::build(doc.project()), &PageSetup::default());
    assert_ne!(a, c);
}

fn distribution_board_m1(project: &Project) -> novo_core::DeviceId {
    project
        .devices()
        .find(|d| d.tag.to_string() == "=A1+FLD-M1")
        .unwrap()
        .id
}

#[test]
fn a_smaller_sheet_scales_the_drawing_down_instead_of_cutting_it() {
    let sl = single_line::build(board().project());
    let a3 = single_line::sheet_layout_of(&sl, 0, &PageSetup::default()).unwrap();
    let setup = PageSetup::for_format(novo_core::PageFormat {
        size: novo_core::PaperSize::A4,
        landscape: true,
        ..novo_core::PageFormat::default()
    });
    let a4 = single_line::sheet_layout_of(&sl, 0, &setup).unwrap();
    let height =
        |l: &novo_reports::PageLayout| l.texts.iter().map(|t| t.text_mm).fold(0.0, f64::max);
    assert!(height(&a4) <= height(&a3));
    assert_eq!(a3.texts.len(), a4.texts.len());
}
