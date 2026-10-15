//! Gate 2, "terminal, cable and report pages generate live": on the Gate 1
//! motor starter, change the schematic (a terminal, a cable core, a device,
//! a second strip) and check that the terminal diagram, the cable diagram
//! and the bill of materials, device list and wire list follow without a
//! regenerate step, in the live cache, on the generated pages' sheets and
//! in the printed PDF.
//!
//! The engine tests pass on main. The `gap_*` tests are the steps the app
//! does not do yet; they are ignored and name the gap (TEST_REPORT Round
//! 12), so `cargo test -p novo-tests --test gate2_live_pages -- --ignored`
//! lists every open Gate 2 gap as a failure.

use novo_app::{EditorCanvas, NovoApp, StorageBackend, page_labels};
use novo_core::*;
use novo_reports::{LiveReports, ReportOutput};
use novo_tests::{MotorStarter, motor_starter};

/// The generated pages of a project, as DESIGN.md §8 lays them out: one
/// page per generated report, numbered after the schematic pages.
const GENERATED: [(&str, &str); 5] = [
    ("20", "terminals"),
    ("21", "cables"),
    ("22", "bom"),
    ("23", "devices"),
    ("24", "wires"),
];

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

/// The starter with its generated pages, and the live reports primed.
fn starter() -> (Document, MotorStarter, LiveReports) {
    let mut doc = Document::default();
    let s = motor_starter(&mut doc).unwrap();
    doc.transact("Generated pages", |tx| {
        for (name, generator) in GENERATED {
            let page = tx.add_page(name)?;
            tx.update::<Page>(page, |p| {
                p.kind = PageKind::Generated {
                    generator: generator.into(),
                }
            })?;
        }
        Ok(())
    })
    .unwrap();
    let mut reports = LiveReports::standard();
    reports.all(doc.project());
    (doc, s, reports)
}

/// Applies one edit as one transaction and tells the live reports, as the
/// app does with every change set. Returns the reports it made stale.
fn edit(
    doc: &mut Document,
    reports: &mut LiveReports,
    label: &str,
    f: impl FnOnce(&mut Tx<'_>) -> Result<()>,
) -> Vec<String> {
    let ((), changes) = doc.transact(label, f).unwrap();
    reports
        .notify(doc.project(), &changes)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// Every live report equals a fresh generate of the same project: the
/// cache never shows an old state.
fn assert_current(doc: &Document, reports: &mut LiveReports) {
    let fresh = LiveReports::standard();
    for id in fresh.ids() {
        let expected = fresh.report(id).unwrap().generate(doc.project());
        let live = reports.get(doc.project(), id).unwrap();
        assert_eq!(live, &expected, "report {id} is stale");
    }
}

/// The text rows of one section of a live report.
fn rows(doc: &Document, reports: &mut LiveReports, id: &str, section: &str) -> Vec<Vec<String>> {
    let output: &ReportOutput = reports.get(doc.project(), id).unwrap();
    output
        .section(section)
        .unwrap_or_else(|| panic!("{id} has no section {section}"))
        .text_rows()
}

fn column(output: &ReportOutput, section: &str, key: &str) -> usize {
    output.section(section).unwrap().column(key).unwrap()
}

/// Every text on every sheet of the generated pages, as the canvas and the
/// PDF draw them.
fn sheet_texts(doc: &Document, reports: &mut LiveReports) -> Vec<String> {
    let mut texts = Vec::new();
    for sheet in reports.generated_sheets(doc.project()) {
        let layout = reports.layout_sheet(doc.project(), &sheet).unwrap();
        texts.extend(layout.texts.into_iter().map(|t| t.text));
    }
    texts
}

/// A terminal, a cable core, a device and a wire added to the starter each
/// show up in the reports that read them, the cache never serves an old
/// table, and undo takes every report back.
#[test]
fn reports_follow_every_edit() {
    let (mut doc, s, mut reports) = starter();
    let before: Vec<ReportOutput> = reports.all(doc.project()).into_iter().cloned().collect();
    let x1 = doc.project().device(s.x1).unwrap().tag.to_string();

    // A fourth feed-through terminal on -X1: a spare row on the strip and,
    // through the strip style, a fourth UT 2,5 on the bill of materials.
    let stale = edit(&mut doc, &mut reports, "Add -X1:4", |tx| {
        tx.add_terminal(s.x1, "4").map(drop)
    });
    for id in ["terminals", "bom"] {
        assert!(
            stale.iter().any(|s| s == id),
            "{id} not refreshed: {stale:?}"
        );
    }
    assert_current(&doc, &mut reports);
    let strip = rows(&doc, &mut reports, "terminals", &x1);
    assert_eq!(strip.len(), 5);
    let output = reports.get(doc.project(), "terminals").unwrap();
    let (name, spare) = (
        column(output, &x1, "terminal"),
        column(output, &x1, "spare"),
    );
    let row = strip
        .iter()
        .find(|r| r[name] == "4")
        .expect("-X1:4 on the strip");
    assert!(
        !row[spare].is_empty(),
        "an unwired terminal is marked spare"
    );
    let output = reports.get(doc.project(), "bom").unwrap();
    let (order, quantity, used_by) = (
        column(output, "parts", "order_number"),
        column(output, "parts", "quantity"),
        column(output, "parts", "used_by"),
    );
    let bom = rows(&doc, &mut reports, "bom", "parts");
    assert!(
        bom.iter().any(|r| r[order] == "3044076"
            && r[quantity] == "4"
            && r[used_by].contains(&format!("{x1}:4"))),
        "{bom:?}"
    );

    // A fifth core on -W1: the cable overview counts five cores, four of
    // them used, and the cable's own table lists it.
    let stale = edit(&mut doc, &mut reports, "Add a core", |tx| {
        tx.update::<Cable>(s.w1, |c| {
            let mut core = CableCore::new("BU");
            core.cross_section_mm2 = Some(1.5);
            c.cores.push(core);
        })
    });
    assert!(stale.iter().any(|s| s == "cables"), "{stale:?}");
    assert_current(&doc, &mut reports);
    let output = reports.get(doc.project(), "cables").unwrap();
    let (used, cores) = (
        column(output, "overview", "used"),
        column(output, "overview", "cores"),
    );
    let overview = rows(&doc, &mut reports, "cables", "overview");
    assert_eq!(
        (overview[0][used].as_str(), overview[0][cores].as_str()),
        ("4", "5")
    );
    // Each cable's table is keyed by its tag.
    let w1 = doc.project().cable(s.w1).unwrap().tag.to_string();
    assert!(
        rows(&doc, &mut reports, "cables", &w1)
            .iter()
            .any(|r| r[0] == "BU")
    );

    // A new device: the device list has it, in tag order.
    let stale = edit(&mut doc, &mut reports, "Add -K2", |tx| {
        tx.add_device(tag("=A1+CAB-K2"), "Second contactor")
            .map(drop)
    });
    assert!(stale.iter().any(|s| s == "devices"), "{stale:?}");
    assert_current(&doc, &mut reports);
    let devices: Vec<String> = rows(&doc, &mut reports, "devices", "devices")
        .into_iter()
        .map(|r| r[0].clone())
        .collect();
    let k1 = devices.iter().position(|t| t == "=A1+CAB-K1").unwrap();
    assert_eq!(devices[k1 + 1], "=A1+CAB-K2");

    // A new wire: the wire list has one more row.
    let wires_before = rows(&doc, &mut reports, "wires", "wires").len();
    // -K1:1 to -F2:1, both on L1: a parallel wire, no short.
    let stale = edit(&mut doc, &mut reports, "Add a wire", |tx| {
        tx.connect(Endpoint::pin(s.k1_main, 0), Endpoint::pin(s.f2_main, 0))
            .map(drop)
    });
    assert!(stale.iter().any(|s| s == "wires"), "{stale:?}");
    assert_current(&doc, &mut reports);
    assert_eq!(
        rows(&doc, &mut reports, "wires", "wires").len(),
        wires_before + 1
    );

    // The generated sheets draw the new rows.
    let texts = sheet_texts(&doc, &mut reports);
    for text in ["BU", "=A1+CAB-K2"] {
        assert!(texts.iter().any(|t| t == text), "{text} not on a sheet");
    }

    // Undo, with each change set told to the cache, gives every report back.
    for _ in 0..4 {
        let changes = doc.undo().unwrap().expect("an edit to undo");
        reports.notify(doc.project(), &changes);
    }
    assert_current(&doc, &mut reports);
    let after: Vec<ReportOutput> = reports.all(doc.project()).into_iter().cloned().collect();
    let strip = |outputs: &[ReportOutput]| -> Vec<_> {
        outputs.iter().map(|o| o.sections.clone()).collect()
    };
    assert_eq!(strip(&after), strip(&before));
}

/// A second strip gets its own table, so its generated page grows by a
/// sheet named with a letter (DESIGN.md §8), and the strip's tag names it.
#[test]
fn a_new_strip_gets_its_own_sheet() {
    let (mut doc, _, mut reports) = starter();
    let page = doc.project().pages().find(|p| p.name == "20").unwrap().id;
    let before = reports.sheets_of_page(doc.project(), page);
    assert_eq!(
        before.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["20"]
    );
    edit(&mut doc, &mut reports, "Add -X2", |tx| {
        let strip = tx.add_terminal_strip(tag("=A1+CAB-X2"))?;
        for name in ["1", "2"] {
            tx.add_terminal(strip, name)?;
        }
        Ok(())
    });
    let after = reports.sheets_of_page(doc.project(), page);
    assert_eq!(
        after.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["20a", "20b"]
    );
    assert!(
        sheet_texts(&doc, &mut reports)
            .iter()
            .any(|t| t.contains("=A1+CAB-X2"))
    );
}

/// The printed PDF regenerates its report sheets from the project it is
/// given: an edit shows up in the next export with no other step.
#[test]
fn the_pdf_prints_the_edit() {
    let (mut doc, s, mut reports) = starter();
    let pdf = |doc: &Document| {
        let bytes = novo_pdf::export_pdf(doc.project(), &Default::default()).unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    };
    assert!(!pdf(&doc).contains("(=A1+CAB-K2)"));
    edit(&mut doc, &mut reports, "Add -K2", |tx| {
        tx.add_device(tag("=A1+CAB-K2"), "Second contactor")
            .map(drop)
    });
    edit(&mut doc, &mut reports, "Add a core", |tx| {
        tx.update::<Cable>(s.w1, |c| c.cores.push(CableCore::new("BU")))
    });
    let text = pdf(&doc);
    for needle in ["(=A1+CAB-K2)", "(BU)"] {
        assert!(text.contains(needle), "{needle} not printed");
    }
}

/// The starter saved as a project file, opened in the app's window.
fn open_in_window(dir: &std::path::Path) -> NovoApp {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let path = dir.join("starter.novo");
    novo_storage::Session::create(&path, doc.project()).unwrap();
    let mut app = NovoApp::with_settings(
        Box::new(StorageBackend::default()),
        Box::new(EditorCanvas::default()),
        Default::default(),
    );
    app.open_on_start(path);
    let ctx = egui::Context::default();
    novo_theme::apply(&ctx);
    let mut output = ctx.run_ui(Default::default(), |ui| app.show(ui));
    // Headless: no renderer takes the textures.
    output.textures_delta.clear();
    assert!(app.document().is_some(), "{:?}", app.messages());
    app
}

/// G2-1: generated pages are automatic (DESIGN.md §2: "Terminal, cable and
/// report pages: the model, automatic and live"). A project with a terminal
/// strip, a cable and parts should have its terminal diagram, cable
/// diagram, bill of materials, device list and wire list pages, after the
/// schematic pages, without the user adding them. Neither the app nor any
/// engine adds a `PageKind::Generated` page today.
#[test]
fn gap_the_project_gets_its_generated_pages() {
    let dir = tempfile::tempdir().unwrap();
    let app = open_in_window(dir.path());
    let project = app.document().unwrap().project();
    let generators: Vec<String> = project
        .pages_in_order()
        .into_iter()
        .filter_map(|p| match &p.kind {
            PageKind::Generated { generator } => Some(generator.clone()),
            _ => None,
        })
        .collect();
    for id in ["terminals", "cables", "bom", "devices", "wires"] {
        assert!(
            generators.iter().any(|g| g == id),
            "no {id} page: {generators:?}"
        );
    }
}

/// G2-2: the project panel lists a generated page once, by its record
/// name, even when its report needs several sheets. It should list each
/// sheet by its name and what it shows (`20a  Terminals · -X1`, DESIGN.md
/// §8), so a sheet can be opened from the panel.
#[test]
fn gap_the_project_panel_lists_each_generated_sheet() {
    let (mut doc, _, mut reports) = starter();
    edit(&mut doc, &mut reports, "Add -X2", |tx| {
        let strip = tx.add_terminal_strip(tag("=A1+CAB-X2"))?;
        tx.add_terminal(strip, "1").map(drop)
    });
    let labels: Vec<String> = page_labels(&doc).into_iter().map(|(_, l)| l).collect();
    for sheet in ["20a", "20b"] {
        assert!(
            labels.iter().any(|l| l.starts_with(sheet)),
            "{sheet} not in {labels:?}"
        );
    }
}
