//! Files as people really hand them back: Excel numbers in text columns,
//! stray spaces, reordered and extra columns, blank and repeated rows,
//! a Dutch-locale CSV. Plus jumper bridges on the terminal sheet.

use novo_core::{Document, Terminal};
use novo_interop::sheets::{
    Cell, TableKind, Workbook, export_project, plan_import, read_csv, read_xlsx, xlsx_bytes,
};
use novo_tests::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let handles = motor_starter(&mut doc).expect("fixture builds");
    (doc, handles)
}

fn terminal(doc: &Document, name: &str) -> Terminal {
    doc.project()
        .terminals()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("terminal {name}"))
        .clone()
}

fn set_jumper(book: &mut Workbook, id: u64, cell: Cell) {
    let t = book
        .tables
        .iter_mut()
        .find(|t| t.kind == TableKind::Terminals)
        .unwrap();
    let (idc, jc) = (t.column("Id").unwrap(), t.column("Jumper").unwrap());
    let row = t
        .rows
        .iter_mut()
        .find(|r| r.cells[idc] == Cell::Number(id as f64))
        .unwrap();
    row.cells[jc] = cell;
}

#[test]
fn bridged_with_lists_the_other_terminals_of_the_group() {
    let (mut doc, _) = starter();
    let (a, b, c) = (
        terminal(&doc, "1"),
        terminal(&doc, "2"),
        terminal(&doc, "3"),
    );
    doc.transact("bridge", |tx| {
        for t in [&a, &b] {
            tx.update::<Terminal>(t.id, |t| t.jumper = Some(7))?;
        }
        Ok(())
    })
    .unwrap();
    let book = export_project(doc.project());
    let terminals = book.table(TableKind::Terminals).unwrap();
    let (idc, bc) = (
        terminals.column("Id").unwrap(),
        terminals.column("Bridged with").unwrap(),
    );
    let bridged = |id: u64| {
        terminals
            .rows
            .iter()
            .find(|r| r.cells[idc] == Cell::Number(id as f64))
            .map(|r| r.cells[bc].as_text())
            .unwrap()
    };
    assert_eq!(bridged(a.id.raw()), "2");
    assert_eq!(bridged(b.id.raw()), "1");
    assert_eq!(bridged(c.id.raw()), "");
    // Orientation only: changing it changes nothing.
    let i = terminals
        .headers
        .iter()
        .position(|h| h == "Bridged with")
        .unwrap();
    assert!(terminals.locked[i]);
}

#[test]
fn a_jumper_group_of_one_terminal_warns() {
    let (doc, _) = starter();
    let (a, b) = (terminal(&doc, "1"), terminal(&doc, "2"));
    let mut book = export_project(doc.project());
    set_jumper(&mut book, a.id.raw(), Cell::Number(3.0));
    let plan = plan_import(doc.project(), &book);
    assert_eq!(plan.selected(), 1);
    assert_eq!(plan.warnings.len(), 1, "{:?}", plan.warnings);
    assert!(plan.warnings[0].contains("group 3") && plan.warnings[0].contains("-X1"));

    set_jumper(&mut book, b.id.raw(), Cell::text("3"));
    let plan = plan_import(doc.project(), &book);
    assert_eq!(plan.selected(), 2);
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
}

#[test]
fn excel_turns_names_into_numbers_and_still_matches() {
    // Terminal names `1`, `2`, `3` come back from Excel as numbers.
    let (doc, _) = starter();
    let mut book = export_project(doc.project());
    let t = book
        .tables
        .iter_mut()
        .find(|t| t.kind == TableKind::Terminals)
        .unwrap();
    let nc = t.column("Name").unwrap();
    for row in &mut t.rows {
        if let Ok(n) = row.cells[nc].as_text().parse::<f64>() {
            row.cells[nc] = Cell::Number(n);
        }
    }
    let book = read_xlsx(&xlsx_bytes(&book).unwrap()).unwrap();
    let plan = plan_import(doc.project(), &book);
    assert!(plan.changes.is_empty() && plan.rejected.is_empty());
}

#[test]
fn a_messy_dutch_csv_still_imports_what_it_can() {
    let (mut doc, h) = starter();
    let w1 = doc.project().cable(h.w1).unwrap().clone();
    let id = h.w1.raw();
    // Columns reordered, an extra Notes column, a blank line, trailing
    // spaces, the tag with Excel's text marker, a decimal comma, a row for
    // a cable that is gone and a repeat of the first row.
    let csv = format!(
        "\u{feff}Length [m];Notes;Tag;Id;Description\r\n\
         12,5;check;'{tag};{id};  Motor cable 4G2.5  \r\n\
         \r\n\
         3;;'=A1-W9;99999;gone\r\n\
         99;;'{tag};{id};second row\r\n",
        tag = w1.tag
    );
    let book = read_csv(&csv, Some(TableKind::Cables)).unwrap();
    let plan = plan_import(doc.project(), &book);
    assert_eq!(plan.rows_read, 3);
    assert_eq!(plan.rejected.len(), 2, "{}", plan.rejected_report());
    assert!(plan.rejected_report().contains("no longer exists"));
    assert!(plan.rejected_report().contains("already on row 2"));
    assert!(plan.notes.iter().any(|n| n.contains("Notes")));
    assert_eq!(plan.selected(), 2, "length and description");
    plan.apply(&mut doc).unwrap();
    let after = doc.project().cable(h.w1).unwrap();
    assert_eq!(after.length_m, Some(12.5));
    assert_eq!(after.description, "Motor cable 4G2.5");
    assert_eq!(after.tag, w1.tag);
}
