//! The spreadsheet round trip against the Gate 1 motor starter fixture.

use novo_core::{Command, Device, DeviceTag, Document, Value};
use novo_interop::sheets::{
    ApplyError, Cell, CsvOptions, Resolution, TableKind, Workbook, export_project, plan_import,
    read_csv, read_xlsx, to_csv, xlsx_bytes,
};
use novo_tests::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let handles = motor_starter(&mut doc).expect("fixture builds");
    (doc, handles)
}

/// Through a real `.xlsx` file, as Excel would hand it back.
fn via_xlsx(book: &Workbook) -> Workbook {
    read_xlsx(&xlsx_bytes(book).expect("writes")).expect("reads")
}

fn set(book: &mut Workbook, kind: TableKind, id: u64, header: &str, cell: Cell) {
    let table = book
        .tables
        .iter_mut()
        .find(|t| t.kind == kind)
        .expect("table");
    let id_col = table.column("Id").expect("Id column");
    let col = match table.column(header) {
        Some(c) => c,
        None => {
            table.headers.push(header.to_owned());
            table.headers.len() - 1
        }
    };
    let row = table
        .rows
        .iter_mut()
        .find(|r| r.cells[id_col] == Cell::Number(id as f64))
        .expect("row");
    if row.cells.len() <= col {
        row.cells.resize(col + 1, Cell::Empty);
    }
    row.cells[col] = cell;
}

fn tag(doc: &Document, id: novo_core::DeviceId) -> String {
    doc.project().device(id).unwrap().tag.to_string()
}

#[test]
fn export_has_every_table_with_ids() {
    let (doc, h) = starter();
    let book = export_project(doc.project());
    let kinds: Vec<_> = book.tables.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        [
            TableKind::Devices,
            TableKind::Terminals,
            TableKind::Wires,
            TableKind::Cables
        ]
    );
    let devices = book.table(TableKind::Devices).unwrap();
    assert_eq!(devices.headers[0], "Id");
    assert_eq!(devices.rows.len(), doc.project().devices().count());
    let tag_col = devices.column("Tag").unwrap();
    let k1 = devices
        .rows
        .iter()
        .find(|r| r.cells[0] == Cell::Number(h.k1.raw() as f64))
        .unwrap();
    assert_eq!(k1.cells[tag_col], Cell::text("=A1+CAB-K1"));
    // The motor's power is a property column.
    assert!(devices.column("properties.power_kw").is_some());
    assert_eq!(book.meta.revision, Some(doc.project().revision()));
}

#[test]
fn unchanged_export_imports_as_nothing() {
    let (doc, _) = starter();
    let book = via_xlsx(&export_project(doc.project()));
    let plan = plan_import(doc.project(), &book);
    assert!(plan.changes.is_empty(), "{:?}", plan.changes);
    assert!(plan.rejected.is_empty(), "{}", plan.rejected_report());
    assert_eq!(plan.rows_read, plan.rows_unchanged);
    assert!(plan.has_baseline);
    assert!(plan.command(doc.project()).unwrap().is_none());
}

#[test]
fn xlsx_is_deterministic_and_keeps_every_cell() {
    let (doc, _) = starter();
    let book = export_project(doc.project());
    assert_eq!(xlsx_bytes(&book).unwrap(), xlsx_bytes(&book).unwrap());
    let back = via_xlsx(&book);
    for (a, b) in book.tables.iter().zip(&back.tables) {
        assert_eq!(a.headers, b.headers);
        let rows = |t: &novo_interop::sheets::Table| -> Vec<Vec<String>> {
            t.rows
                .iter()
                .map(|r| {
                    (0..t.headers.len())
                        .map(|c| r.cells.get(c).map(Cell::as_text).unwrap_or_default())
                        .collect()
                })
                .collect()
        };
        assert_eq!(rows(a), rows(b), "{}", a.kind.name());
    }
    assert_eq!(back.baseline.len(), book.baseline.len());
    assert_eq!(back.meta, book.meta);
}

#[test]
fn bulk_edit_is_one_undo_step_and_a_rename_follows_the_id() {
    let (mut doc, h) = starter();
    let mut book = export_project(doc.project());
    set(
        &mut book,
        TableKind::Devices,
        h.k1.raw(),
        "Tag",
        Cell::text("=A1+CAB-K11"),
    );
    set(
        &mut book,
        TableKind::Devices,
        h.k1.raw(),
        "Description",
        Cell::text("Pump contactor"),
    );
    set(
        &mut book,
        TableKind::Devices,
        h.f1.raw(),
        "Description",
        Cell::Empty,
    );
    set(
        &mut book,
        TableKind::Devices,
        h.m1.raw(),
        "properties.power_kw",
        Cell::Number(3.0),
    );
    set(
        &mut book,
        TableKind::Devices,
        h.m1.raw(),
        "properties.note",
        Cell::text("rev B"),
    );
    let book = via_xlsx(&book);

    let plan = plan_import(doc.project(), &book);
    assert_eq!(plan.selected(), 5, "{:#?}", plan.changes);
    assert!(plan.rejected.is_empty());
    let depth = doc.undo_depth();
    let objects = plan.apply(&mut doc).unwrap();
    assert_eq!(objects, 3);
    assert_eq!(doc.undo_depth(), depth + 1);

    let k1 = doc.project().device(h.k1).unwrap();
    assert_eq!(k1.tag.to_string(), "=A1+CAB-K11");
    assert_eq!(k1.description, "Pump contactor");
    assert_eq!(doc.project().device(h.f1).unwrap().description, "");
    let m1 = doc.project().device(h.m1).unwrap();
    assert_eq!(m1.properties["power_kw"], Value::Float(3.0));
    assert_eq!(m1.properties["note"], Value::Text("rev B".into()));

    doc.undo().unwrap();
    assert_eq!(tag(&doc, h.k1), "=A1+CAB-K1");
    assert_eq!(
        doc.project().device(h.m1).unwrap().properties.get("note"),
        None
    );
}

#[test]
fn a_conflict_waits_for_the_user() {
    let (mut doc, h) = starter();
    let mut book = export_project(doc.project());
    // The sheet edits k1 and f1; meanwhile the project edits k1 differently.
    set(
        &mut book,
        TableKind::Devices,
        h.k1.raw(),
        "Description",
        Cell::text("From sheet"),
    );
    set(
        &mut book,
        TableKind::Devices,
        h.f1.raw(),
        "Description",
        Cell::text("Fuses (sheet)"),
    );
    doc.execute(Command::Replace {
        record: novo_core::Record::Device({
            let mut d: Device = doc.project().device(h.k1).unwrap().clone();
            d.description = "From project".into();
            d
        }),
    })
    .unwrap();
    // And the project edits s0, which the sheet never touched.
    let s0 = {
        let mut d: Device = doc.project().device(h.s0).unwrap().clone();
        d.description = "Stop (project)".into();
        d
    };
    doc.execute(Command::Replace {
        record: novo_core::Record::Device(s0),
    })
    .unwrap();

    let book = via_xlsx(&book);
    let mut plan = plan_import(doc.project(), &book);
    // s0's later edit is neither a change nor a conflict.
    assert!(plan.changes.iter().all(|c| c.id != h.s0.into()));
    assert_eq!(plan.changes.len(), 2);
    let conflict = plan.conflicts().next().expect("one conflict");
    assert_eq!(conflict.after, "From sheet");
    assert_eq!(conflict.before, "From project");
    assert_eq!(conflict.conflict.as_deref(), Some("Motor contactor"));
    assert_eq!(plan.selected(), 1, "the conflict starts unselected");
    assert!(plan.summary().contains("1 conflict"), "{}", plan.summary());

    plan.resolve(Resolution::Sheet);
    assert_eq!(plan.selected(), 2);
    plan.apply(&mut doc).unwrap();
    assert_eq!(
        doc.project().device(h.k1).unwrap().description,
        "From sheet"
    );
    assert_eq!(
        doc.project().device(h.s0).unwrap().description,
        "Stop (project)"
    );
}

#[test]
fn bad_rows_are_rejected_with_reasons_and_the_rest_still_applies() {
    let (mut doc, h) = starter();
    let mut book = export_project(doc.project());
    set(
        &mut book,
        TableKind::Devices,
        h.k1.raw(),
        "Tag",
        Cell::text("not a tag"),
    );
    set(
        &mut book,
        TableKind::Devices,
        h.k1.raw(),
        "Description",
        Cell::text("never applied"),
    );
    set(
        &mut book,
        TableKind::Devices,
        h.f1.raw(),
        "Description",
        Cell::text("fine"),
    );
    // A row for something that was deleted, one with a formula error in the tag,
    // and a repeated row.
    let devices = book
        .tables
        .iter_mut()
        .find(|t| t.kind == TableKind::Devices)
        .unwrap();
    let width = devices.headers.len();
    let mut ghost = vec![Cell::Empty; width];
    ghost[0] = Cell::Number(999_999.0);
    devices.push(ghost);
    let again = devices
        .rows
        .iter()
        .find(|r| r.cells[0] == Cell::Number(h.f1.raw() as f64))
        .unwrap()
        .cells
        .clone();
    devices.push(again);
    set(
        &mut book,
        TableKind::Devices,
        h.s0.raw(),
        "Tag",
        Cell::text("#NAME?"),
    );

    let book = via_xlsx(&book);
    let plan = plan_import(doc.project(), &book);
    let report = plan.rejected_report();
    assert_eq!(plan.rejected.len(), 4, "{report}");
    assert!(report.contains("not a valid tag"), "{report}");
    assert!(report.contains("no longer exists"), "{report}");
    assert!(report.contains("already on row"), "{report}");
    assert!(report.contains("spreadsheet error"), "{report}");
    // The CSV of the rejects has a header and one line per reject.
    assert_eq!(plan.rejected_table().len(), 5);

    plan.apply(&mut doc).unwrap();
    assert_eq!(doc.project().device(h.f1).unwrap().description, "fine");
    assert_eq!(
        doc.project().device(h.k1).unwrap().description,
        "Motor contactor"
    );
    assert_eq!(tag(&doc, h.k1), "=A1+CAB-K1");
}

#[test]
fn a_plan_for_an_older_project_refuses_to_apply() {
    let (mut doc, h) = starter();
    let mut book = export_project(doc.project());
    set(
        &mut book,
        TableKind::Devices,
        h.f1.raw(),
        "Description",
        Cell::text("x"),
    );
    let plan = plan_import(doc.project(), &book);
    doc.transact("Edit", |tx| {
        tx.set_tag(h.s0, DeviceTag::parse("=A1+CAB-S9").unwrap())
    })
    .unwrap();
    assert!(matches!(plan.apply(&mut doc), Err(ApplyError::Outdated)));
}

#[test]
fn terminals_wires_and_cables_round_trip() {
    let (mut doc, h) = starter();
    let mut book = export_project(doc.project());
    let terminal = {
        let t = book.table(TableKind::Terminals).unwrap();
        let id = t.column("Id").unwrap();
        let name = t.column("Name").unwrap();
        let row = t
            .rows
            .iter()
            .find(|r| r.cells[name] == Cell::text("2"))
            .unwrap();
        match row.cells[id] {
            Cell::Number(n) => n as u64,
            _ => panic!(),
        }
    };
    set(
        &mut book,
        TableKind::Terminals,
        terminal,
        "Name",
        Cell::text("L2"),
    );
    set(
        &mut book,
        TableKind::Terminals,
        terminal,
        "Jumper",
        Cell::Number(7.0),
    );
    let wire = {
        let t = book.table(TableKind::Wires).unwrap();
        match t.rows[0].cells[0] {
            Cell::Number(n) => n as u64,
            _ => panic!(),
        }
    };
    // Excel makes 101 a number; 2,5 typed with a decimal comma stays text.
    set(
        &mut book,
        TableKind::Wires,
        wire,
        "Number",
        Cell::Number(101.0),
    );
    set(&mut book, TableKind::Wires, wire, "Color", Cell::text("BK"));
    set(
        &mut book,
        TableKind::Wires,
        wire,
        "Cross-section [mm²]",
        Cell::text("2,5"),
    );
    let cable = {
        let t = book.table(TableKind::Cables).unwrap();
        match t.rows[0].cells[0] {
            Cell::Number(n) => n as u64,
            _ => panic!(),
        }
    };
    set(
        &mut book,
        TableKind::Cables,
        cable,
        "Length [m]",
        Cell::Number(12.5),
    );
    let book = via_xlsx(&book);
    let plan = plan_import(doc.project(), &book);
    assert!(plan.rejected.is_empty(), "{}", plan.rejected_report());
    assert_eq!(
        plan.selected(),
        6,
        "{:#?}",
        plan.changes
            .iter()
            .map(|c| (&c.header, &c.after))
            .collect::<Vec<_>>()
    );
    plan.apply(&mut doc).unwrap();
    let p = doc.project();
    let t = p.terminal(novo_core::TerminalId(terminal)).unwrap();
    assert_eq!((t.name.as_str(), t.jumper), ("L2", Some(7)));
    let w = p.wire(novo_core::WireId(wire)).unwrap();
    assert_eq!(w.number.as_deref(), Some("101"));
    assert_eq!(w.color.as_deref(), Some("BK"));
    assert_eq!(w.cross_section_mm2, Some(2.5));
    assert_eq!(
        p.cable(novo_core::CableId(cable)).unwrap().length_m,
        Some(12.5)
    );
    let _ = h;
}

#[test]
fn csv_round_trips_with_a_decimal_comma_and_no_baseline() {
    let (mut doc, h) = starter();
    let book = export_project(doc.project());
    let options = CsvOptions {
        delimiter: ';',
        ..CsvOptions::default()
    };
    let wires = book.table(TableKind::Wires).unwrap();
    let text = to_csv(wires, &options);
    // Tags and numbers survive Excel: apostrophes only on text that starts like a formula.
    let devices_csv = to_csv(book.table(TableKind::Devices).unwrap(), &options);
    assert!(devices_csv.contains("'=A1+CAB-K1"), "{devices_csv}");

    // A wire edited in the CSV.
    let wire_id = match wires.rows[0].cells[0] {
        Cell::Number(n) => n as u64,
        _ => panic!(),
    };
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let header: Vec<&str> = lines[0].trim_start_matches('\u{feff}').split(';').collect();
    let cross = header
        .iter()
        .position(|h| *h == "Cross-section [mm²]")
        .unwrap();
    let mut row: Vec<String> = lines[1].split(';').map(str::to_owned).collect();
    row[cross] = "1,5".into();
    lines[1] = row.join(";");
    let edited = lines.join("\r\n");
    let read = read_csv(&edited, None).unwrap();
    assert_eq!(read.tables[0].kind, TableKind::Wires);
    let plan = plan_import(doc.project(), &read);
    assert!(!plan.has_baseline);
    assert!(plan.notes.iter().any(|n| n.contains("no baseline")));
    assert_eq!(plan.selected(), 1, "{:#?}", plan.changes);
    plan.apply(&mut doc).unwrap();
    assert_eq!(
        doc.project()
            .wire(novo_core::WireId(wire_id))
            .unwrap()
            .cross_section_mm2,
        Some(1.5)
    );

    // Devices from CSV: matched by Id, tag apostrophe removed on the way in.
    let dev = read_csv(&devices_csv, Some(TableKind::Devices)).unwrap();
    let plan = plan_import(doc.project(), &dev);
    assert!(plan.changes.is_empty(), "{:#?}", plan.changes);
    let _ = h;
}

#[test]
fn a_row_without_an_id_matches_by_tag() {
    let (mut doc, h) = starter();
    let csv = "Tag,Description\r\n'=A1+CAB-K1,Renamed by tag\r\n'=A1+CAB-ZZ9,Unknown\r\n";
    let book = read_csv(csv, None).unwrap();
    let plan = plan_import(doc.project(), &book);
    assert_eq!(plan.selected(), 1);
    assert_eq!(plan.rejected.len(), 1);
    assert!(plan.rejected_report().contains("nothing has the tag"));
    plan.apply(&mut doc).unwrap();
    assert_eq!(
        doc.project().device(h.k1).unwrap().description,
        "Renamed by tag"
    );
}

#[test]
fn unknown_columns_and_sheets_are_noted_not_fatal() {
    let (doc, h) = starter();
    let mut book = export_project(doc.project());
    set(
        &mut book,
        TableKind::Devices,
        h.k1.raw(),
        "My own column",
        Cell::text("hello"),
    );
    let plan = plan_import(doc.project(), &via_xlsx(&book));
    assert!(plan.changes.is_empty());
    assert!(
        plan.notes.iter().any(|n| n.contains("My own column")),
        "{:?}",
        plan.notes
    );
}

#[test]
fn two_devices_ending_up_with_one_tag_warns() {
    let (doc, h) = starter();
    let mut book = export_project(doc.project());
    set(
        &mut book,
        TableKind::Devices,
        h.k1.raw(),
        "Tag",
        Cell::text("=A1+CAB-F1"),
    );
    let plan = plan_import(doc.project(), &via_xlsx(&book));
    assert_eq!(plan.selected(), 1);
    assert!(
        plan.warnings.iter().any(|w| w.contains("=A1+CAB-F1")),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn not_a_koper_workbook_is_an_error() {
    let mut book = XlsxBook::new();
    book.add_worksheet().set_name("Sheet1").unwrap();
    let bytes = book.save_to_buffer().unwrap();
    assert!(matches!(
        read_xlsx(&bytes),
        Err(novo_interop::sheets::SheetError::NoTables)
    ));
    assert!(read_xlsx(b"not a zip").is_err());
}

use rust_xlsxwriter::Workbook as XlsxBook;

#[test]
fn ids_may_carry_their_kind_and_terminals_may_say_number() {
    let (doc, h) = starter();
    let csv = format!("Id,Description\r\nDevice#{},Via typed id\r\n", h.k1.raw());
    let plan = plan_import(
        doc.project(),
        &read_csv(&csv, Some(TableKind::Devices)).unwrap(),
    );
    assert_eq!(plan.selected(), 1, "{}", plan.rejected_report());
    let book = export_project(doc.project());
    let t = book.table(TableKind::Terminals).unwrap();
    let id = t.rows[0].cells[0].as_text();
    let csv = format!("Id,Number\r\nTerminal#{id},77\r\n");
    let plan = plan_import(
        doc.project(),
        &read_csv(&csv, Some(TableKind::Terminals)).unwrap(),
    );
    assert_eq!(plan.selected(), 1, "{}", plan.rejected_report());
}
