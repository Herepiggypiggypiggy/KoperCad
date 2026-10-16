//! New terminal, cable and core rows on import: export, add rows, import.

use novo_cables::{is_spare, spare_cores};
use novo_core::{DeviceTag, Document};
use novo_interop::sheets::{
    Cell, Row, TableKind, Workbook, export_project, plan_import, read_xlsx, rejected_workbook,
    xlsx_bytes,
};
use novo_tests::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let handles = motor_starter(&mut doc).expect("fixture builds");
    (doc, handles)
}

fn via_xlsx(book: &Workbook) -> Workbook {
    read_xlsx(&xlsx_bytes(book).expect("writes")).expect("reads")
}

fn tag(s: &str) -> DeviceTag {
    DeviceTag::parse(s).unwrap()
}

/// Appends a row to a sheet with only the given cells set.
fn add(book: &mut Workbook, kind: TableKind, cells: &[(&str, Cell)]) -> usize {
    let table = book
        .tables
        .iter_mut()
        .find(|t| t.kind == kind)
        .expect("table");
    let mut row = vec![Cell::Empty; table.headers.len()];
    for (header, cell) in cells {
        row[table.column(header).expect("column")] = cell.clone();
    }
    let line = table.rows.len() + 2;
    table.rows.push(Row { line, cells: row });
    line
}

fn t(s: &str) -> Cell {
    Cell::text(s)
}

const STRIP: &str = "=A1+CAB-X1";
const CABLE: &str = "=A1-W1";

#[test]
fn a_new_terminal_goes_after_the_last_one_of_its_strip() {
    let (mut doc, h) = starter();
    let before = doc.project().terminals_of_strip(h.x1).len();
    let max = doc
        .project()
        .terminals_of_strip(h.x1)
        .iter()
        .map(|x| x.order)
        .max()
        .unwrap();
    let mut book = via_xlsx(&export_project(doc.project()));
    add(
        &mut book,
        TableKind::Terminals,
        &[
            ("Strip", t(STRIP)),
            ("Name", t("4")),
            ("Jumper", Cell::Number(7.0)),
            ("Kind", t("PE")),
        ],
    );
    add(
        &mut book,
        TableKind::Terminals,
        &[("Strip", t(STRIP)), ("Name", t("5"))],
    );
    let plan = plan_import(doc.project(), &book);
    assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
    assert_eq!(plan.new_terminals.len(), 2);
    assert!(
        plan.summary().contains("2 new terminals"),
        "{}",
        plan.summary()
    );
    plan.apply(&mut doc).unwrap();
    let strip = doc.project().terminals_of_strip(h.x1);
    assert_eq!(strip.len(), before + 2);
    let four = strip.iter().find(|x| x.name == "4").unwrap();
    let five = strip.iter().find(|x| x.name == "5").unwrap();
    assert_eq!(four.order, max + 1);
    assert_eq!(five.order, max + 2);
    assert_eq!(four.jumper, Some(7));
    assert_eq!(four.kind, novo_core::TerminalKind::ProtectiveEarth);
    doc.undo().unwrap();
    assert_eq!(doc.project().terminals_of_strip(h.x1).len(), before);
}

#[test]
fn a_new_terminal_that_cannot_go_anywhere_says_why() {
    let (doc, _) = starter();
    let mut book = export_project(doc.project());
    add(
        &mut book,
        TableKind::Terminals,
        &[("Strip", t("=A1+CAB-X9")), ("Name", t("1"))],
    );
    add(
        &mut book,
        TableKind::Terminals,
        &[("Strip", t("=A1+CAB-K1")), ("Name", t("1"))],
    );
    add(
        &mut book,
        TableKind::Terminals,
        &[("Strip", t(STRIP)), ("Name", t("8")), ("Kind", t("banana"))],
    );
    add(
        &mut book,
        TableKind::Terminals,
        &[("Strip", t(STRIP)), ("Name", t("9"))],
    );
    add(
        &mut book,
        TableKind::Terminals,
        &[("Strip", t(STRIP)), ("Name", t("9"))],
    );
    let plan = plan_import(doc.project(), &book);
    assert_eq!(plan.new_terminals.len(), 1);
    let why: Vec<String> = plan.rejected.iter().map(|r| r.reasons.join(";")).collect();
    assert_eq!(why.len(), 4, "{why:?}");
    assert!(why[0].contains("no terminal strip"), "{}", why[0]);
    assert!(why[1].contains("not a terminal strip"), "{}", why[1]);
    assert!(why[2].contains("not a terminal kind"), "{}", why[2]);
    assert!(why[3].contains("already added on row"), "{}", why[3]);
}

#[test]
fn terminals_on_a_new_strip_follow_the_strip() {
    let (mut doc, _) = starter();
    let mut book = export_project(doc.project());
    add(
        &mut book,
        TableKind::Devices,
        &[("Tag", t("=A1+CAB-X2")), ("Kind", t("terminal strip"))],
    );
    for n in ["1", "2"] {
        add(
            &mut book,
            TableKind::Terminals,
            &[("Strip", t("=A1+CAB-X2")), ("Name", t(n))],
        );
    }
    let mut plan = plan_import(doc.project(), &book);
    assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
    assert_eq!(plan.selected(), 3);
    let rows = plan.added_rows();
    assert_eq!(rows.len(), 3);
    // Leave the strip out: its terminals go with it.
    plan.select_added(0, false);
    assert_eq!(plan.selected(), 0);
    let rows = plan.added_rows();
    assert_eq!(rows[1].left_out_with.as_deref(), Some("=A1+CAB-X2"));
    plan.select_added(0, true);
    plan.apply(&mut doc).unwrap();
    let strip = doc
        .project()
        .devices()
        .find(|d| d.tag == tag("=A1+CAB-X2"))
        .unwrap();
    let names: Vec<&str> = doc
        .project()
        .terminals_of_strip(strip.id)
        .iter()
        .map(|x| x.name.as_str())
        .collect();
    assert_eq!(names, ["1", "2"]);
    doc.undo().unwrap();
    assert!(doc.project().devices().all(|d| d.tag != tag("=A1+CAB-X2")));
}

#[test]
fn a_new_cable_comes_with_its_cores_and_spare_marks() {
    let (mut doc, _) = starter();
    let cables = doc.project().cables().count();
    let mut book = via_xlsx(&export_project(doc.project()));
    add(
        &mut book,
        TableKind::Cables,
        &[
            ("Tag", t("=A1-W7")),
            ("Description", t("Sensor cable")),
            ("Length [m]", Cell::Number(12.5)),
        ],
    );
    for (no, name, spare) in [(1.0, "BN", "no"), (2.0, "BU", "yes")] {
        add(
            &mut book,
            TableKind::Cores,
            &[
                ("Cable", t("=A1-W7")),
                ("Core No", Cell::Number(no)),
                ("Core", t(name)),
                ("Cross-section [mm²]", Cell::Number(0.75)),
                ("Spare", t(spare)),
            ],
        );
    }
    let plan = plan_import(doc.project(), &book);
    assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
    assert_eq!((plan.new_cables.len(), plan.new_cores.len()), (1, 2));
    plan.apply(&mut doc).unwrap();
    assert_eq!(doc.project().cables().count(), cables + 1);
    let w7 = doc
        .project()
        .cables()
        .find(|c| c.tag == tag("=A1-W7"))
        .unwrap();
    assert_eq!(w7.description, "Sensor cable");
    assert_eq!(w7.length_m, Some(12.5));
    let names: Vec<&str> = w7.cores.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["BN", "BU"]);
    assert_eq!(w7.cores[0].cross_section_mm2, Some(0.75));
    assert!(is_spare(w7, 1) && !is_spare(w7, 0));
    // Export again: the new cable round trips unchanged.
    let again = plan_import(doc.project(), &via_xlsx(&export_project(doc.project())));
    assert!(again.is_empty() && again.rejected.is_empty());
    doc.undo().unwrap();
    assert_eq!(doc.project().cables().count(), cables);
}

#[test]
fn a_cable_left_out_takes_its_cores_with_it() {
    let (doc, _) = starter();
    let mut book = export_project(doc.project());
    add(&mut book, TableKind::Cables, &[("Tag", t("=A1-W7"))]);
    add(
        &mut book,
        TableKind::Cores,
        &[("Cable", t("=A1-W7")), ("Core", t("1"))],
    );
    let mut plan = plan_import(doc.project(), &book);
    assert_eq!(plan.selected(), 2);
    plan.select_added(0, false);
    assert_eq!(plan.selected(), 0);
    assert!(plan.command(doc.project()).unwrap().is_none());
}

#[test]
fn new_cores_are_added_in_order_to_a_cable_the_project_has() {
    let (mut doc, h) = starter();
    let count = doc.project().cable(h.w1).unwrap().cores.len();
    let id = Cell::Number(h.w1.raw() as f64);
    let mut book = via_xlsx(&export_project(doc.project()));
    // Core No one past the end, and an empty Core No by the cable's tag.
    add(
        &mut book,
        TableKind::Cores,
        &[
            ("Id", id.clone()),
            ("Core No", Cell::Number((count + 1) as f64)),
            ("Core", t("X1")),
            ("Spare", t("yes")),
        ],
    );
    add(
        &mut book,
        TableKind::Cores,
        &[("Cable", t(CABLE)), ("Core", t("X2"))],
    );
    let plan = plan_import(doc.project(), &book);
    assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
    let nos: Vec<usize> = plan.new_cores.iter().map(|c| c.no).collect();
    assert_eq!(nos, [count + 1, count + 2]);
    plan.apply(&mut doc).unwrap();
    let w1 = doc.project().cable(h.w1).unwrap();
    assert_eq!(w1.cores.len(), count + 2);
    assert_eq!(w1.cores[count].name, "X1");
    assert_eq!(w1.cores[count + 1].name, "X2");
    assert_eq!(spare_cores(w1), ["X1"]);
}

#[test]
fn a_spare_mark_follows_a_rename_in_the_same_import_as_a_new_core() {
    let (mut doc, h) = starter();
    // Mark the first core spare and export.
    doc.transact("spare", |tx| novo_cables::set_spare(tx, h.w1, 0, true))
        .unwrap();
    let count = doc.project().cable(h.w1).unwrap().cores.len();
    let mut book = via_xlsx(&export_project(doc.project()));
    let cores = book
        .tables
        .iter_mut()
        .find(|t| t.kind == TableKind::Cores)
        .unwrap();
    let (name_col, id_col) = (cores.column("Core").unwrap(), cores.column("Id").unwrap());
    let row = cores
        .rows
        .iter_mut()
        .find(|r| r.cells[id_col] == Cell::Number(h.w1.raw() as f64))
        .unwrap();
    row.cells[name_col] = t("RENAMED");
    add(
        &mut book,
        TableKind::Cores,
        &[
            ("Id", Cell::Number(h.w1.raw() as f64)),
            ("Core No", Cell::Number((count + 1) as f64)),
            ("Core", t("NEW")),
        ],
    );
    let plan = plan_import(doc.project(), &book);
    assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
    plan.apply(&mut doc).unwrap();
    let w1 = doc.project().cable(h.w1).unwrap();
    assert_eq!(w1.cores[0].name, "RENAMED");
    assert_eq!(w1.cores[count].name, "NEW");
    assert_eq!(spare_cores(w1), ["RENAMED"]);
}

#[test]
fn rejected_core_rows_say_why() {
    let (doc, h) = starter();
    let count = doc.project().cable(h.w1).unwrap().cores.len();
    let existing = doc.project().cable(h.w1).unwrap().cores[0].name.clone();
    let wire = doc.project().cable(h.w1).unwrap().cores[0].wire.unwrap();
    let id = Cell::Number(h.w1.raw() as f64);
    let mut book = export_project(doc.project());
    // A gap, a name the cable has, a wire already on a core, no name, and a
    // row that edits a core without the cable's Id.
    add(
        &mut book,
        TableKind::Cores,
        &[
            ("Id", id.clone()),
            ("Core No", Cell::Number((count + 3) as f64)),
            ("Core", t("GAP")),
        ],
    );
    add(
        &mut book,
        TableKind::Cores,
        &[
            ("Id", id.clone()),
            ("Core No", Cell::Number((count + 1) as f64)),
            ("Core", t(&existing)),
        ],
    );
    add(
        &mut book,
        TableKind::Cores,
        &[
            ("Id", id.clone()),
            ("Core No", Cell::Number((count + 1) as f64)),
            ("Core", t("TAKEN")),
            ("Wire Id", Cell::Number(wire.raw() as f64)),
        ],
    );
    add(
        &mut book,
        TableKind::Cores,
        &[("Id", id), ("Core No", Cell::Number((count + 1) as f64))],
    );
    add(
        &mut book,
        TableKind::Cores,
        &[
            ("Cable", t(CABLE)),
            ("Core No", Cell::Number(1.0)),
            ("Core", t("EDIT")),
        ],
    );
    let plan = plan_import(doc.project(), &book);
    let why: Vec<String> = plan.rejected.iter().map(|r| r.reasons.join(";")).collect();
    assert!(plan.new_cores.is_empty(), "{:?}", plan.new_cores);
    assert_eq!(why.len(), 5, "{why:?}");
    let all = why.join("\n");
    for needle in [
        "leave a gap",
        "already has a core called",
        "is already on a core",
        "the core name is empty",
        "needs the cable's Id",
    ] {
        assert!(all.contains(needle), "missing '{needle}' in\n{all}");
    }
}

#[test]
fn rejected_new_rows_come_back_fixed_and_import() {
    let (mut doc, h) = starter();
    let mut book = via_xlsx(&export_project(doc.project()));
    add(
        &mut book,
        TableKind::Terminals,
        &[("Strip", t("=A1+CAB-X9")), ("Name", t("77"))],
    );
    let plan = plan_import(doc.project(), &book);
    assert_eq!(plan.rejected.len(), 1);
    let mut fix = rejected_workbook(&plan, &book).expect("rejected rows");
    let table = fix
        .tables
        .iter_mut()
        .find(|t| t.kind == TableKind::Terminals)
        .unwrap();
    let strip_col = table.column("Strip").unwrap();
    table.rows[0].cells[strip_col] = t(STRIP);
    let before = doc.project().terminals_of_strip(h.x1).len();
    let plan = plan_import(doc.project(), &via_xlsx(&fix));
    assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
    plan.apply(&mut doc).unwrap();
    assert_eq!(doc.project().terminals_of_strip(h.x1).len(), before + 1);
}
