//! The four commands that used to be greyed "Not in the app yet":
//! `edit.renumber`, `draw.tidy`, `project.rules` (and `nav.changes`, which
//! duplicated `view.changes` and is gone).

use novo_core::{
    AnyId, DeviceTag, Endpoint, JoinLink, Pin, Placement, Point, SameName, SymbolRole, WireId,
};

use super::gate1_tests::Drive;
use crate::actions::Action;

fn open() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

fn tags(d: &Drive) -> Vec<String> {
    let mut all: Vec<String> = d
        .doc()
        .project()
        .devices()
        .map(|dev| dev.tag.to_string())
        .collect();
    all.sort();
    all
}

/// Two relays placed out of reading order: -K7 above, -K3 below.
fn two_relays(d: &mut Drive) {
    let page = d.doc().project().pages().next().unwrap().id;
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("setup", |tx| {
            for (tag, y) in [("-K7", 20.0), ("-K3", 90.0)] {
                let device = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.place_symbol(
                    page,
                    "iec/coils/relay",
                    Placement::at(Point::new(40.0, y)),
                    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 8.0, 0.0)],
                    SymbolRole::Device { device, main: true },
                )?;
            }
            Ok(())
        })
        .unwrap();
    d.frame();
}

/// The lesson step commands run (`learn_run_command`), so the command field
/// must not grey them as "Comes later".
#[test]
fn lesson_step_commands_are_not_greyed() {
    for id in ["learn.next", "learn.back", "learn.restart", "learn.close"] {
        assert!(super::workbench::app_runs(id), "{id}");
    }
}

#[test]
fn nothing_is_greyed_any_more() {
    for id in ["edit.renumber", "draw.tidy", "project.rules"] {
        assert!(super::workbench::app_runs(id), "{id}");
    }
    assert!(
        !novo_workflow::commands::is_known("nav.changes"),
        "nav.changes duplicated view.changes"
    );
    assert!(novo_workflow::commands::is_known("view.changes"));
}

#[test]
fn renumber_previews_then_applies_as_one_undo_step() {
    let (_dir, mut d) = open();
    let ctx = d.ctx.clone();
    two_relays(&mut d);
    assert_eq!(tags(&d), ["-K3", "-K7"]);
    d.app.run_command(&ctx, "edit.renumber", None);
    d.frame();
    // The window previews; nothing changed yet.
    assert!(d.app.wb.device_renumber.is_some());
    assert_eq!(tags(&d), ["-K3", "-K7"]);
    let plan =
        novo_workflow::renumber::plan(d.doc().project(), &novo_workflow::renumber::Scope::Project);
    assert_eq!(plan.len(), 2, "-K7 becomes -K1, -K3 becomes -K2");
    d.app.apply_device_renumber(&plan);
    d.frame();
    assert_eq!(tags(&d), ["-K1", "-K2"]);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(tags(&d), ["-K3", "-K7"], "one undo step");
}

#[test]
fn renumber_takes_its_scope_from_the_argument() {
    let (_dir, mut d) = open();
    let ctx = d.ctx.clone();
    two_relays(&mut d);
    d.app.run_command(&ctx, "edit.renumber", Some("page"));
    d.frame();
    assert_eq!(
        d.app.wb.device_renumber.as_ref().unwrap().scope,
        super::device_renumber::DeviceScope::ThisPage
    );
}

#[test]
fn tidy_reroutes_the_selected_wire_in_one_undo_step() {
    let (_dir, mut d) = open();
    let ctx = d.ctx.clone();
    let page = d.doc().project().pages().next().unwrap().id;
    let wire: WireId = d
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
                Placement::at(Point::new(120.0, 90.0)),
                pins(),
                SymbolRole::Unassigned,
            )?;
            let w = tx.connect(Endpoint::pin(a, 0), Endpoint::pin(b, 0))?;
            tx.update::<novo_core::Wire>(w, |w| {
                w.route = vec![Point::new(10.0, 5.0), Point::new(200.0, 5.0)];
            })?;
            Ok(w)
        })
        .unwrap()
        .0;
    let route = |d: &Drive| d.doc().project().wire(wire).unwrap().route.clone();
    let before = route(&d);
    // Nothing selected: a hint, no change.
    d.app.run_command(&ctx, "draw.tidy", None);
    d.frame();
    assert_eq!(route(&d), before);
    let view = d.app.canvas.schematic().expect("editor");
    view.canvas.selection.clear();
    view.canvas.selection.apply(
        [novo_canvas::model::item_id(AnyId::Wire(wire))],
        novo_canvas::SelectOp::Replace,
    );
    d.frame();
    d.app.run_command(&ctx, "draw.tidy", None);
    d.frame();
    assert_ne!(route(&d), before, "the wire has a fresh route");
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(route(&d), before, "one undo step");
}

#[test]
fn project_rules_are_saved_and_undone() {
    let (_dir, mut d) = open();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "project.rules", None);
    d.frame();
    assert!(d.app.wb.project_rules);
    let mut rules = d.app.project_rules();
    assert_eq!(
        rules.terminals.same_name,
        SameName::Join,
        "new projects join"
    );
    rules.terminals.same_name = SameName::Off;
    rules.terminals.join_link = JoinLink::Jumper;
    rules.potentials.from_terminal_names = false;
    d.app.set_project_rules(rules);
    d.frame();
    let now = d.app.project_rules();
    assert_eq!(now.terminals.same_name, SameName::Off);
    assert!(!now.potentials.from_terminal_names);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    let back = d.app.project_rules();
    assert_eq!(back.terminals.same_name, SameName::Join);
    assert!(back.potentials.from_terminal_names, "one undo step");
}

#[test]
fn a_switched_off_generated_page_can_be_wanted_again() {
    let (_dir, mut d) = open();
    let mut rules = d.app.project_rules();
    rules.generated_pages_off.insert("bom".into());
    d.app.set_project_rules(rules);
    d.frame();
    assert!(!d.app.project_rules().wants_generated("bom"));
    let mut rules = d.app.project_rules();
    rules.generated_pages_off.remove("bom");
    d.app.set_project_rules(rules);
    d.frame();
    assert!(d.app.project_rules().wants_generated("bom"));
}
