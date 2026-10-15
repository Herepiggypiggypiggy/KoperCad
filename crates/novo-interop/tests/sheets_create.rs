//! New-device rows and the Order number column of the spreadsheet import.

use novo_core::{Command, DeviceId, DeviceKind, DeviceTag, Document, Value};
use novo_interop::sheets::{
    Cell, TableKind, Workbook, export_project, plan_import, plan_import_with, read_xlsx, xlsx_bytes,
};
use novo_parts::{Catalog, CatalogPart};

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn project() -> (Document, DeviceId) {
    let mut doc = Document::default();
    let mut first = None;
    for (t, d) in [("-K1", "Main contactor"), ("-Q1", "Motor breaker")] {
        let out = doc
            .execute(Command::AddDevice {
                tag: tag(t),
                description: d.into(),
                kind: Default::default(),
            })
            .unwrap();
        first.get_or_insert(DeviceId::from(out.created[0].raw()));
    }
    (doc, first.unwrap())
}

fn via_xlsx(book: &Workbook) -> Workbook {
    read_xlsx(&xlsx_bytes(book).unwrap()).unwrap()
}

/// Adds a Devices row; columns missing from the table are appended.
fn add_row(book: &mut Workbook, cells: &[(&str, &str)]) {
    let table = book
        .tables
        .iter_mut()
        .find(|t| t.kind == TableKind::Devices)
        .unwrap();
    for (header, _) in cells {
        if table.column(header).is_none() {
            table.headers.push((*header).to_owned());
        }
    }
    let mut row = vec![Cell::Empty; table.headers.len()];
    for (header, value) in cells {
        row[table.column(header).unwrap()] = Cell::text(*value);
    }
    table.push(row);
}

fn set_cell(book: &mut Workbook, id: DeviceId, header: &str, value: &str) {
    let table = book
        .tables
        .iter_mut()
        .find(|t| t.kind == TableKind::Devices)
        .unwrap();
    if table.column(header).is_none() {
        table.headers.push(header.to_owned());
    }
    let col = table.column(header).unwrap();
    let id_col = table.column("Id").unwrap();
    let row = table
        .rows
        .iter_mut()
        .find(|r| r.cells[id_col] == Cell::Number(id.raw() as f64))
        .unwrap();
    row.cells.resize(table.headers.len(), Cell::Empty);
    row.cells[col] = Cell::text(value);
}

#[test]
fn rows_without_an_id_add_devices_in_one_undo_step() {
    let (mut doc, _) = project();
    let mut book = export_project(doc.project());
    add_row(
        &mut book,
        &[
            ("Tag", "-M1"),
            ("Description", "Pump motor"),
            ("Order number", "1LA7096"),
            ("Manufacturer", "Siemens"),
            ("properties.power_kw", "2,2"),
        ],
    );
    add_row(&mut book, &[("Tag", "-X5"), ("Kind", "terminal strip")]);
    let plan = plan_import(doc.project(), &via_xlsx(&book));
    assert_eq!(plan.new_devices.len(), 2);
    assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
    assert_eq!(plan.new_devices[0].order_number(), Some("1LA7096"));
    assert!(plan.summary().contains("2 new devices"));

    let depth = doc.undo_depth();
    assert_eq!(plan.apply(&mut doc).unwrap(), 2);
    assert_eq!(doc.undo_depth(), depth + 1);
    let m1 = doc
        .project()
        .devices()
        .find(|d| d.tag == tag("-M1"))
        .unwrap();
    assert_eq!(m1.description, "Pump motor");
    assert_eq!(m1.properties.get("power_kw"), Some(&Value::Float(2.2)));
    let part = doc.project().part(m1.parts[0]).unwrap();
    assert_eq!(
        (part.manufacturer.as_str(), part.order_number.as_str()),
        ("Siemens", "1LA7096")
    );
    let x5 = doc
        .project()
        .devices()
        .find(|d| d.tag == tag("-X5"))
        .unwrap();
    assert_eq!(x5.kind, DeviceKind::TerminalStrip);
    doc.undo().unwrap();
    assert!(doc.project().devices().all(|d| d.tag != tag("-M1")));
    assert_eq!(doc.project().parts().count(), 0);
}

#[test]
fn new_rows_can_be_left_out_and_bad_ones_are_rejected() {
    let (doc, _) = project();
    let mut book = export_project(doc.project());
    add_row(&mut book, &[("Tag", "-M1")]);
    add_row(&mut book, &[("Tag", "-M1"), ("Description", "again")]);
    add_row(
        &mut book,
        &[("Tag", "=A1"), ("Description", "no device part")],
    );
    add_row(&mut book, &[("Tag", "-Z1"), ("Kind", "toaster")]);
    let mut plan = plan_import(doc.project(), &via_xlsx(&book));
    assert_eq!(plan.new_devices.len(), 1);
    assert_eq!(plan.rejected.len(), 3);
    assert!(plan.rejected[0].reasons[0].contains("already added"));
    assert!(plan.rejected[2].reasons[0].contains("not a device kind"));
    plan.select_new(0, false);
    assert!(plan.is_empty());
}

#[test]
fn an_existing_tag_without_id_is_an_edit_not_a_new_device() {
    let (mut doc, k1) = project();
    let mut book = export_project(doc.project());
    add_row(
        &mut book,
        &[("Tag", "-K1"), ("Description", "Pump contactor")],
    );
    let plan = plan_import(doc.project(), &via_xlsx(&book));
    assert!(plan.new_devices.is_empty());
    // The exported -K1 row and the added one are both -K1: the first counts.
    assert_eq!(plan.rejected.len(), 1);
    assert!(plan.rejected[0].reasons[0].contains("first row counts"));
    let _ = (&mut doc, k1);
}

#[test]
fn order_number_sets_the_part_from_the_catalog() {
    let (mut doc, k1) = project();
    let mut catalog = Catalog::new();
    let mut part = CatalogPart::new("Siemens", "3RT2016-1BB41");
    part.type_number = "SIRIUS 3RT2".into();
    catalog.upsert(part);
    let mut book = export_project(doc.project());
    set_cell(&mut book, k1, "Order number", "3rt20161bb41");
    let plan = plan_import_with(doc.project(), &via_xlsx(&book), &catalog);
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].header, "Order number");
    plan.apply(&mut doc).unwrap();
    let part = doc
        .project()
        .part(doc.project().device(k1).unwrap().parts[0])
        .unwrap();
    assert_eq!(part.order_number, "3RT2016-1BB41");
    assert_eq!(part.manufacturer, "Siemens");

    // The same order number typed differently changes nothing.
    let mut book = export_project(doc.project());
    set_cell(&mut book, k1, "Order number", "3RT2016 1BB41");
    let plan = plan_import_with(doc.project(), &via_xlsx(&book), &catalog);
    assert!(plan.is_empty(), "{:?}", plan.changes);

    // Clearing the cell removes the part.
    let mut book = export_project(doc.project());
    set_cell(&mut book, k1, "Order number", "");
    let plan = plan_import(doc.project(), &via_xlsx(&book));
    plan.apply(&mut doc).unwrap();
    assert!(doc.project().device(k1).unwrap().parts.is_empty());
}
