//! The `Cores` sheet: names, sections, shield, spare marks and the wire on
//! each core of a cable, edited in a spreadsheet and imported back.

use novo_cables::{is_spare, spare_cores};
use novo_core::{Cable, CableCore, CableId, DeviceTag, Document, Record};
use novo_interop::sheets::{
    Cell, CsvOptions, TableKind, Workbook, export_project, plan_import, read_csv, read_xlsx,
    to_csv, xlsx_bytes,
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

/// Sets the cell of core `no` (1-based) of the cable with `cable` as Id.
fn set(book: &mut Workbook, cable: CableId, no: usize, header: &str, cell: Cell) {
    let table = book
        .tables
        .iter_mut()
        .find(|t| t.kind == TableKind::Cores)
        .expect("cores table");
    let (id_col, no_col, col) = (
        table.column("Id").unwrap(),
        table.column("Core No").unwrap(),
        table.column(header).expect("column"),
    );
    let row = table
        .rows
        .iter_mut()
        .find(|r| {
            r.cells[id_col] == Cell::Number(cable.raw() as f64)
                && r.cells[no_col] == Cell::Number(no as f64)
        })
        .expect("row");
    row.cells[col] = cell;
}

fn cable(doc: &Document, id: CableId) -> Cable {
    doc.project().cable(id).unwrap().clone()
}

#[test]
fn export_has_a_row_per_core_with_the_wire() {
    let (doc, h) = starter();
    let book = export_project(doc.project());
    let cores = book.table(TableKind::Cores).expect("cores sheet");
    assert_eq!(
        cores.headers,
        [
            "Id",
            "Cable",
            "Core No",
            "Core",
            "Cross-section [mm²]",
            "Shield",
            "Spare",
            "Wire Id",
            "Wire number"
        ]
    );
    assert_eq!(cores.rows.len(), 4);
    let col = |name| cores.column(name).unwrap();
    for (i, row) in cores.rows.iter().enumerate() {
        assert_eq!(row.cells[col("Id")], Cell::Number(h.w1.raw() as f64));
        assert_eq!(row.cells[col("Core No")], Cell::Number((i + 1) as f64));
        assert_eq!(
            row.cells[col("Wire Id")],
            Cell::Number(h.motor_wires[i].raw() as f64)
        );
        assert_eq!(row.cells[col("Spare")], Cell::text("no"));
    }
    assert_eq!(cores.rows[3].cells[col("Core")], Cell::text("GNYE"));
    // Only the greyed key and orientation columns are locked.
    let locked: Vec<&str> = cores
        .headers
        .iter()
        .zip(&cores.locked)
        .filter(|(_, l)| **l)
        .map(|(h, _)| h.as_str())
        .collect();
    assert_eq!(locked, ["Id", "Cable", "Core No", "Wire number"]);
}

#[test]
fn unchanged_cores_sheet_changes_nothing() {
    let (doc, _) = starter();
    let book = via_xlsx(&export_project(doc.project()));
    let plan = plan_import(doc.project(), &book);
    assert!(plan.changes.is_empty() && plan.rejected.is_empty());
}

#[test]
fn core_edits_apply_as_one_undo_step() {
    let (mut doc, h) = starter();
    let before = cable(&doc, h.w1);
    let mut book = via_xlsx(&export_project(doc.project()));
    set(&mut book, h.w1, 1, "Core", Cell::text("L1"));
    set(&mut book, h.w1, 2, "Cross-section [mm²]", Cell::Number(2.5));
    set(&mut book, h.w1, 3, "Spare", Cell::text("yes"));
    set(&mut book, h.w1, 4, "Shield", Cell::text("Yes"));
    set(&mut book, h.w1, 4, "Cross-section [mm²]", Cell::Empty);
    let plan = plan_import(doc.project(), &book);
    assert!(plan.rejected.is_empty(), "{}", plan.rejected_report());
    assert_eq!(plan.selected(), 5);
    assert_eq!(plan.apply(&mut doc).unwrap(), 1, "one cable changed");

    let after = cable(&doc, h.w1);
    assert_eq!(after.cores[0].name, "L1");
    assert_eq!(after.cores[1].cross_section_mm2, Some(2.5));
    assert!(is_spare(&after, 2));
    assert!(after.cores[3].shield);
    assert_eq!(after.cores[3].cross_section_mm2, None);
    // Nothing else moved.
    assert_eq!(after.cores[0].wire, before.cores[0].wire);
    assert_eq!(after.tag, before.tag);

    doc.undo().unwrap();
    assert_eq!(cable(&doc, h.w1), before, "one undo restores the cable");
}

#[test]
fn spare_mark_follows_a_renamed_core() {
    let (mut doc, h) = starter();
    doc.transact("spare", |tx| novo_cables::set_spare(tx, h.w1, 1, true))
        .unwrap();
    let mut book = via_xlsx(&export_project(doc.project()));
    set(&mut book, h.w1, 2, "Core", Cell::text("BU"));
    let plan = plan_import(doc.project(), &book);
    plan.apply(&mut doc).unwrap();
    let after = cable(&doc, h.w1);
    assert_eq!(after.cores[1].name, "BU");
    assert_eq!(spare_cores(&after), ["BU"]);
}

#[test]
fn wires_can_be_freed_and_swapped_between_cores() {
    let (mut doc, h) = starter();
    let [w0, w1, ..] = h.motor_wires;
    let mut book = via_xlsx(&export_project(doc.project()));
    // Swap cores 1 and 2, free core 3.
    set(&mut book, h.w1, 1, "Wire Id", Cell::Number(w1.raw() as f64));
    set(
        &mut book,
        h.w1,
        2,
        "Wire Id",
        Cell::text(format!("Wire#{}", w0.raw())),
    );
    set(&mut book, h.w1, 3, "Wire Id", Cell::Empty);
    let plan = plan_import(doc.project(), &book);
    assert!(plan.rejected.is_empty(), "{}", plan.rejected_report());
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    plan.apply(&mut doc).unwrap();
    let after = cable(&doc, h.w1);
    assert_eq!(after.cores[0].wire, Some(w1));
    assert_eq!(after.cores[1].wire, Some(w0));
    assert_eq!(after.cores[2].wire, None);
    assert_eq!(after.cores[3].wire, Some(h.motor_wires[3]));
    assert_eq!(doc.project().core_of_wire(h.motor_wires[2]), None);
}

#[test]
fn a_wire_on_another_cable_or_missing_is_refused() {
    let (mut doc, h) = starter();
    // A second cable with one free core.
    let (w2, _) = doc
        .transact("w2", |tx| {
            tx.add_cable(
                DeviceTag::parse("=A1-W2").unwrap(),
                vec![CableCore::new("1")],
            )
        })
        .unwrap();
    let mut book = via_xlsx(&export_project(doc.project()));
    set(
        &mut book,
        w2,
        1,
        "Wire Id",
        Cell::Number(h.motor_wires[0].raw() as f64),
    );
    let plan = plan_import(doc.project(), &book);
    assert!(plan.changes.is_empty());
    assert_eq!(plan.rejected.len(), 1);
    assert!(
        plan.rejected[0].reasons[0].contains("already on cable =A1-W1"),
        "{:?}",
        plan.rejected
    );

    let mut book = via_xlsx(&export_project(doc.project()));
    set(&mut book, h.w1, 1, "Wire Id", Cell::Number(999_999.0));
    let plan = plan_import(doc.project(), &book);
    assert!(plan.changes.is_empty());
    assert!(plan.rejected[0].reasons[0].contains("does not exist"));
}

#[test]
fn bad_cells_reject_the_whole_row_with_every_reason() {
    let (doc, h) = starter();
    let mut book = via_xlsx(&export_project(doc.project()));
    set(&mut book, h.w1, 2, "Core", Cell::Empty);
    set(&mut book, h.w1, 2, "Shield", Cell::text("maybe"));
    set(&mut book, h.w1, 2, "Cross-section [mm²]", Cell::text("-1"));
    // A good edit on another row still goes through.
    set(&mut book, h.w1, 1, "Core", Cell::text("L1"));
    let plan = plan_import(doc.project(), &book);
    assert_eq!(plan.selected(), 1);
    assert_eq!(plan.rejected.len(), 1);
    assert_eq!(plan.rejected[0].reasons.len(), 3);
    assert!(plan.rejected[0].item.contains("core 2"));
}

#[test]
fn no_cores_are_added_and_rows_must_name_a_core() {
    let (doc, h) = starter();
    let mut book = via_xlsx(&export_project(doc.project()));
    set(&mut book, h.w1, 4, "Core No", Cell::Number(5.0)); // pretend a 5th core
    let plan = plan_import(doc.project(), &book);
    assert_eq!(plan.rejected.len(), 1);
    assert!(plan.rejected[0].reasons[0].contains("does not add cores"));
    let mut book = via_xlsx(&export_project(doc.project()));
    set(&mut book, h.w1, 4, "Core No", Cell::Empty);
    let plan = plan_import(doc.project(), &book);
    assert!(plan.rejected[0].reasons[0].contains("Core No"));
}

#[test]
fn cores_reordered_after_the_export_are_refused_not_misapplied() {
    let (mut doc, h) = starter();
    let mut book = via_xlsx(&export_project(doc.project()));
    set(&mut book, h.w1, 1, "Core", Cell::text("L1")); // "BK" -> "L1"
    // Meanwhile someone puts BK last.
    let mut cable = cable(&doc, h.w1);
    cable.cores.rotate_left(1);
    doc.execute(novo_core::Command::Replace {
        record: Record::Cable(cable),
    })
    .unwrap();
    let plan = plan_import(doc.project(), &book);
    assert!(plan.changes.is_empty());
    assert_eq!(plan.rejected.len(), 1);
    assert!(plan.rejected[0].reasons[0].contains("reordered"));
}

#[test]
fn warns_when_two_cores_would_share_a_name_or_wire() {
    let (doc, h) = starter();
    let mut book = via_xlsx(&export_project(doc.project()));
    set(&mut book, h.w1, 2, "Core", Cell::text("BK"));
    set(
        &mut book,
        h.w1,
        3,
        "Wire Id",
        Cell::Number(h.motor_wires[0].raw() as f64),
    );
    set(
        &mut book,
        h.w1,
        4,
        "Wire Id",
        Cell::Number(h.motor_wires[0].raw() as f64),
    );
    let plan = plan_import(doc.project(), &book);
    assert!(plan.warnings.iter().any(|w| w.contains("called BK")));
    assert!(
        plan.warnings
            .iter()
            .any(|w| w.contains("only the last one"))
    );
}

#[test]
fn a_semicolon_csv_with_decimal_commas_round_trips() {
    let (mut doc, h) = starter();
    let options = CsvOptions {
        delimiter: ';',
        ..CsvOptions::default()
    };
    let book = export_project(doc.project());
    let text = to_csv(book.table(TableKind::Cores).unwrap(), &options);
    // Excel in a Dutch locale: 1,5 -> 2,5, and the CSV comes back as it is.
    let edited = text
        .lines()
        .enumerate()
        .map(|(i, l)| {
            if i == 1 {
                l.replacen("1,5", "2,5", 1)
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    let read = read_csv(&edited, None).unwrap();
    assert_eq!(
        read.tables[0].kind,
        TableKind::Cores,
        "found from its columns"
    );
    let plan = plan_import(doc.project(), &read);
    assert!(!plan.has_baseline);
    assert_eq!(plan.selected(), 1, "{:?}", plan.changes);
    plan.apply(&mut doc).unwrap();
    assert_eq!(cable(&doc, h.w1).cores[0].cross_section_mm2, Some(2.5));
}

#[test]
fn a_project_edit_after_the_export_is_a_conflict_not_an_overwrite() {
    let (mut doc, h) = starter();
    let mut book = via_xlsx(&export_project(doc.project()));
    set(&mut book, h.w1, 1, "Cross-section [mm²]", Cell::Number(4.0));
    doc.transact("later", |tx| {
        tx.update::<Cable>(h.w1, |c| c.cores[0].cross_section_mm2 = Some(6.0))
    })
    .unwrap();
    let plan = plan_import(doc.project(), &book);
    assert_eq!(plan.conflicts().count(), 1);
    assert_eq!(plan.selected(), 0);
}
