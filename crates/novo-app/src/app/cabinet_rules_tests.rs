//! Place rules in the Cabinet: Place all follows them, the inspector names the
//! rule, the cut list trims rails as one undo step.

use novo_core::{CabinetPlacement, DeviceTag, Project, Rail, RailKind, Value};

use super::cabinet::Ask;
use super::cabinet_inspector::{DeviceFacts, Facts};
use super::cabinet_rules::{self, RULES_FILE};
use super::gate1_tests::Drive;
use crate::actions::Action;

fn project(d: &Drive) -> &Project {
    d.app.backend.document().unwrap().project()
}

/// A cabinet with a relay K1, a breaker Q1 and a drive T1 (with its own room).
fn cabinet(d: &mut Drive) -> novo_core::StructureId {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            for (tag, w, h) in [
                ("=A1+CAB1-K1", 45.0, 90.0),
                ("=A1+CAB1-Q1", 18.0, 90.0),
                ("=A1+CAB1-T1", 80.0, 150.0),
            ] {
                let part = tx.add_part("ACME", tag)?;
                tx.set_property(part, "width_mm", Some(Value::Float(w)))?;
                tx.set_property(part, "height_mm", Some(Value::Float(h)))?;
                let device = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(device, part)?;
            }
            Ok(())
        })
        .unwrap();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = project(d).structures().find(|s| s.enclosure).unwrap().id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    d.frame();
    cab
}

fn place_all(d: &mut Drive, cab: novo_core::StructureId) {
    let layout = novo_layout::model::load(project(d), cab);
    let cands = novo_layout::model::candidates(project(d), cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.frame();
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();
}

#[test]
fn place_all_follows_the_rules_with_ducts_and_says_why_in_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let cab = cabinet(&mut d);
    let layout = novo_layout::model::load(project(&d), cab);
    let cands = novo_layout::model::candidates(project(&d), cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.frame();
    // The explanation exists as soon as the plan does.
    let why = d.app.cabinet.rules.explain.clone().expect("an explanation");
    assert_eq!(why.decisions.len(), 3);
    assert!(
        why.describe(why.decisions[0].id, &d.app.cabinet.rules.rules)
            .starts_with("Rule `")
    );
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();

    // Q1 (power) is above K1 (signal); the drive T1 is at the bottom.
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    assert_eq!(layout.placements.len(), 3);
    let y_of = |tag: &str| {
        let id = project(&d)
            .all::<novo_core::Device>()
            .find(|dev| dev.tag.to_string().ends_with(tag))
            .unwrap()
            .id
            .raw();
        layout.placements.iter().find(|p| p.id == id).unwrap().y
    };
    assert!(y_of("-Q1") < y_of("-K1") && y_of("-K1") < y_of("-T1"));

    // A power duct and a signal duct came along.
    let ducts: Vec<&Rail> = project(&d)
        .all::<Rail>()
        .filter(|r| r.kind == RailKind::Duct)
        .collect();
    assert!(ducts.len() >= 2);
    let classes: Vec<_> = novo_layout::model::ducts(project(&d), cab)
        .iter()
        .map(|duct| duct.class.as_str())
        .collect();
    assert!(classes.contains(&"power") && classes.contains(&"signal"));

    // And none of the rules is broken.
    let judged = cabinet_rules::judge(project(&d), cab, &d.app.cabinet.rules.rules);
    assert!(judged.violations.is_empty(), "{:?}", judged.violations);

    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(project(&d).all::<CabinetPlacement>().count(), 0);
    assert_eq!(project(&d).all::<Rail>().count(), 0);
}

#[test]
fn the_inspector_names_the_rule_and_the_rule_a_hand_edit_breaks() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d);
    // Put the breaker below the relay by hand.
    let id_of = |d: &Drive, tag: &str| {
        d.app
            .backend
            .document()
            .unwrap()
            .project()
            .all::<novo_core::Device>()
            .find(|dev| dev.tag.to_string().ends_with(tag))
            .unwrap()
            .id
            .raw()
    };
    let (k, q) = (id_of(&d, "-K1"), id_of(&d, "-Q1"));
    for (id, x, y) in [(k, 100.0, 100.0), (q, 100.0, 500.0)] {
        let l = novo_layout::model::load(project(&d), cab);
        d.app.cabinet_ask(Ask::Drop(id, x, y), Some(cab), l, &[]);
    }
    d.app.cabinet.rules.ensure_loaded();
    let judged = cabinet_rules::judge(project(&d), cab, &d.app.cabinet.rules.rules);
    assert_eq!(judged.violations.len(), 1);

    let mut facts = Facts::Devices(vec![DeviceFacts {
        id: q,
        tag: "-Q1".into(),
        description: String::new(),
        part: None,
        size: String::new(),
        guessed: false,
        position: String::new(),
        rotated: false,
        rail: None,
        problems: Vec::new(),
        rule: None,
        why: None,
        surface: None,
    }]);
    cabinet_rules::annotate(&mut facts, &d.app.cabinet.rules, &judged, &|id| {
        format!("-D{id}")
    });
    let Facts::Devices(devices) = facts else {
        unreachable!()
    };
    assert!(
        devices[0]
            .rule
            .as_deref()
            .unwrap()
            .starts_with("rule Q F G zone=top")
    );
    assert_eq!(devices[0].problems.len(), 1);
    assert!(
        devices[0].problems[0].contains("zone"),
        "{:?}",
        devices[0].problems
    );
}

#[test]
fn cutting_rails_to_length_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let cab = cabinet(&mut d);
    place_all(&mut d, cab);
    let rails: Vec<(u64, f64)> = project(&d)
        .all::<Rail>()
        .filter(|r| r.kind == RailKind::DinRail)
        .map(|r| (r.id.raw(), r.length))
        .collect();
    assert!(!rails.is_empty());
    let (id, before) = rails[0];

    let lists = cabinet_rules::cut_lists(project(&d), cab).unwrap();
    assert!(lists.rails.is_some());
    assert!(!lists.ducts.is_empty());

    d.app
        .cabinet_ask(Ask::SetRailLengths(vec![(id, 300.0)]), Some(cab), None, &[]);
    d.frame();
    let length = |d: &Drive| {
        project(d)
            .get::<Rail>(novo_core::RailId::from(id))
            .unwrap()
            .length
    };
    assert_eq!(length(&d), 300.0);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(length(&d), before);
}

#[test]
fn the_cut_list_names_ducts_as_the_wire_production_report_does() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d);
    place_all(&mut d, cab);
    let lists = cabinet_rules::cut_lists(project(&d), cab).unwrap();
    let names = novo_reports::duct_names(project(&d));
    assert!(!lists.ducts.is_empty() && !names.is_empty());
    assert_eq!(lists.duct_names, names);
    // Every piece in the bars is a duct rail that has a D-name.
    for (_, _, plan) in &lists.ducts {
        for bar in &plan.bars {
            let line = bar.line(&plan.stock, &|id: u64| lists.duct_names[&id].clone());
            assert!(line.contains('D') && !line.contains("duct ?"), "{line}");
        }
    }
}

#[test]
fn the_rules_file_is_read_saved_and_a_bad_one_falls_back_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(RULES_FILE);
    // No file: the standard rules, nothing to say.
    let (rules, problem) = cabinet_rules::load_rules(&path);
    assert_eq!(rules, novo_layout::rules::RuleSet::standard());
    assert!(problem.is_none());

    let mut mine = novo_layout::rules::RuleSet::standard();
    mine.separation = 80.0;
    cabinet_rules::save_rules(&mine.to_text(), &path).unwrap();
    assert_eq!(cabinet_rules::load_rules(&path).0, mine);

    std::fs::write(&path, "gap 10\nrule K zone=sideways\n").unwrap();
    let (rules, problem) = cabinet_rules::load_rules(&path);
    assert_eq!(rules, novo_layout::rules::RuleSet::standard());
    let problem = problem.unwrap();
    assert!(
        problem.contains("line 2") && problem.contains("sideways"),
        "{problem}"
    );
}

#[test]
fn place_all_keeps_clear_of_a_duct_already_on_the_plate_and_a_hand_edit_onto_it_is_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d);
    let plate = novo_layout::model::load(project(&d), cab).unwrap().plate;
    // A vertical duct down the middle of the plate.
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Duct", |tx| {
            let duct = novo_layout::plan::PlannedDuct {
                x: plate.width / 2.0,
                y: plate.margin,
                length: plate.height - 2.0 * plate.margin,
                vertical: true,
                width: 30.0,
                height: 60.0,
                class: novo_layout::rules::Class::Mixed,
            };
            novo_layout::model::apply_plan(
                tx,
                cab,
                &novo_layout::plan::Plan {
                    ducts: vec![duct],
                    ..Default::default()
                },
            )?;
            Ok(())
        })
        .unwrap();
    place_all(&mut d, cab);

    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    assert_eq!(layout.placements.len(), 3);
    assert!(!layout.ducts.is_empty());
    // The status counts nothing and the check finds nothing.
    assert!(layout.report().duct_overlaps.is_empty());
    assert!(layout.report().is_clean(), "{:?}", layout.report());

    // Dragging a device onto the duct is a problem with a clear line.
    let k1 = project(&d)
        .all::<novo_core::Device>()
        .find(|dev| dev.tag.to_string().ends_with("-K1"))
        .unwrap()
        .id
        .raw();
    let duct = layout.ducts.iter().find(|x| x.vertical).unwrap();
    let l = Some(layout.clone());
    d.app
        .cabinet_ask(Ask::Drop(k1, duct.x - 10.0, 300.0), Some(cab), l, &[]);
    d.frame();
    let layout = novo_layout::model::load(project(&d), cab).unwrap();
    assert!(!layout.report().duct_overlaps.is_empty());
    let lines = super::cabinet::problem_lines(&layout, k1, &|_| "-K1".into());
    assert!(
        lines.iter().any(|l| l.contains("under a duct")),
        "{lines:?}"
    );
}

#[test]
fn heat_aware_place_all_lowers_a_sensitive_breaker_and_the_rule_check_agrees() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let cab = cabinet(&mut d);
    // The drive T1 loses 80 W; the breaker Q1 may not see more than 45 C, so the
    // rules' top zone would put it in the plume of the drive.
    let parts: Vec<(novo_core::PartId, String)> = project(&d)
        .all::<novo_core::Part>()
        .map(|p| (p.id, p.order_number.clone()))
        .collect();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Heat data", |tx| {
            for (id, model) in &parts {
                let (loss, limit) = if model.ends_with("-T1") {
                    (80.0, 55)
                } else if model.ends_with("-Q1") {
                    (2.0, 45)
                } else {
                    (3.0, 60)
                };
                tx.set_property(*id, "power_loss_w", Some(Value::Float(loss)))?;
                tx.set_property(*id, "max_ambient_c", Some(Value::Int(limit)))?;
            }
            Ok(())
        })
        .unwrap();
    let layout = novo_layout::model::load(project(&d), cab);
    let cands = novo_layout::model::candidates(project(&d), cab);
    d.app.cabinet_ask(Ask::PlaceAll, Some(cab), layout, &cands);
    d.frame();
    let why = d.app.cabinet.rules.explain.clone().expect("an explanation");
    let heat = why.heat.as_ref().expect("the heat step ran");
    assert_eq!(heat.sources, 1);
    assert_eq!(heat.lowered, 1, "{heat:?}");
    let q1 = project(&d)
        .all::<novo_core::Device>()
        .find(|dev| dev.tag.to_string().ends_with("-Q1"))
        .unwrap()
        .id
        .raw();
    assert!(
        why.describe(q1, &d.app.cabinet.rules.rules)
            .contains("brought it below the hot zone")
            || why
                .describe(q1, &d.app.cabinet.rules.rules)
                .contains("below the hot zone")
    );
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();
    // The place rules check does not call the lowered breaker out of its zone.
    let judged = cabinet_rules::judge(project(&d), cab, &d.app.cabinet.rules.rules);
    assert!(judged.violations.is_empty(), "{:?}", judged.violations);
}

#[test]
fn the_heat_line_in_the_rule_text_is_switched_without_touching_the_rest() {
    let text = "# mine\ngap 10\nrule * zone=middle\n";
    let on = cabinet_rules::set_heat_line_for_test(text, true);
    assert!(on.contains("heat on") && on.contains("# mine") && on.contains("gap 10"));
    assert!(on.find("heat on").unwrap() < on.find("rule *").unwrap());
    let off = cabinet_rules::set_heat_line_for_test(&on, false);
    assert!(off.contains("heat off") && !off.contains("heat on"));
    assert_eq!(off.matches("heat").count(), 1);
}
