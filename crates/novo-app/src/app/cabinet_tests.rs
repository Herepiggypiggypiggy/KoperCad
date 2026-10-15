//! The Cabinet canvas: plate, place all as one undo step.

use novo_core::{CabinetPlacement, DeviceTag, Project, Rail, Value};

use super::cabinet::Ask;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn add_cabinet(d: &mut Drive) {
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-K2", "=A1+CAB1-Q1"] {
                let device = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(device, part)?;
            }
            Ok(())
        })
        .unwrap();
}

fn layout_of(d: &Drive, cab: novo_core::StructureId) -> Option<novo_layout::state::CabinetLayout> {
    novo_layout::model::load(project(d), cab)
}

fn project(d: &Drive) -> &Project {
    d.app.backend.document().unwrap().project()
}

#[test]
fn place_all_previews_then_accepts_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    assert!(d.app.cabinet_open);
    assert_eq!(
        d.app.cabinet.location.as_ref().unwrap().to_string(),
        "+CAB1"
    );

    // No plate yet: the first ask sets it.
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    assert!(
        project(&d)
            .get::<novo_core::Structure>(cab)
            .unwrap()
            .plate
            .is_some()
    );

    // Place all only previews.
    let layout = novo_layout::model::load(project(&d), cab);
    let cands = novo_layout::model::candidates(project(&d), cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.frame();
    assert!(d.app.cabinet.preview.is_some());
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);

    // Accepting places all three with rails, in one step.
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 3);
    assert!(project(&d).all::<Rail>().count() >= 1);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);
    assert_eq!(project(&d).all::<Rail>().count(), 0);

    d.app.run(&ctx, Action::ShowSchematic);
    assert!(!d.app.cabinet_open);
}

#[test]
fn drop_move_rotate_and_remove_are_each_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    let device = novo_layout::model::candidates(project(&d), cab)[0].id;

    let l = layout_of(&d, cab);
    d.app
        .cabinet_ask(Ask::Drop(device, 100.0, 120.0), Some(cab), l, &[]);
    let placed = |d: &Drive| {
        let l = novo_layout::model::load(d.app.backend.document().unwrap().project(), cab).unwrap();
        l.placements.first().map(|p| (p.x, p.y, p.rotated))
    };
    assert_eq!(placed(&d), Some((100.0, 120.0, false)));

    let l = layout_of(&d, cab);
    d.app
        .cabinet_ask(Ask::Rotate(vec![device]), Some(cab), l, &[]);
    assert_eq!(placed(&d), Some((100.0, 120.0, true)));

    // Moving keeps the turn.
    let l = layout_of(&d, cab);
    d.app
        .cabinet_ask(Ask::Drop(device, 200.0, 40.0), Some(cab), l, &[]);
    assert_eq!(placed(&d), Some((200.0, 40.0, true)));

    let l = layout_of(&d, cab);
    d.app
        .cabinet_ask(Ask::Remove(vec![device]), Some(cab), l, &[]);
    assert_eq!(placed(&d), None);
    d.app.run(&ctx, Action::Undo);
    assert_eq!(placed(&d), Some((200.0, 40.0, true)));
}

#[test]
fn picking_a_page_leaves_the_cabinet() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page = d.app.current_page.expect("a page is shown");
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    assert!(d.app.cabinet_open);
    d.app.show_page(page);
    assert!(!d.app.cabinet_open);
}

#[test]
fn a_ghost_is_drawn_but_never_stored() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    let device = novo_layout::model::candidates(project(&d), cab)[0].id;
    d.app.cabinet.ghost = Some((device, 100.0, 100.0));
    d.frame();
    d.frame();
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);
    // Without a hovering payload the ghost is cleared again.
    assert!(d.app.cabinet.ghost.is_none());
}

#[test]
fn the_table_face_toggles_and_problems_are_in_words() {
    use novo_layout::state::CabinetLayout;
    use novo_layout::{Plate, Size};

    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app.run_command(&ctx, "view.cabinet_table", None);
    assert!(d.app.cabinet_open && d.app.cabinet.table);
    d.frame();
    d.app.run_command(&ctx, "view.cabinet_table", None);
    assert!(d.app.cabinet_open && !d.app.cabinet.table);
    d.frame();

    // Rotate and delete from the palette go to the Cabinet, not the page.
    d.app.run_command(&ctx, "edit.delete", None);
    assert_eq!(d.app.cabinet.keys, vec!["edit.delete".to_owned()]);

    let mut l = CabinetLayout::new(Plate {
        width: 200.0,
        height: 200.0,
        margin: 10.0,
    });
    let size = Size::from_part(Some(45.0), Some(90.0));
    l.drop_device(1, size, 50.0, 50.0);
    l.drop_device(2, size, 60.0, 50.0);
    l.drop_device(3, Size::from_part(None, None), 0.0, 0.0);
    let name = |id: u64| format!("-K{id}");
    let a = super::cabinet::problem_lines(&l, 1, &name);
    assert_eq!(a[0], "Overlaps -K2");
    assert_eq!(a[1], "Only 5 mm from -K3");
    let c = super::cabinet::problem_lines(&l, 3, &name);
    assert!(c.iter().any(|t| t.contains("past the usable plate")));
    assert!(c.iter().any(|t| t.contains("guess")));
}

#[test]
fn a_device_too_deep_for_the_door_gets_its_own_mark_and_sentence() {
    use novo_canvas::cabinet::BoxState;
    use novo_layout::enclosure::Enclosure;
    use novo_layout::state::CabinetLayout;
    use novo_layout::{Plate, Size};
    let mut l = CabinetLayout::new(Plate {
        width: 300.0,
        height: 300.0,
        margin: 10.0,
    });
    let size = Size::from_part(Some(45.0), Some(90.0));
    l.drop_device(1, size, 20.0, 20.0);
    l.drop_device(2, size, 120.0, 20.0);
    l.enclosure = Some(Enclosure {
        width: 400.0,
        height: 400.0,
        depth: 200.0,
        plate_setback: 20.0,
        door: None,
        front_clearance: None,
        hinge_side_clearance: None,
    });
    l.depths.insert(1, 220.0);
    l.depths.insert(2, 80.0);
    let name = |id: u64| format!("-K{id}");
    let view = super::cabinet::cabinet_view(&l, &name);
    let state = |id| view.boxes.iter().find(|b| b.id == id).unwrap().state;
    assert_eq!(state(1), BoxState::TooDeep);
    assert_eq!(state(2), BoxState::Ok);
    let lines = super::cabinet::problem_lines(&l, 1, &name);
    assert!(lines.iter().any(|t| t.contains("40 mm too deep")));
    assert!(super::cabinet::problem_lines(&l, 2, &name).is_empty());
}

#[test]
fn keys_pressed_in_the_table_do_not_wait_for_the_plate() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    d.app.run_command(&ctx, "view.cabinet_table", None);
    d.frame();
    d.app.run_command(&ctx, "edit.delete", None);
    assert_eq!(d.app.cabinet.keys.len(), 1);
    d.frame();
    assert!(d.app.cabinet.keys.is_empty(), "the table drops the key");
    // Back on the plate, nothing is deleted by the stale key.
    d.app.run_command(&ctx, "view.cabinet_table", None);
    d.frame();
    assert!(!d.app.cabinet.table);
    assert!(d.app.cabinet.keys.is_empty());
}

#[test]
fn inspector_sets_the_plate_size_and_lists_a_device_s_problems() {
    use super::cabinet_inspector::{Facts, gather};

    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);

    // The plate card is off until asked for, so the plate gets the window.
    assert!(!d.app.cabinet.plate_card);
    d.app
        .cabinet_ask(Ask::TogglePlateCard, Some(cab), None, &[]);
    assert!(d.app.cabinet.plate_card);

    // The plate size comes from the inspector, as one undo step.
    let plate = novo_core::MountingPlate {
        width: 500.0,
        height: 700.0,
        margin: 15.0,
    };
    d.app
        .cabinet_ask(Ask::SetPlateSize(cab, plate), Some(cab), None, &[]);
    let stored = layout_of(&d, cab).unwrap();
    assert_eq!((stored.plate.width, stored.plate.height), (500.0, 700.0));
    d.app.run(&ctx, Action::Undo);
    assert_eq!(layout_of(&d, cab).unwrap().plate.width, 600.0);

    // Two devices on top of each other: each names the other.
    let ids: Vec<u64> = novo_layout::model::candidates(project(&d), cab)
        .iter()
        .map(|c| c.id)
        .collect();
    for id in &ids[..2] {
        let l = layout_of(&d, cab);
        d.app
            .cabinet_ask(Ask::Drop(*id, 100.0, 100.0), Some(cab), l, &[]);
    }
    let stored = layout_of(&d, cab).unwrap();
    let Facts::Devices(facts) = gather(project(&d), cab, &stored, &stored, &ids[..1], &|id| {
        id.to_string()
    }) else {
        panic!("a device is selected");
    };
    assert_eq!(facts.len(), 1);
    assert!(facts[0].problems.iter().any(|p| p.starts_with("Overlaps")));
    assert!(facts[0].part.as_deref().unwrap().starts_with("ACME"));
    assert!(matches!(
        gather(project(&d), cab, &stored, &stored, &[], &|id| id
            .to_string()),
        Facts::Plate { placed: 2, .. }
    ));
}

#[test]
fn the_enclosure_is_set_in_one_undo_step_and_drawn_with_its_checks() {
    use novo_layout::enclosure::{Door, Enclosure, Hinge};
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(&d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    assert!(layout_of(&d, cab).unwrap().enclosure.is_none());

    let e = Enclosure {
        width: 650.0,
        height: 850.0,
        depth: 250.0,
        plate_setback: 20.0,
        door: Some(Door {
            hinge: Hinge::Right,
            max_angle: 110.0,
            inner_depth: 0.0,
        }),
        front_clearance: Some(400.0),
        hinge_side_clearance: None,
    };
    d.app
        .cabinet_ask(Ask::SetEnclosure(e), Some(cab), None, &[]);
    // Drawing the card with the form, the checks and the top view works.
    d.app.cabinet.plate_card = true;
    d.frame();
    d.frame();
    let layout = layout_of(&d, cab).unwrap();
    assert_eq!(layout.enclosure, Some(e));
    // 650 mm door at 110 degrees: 650 in front, 400 free is 250 short.
    let report = layout.door_report().unwrap();
    assert!((report.front_short.unwrap() - 250.0).abs() < 1e-6);
    let lines = super::cabinet_door::sentences(&e, &report, &|id| format!("D{id}"));
    assert!(
        lines
            .iter()
            .any(|(bad, t)| *bad && t.contains("250 mm short"))
    );

    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert!(layout_of(&d, cab).unwrap().enclosure.is_none());
}

/// Y of each device's top edge on the plate, by tag text.
fn plate_y(d: &Drive, cab: novo_core::StructureId, tag: &str) -> f64 {
    let id = project(d)
        .all::<novo_core::Device>()
        .find(|dev| dev.tag.to_string() == tag)
        .unwrap()
        .id
        .raw();
    layout_of(d, cab)
        .unwrap()
        .placements
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("{tag} is placed"))
        .y
}

fn place_all(d: &mut Drive, cab: novo_core::StructureId) {
    let layout = novo_layout::model::load(project(d), cab);
    let cands = novo_layout::model::candidates(project(d), cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();
}

fn open_cabinet(d: &mut Drive) -> novo_core::StructureId {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_cabinet(d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    cab
}

#[test]
fn place_all_follows_the_place_rules_and_they_are_kept_in_the_users_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let file = dir.path().join("place-rules.txt");
    let cab = open_cabinet(&mut d);
    d.app.cabinet.rules.path = Some(file.clone());

    // The standard rules put protection (Q) above relays (K).
    place_all(&mut d, cab);
    assert!(plate_y(&d, cab, "=A1+CAB1-Q1") < plate_y(&d, cab, "=A1+CAB1-K1"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    d.frame();

    // The user turns it around; the file is written and the next Place all
    // follows it.
    let rules = novo_layout::rules::RuleSet::parse(
        "rule K zone=top rail=shared\nrule Q zone=bottom rail=own\nrule * zone=middle rail=shared\n",
    )
    .unwrap();
    d.app
        .cabinet_ask(Ask::SetPlaceRules(rules.clone()), Some(cab), None, &[]);
    assert_eq!(
        novo_layout::rules::RuleSet::parse(&std::fs::read_to_string(&file).unwrap()).unwrap(),
        rules
    );
    place_all(&mut d, cab);
    assert!(plate_y(&d, cab, "=A1+CAB1-K1") < plate_y(&d, cab, "=A1+CAB1-Q1"));

    // A fresh start reads the file back; Reset removes it.
    let (state, warning) = super::cabinet_rules::RulesState::load(file.clone());
    assert_eq!(warning, None);
    assert_eq!(state.rules, rules);
    d.app
        .cabinet_ask(Ask::ResetPlaceRules, Some(cab), None, &[]);
    assert!(!file.exists());
    assert_eq!(
        d.app.cabinet.rules.rules,
        novo_layout::rules::RuleSet::standard()
    );
}

#[test]
fn a_place_rules_file_with_a_mistake_is_named_and_not_used() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("place-rules.txt");
    std::fs::write(&file, "gap 10\nrule Q zone=sideways\n").unwrap();
    let (state, warning) = super::cabinet_rules::RulesState::load(file);
    let warning = warning.expect("a warning");
    assert!(warning.contains("line 2"), "{warning}");
    assert_eq!(state.rules, novo_layout::rules::RuleSet::standard());
}
