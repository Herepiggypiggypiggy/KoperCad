//! The guided walk in the Commissioning workspace: Next open, the big tick
//! and Space, wire and cable core told apart, and the punch list PDF.

use eframe::egui;
use novo_commissioning::{CheckState, Target, WireKind, WirePlan, targets};

use super::gate1_tests::Drive;
use super::workspace::Workspace;
use crate::actions::Action;

fn walking() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.app.switch_workspace(Workspace::Commissioning);
    for _ in 0..3 {
        d.frame();
    }
    assert_eq!(d.app.workspace(), Workspace::Commissioning);
    (dir, d)
}

fn space(pressed: bool) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn tap_space(d: &mut Drive) {
    let at = egui::Event::PointerMoved(egui::pos2(600.0, 400.0));
    d.frame_with(vec![at, space(true)]);
    d.frame_with(vec![space(false)]);
    d.frame();
}

fn decided(d: &Drive) -> usize {
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    let project = d.doc().project();
    targets(project)
        .iter()
        .flat_map(|i| i.target.checks().iter().map(move |&c| (i.target, c)))
        .filter(|&(t, c)| mode.store.state(t, c) != CheckState::Pending)
        .count()
}

#[test]
fn space_starts_the_walk_then_ticks_and_moves_on() {
    let (_dir, mut d) = walking();
    assert!(d.app.wb.commissioning.as_ref().unwrap().selected.is_none());
    // The first tap has nothing to tick: it selects the first open target.
    tap_space(&mut d);
    let first = d.app.wb.commissioning.as_ref().unwrap().selected;
    assert!(first.is_some(), "Space started the walk");
    assert_eq!(decided(&d), 0);
    assert!(
        !d.app.wb.field.is_open(),
        "Space did not open the command field"
    );
    // The second tap passes that target's open checks and moves on.
    tap_space(&mut d);
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    let first = first.unwrap();
    for &check in first.checks() {
        assert_eq!(mode.store.state(first, check), CheckState::Passed);
    }
    assert_ne!(mode.selected, Some(first), "the walk moved on");
    assert_eq!(decided(&d), first.checks().len());
}

#[test]
fn a_walked_wire_is_selected_in_the_drawing() {
    let (_dir, mut d) = walking();
    d.app.wb.commissioning.as_mut().unwrap().scope = novo_commissioning::WalkScope::Wires;
    tap_space(&mut d);
    for _ in 0..3 {
        d.frame();
    }
    let Some(Target::Wire(wire)) = d.app.wb.commissioning.as_ref().unwrap().selected else {
        panic!("the walk is on a wire");
    };
    let selected = d.app.canvas.schematic_ref().unwrap().canvas.selection.ids();
    assert!(
        selected.contains(&novo_canvas::model::item_id(novo_core::AnyId::Wire(wire))),
        "the wire is selected on its page"
    );
}

#[test]
fn the_example_tells_a_core_from_a_wire() {
    let (_dir, d) = walking();
    let plan = WirePlan::of(d.doc().project());
    let cores = d
        .doc()
        .project()
        .wires()
        .filter_map(|w| plan.step(w.id))
        .filter(|s| matches!(s.kind, WireKind::Core { .. }))
        .count();
    assert!(cores > 0, "the example has cable cores");
}

#[test]
fn the_punch_list_is_written_as_pdf() {
    let (dir, mut d) = walking();
    assert!(novo_workflow::commands::get("export.punch_list").is_some());
    let wire = targets(d.doc().project())
        .into_iter()
        .find(|t| matches!(t.target, Target::Wire(_)))
        .unwrap()
        .target;
    d.app.wb.commissioning.as_mut().unwrap().decide(
        wire,
        novo_commissioning::CheckKind::Continuity,
        CheckState::Failed,
    );
    d.frame();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "export.punch_list", None);
    d.frame();
    let path = d.app.wb.last_export.clone().expect("a file was written");
    assert!(path.starts_with(dir.path()) || path.exists(), "{path:?}");
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn back_takes_the_last_pass_back() {
    let (_dir, mut d) = walking();
    tap_space(&mut d); // starts the walk
    let first = d.app.wb.commissioning.as_ref().unwrap().selected.unwrap();
    tap_space(&mut d); // passes it
    assert_eq!(decided(&d), first.checks().len());
    let page = d.app.current_page();
    let mut go = None;
    let project = d.doc().project().clone();
    let mode = d.app.wb.commissioning.as_mut().unwrap();
    assert!(mode.back(&project, page, &mut go));
    assert_eq!(mode.selected, Some(first), "the walk is back on it");
    for &check in first.checks() {
        assert_eq!(mode.store.state(first, check), CheckState::Pending);
    }
    assert!(!mode.back(&project, page, &mut go), "nothing more to undo");
}

fn key(d: &mut Drive, k: egui::Key) {
    let ev = |pressed| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    d.frame_with(vec![ev(true)]);
    d.frame_with(vec![ev(false)]);
}

#[test]
fn fail_opens_the_note_with_the_ends_then_records_on_the_second_press() {
    let (_dir, mut d) = walking();
    tap_space(&mut d);
    let target = d.app.wb.commissioning.as_ref().unwrap().selected.unwrap();
    key(&mut d, egui::Key::F);
    {
        let mode = d.app.wb.commissioning.as_ref().unwrap();
        let first = target.checks()[0];
        assert_eq!(mode.store.state(target, first), CheckState::Pending);
        assert!(
            mode.note.contains(" to "),
            "the note starts with the ends: {}",
            mode.note
        );
    }
    // The reason is typed in the focused Note, then Enter records.
    d.frame_with(vec![egui::Event::Text("loose".into())]);
    d.frame_with(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    d.frame();
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    let first = target.checks()[0];
    assert_eq!(mode.store.state(target, first), CheckState::Failed);
    let item = mode.store.punch_items().find(|p| p.target == Some(target));
    assert!(item.is_some(), "the failure is on the punch list");
}

#[test]
fn n_marks_not_applicable_and_backspace_takes_a_pass_back() {
    let (_dir, mut d) = walking();
    tap_space(&mut d);
    let target = d.app.wb.commissioning.as_ref().unwrap().selected.unwrap();
    key(&mut d, egui::Key::N);
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    assert_eq!(
        mode.store.state(target, target.checks()[0]),
        CheckState::NotApplicable
    );
    tap_space(&mut d); // passes the rest, moves on
    let moved = d.app.wb.commissioning.as_ref().unwrap().selected.unwrap();
    assert_ne!(moved, target);
    key(&mut d, egui::Key::Backspace);
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    assert_eq!(mode.selected, Some(target), "Backspace went back one wire");
}
