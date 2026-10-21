//! Wire numbering in the app: the commands, the window and the one-undo-step
//! rule.

use novo_core::{AnyId, Endpoint, Pin, Placement, Point, SymbolRole, WireId};

use super::gate1_tests::Drive;
use crate::actions::Action;

/// Two wires between two blocks on the open page.
fn two_wires(d: &mut Drive) -> Vec<WireId> {
    let page = d.doc().project().pages().next().unwrap().id;
    let (wires, _) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("setup", |tx| {
            let pins = || vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 8.0, 0.0)];
            let a = tx.place_symbol(
                page,
                "x",
                Placement::at(Point::new(40.0, 20.0)),
                pins(),
                SymbolRole::Unassigned,
            )?;
            let b = tx.place_symbol(
                page,
                "x",
                Placement::at(Point::new(40.0, 90.0)),
                pins(),
                SymbolRole::Unassigned,
            )?;
            Ok(vec![
                tx.connect(Endpoint::pin(a, 0), Endpoint::pin(b, 0))?,
                tx.connect(Endpoint::pin(a, 1), Endpoint::pin(b, 1))?,
            ])
        })
        .unwrap();
    wires
}

fn select_wire(d: &mut Drive, wire: WireId) {
    let view = d.app.canvas.schematic().expect("editor");
    view.canvas.selection.clear();
    view.canvas.selection.apply(
        [novo_canvas::model::item_id(AnyId::Wire(wire))],
        novo_canvas::SelectOp::Replace,
    );
    d.frame();
}

fn numbers(d: &Drive, wires: &[WireId]) -> Vec<Option<String>> {
    wires
        .iter()
        .map(|&w| d.doc().project().wire(w).unwrap().number.clone())
        .collect()
}

#[test]
fn number_wires_fills_in_one_undo_step_and_is_then_a_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let wires = two_wires(&mut d);
    for id in [
        "wires.number",
        "wires.numbering",
        "wires.renumber",
        "wires.check_numbers",
        "wires.set_number",
        "wires.unlock_number",
    ] {
        assert!(super::workbench::app_runs(id), "{id}");
    }
    d.app.run_command(&ctx, "wires.number", None);
    d.frame();
    assert_eq!(numbers(&d, &wires), [Some("1".into()), Some("2".into())]);
    d.app.run_command(&ctx, "wires.number", None);
    d.frame();
    assert_eq!(numbers(&d, &wires), [Some("1".into()), Some("2".into())]);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(numbers(&d, &wires), [None, None], "one undo step");
}

#[test]
fn a_typed_number_is_locked_refused_when_used_and_released() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let wires = two_wires(&mut d);
    d.app.run_command(&ctx, "wires.number", None);
    d.frame();
    select_wire(&mut d, wires[0]);
    // From the menu there is no number: it asks instead of clearing.
    d.app.run_command(&ctx, "wires.set_number", None);
    assert!(d.app.wb.number_prompt.is_some());
    d.app.wb.number_prompt = None;
    d.app.run_command(&ctx, "wires.set_number", Some("105"));
    d.frame();
    assert_eq!(numbers(&d, &wires)[0].as_deref(), Some("105"));
    assert!(novo_numbering::is_locked(d.doc().project(), wires[0]));
    // Another net already carries 2: refused, nothing changes.
    d.app.run_command(&ctx, "wires.set_number", Some("2"));
    d.frame();
    assert_eq!(numbers(&d, &wires)[0].as_deref(), Some("105"));
    d.app.run_command(&ctx, "wires.unlock_number", None);
    d.frame();
    assert!(!novo_numbering::is_locked(d.doc().project(), wires[0]));
}

#[test]
fn the_window_previews_and_applies_a_renumber_as_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let wires = two_wires(&mut d);
    d.app.run_command(&ctx, "wires.number", None);
    d.frame();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Scramble", |tx| {
            tx.update::<novo_core::Wire>(wires[0], |w| w.number = Some("9".into()))?;
            tx.update::<novo_core::Wire>(wires[1], |w| w.number = Some("8".into()))
        })
        .unwrap();
    d.app.run_command(&ctx, "wires.renumber", None);
    d.frame();
    // The preview is computed for the open window; the project is untouched.
    assert!(d.app.wb.numbering.is_some());
    assert_eq!(numbers(&d, &wires), [Some("9".into()), Some("8".into())]);
    let mut win = d.app.wb.numbering.take().unwrap();
    d.app.apply_numbering(&mut win, true);
    d.frame();
    assert_eq!(numbers(&d, &wires), [Some("1".into()), Some("2".into())]);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(numbers(&d, &wires), [Some("9".into()), Some("8".into())]);
    d.app.run_command(&ctx, "wires.check_numbers", None);
    d.frame();
    assert!(d.app.wb.numbering.is_some());
}

#[test]
fn numbers_tab_types_and_unlocks_per_net_and_skips_locked_pages() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let wires = two_wires(&mut d);
    d.app.run_command(&ctx, "wires.number", None);
    d.frame();
    // The menu command opens the window on the Numbers tab.
    d.app.run_command(&ctx, "wires.numbering", None);
    d.frame();
    assert_eq!(
        d.app.wb.numbering.as_ref().unwrap().tab,
        super::numbering::Tab::Numbers
    );
    // A row edit types the number, fixes it, and is one undo step.
    d.app.set_wire_number_of(wires[0], "105");
    d.frame();
    assert_eq!(numbers(&d, &wires)[0].as_deref(), Some("105"));
    assert!(novo_numbering::is_locked(d.doc().project(), wires[0]));
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(numbers(&d, &wires)[0].as_deref(), Some("1"));
    // A revision-locked page: runs leave it alone and say so.
    let page = d.doc().project().wire(wires[0]).unwrap().page;
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Lock", |tx| tx.lock_page(page, "A"))
        .unwrap();
    let scheme = novo_numbering::Scheme::load(d.doc().project());
    let plan = novo_numbering::plan(
        d.doc().project(),
        &scheme,
        novo_numbering::Mode::Renumber,
        &novo_numbering::Scope::Project,
    );
    assert!(plan.is_empty());
    assert!(
        plan.groups
            .iter()
            .all(|g| g.page_lock.as_deref() == Some("A"))
    );
}
