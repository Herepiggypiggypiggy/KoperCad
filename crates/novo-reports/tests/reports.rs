//! The built-in reports against the Gate 1 motor starter fixture.

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use novo_core::{
    Device, DeviceTag, Document, Endpoint, MountingPlate, Page, PageFormat, PaperSize, Pin,
    Placement, Point, RailKind, SymbolRole, Terminal, Wire,
};
use novo_reports::{
    Cell, CsvOptions, CutList, LabelLayout, LiveReports, PageSetup, ReportOutput, generate_all,
    labels_csv, layout_page, paginate, tag_cmp, to_csv, xlsx_bytes,
};
use novo_tests::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let handles = motor_starter(&mut doc).expect("fixture builds");
    (doc, handles)
}

fn report(doc: &Document, id: &str) -> ReportOutput {
    LiveReports::standard()
        .get(doc.project(), id)
        .expect("built-in report")
        .clone()
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
fn table_of_contents_lists_pages_in_order() {
    let (doc, _) = starter();
    let toc = report(&doc, "toc");
    assert_eq!(
        rows(&toc, "pages"),
        strings(&[
            &["1", "Main circuit", "", "", "Schematic"],
            &["2", "Control circuit", "", "", "Schematic"],
        ])
    );
}

#[test]
fn bill_of_materials_groups_parts() {
    let (doc, _) = starter();
    let bom = report(&doc, "bom");
    assert_eq!(
        rows(&bom, "parts"),
        strings(&[
            &["1", "1", "Lapp", "1119304", "", "", "12", "=A1-W1"],
            &[
                "2",
                "3",
                "Phoenix Contact",
                "3044076",
                "",
                "",
                "",
                "=A1+CAB-X1:1, =A1+CAB-X1:2, =A1+CAB-X1:3"
            ],
            &[
                "3",
                "1",
                "Phoenix Contact",
                "3044089",
                "",
                "",
                "",
                "=A1+CAB-X1:PE"
            ],
            &[
                "4",
                "1",
                "Siemens",
                "1LE1003-0EB42-2AA4",
                "",
                "",
                "",
                "=A1+FLD-M1"
            ],
            &["5", "1", "Siemens", "3NW7033", "", "", "", "=A1+CAB-F1"],
            &[
                "6",
                "1",
                "Siemens",
                "3RT2016-1AP01",
                "",
                "",
                "",
                "=A1+CAB-K1"
            ],
            &[
                "7",
                "1",
                "Siemens",
                "3RU2116-1FB0",
                "",
                "",
                "",
                "=A1+CAB-F2"
            ],
            &[
                "8",
                "1",
                "Siemens",
                "3SU1100-0AB20-1CA0",
                "",
                "",
                "",
                "=A1+CAB-S0"
            ],
            &[
                "9",
                "1",
                "Siemens",
                "3SU1100-0AB40-1BA0",
                "",
                "",
                "",
                "=A1+CAB-S1"
            ],
            &["10", "1", "Siemens", "5SY6106-7", "", "", "", "=A1+CAB-F3"],
        ])
    );
    // Every fixture device has a part.
    assert!(bom.section("without_parts").is_none());
    // Quantities are numbers, not text.
    let qty = &bom.sections[0].rows[0].cells[1];
    assert_eq!(qty, &Cell::Int(1));
}

#[test]
fn bill_of_materials_counts_uses_and_lists_devices_without_parts() {
    let (mut doc, h) = starter();
    doc.transact("More devices", |tx| {
        // A second contactor with the same article as -K1.
        let k2 = tx.add_device(DeviceTag::parse("=A1+CAB-K2")?, "Second contactor")?;
        let k1_part = tx.project().device(h.k1).expect("k1").parts[0];
        tx.assign_part(k2, k1_part)?;
        // Terminal parts are counted per terminal.
        let clamp = tx.add_part("Phoenix Contact", "3031212")?;
        for terminal in tx
            .project()
            .terminals_of_strip(h.x1)
            .iter()
            .map(|t| t.id)
            .collect::<Vec<_>>()
        {
            tx.update::<Terminal>(terminal, |t| t.part = Some(clamp))?;
        }
        tx.add_device(DeviceTag::parse("=A1+CAB-K10")?, "Spare relay")?;
        tx.add_device(DeviceTag::parse("=A1+CAB-H1")?, "Lamp")?;
        Ok(())
    })
    .expect("edit");
    let bom = report(&doc, "bom");
    let parts = rows(&bom, "parts");
    let k1 = parts
        .iter()
        .find(|r| r[3] == "3RT2016-1AP01")
        .expect("k1 row");
    assert_eq!(k1[1], "2");
    assert_eq!(k1[7], "=A1+CAB-K1, =A1+CAB-K2");
    let clamps = parts.iter().find(|r| r[3] == "3031212").expect("clamps");
    assert_eq!(clamps[1], "4");
    assert_eq!(
        clamps[7],
        "=A1+CAB-X1:1, =A1+CAB-X1:2, =A1+CAB-X1:3, =A1+CAB-X1:PE"
    );
    // Natural tag order: K10 after K2's neighbours, H1 before K10.
    assert_eq!(
        rows(&bom, "without_parts"),
        strings(&[
            &["=A1+CAB-H1", "Lamp", "Device"],
            &["=A1+CAB-K10", "Spare relay", "Device"],
        ])
    );
}

#[test]
fn device_list_has_structure_parts_and_locations() {
    let (doc, _) = starter();
    let devices = report(&doc, "devices");
    assert_eq!(
        rows(&devices, "devices"),
        strings(&[
            &[
                "=A1+CAB-F1",
                "=A1",
                "+CAB",
                "Main circuit fuses",
                "Device",
                "3NW7033",
                "Siemens",
                "1.1"
            ],
            &[
                "=A1+CAB-F2",
                "=A1",
                "+CAB",
                "Overload relay",
                "Device",
                "3RU2116-1FB0",
                "Siemens",
                "1.1, 2.1"
            ],
            &[
                "=A1+CAB-F3",
                "=A1",
                "+CAB",
                "Control circuit breaker",
                "Device",
                "5SY6106-7",
                "Siemens",
                "2.1"
            ],
            // Main function (the coil) first, then reading order.
            &[
                "=A1+CAB-K1",
                "=A1",
                "+CAB",
                "Motor contactor",
                "Device",
                "3RT2016-1AP01",
                "Siemens",
                "2.1, 1.1, 2.2"
            ],
            &[
                "=A1+CAB-S0",
                "=A1",
                "+CAB",
                "Stop",
                "Device",
                "3SU1100-0AB20-1CA0",
                "Siemens",
                "2.1"
            ],
            &[
                "=A1+CAB-S1",
                "=A1",
                "+CAB",
                "Start",
                "Device",
                "3SU1100-0AB40-1BA0",
                "Siemens",
                "2.1"
            ],
            &[
                "=A1+CAB-X1",
                "=A1",
                "+CAB",
                "",
                "Terminal strip",
                "",
                "",
                "1.1, 1.2"
            ],
            &[
                "=A1+FLD-M1",
                "=A1",
                "+FLD",
                "Pump motor 2.2 kW",
                "Device",
                "1LE1003-0EB42-2AA4",
                "Siemens",
                "1.1"
            ],
        ])
    );
}

#[test]
fn wire_list_runs_pin_to_pin() {
    let (doc, _) = starter();
    let wires = report(&doc, "wires");
    // The first eight columns; the cut length has its own tests.
    let first_eight = |rows: Vec<Vec<String>>| -> Vec<Vec<String>> {
        rows.into_iter().map(|r| r[..8].to_vec()).collect()
    };
    assert_eq!(
        first_eight(rows(&wires, "wires")),
        strings(&[
            &["", "=A1+CAB-F1:2", "=A1+CAB-K1:1", "L1", "", "", "", "1.1"],
            &["", "=A1+CAB-K1:2", "=A1+CAB-F2:1", "L1", "", "", "", "1.1"],
            &["", "=A1+CAB-F2:2", "=A1+CAB-X1:1", "L1", "", "", "", "1.1"],
            &[
                "",
                "=A1+CAB-X1:1",
                "=A1+FLD-M1:U1",
                "L1",
                "",
                "1.5",
                "=A1-W1 BK",
                "1.1"
            ],
            &["", "=A1+CAB-F1:4", "=A1+CAB-K1:3", "L2", "", "", "", "1.2"],
            &["", "=A1+CAB-K1:4", "=A1+CAB-F2:3", "L2", "", "", "", "1.2"],
            &["", "=A1+CAB-F2:4", "=A1+CAB-X1:2", "L2", "", "", "", "1.2"],
            &[
                "",
                "=A1+CAB-X1:2",
                "=A1+FLD-M1:V1",
                "L2",
                "",
                "1.5",
                "=A1-W1 BN",
                "1.2"
            ],
            &["", "=A1+CAB-F1:6", "=A1+CAB-K1:5", "L3", "", "", "", "1.2"],
            &["", "=A1+CAB-K1:6", "=A1+CAB-F2:5", "L3", "", "", "", "1.2"],
            &["", "=A1+CAB-F2:6", "=A1+CAB-X1:3", "L3", "", "", "", "1.2"],
            &[
                "",
                "=A1+CAB-X1:3",
                "=A1+FLD-M1:W1",
                "L3",
                "",
                "1.5",
                "=A1-W1 GY",
                "1.2"
            ],
            &[
                "",
                "=A1+CAB-X1:PE",
                "=A1+FLD-M1:PE",
                "PE",
                "",
                "1.5",
                "=A1-W1 GNYE",
                "1.2"
            ],
            &["", "=A1+CAB-F3:2", "=A1+CAB-S0:11", "L", "", "", "", "2.1"],
            // The T-junction below -S0 becomes a daisy chain, top to bottom.
            &["", "=A1+CAB-S0:12", "=A1+CAB-S1:13", "L", "", "", "", "2.1"],
            &["", "=A1+CAB-S1:13", "=A1+CAB-K1:13", "L", "", "", "", "2.1"],
            &["", "=A1+CAB-S1:14", "=A1+CAB-K1:14", "L", "", "", "", "2.1"],
            &["", "=A1+CAB-K1:14", "=A1+CAB-F2:95", "L", "", "", "", "2.2"],
            &["", "=A1+CAB-F2:96", "=A1+CAB-K1:A1", "L", "", "", "", "2.1"],
        ])
    );
    // The supply feeds (L1 into -F1, -K1:A2 to N) are not wiring work.
    let all = rows(&wires, "wires");
    assert!(
        !all.iter()
            .any(|r| r[1].ends_with(":A2") || r[2].ends_with(":A2"))
    );
    // Rows point back at their wires.
    assert!(wires.sections[0].rows.iter().all(|r| !r.sources.is_empty()));
}

#[test]
fn wire_list_reads_numbers_colours_and_joins_interruption_points() {
    let (mut doc, h) = starter();
    doc.transact("Number and continue", |tx| {
        let wire = tx
            .project()
            .wires_at(Endpoint::pin(h.k1_coil, 0))
            .next()
            .expect("coil wire");
        tx.update::<Wire>(wire, |w| {
            w.number = Some("12".into());
            w.color = Some("BU".into());
            w.cross_section_mm2 = Some(0.75);
        })?;
        // -K1:A2 continues to page 3 through an interruption point "N1".
        let page3 = tx.add_page("3")?;
        let on2 = tx.place_symbol(
            h.control_page,
            "iec/connectivity/interruption",
            Placement::at(Point::new(80.0, 184.0)),
            vec![Pin::new("1", 0.0, 0.0)],
            SymbolRole::Interruption { name: "N1".into() },
        )?;
        let on3 = tx.place_symbol(
            page3,
            "iec/connectivity/interruption",
            Placement::at(Point::new(20.0, 20.0)),
            vec![Pin::new("1", 0.0, 0.0)],
            SymbolRole::Interruption { name: "N1".into() },
        )?;
        let h1 = tx.add_device(DeviceTag::parse("=A1+CAB-H1")?, "Lamp")?;
        let lamp = tx.place_symbol(
            page3,
            "iec/signalling/lamp",
            Placement::at(Point::new(20.0, 40.0)),
            vec![Pin::new("X1", 0.0, 0.0), Pin::new("X2", 0.0, 16.0)],
            SymbolRole::Device {
                device: h1,
                main: true,
            },
        )?;
        tx.connect(Endpoint::pin(h.k1_coil, 1), Endpoint::pin(on2, 0))?;
        tx.connect(Endpoint::pin(on3, 0), Endpoint::pin(lamp, 1))?;
        Ok(())
    })
    .expect("edit");
    let wires = report(&doc, "wires");
    let all = rows(&wires, "wires");
    let coil = all
        .iter()
        .find(|r| r[2] == "=A1+CAB-K1:A1")
        .expect("coil row");
    assert_eq!(coil[0], "12");
    assert_eq!(coil[4], "BU");
    assert_eq!(coil[5], "0.75");
    let numeric = &wires.sections[0].rows[all
        .iter()
        .position(|r| r[2] == "=A1+CAB-K1:A1")
        .expect("row")]
    .cells[5];
    assert_eq!(numeric, &Cell::Number(0.75));
    // -K1:A2 now reaches -H1:X2 on page 3 (it also touches the N feed).
    let across = all
        .iter()
        .find(|r| r[1] == "=A1+CAB-K1:A2")
        .expect("continued connection");
    assert_eq!(across[2], "=A1+CAB-H1:X2");
    assert_eq!(across[3], "N");
}

#[test]
fn tags_sort_naturally() {
    let parse = |t| DeviceTag::parse(t).expect("tag");
    let mut tags = [
        parse("=A2-K1"),
        parse("=A1-K10"),
        parse("=A1-K2"),
        parse("=A1+CAB2-K1"),
        parse("=A1+CAB10-K1"),
    ];
    tags.sort_by(tag_cmp);
    let text: Vec<String> = tags.iter().map(ToString::to_string).collect();
    assert_eq!(
        text,
        ["=A1-K2", "=A1-K10", "=A1+CAB2-K1", "=A1+CAB10-K1", "=A2-K1"]
    );
}

#[test]
fn reports_are_deterministic() {
    let (a, _) = starter();
    let (b, _) = starter();
    assert_eq!(generate_all(a.project()), generate_all(b.project()));
    // Same bytes for the same project: exports can be compared and cached.
    let outputs = generate_all(a.project());
    let refs: Vec<&ReportOutput> = outputs.iter().collect();
    let csv: Vec<String> = outputs
        .iter()
        .map(|o| to_csv(o, &CsvOptions::default()))
        .collect();
    let again = generate_all(b.project());
    let csv_again: Vec<String> = again
        .iter()
        .map(|o| to_csv(o, &CsvOptions::default()))
        .collect();
    assert_eq!(csv, csv_again);
    // xlsx too, even across a clock tick (a creation timestamp would
    // change the bytes once a second).
    let first = xlsx_bytes(&refs).expect("xlsx");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let again_refs: Vec<&ReportOutput> = again.iter().collect();
    assert_eq!(first, xlsx_bytes(&again_refs).expect("xlsx"));
}

/// A project with `n` devices, for paging.
fn many_devices(n: usize) -> Document {
    let mut doc = Document::default();
    doc.transact("Devices", |tx| {
        for i in 1..=n {
            tx.add_device(DeviceTag::parse(&format!("-K{i}"))?, "Relay")?;
        }
        Ok(())
    })
    .expect("devices");
    doc
}

#[test]
fn paging_repeats_headers_and_numbers_pages() {
    let doc = many_devices(100);
    let devices = report(&doc, "devices");
    let setup = PageSetup::for_format(PageFormat {
        size: PaperSize::A4,
        landscape: false,
        columns: 10,
    });
    let per_page = setup.rows_per_page();
    // Inside the canvas frame: 297 - 15 - 12 - 10 - 40 = 220 mm; minus the
    // 8 mm header: 35 rows of 6 mm.
    assert_eq!(per_page, 35);
    let pages = paginate(&devices, &setup);
    assert_eq!(pages.len(), 3);
    assert_eq!(pages[0].rows, 0..35);
    assert_eq!(pages[2].rows, 70..100);
    assert!(!pages[0].continued && pages[1].continued);
    for page in &pages {
        assert_eq!(page.count, 3);
        let layout = layout_page(&devices, page, &setup);
        assert_eq!((layout.width, layout.height), (210.0, 297.0));
        // Header repeated on every page.
        assert!(layout.texts.iter().any(|t| t.text == "Tag" && t.bold));
        assert!(
            layout
                .texts
                .iter()
                .any(|t| t.text == format!("page {} / 3", page.number))
        );
        // Everything inside the frame and above the title block.
        for text in &layout.texts {
            // 5 mm inside the 10 mm frame, clear of the 36 mm title block.
            assert!(text.x >= 15.0 && text.x + text.width <= 195.0 + 1e-9);
            assert!(text.y >= 15.0 && text.y + text.height <= 297.0 - 10.0 - 36.0 + 1e-9);
        }
    }
    // Natural order across pages: -K35 ends page 1, -K36 starts page 2.
    let tags = |page: &novo_reports::ReportPage| -> Vec<String> {
        layout_page(&devices, page, &setup)
            .texts
            .iter()
            .filter(|t| t.text.starts_with("-K"))
            .map(|t| t.text.clone())
            .collect()
    };
    assert_eq!(tags(&pages[0]).last().map(String::as_str), Some("-K35"));
    assert_eq!(tags(&pages[1]).first().map(String::as_str), Some("-K36"));
    assert!(
        layout_page(&devices, &pages[1], &setup)
            .texts
            .iter()
            .any(|t| t.text == "Device list (continued)")
    );
}

#[test]
fn every_section_starts_a_page_and_empty_reports_get_one() {
    let empty = Document::default();
    let setup = PageSetup::default();
    for output in generate_all(empty.project()) {
        let pages = paginate(&output, &setup);
        assert_eq!(pages.len(), 1, "{}", output.title);
        assert_eq!(pages[0].rows, 0..0);
        let layout = layout_page(&output, &pages[0], &setup);
        assert!(
            layout
                .texts
                .iter()
                .any(|t| t.bold && t.text.starts_with(&output.title)),
            "{}",
            output.title
        );
    }
    let mut doc = Document::default();
    doc.transact("Unassigned", |tx| {
        tx.add_device(DeviceTag::parse("-K1")?, "No part")?;
        Ok(())
    })
    .expect("device");
    let bom = report(&doc, "bom");
    let pages = paginate(&bom, &setup);
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[1].section, 1);
    assert!(
        layout_page(&bom, &pages[1], &setup)
            .texts
            .iter()
            .any(|t| t.text == "Bill of materials · Devices without parts")
    );
}

#[test]
fn xlsx_round_trips() {
    let (doc, _) = starter();
    let outputs = generate_all(doc.project());
    let refs: Vec<&ReportOutput> = outputs.iter().collect();
    let bytes = xlsx_bytes(&refs).expect("xlsx");
    let mut book: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).expect("opens");
    assert_eq!(
        book.sheet_names(),
        [
            "Table of contents",
            "Bill of materials",
            "Device list",
            "Wire list",
            "Cut list",
            "Wire production",
            "Terminal diagram",
            "Cable diagram",
            "IO list",
            "Cable schedule",
            "Plug connection list",
            "Labels"
        ]
    );
    // Cell by cell for the one-table sheets (a sheet with several sections
    // stacks them under titles).
    for output in outputs.iter().filter(|o| o.sections.len() == 1) {
        let range = book.worksheet_range(&output.title).expect("sheet");
        let sheet: Vec<Vec<Data>> = range.rows().map(<[Data]>::to_vec).collect();
        let section = &output.sections[0];
        let header: Vec<String> = sheet[0].iter().map(ToString::to_string).collect();
        let expected: Vec<String> = section.columns.iter().map(|c| c.header.clone()).collect();
        assert_eq!(header, expected, "{}", output.title);
        assert_eq!(sheet.len(), 1 + section.rows.len(), "{}", output.title);
        for (data, row) in sheet[1..].iter().zip(&section.rows) {
            for (value, cell) in data.iter().zip(&row.cells) {
                match cell {
                    Cell::Empty => assert_eq!(value, &Data::Empty),
                    Cell::Text(text) => assert_eq!(value, &Data::String(text.clone())),
                    Cell::Int(n) => assert_eq!(value, &Data::Float(*n as f64)),
                    Cell::Number(n) => assert_eq!(value, &Data::Float(*n)),
                }
            }
        }
    }
}

#[test]
fn csv_quotes_and_localises() {
    let (mut doc, h) = starter();
    doc.transact("Quote", |tx| {
        tx.update::<Device>(h.m1, |d| d.description = "Pump, \"main\"".into())?;
        tx.update::<Page>(h.main_page, |p| p.title = "Main; power".into())
    })
    .expect("edit");
    let devices = report(&doc, "devices");
    let csv = to_csv(&devices, &CsvOptions::default());
    assert!(csv.starts_with("\u{feff}Tag,Function,Location,Description,"));
    assert!(csv.contains("'=A1+FLD-M1,'=A1,'+FLD,\"Pump, \"\"main\"\"\",Device,"));
    assert!(csv.ends_with("\r\n"));

    let wires = report(&doc, "wires");
    let semicolon = CsvOptions {
        delimiter: ';',
        bom: false,
        excel_safe: false,
    };
    let csv = to_csv(&wires, &semicolon);
    assert!(csv.starts_with("Wire no.;From;To;"));
    assert!(csv.contains(";1,5;=A1-W1 BK;"));
    let toc = to_csv(&report(&doc, "toc"), &semicolon);
    assert!(toc.contains("1;\"Main; power\";;;Schematic\r\n"));
}

#[test]
fn csv_for_excel_never_starts_a_text_field_with_a_formula_character() {
    let (mut doc, h) = starter();
    doc.transact("Hostile", |tx| {
        tx.update::<Device>(h.m1, |d| d.description = "@SUM(1+1)".into())?;
        tx.add_device(DeviceTag::parse("-K1")?, "-24 V relay")?;
        Ok(())
    })
    .expect("edit");
    for delimiter in [',', ';'] {
        let options = CsvOptions {
            delimiter,
            ..CsvOptions::default()
        };
        assert!(options.excel_safe, "on by default");
        for output in generate_all(doc.project()) {
            let csv = to_csv(&output, &options);
            for line in csv.trim_start_matches('\u{feff}').lines() {
                for field in line.split(delimiter) {
                    let field = field.trim_start_matches('"');
                    assert!(
                        !field.starts_with(['=', '+', '-', '@', '\t']),
                        "{}: {field:?} in {line:?}",
                        output.title
                    );
                }
            }
        }
    }
    let devices = report(&doc, "devices");
    let csv = to_csv(&devices, &CsvOptions::default());
    assert!(csv.contains("\r\n'-K1,,,'-24 V relay,"));
    assert!(csv.contains(",'@SUM(1+1),"));
    // Numbers stay numbers, even negative ones.
    let mut section =
        novo_reports::Section::new("n", "", vec![novo_reports::Column::new("v", "Value", 1.0)]);
    section.push(vec![Cell::Number(-1.5)], vec![]);
    let output = ReportOutput {
        report: "n".into(),
        title: "Numbers".into(),
        revision: 0,
        sections: vec![section],
    };
    assert_eq!(
        to_csv(&output, &CsvOptions::default()),
        "\u{feff}Value\r\n-1.5\r\n"
    );
    // Raw values for machine readers.
    let raw = CsvOptions {
        excel_safe: false,
        ..CsvOptions::default()
    };
    assert!(to_csv(&devices, &raw).contains("\r\n-K1,,,-24 V relay,"));
}

#[test]
fn xlsx_sheet_names_are_always_valid() {
    let named = |title: &str| ReportOutput {
        report: "x".into(),
        title: title.into(),
        revision: 0,
        sections: Vec::new(),
    };
    // 30 characters then an apostrophe that ends up last after cutting.
    let long = format!("{}'{}", "a".repeat(30), "b".repeat(5));
    let outputs = [named("History"), named(&long), named(&long), named("'")];
    let refs: Vec<&ReportOutput> = outputs.iter().collect();
    let bytes = xlsx_bytes(&refs).expect("valid names");
    let book: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).expect("opens");
    let names = book.sheet_names();
    assert_eq!(names.len(), 4);
    for name in &names {
        assert!(name.chars().count() <= 31, "{name}");
        assert!(!name.starts_with('\'') && !name.ends_with('\''), "{name}");
        assert!(!name.eq_ignore_ascii_case("history"), "{name}");
    }
}

#[test]
fn save_xlsx_replaces_the_file_whole() {
    let (doc, _) = starter();
    let outputs = generate_all(doc.project());
    let refs: Vec<&ReportOutput> = outputs.iter().collect();
    let dir = std::env::temp_dir().join(format!("novo-reports-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("reports.xlsx");
    std::fs::write(&path, b"old").expect("old file");
    novo_reports::save_xlsx(&refs, &path).expect("saved");
    let bytes = std::fs::read(&path).expect("read");
    assert_eq!(bytes, xlsx_bytes(&refs).expect("xlsx"));
    assert!(!dir.join("reports.xlsx.partial").exists());
    // A failed write leaves the previous file alone.
    let missing = dir.join("no-such-dir").join("reports.xlsx");
    assert!(novo_reports::save_xlsx(&refs, &missing).is_err());
    assert_eq!(std::fs::read(&path).expect("still there"), bytes);
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

fn wire_row(wires: &ReportOutput, from: &str, to: &str) -> Vec<String> {
    rows(wires, "wires")
        .into_iter()
        .find(|r| r[1] == from && r[2] == to)
        .unwrap_or_else(|| panic!("no wire {from} to {to}"))
}

#[test]
fn wire_list_estimates_lengths_until_devices_are_placed() {
    let (doc, _) = starter();
    let wires = report(&doc, "wires");
    let headers: Vec<&str> = wires.section("wires").unwrap().columns[8..]
        .iter()
        .map(|c| c.header.as_str())
        .collect();
    assert_eq!(headers, ["Cut length mm", "Length basis"]);
    for row in rows(&wires, "wires") {
        assert_eq!(row[9], "estimate", "{row:?}");
        // At least the minimum run plus a service loop at each end.
        assert!(row[8].parse::<f64>().unwrap() >= 200.0, "{row:?}");
    }
    let cell = &wires.section("wires").unwrap().rows[0].cells[8];
    assert!(matches!(cell, Cell::Number(_)), "stays a number for Excel");
}

#[test]
fn wire_list_routes_lengths_through_the_ducts_of_the_plate() {
    let (mut doc, h) = starter();
    let (cab, duct) = doc
        .transact("Cabinet", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB")?, "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 600.0,
                    margin: 10.0,
                }),
            )?;
            tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, None)?;
            tx.place_device(h.k1, cab, Point::new(400.0, 100.0), false, None)?;
            let duct = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
            tx.set_rail_size(duct, 700.0, 50.0, 60.0)?;
            Ok((cab, duct))
        })
        .expect("layout")
        .0;
    let wires = report(&doc, "wires");
    // Both footprints are the default 45 x 90 mm: anchors at the centres,
    // 105 mm down to the duct each, 300 mm along it, 50 mm slack per end, and
    // 30 mm per end from the plate to the middle of the 60 mm duct: the
    // parts have no depth yet, so the list says it measured from the plate.
    let row = wire_row(&wires, "=A1+CAB-F1:2", "=A1+CAB-K1:1");
    assert_eq!(
        (row[8].as_str(), row[9].as_str()),
        ("670", "routed, no depth")
    );
    // With a depth the terminals stand 80 mm off the plate: 50 mm to the duct.
    let parts: Vec<_> = [h.f1, h.k1]
        .iter()
        .flat_map(|d| doc.project().device(*d).unwrap().parts.clone())
        .collect();
    assert!(!parts.is_empty(), "the starter's devices have parts");
    doc.transact("Depth", |tx| {
        for part in &parts {
            tx.set_property(*part, "depth_mm", Some(novo_core::Value::Float(80.0)))?;
        }
        Ok(())
    })
    .expect("depth");
    let wires = report(&doc, "wires");
    let row = wire_row(&wires, "=A1+CAB-F1:2", "=A1+CAB-K1:1");
    assert_eq!((row[8].as_str(), row[9].as_str()), ("710", "routed"));
    // F2 is not placed: that wire stays an estimate.
    let row = wire_row(&wires, "=A1+CAB-K1:2", "=A1+CAB-F2:1");
    assert_eq!(row[9], "estimate");

    // Without the duct the same wire runs in free air: Manhattan + slack.
    doc.transact("Remove duct", |tx| tx.remove(duct))
        .expect("delete");
    let wires = report(&doc, "wires");
    let row = wire_row(&wires, "=A1+CAB-F1:2", "=A1+CAB-K1:1");
    assert_eq!((row[8].as_str(), row[9].as_str()), ("400", "free air"));

    // Moving a device changes the length with no extra edit.
    doc.transact("Move", |tx| {
        tx.place_device(h.k1, cab, Point::new(500.0, 100.0), false, None)
    })
    .expect("move");
    let wires = report(&doc, "wires");
    let row = wire_row(&wires, "=A1+CAB-F1:2", "=A1+CAB-K1:1");
    assert_eq!(row[8], "500");
}

#[test]
fn wire_list_csv_carries_the_cut_length() {
    let (doc, _) = starter();
    let csv = to_csv(&report(&doc, "wires"), &CsvOptions::default());
    let header = csv.lines().next().expect("header");
    assert!(
        header.ends_with("Location,Cut length mm,Length basis"),
        "{header}"
    );
    assert!(csv.contains(",estimate\r\n"));
}

#[test]
fn cut_list_groups_by_size_and_colour_longest_first() {
    use novo_reports::Report;
    let (mut doc, h) = starter();
    doc.transact("Colours", |tx| {
        let wire = tx
            .project()
            .wires_at(Endpoint::pin(h.k1_coil, 0))
            .next()
            .expect("coil wire");
        tx.update::<Wire>(wire, |w| {
            w.color = Some("BU".into());
            w.cross_section_mm2 = Some(0.75);
        })
    })
    .expect("edit");
    let cuts = CutList::default().generate(doc.project());
    let wires = report(&doc, "wires");
    let total_wires = wires.section("wires").unwrap().rows.len();

    // Summary first, then one table per cross-section and colour.
    let summary = cuts.section("summary").expect("summary");
    let counts: i64 = summary
        .rows
        .iter()
        .map(|r| match r.cells[1] {
            Cell::Int(n) => n,
            _ => panic!("count is an integer"),
        })
        .sum();
    assert_eq!(counts as usize, total_wires);
    let titles: Vec<String> = summary
        .text_rows()
        .into_iter()
        .map(|r| r[0].clone())
        .collect();
    assert_eq!(titles.first().map(String::as_str), Some("0.75 mm² BU"));
    assert_eq!(titles.last().map(String::as_str), Some("Size unknown"));
    assert!(titles.contains(&"1.5 mm²".to_owned()), "{titles:?}");

    // Ferrule allowance: 10 mm at each end on top of the wire list's length.
    let table = &cuts.sections[1];
    let row = &table.text_rows()[0];
    let (wire, ends, cut): (f64, f64, f64) = (
        row[5].parse().unwrap(),
        row[6].parse().unwrap(),
        row[7].parse().unwrap(),
    );
    assert_eq!((ends, cut), (20.0, wire + 20.0));
    // Longest first within a table.
    for table in &cuts.sections[1..] {
        let lengths: Vec<f64> = table
            .text_rows()
            .iter()
            .map(|r| r[7].parse().unwrap())
            .collect();
        assert!(lengths.windows(2).all(|w| w[0] >= w[1]), "{lengths:?}");
    }
}

#[test]
fn cut_list_gives_big_wires_lug_allowance() {
    use novo_reports::Report;
    let (mut doc, h) = starter();
    doc.transact("Big wire", |tx| {
        let wire = tx
            .project()
            .wires_at(Endpoint::pin(h.k1_coil, 0))
            .next()
            .expect("coil wire");
        tx.update::<Wire>(wire, |w| w.cross_section_mm2 = Some(35.0))
    })
    .expect("edit");
    let cuts = CutList::default().generate(doc.project());
    let table = cuts
        .sections
        .iter()
        .find(|s| s.title == "35 mm²")
        .expect("35 mm² table");
    assert_eq!(table.text_rows()[0][6], "40");
}

#[test]
fn cut_list_is_empty_without_wires_and_exports_to_csv() {
    use novo_reports::Report;
    let doc = Document::default();
    assert_eq!(CutList::default().generate(doc.project()).row_count(), 0);
    let (doc, _) = starter();
    let csv = to_csv(&report(&doc, "cutlist"), &CsvOptions::default());
    assert!(csv.contains("Wire no.,From,To,Cable,Length basis,Wire mm,Ends mm,Cut mm"));
}

#[test]
fn labels_cover_wires_terminals_and_devices() {
    let (doc, _) = starter();
    let labels = report(&doc, "labels");
    let kinds: Vec<&str> = labels.sections.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(kinds, ["wires", "terminals", "devices", "cables"]);

    // Two labels per wire-list row: this end shows where the other goes.
    let wires = rows(&labels, "wires");
    let list = rows(&report(&doc, "wires"), "wires");
    assert_eq!(wires.len(), 2 * list.len());
    assert_eq!(wires[0], ["", "=A1+CAB-K1:1", "=A1+CAB-F1:2"]);
    assert_eq!(wires[1], ["", "=A1+CAB-F1:2", "=A1+CAB-K1:1"]);

    let terminals = rows(&labels, "terminals");
    assert_eq!(terminals.len(), 4);
    assert_eq!(terminals[0][..2], ["=A1+CAB-X1", "1"]);
    assert_eq!(terminals[3][..2], ["=A1+CAB-X1", "PE"]);

    let devices = rows(&labels, "devices");
    assert!(
        devices
            .iter()
            .any(|r| r[0] == "=A1+CAB-K1" && r[1] == "Motor contactor")
    );
    // Rows point back at their objects.
    assert!(
        labels.sections[2]
            .rows
            .iter()
            .all(|r| !r.sources.is_empty())
    );
}

#[test]
fn label_csv_layouts() {
    let (doc, _) = starter();
    let labels = report(&doc, "labels");
    let generic = labels_csv(&labels, &["terminals"], LabelLayout::Generic);
    assert!(generic.contains("Line 1,Line 2,Line 3"), "{generic}");
    assert!(generic.contains("'=A1+CAB-X1,PE,"), "{generic}");
    assert!(!generic.contains("Wire labels"), "only the asked kind");

    let brother = labels_csv(&labels, &["terminals"], LabelLayout::Brother);
    assert!(!brother.starts_with('\u{feff}'));
    assert!(!brother.contains("Line 1"), "no header row");
    assert!(brother.starts_with("=A1+CAB-X1\t1\t\r\n"), "{brother:?}");
    assert_eq!(brother.lines().count(), 4);
    assert_eq!(LabelLayout::parse("P-Touch"), Some(LabelLayout::Brother));
    assert_eq!(LabelLayout::parse("zebra"), None);
}
