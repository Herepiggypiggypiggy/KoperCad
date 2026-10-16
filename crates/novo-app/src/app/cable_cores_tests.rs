//! Cable editing from the Cables drawer and the cable commands: each is one
//! undo step, and the menu, command field and drawer share the code.

use novo_cables::CableType;
use novo_core::{AnyId, CableId, DeviceTag, WireId};
use novo_workflow::palette::Target;

use super::cable_controls::CableAction;
use super::gate1_tests::Drive;
use super::workbench::Drawer;
use crate::actions::Action;

struct Fixture {
    d: Drive,
    ctx: eframe::egui::Context,
    wires: [WireId; 4],
    first_cable: CableId,
}

/// The Gate 1 starter with its motor cable accepted: `-W1` is `4G1.5`.
fn fixture() -> (Fixture, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let doc = d.app.backend.document_mut().unwrap();
    let (m, _) = doc
        .transact("Gate 1", |tx| {
            let m = novo_tests::motor_starter::motor_starter_in(tx)?;
            tx.remove(m.w1)?;
            Ok(m)
        })
        .unwrap();
    d.frame();
    let proposals = novo_cables::propose_cables(d.doc().project());
    assert_eq!(proposals.len(), 1);
    let doc = d.app.backend.document_mut().unwrap();
    let (cables, _) = doc
        .transact("Accept", |tx| {
            novo_cables::accept_proposals(tx, &proposals, None)
        })
        .unwrap();
    d.frame();
    (
        Fixture {
            d,
            ctx,
            wires: m.motor_wires,
            first_cable: cables[0],
        },
        dir,
    )
}

fn cable(f: &Fixture) -> novo_core::Cable {
    f.d.doc().project().cable(f.first_cable).unwrap().clone()
}

#[test]
fn per_core_edits_are_one_undo_step_each() {
    let (mut f, _dir) = fixture();
    let c = f.first_cable;
    let [w0, w1, _, _] = f.wires;
    let depth = f.d.doc().undo_depth();

    f.d.app.cable_action(c, CableAction::SwapCores(0, 1));
    assert_eq!(f.d.doc().undo_depth(), depth + 1);
    assert_eq!(cable(&f).cores[0].wire, Some(w1));
    assert_eq!(cable(&f).cores[1].wire, Some(w0));

    f.d.app.cable_action(
        c,
        CableAction::SetCoreWire {
            core: 1,
            wire: None,
        },
    );
    assert_eq!(f.d.doc().undo_depth(), depth + 2);
    assert_eq!(cable(&f).cores[1].wire, None);

    f.d.app.cable_action(
        c,
        CableAction::SetCoreWire {
            core: 1,
            wire: Some(w0),
        },
    );
    assert_eq!(f.d.doc().undo_depth(), depth + 3);
    assert_eq!(cable(&f).cores[1].wire, Some(w0));

    f.d.app.cable_action(
        c,
        CableAction::SetSpare {
            core: 2,
            spare: true,
        },
    );
    assert_eq!(f.d.doc().undo_depth(), depth + 4);
    assert!(novo_cables::is_spare(&cable(&f), 2));
    f.d.app.cable_action(
        c,
        CableAction::SetSpare {
            core: 2,
            spare: false,
        },
    );
    assert!(!novo_cables::is_spare(&cable(&f), 2));

    // The same choices undo one by one.
    f.d.app.run(&f.ctx.clone(), Action::Undo);
    f.d.app.run(&f.ctx.clone(), Action::Undo);
    assert!(novo_cables::spare_cores(&cable(&f)).is_empty());
    assert_eq!(cable(&f).cores[1].wire, Some(w0));
}

#[test]
fn the_cable_commands_rename_retype_delete_and_undo() {
    let (mut f, _dir) = fixture();
    let ctx = f.ctx.clone();
    let c = f.first_cable;
    let depth = f.d.doc().undo_depth();

    f.d.app.run_command(&ctx, "cable.rename", Some("W9"));
    assert_eq!(cable(&f).tag, DeviceTag::parse("-W9").unwrap());
    assert_eq!(f.d.doc().undo_depth(), depth + 1);
    // A tag that is not a tag changes nothing.
    f.d.app.run_command(&ctx, "cable.rename", Some("=="));
    assert_eq!(f.d.doc().undo_depth(), depth + 1);

    f.d.app
        .run_command(&ctx, "cable.retype", Some("5G1.5 shielded"));
    assert_eq!(f.d.doc().undo_depth(), depth + 2);
    assert_eq!(cable(&f).description, "5G1.5 shielded");
    // Not a usual type, or one the wires do not fit: nothing happens.
    f.d.app.run_command(&ctx, "cable.retype", Some("3x0.75"));
    f.d.app.run_command(&ctx, "cable.retype", Some("nonsense"));
    assert_eq!(f.d.doc().undo_depth(), depth + 2);

    // Without a name the rename command opens the drawer on the field.
    f.d.app.wb.drawer = None;
    f.d.app.wb.queued.push("cable.rename");
    f.d.frame();
    assert_eq!(f.d.app.wb.drawer, Some(Drawer::Cables));
    f.d.frame();

    f.d.app.wb.queued.push("cable.delete");
    f.d.frame();
    assert!(f.d.doc().project().cable(c).is_none());
    assert_eq!(f.d.doc().undo_depth(), depth + 3);
    assert_eq!(novo_cables::propose_cables(f.d.doc().project()).len(), 1);
    f.d.app.run(&ctx, Action::Undo);
    assert!(f.d.doc().project().cable(c).is_some());
}

#[test]
fn a_proposal_is_accepted_on_a_chosen_type_declined_and_restored() {
    let (mut f, _dir) = fixture();
    let ctx = f.ctx.clone();
    // Remove the accepted cable so its wires are proposed again.
    f.d.app.run_command(&ctx, "cable.delete", None);
    let key = f.wires[0].raw();
    let depth = f.d.doc().undo_depth();

    f.d.app.decline_one_cable(key);
    assert_eq!(f.d.doc().undo_depth(), depth + 1);
    assert!(novo_cables::propose_cables(f.d.doc().project()).is_empty());
    assert_eq!(novo_cables::declined_wires(f.d.doc().project()).len(), 4);

    f.d.app.wb.queued.push("cable.restore_proposals");
    f.d.frame();
    assert_eq!(f.d.doc().undo_depth(), depth + 2);
    assert_eq!(novo_cables::propose_cables(f.d.doc().project()).len(), 1);

    let five = CableType::standard(5, true, 1.5).shielded();
    f.d.app.accept_one_cable(key, Some(five));
    assert_eq!(f.d.doc().undo_depth(), depth + 3);
    let created = f.d.app.wb.drawer_cable.expect("the new cable is shown");
    assert_eq!(
        f.d.doc().project().cable(created).unwrap().description,
        "5G1.5 shielded"
    );
    // Restoring with nothing declined says so and changes nothing.
    f.d.app.run_command(&ctx, "cable.restore_proposals", None);
    assert_eq!(f.d.doc().undo_depth(), depth + 3);
}

#[test]
fn the_cables_drawer_draws_its_editing_controls() {
    let (mut f, _dir) = fixture();
    f.d.app.wb.drawer = Some(Drawer::Cables);
    f.d.app.wb.drawer_cable = Some(f.first_cable);
    f.d.frame();
    f.d.frame();
    assert_eq!(f.d.app.wb.cable_edit.ui.name, "=A1-W1");
    assert_eq!(f.d.app.shown_cable(), Some(f.first_cable));
    let candidates = super::cable_cores::candidate_wires(f.d.doc().project(), f.first_cable);
    for wire in f.wires {
        assert!(candidates.iter().any(|(w, _)| *w == wire), "{wire:?}");
    }
    let free = f.d.doc().project().wires().count() - 4;
    assert!(candidates.len() <= 4 + free, "only free wires besides");
}

#[test]
fn palette_rows_open_the_cable_and_select_the_wire() {
    let (mut f, _dir) = fixture();
    let ctx = f.ctx.clone();
    let wire = f.wires[0];
    f.d.app.wb.drawer = None;
    f.d.app.run_target(&ctx, Target::Cable(f.first_cable));
    assert_eq!(f.d.app.wb.drawer, Some(Drawer::Cables));
    assert_eq!(f.d.app.wb.drawer_cable, Some(f.first_cable));

    let page = f.d.doc().project().wire(wire).unwrap().page;
    f.d.app.run_target(&ctx, Target::Wire(wire));
    assert_eq!(f.d.app.wb.pending_item, Some((page, AnyId::Wire(wire))));
    f.d.frame();
    f.d.frame();
    assert!(f.d.app.selected_wires().contains(&wire));
}

#[test]
fn the_wire_panel_types_and_unlocks_a_number() {
    let (mut f, _dir) = fixture();
    let ctx = f.ctx.clone();
    let wire = f.wires[0];
    f.d.app.run_target(&ctx, Target::Wire(wire));
    f.d.frame();
    f.d.frame();
    f.d.frame();
    assert_eq!(f.d.app.wb.wire_panel.wire, Some(wire));

    let depth = f.d.doc().undo_depth();
    f.d.app.set_wire_number(Some("707"));
    assert_eq!(f.d.doc().undo_depth(), depth + 1);
    assert_eq!(
        f.d.doc().project().wire(wire).unwrap().number.as_deref(),
        Some("707")
    );
    assert!(novo_numbering::is_locked(f.d.doc().project(), wire));
    f.d.frame();
    f.d.frame();
    assert_eq!(f.d.app.wb.wire_panel.number.as_str(), "707");
    f.d.app.unlock_wire_number();
    assert!(!novo_numbering::is_locked(f.d.doc().project(), wire));
    assert_eq!(f.d.doc().undo_depth(), depth + 2);
}
