//! The assembly checklist against the Gate 1 motor starter fixture.

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, StructureId, Value};
use novo_reports::{
    ASSEMBLY_BOX, AssemblyChecklist, CsvOptions, LiveReports, Report, ReportOutput, Section,
    default_generated_pages, to_csv,
};
use novo_tests::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let handles = motor_starter(&mut doc).expect("fixture builds");
    (doc, handles)
}

/// The starter with a cabinet `+CAB`: a plate, two DIN rails of the same
/// length, a duct, F1 and K1 on the first rail, F2 free on the plate.
fn cabinet(doc: &mut Document, h: &MotorStarter) -> StructureId {
    doc.transact("Cabinet", |tx| {
        let cab = tx.add_structure(DeviceTag::parse("+CAB")?, "Cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 800.0,
                height: 600.0,
                margin: 10.0,
            }),
        )?;
        let r2 = tx.add_rail(cab, 50.0, 300.0, 400.0, RailKind::DinRail)?;
        let r1 = tx.add_rail(cab, 50.0, 100.0, 400.0, RailKind::DinRail)?;
        let duct = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
        tx.set_rail_size(duct, 700.0, 50.0, 60.0)?;
        tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, Some((r1, 50.0)))?;
        tx.place_device(
            h.k1,
            cab,
            Point::new(200.0, 100.0),
            false,
            Some((r1, 150.0)),
        )?;
        tx.place_device(h.f2, cab, Point::new(80.0, 400.0), true, None)?;
        let _ = r2;
        Ok(cab)
    })
    .expect("layout")
    .0
}

fn generate(doc: &Document) -> ReportOutput {
    AssemblyChecklist.generate(doc.project())
}

fn section<'a>(out: &'a ReportOutput, step: &str) -> &'a Section {
    out.sections
        .iter()
        .find(|s| s.key.ends_with(&format!(":{step}")))
        .unwrap_or_else(|| panic!("no {step} section"))
}

fn rows(out: &ReportOutput, step: &str) -> Vec<Vec<String>> {
    section(out, step).text_rows()
}

#[test]
fn a_cabinet_gets_six_steps_in_order() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let out = generate(&doc);
    let keys: Vec<&str> = out.sections.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "+CAB:prepare",
            "+CAB:drill",
            "+CAB:mount",
            "+CAB:wire",
            "+CAB:label",
            "+CAB:check"
        ]
    );
    let titles: Vec<&str> = out.sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles[0], "+CAB  1 Prepare");
    assert_eq!(titles[1], "+CAB  2 Drill");
    assert_eq!(titles[2], "+CAB  3 Mount");
    assert_eq!(titles[5], "+CAB  6 Check");
    // Every step ends in a Done column.
    for s in &out.sections {
        assert_eq!(s.columns.last().unwrap().key, "done", "{}", s.key);
    }
}

#[test]
fn prepare_lists_the_plate_and_cuts_equal_lengths_together() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let prepare = rows(&generate(&doc), "prepare");
    assert_eq!(prepare[0][1], "Mounting plate 800 x 600 mm, margin 10 mm");
    // Two 400 mm rails are one cut of two pieces; the duct is its own line.
    assert_eq!(
        prepare[1],
        ["1.2", "Cut DIN rail", "400", "2", "R1, R2", ASSEMBLY_BOX]
    );
    assert_eq!(
        prepare[2],
        [
            "1.3",
            "Cut Wiring duct 50 x 60",
            "700",
            "1",
            "D1",
            ASSEMBLY_BOX
        ]
    );
}

#[test]
fn mount_lists_rails_then_devices_in_reading_order() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let mount = rows(&generate(&doc), "mount");
    let items: Vec<&str> = mount.iter().map(|r| r[1].as_str()).collect();
    // Rails and ducts by position on the plate: R1 (y 100), the duct (y 250), R2 (y 300).
    assert_eq!(
        &items[..3],
        ["R1 DIN rail", "D1 Wiring duct 50 x 60", "R2 DIN rail"]
    );
    assert_eq!(&items[3..], ["=A1+CAB-F1", "=A1+CAB-K1", "=A1+CAB-F2"]);
    // F1 sits on R1 at its offset along the rail.
    assert_eq!(mount[3][3], "R1 at 50");
    assert_eq!((mount[3][4].as_str(), mount[3][5].as_str()), ("100", "100"));
    assert_eq!(mount[3][6], "");
    // F2 is free on the plate and turned.
    assert_eq!(mount[5][3], "plate");
    assert_eq!(mount[5][6], "90°");
    // Numbers stay numbers for a spreadsheet.
    let x = section(&generate(&doc), "mount").column("x_mm").unwrap();
    assert!(matches!(
        section(&generate(&doc), "mount").rows[3].cells[x],
        novo_reports::Cell::Number(_)
    ));
}

/// Machining data on the starter's parts: F1's part has a hole pattern and
/// F2 sits in the door behind a round cut-out.
fn machined(doc: &mut Document, h: &MotorStarter) {
    doc.transact("Machining", |tx| {
        let body = tx.add_part("ACME", "C1")?;
        tx.set_property(
            body,
            "hole_pattern",
            Some(Value::Text("5,5,4.2; 35,5,4.2".into())),
        )?;
        tx.assign_part(h.f1, body)?;
        let lamp = tx.add_part("ACME", "D1")?;
        tx.set_property(lamp, "mounting", Some(Value::Text("door_cutout".into())))?;
        tx.set_property(lamp, "cutout", Some(Value::Text("round 22.5".into())))?;
        tx.assign_part(h.f2, lamp)?;
        Ok(())
    })
    .expect("machining");
}

#[test]
fn drill_lists_the_tools_then_every_hole_and_cutout() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    machined(&mut doc, &h);
    let drill = rows(&generate(&doc), "drill");
    let items: Vec<&str> = drill.iter().map(|r| r[1].as_str()).collect();
    // Tool lines come first, one per surface and diameter, smallest first.
    assert_eq!(items[0], "Fit drill dia 4.2: 2 holes");
    assert!(items.iter().any(|i| i.starts_with("Fit drill dia 5.5")));
    // Then the holes with their position and what they are for.
    let hole = drill.iter().find(|r| r[1] == "Hole dia 4.2").unwrap();
    assert_eq!(hole[2], "Mounting plate");
    assert_eq!(hole[5], "=A1+CAB-F1");
    assert_eq!(hole.last().unwrap(), ASSEMBLY_BOX);
    // Rail holes come from the rails.
    assert!(drill.iter().any(|r| r[5] == "Rail 1"));
    // The cut-out is last and on the door.
    let cut = drill.last().unwrap();
    assert_eq!(
        (cut[1].as_str(), cut[2].as_str()),
        ("Cut-out dia 22.5", "Door")
    );
    assert_eq!(cut[5], "=A1+CAB-F2");
    // Numbered 2.1, 2.2, ... in order.
    for (i, r) in drill.iter().enumerate() {
        assert_eq!(r[0], format!("2.{}", i + 1));
    }
    // Positions stay numbers for a spreadsheet.
    let out = generate(&doc);
    let section = section(&out, "drill");
    let x = section.column("x_mm").unwrap();
    assert!(matches!(
        section.rows.last().unwrap().cells[x],
        novo_reports::Cell::Number(_)
    ));
}

#[test]
fn drill_says_so_when_nothing_is_machined_and_the_check_reads_the_patterns() {
    let (mut doc, h) = starter();
    doc.transact("Bad pattern", |tx| {
        let cab = tx.add_structure(DeviceTag::parse("+CAB")?, "Cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 400.0,
                height: 400.0,
                margin: 10.0,
            }),
        )?;
        Ok(cab)
    })
    .expect("empty cabinet");
    let out = generate(&doc);
    assert_eq!(rows(&out, "drill")[0][1], "No holes or cut-outs to make");
    let ok = rows(&out, "check")
        .into_iter()
        .find(|r| r[1] == "Every drill pattern could be read")
        .unwrap();
    assert_eq!(ok[2], "yes");

    // A pattern nobody can read is named, not dropped.
    let cab = cabinet_of(&doc);
    doc.transact("Place F1", |tx| {
        let part = tx.add_part("ACME", "C2")?;
        tx.set_property(part, "hole_pattern", Some(Value::Text("nope".into())))?;
        tx.assign_part(h.f1, part)?;
        tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, None)
    })
    .expect("bad part");
    let bad = rows(&generate(&doc), "check")
        .into_iter()
        .find(|r| r[1] == "Every drill pattern could be read")
        .unwrap();
    assert!(bad[2].starts_with("unreadable: "), "{bad:?}");
}

fn cabinet_of(doc: &Document) -> StructureId {
    doc.project()
        .structures()
        .find(|s| s.enclosure)
        .expect("a cabinet")
        .id
}

#[test]
fn wire_lists_the_connections_with_an_end_in_the_cabinet() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let out = generate(&doc);
    let wire = rows(&out, "wire");
    assert!(!wire.is_empty());
    for row in &wire {
        assert!(
            row[2].contains("+CAB") || row[3].contains("+CAB"),
            "{row:?}"
        );
        assert_eq!(row.last().unwrap(), ASSEMBLY_BOX);
    }
    // The same wires, with the same lengths, as the wire list says.
    let list = LiveReports::standard()
        .get(doc.project(), "wires")
        .unwrap()
        .clone();
    let list = list.section("wires").unwrap().text_rows();
    let in_cab = list
        .iter()
        .filter(|r| r[1].contains("+CAB") || r[2].contains("+CAB"))
        .count();
    assert_eq!(wire.len(), in_cab);
    let routed = wire
        .iter()
        .find(|r| r[2] == "=A1+CAB-F1:2" && r[3] == "=A1+CAB-K1:1")
        .expect("F1 to K1");
    assert_eq!(routed[7], "routed, no depth");
    assert!(routed[6].parse::<f64>().unwrap() > 0.0);
}

#[test]
fn label_has_one_line_per_placed_device() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let label = rows(&generate(&doc), "label");
    let texts: Vec<&str> = label.iter().map(|r| r[1].as_str()).collect();
    assert_eq!(texts, ["=A1+CAB-F1", "=A1+CAB-K1", "=A1+CAB-F2"]);
    assert!(label[0][2].ends_with("on R1"), "{:?}", label[0]);
}

#[test]
fn check_names_what_is_not_placed_and_leaves_lines_to_sign() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let check = rows(&generate(&doc), "check");
    let result = |what: &str| {
        check
            .iter()
            .find(|r| r[1] == what)
            .unwrap_or_else(|| panic!("no check {what}"))[2]
            .clone()
    };
    // Only three of the cabinet's devices are placed.
    assert!(
        result("Every device of the cabinet is on the plate").starts_with("not placed: "),
        "{check:?}"
    );
    assert_eq!(result("No device overlaps another"), "yes");
    assert!(result("Part sizes are known").starts_with("guessed size: "));
    // The persons' checks are blank lines with a box.
    let manual = check
        .iter()
        .find(|r| r[1] == "Every terminal is tightened")
        .unwrap();
    assert_eq!((manual[2].as_str(), manual[3].as_str()), ("", ASSEMBLY_BOX));
    // The last two rows are for the builder's name and the date.
    let n = check.len();
    assert_eq!(check[n - 2][1], "Built by (name)");
    assert_eq!(check[n - 1][1], "Date");
    assert_eq!(check[n - 1][3], "");
}

#[test]
fn an_overlap_is_reported_in_the_check() {
    let (mut doc, h) = starter();
    let cab = cabinet(&mut doc, &h);
    doc.transact("Overlap", |tx| {
        tx.place_device(h.k1, cab, Point::new(105.0, 100.0), false, None)
    })
    .expect("move");
    let check = rows(&generate(&doc), "check");
    let overlap = check.iter().find(|r| r[1] == "No device overlaps another");
    assert_eq!(overlap.unwrap()[2], "1 overlapping pairs");
}

#[test]
fn without_a_plate_there_is_one_note_and_no_cabinet() {
    let (doc, _) = starter();
    let out = generate(&doc);
    assert_eq!(out.sections.len(), 1);
    assert_eq!(out.sections[0].key, "none");
    assert!(out.sections[0].text_rows()[0][0].contains("mounting plate"));
}

#[test]
fn two_cabinets_each_get_their_own_steps() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    doc.transact("Second cabinet", |tx| {
        let cab = tx.add_structure(DeviceTag::parse("+CAB2")?, "Cabinet 2", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 400.0,
                height: 400.0,
                margin: 5.0,
            }),
        )?;
        Ok(())
    })
    .expect("second");
    let out = generate(&doc);
    assert_eq!(out.sections.len(), 12);
    assert_eq!(out.sections[6].key, "+CAB2:prepare");
    // An empty cabinet still has its plate and its checks.
    let second = out.section("+CAB2:prepare").expect("second cabinet");
    assert_eq!(second.rows.len(), 1);
    assert!(out.section("+CAB2:check").unwrap().rows.len() > 2);
}

#[test]
fn it_is_deterministic_a_report_and_not_a_default_page() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    assert_eq!(generate(&doc), generate(&doc));
    assert!(!AssemblyChecklist.proposed_by_default());
    assert!(
        default_generated_pages(doc.project())
            .iter()
            .all(|p| p.generator != AssemblyChecklist::ID)
    );
}

#[test]
fn it_stays_live_when_the_layout_changes() {
    let (mut doc, h) = starter();
    let cab = cabinet(&mut doc, &h);
    let mut reports = LiveReports::standard();
    reports.register(Box::new(AssemblyChecklist));
    let before = reports
        .get(doc.project(), AssemblyChecklist::ID)
        .unwrap()
        .clone();
    let (_, changes) = doc
        .transact("Place X1", |tx| {
            tx.place_device(h.x1, cab, Point::new(500.0, 100.0), false, None)
        })
        .expect("place");
    reports.notify(doc.project(), &changes);
    let after = reports.get(doc.project(), AssemblyChecklist::ID).unwrap();
    assert_ne!(&before, after);
    assert_eq!(
        after
            .sections
            .iter()
            .find(|s| s.key == "+CAB:label")
            .unwrap()
            .rows
            .len(),
        4
    );
}

#[test]
fn it_exports_to_csv_with_the_boxes() {
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    let csv = to_csv(&generate(&doc), &CsvOptions::default());
    assert!(csv.contains("[ ]"));
    assert!(csv.contains("Mounting plate 800 x 600 mm"));
}

#[test]
fn the_checklist_reads_in_dutch_with_a_decimal_comma() {
    use novo_reports::ASSEMBLY_BOX;
    use novo_reports::i18n::{ProjectTable, Translator, translate_output};
    let (mut doc, h) = starter();
    cabinet(&mut doc, &h);
    machined(&mut doc, &h);
    let dutch = translate_output(
        &generate(&doc),
        &Translator::new("nl", &ProjectTable::default()),
    );
    let all: Vec<String> = dutch
        .sections
        .iter()
        .flat_map(|s| s.text_rows())
        .flatten()
        .collect();
    for english in [
        "Mounting plate ",
        "Cut ",
        "Hole dia",
        "Fit drill",
        "Cut-out",
        "DIN rail",
        "Wiring duct",
        "Built by",
        "Every ",
        "horizontal",
        "vertical",
    ] {
        assert!(
            !all.iter().any(|c| c.contains(english)),
            "{english:?} is still English in {all:?}"
        );
    }
    assert!(all.iter().any(|c| c == "Boor dia 4,2: 2 gaten"), "{all:?}");
    assert!(all.iter().any(|c| c == "Gat dia 4,2"));
    assert!(all.iter().any(|c| c == "Uitsparing dia 22,5"));
    assert!(all.iter().any(|c| c.starts_with("Montageplaat ")));
    // Project data and the done box are untouched.
    assert!(all.iter().any(|c| c == ASSEMBLY_BOX));
    assert!(all.iter().any(|c| c == "=A1+CAB-F1"));
}
