//! The EMC check window: it opens from the Check menu command, turning the
//! check on puts the findings in the Issues list, and an Apply is one undo step.

use novo_core::*;
use novo_emc::Fix;

use super::gate1_tests::Drive;
use crate::actions::Action;

fn tag(t: &str) -> DeviceTag {
    DeviceTag::parse(t).unwrap()
}

/// A drive and a motor, an analog module and a transmitter: both wires run
/// the length of one duct of a 800 x 400 plate.
fn cabinet(d: &mut Drive) -> DeviceId {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let (drive, _) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            let page = tx.add_page("1")?;
            let cab = tx.add_structure(tag("+CAB"), "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 400.0,
                    margin: 10.0,
                }),
            )?;
            let duct = tx.add_rail(cab, 20.0, 300.0, 760.0, RailKind::Duct)?;
            tx.set_rail_size(duct, 760.0, 60.0, 60.0)?;
            let mut place = |t: &str, key: &str, x: f64| -> Result<(DeviceId, SymbolId)> {
                let dev = tx.add_device(tag(t), "")?;
                tx.set_property(dev, "width_mm", Some(Value::Float(60.0)))?;
                tx.set_property(dev, "height_mm", Some(Value::Float(80.0)))?;
                tx.place_device(dev, cab, Point::new(x, 190.0), false, None)?;
                let s = tx.place_symbol(
                    page,
                    key,
                    Placement::at(Point::new(x, 10.0)),
                    vec![Pin::new("U", 0.0, 0.0), Pin::new("1", 8.0, 0.0)],
                    SymbolRole::Device {
                        device: dev,
                        main: true,
                    },
                )?;
                Ok((dev, s))
            };
            let (drive, s_drive) = place("-T1", "iec/drives/frequency_converter", 40.0)?;
            let (_, s_motor) = place("-M1", "iec/motors/motor_3ph", 700.0)?;
            let (_, s_plc) = place("-A1", "iec/plc/analog_input", 300.0)?;
            let (_, s_b) = place("-B1", "iec/sensors/transmitter", 640.0)?;
            tx.connect(Endpoint::pin(s_drive, 0), Endpoint::pin(s_motor, 0))?;
            tx.connect(Endpoint::pin(s_plc, 1), Endpoint::pin(s_b, 1))?;
            Ok(drive)
        })
        .unwrap();
    drive
}

#[test]
fn the_check_menu_opens_the_window_and_the_findings_join_the_issues_list() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    cabinet(&mut d);
    let ctx = d.ctx.clone();
    // Nothing from EMC until the check is on.
    d.frame();
    assert!(
        d.app
            .shown_problems()
            .iter()
            .all(|p| p.kind != novo_checks::ProblemKind::Emc)
    );
    d.app.run_command(&ctx, "check.emc", None);
    d.frame();
    d.frame();
    assert!(d.app.emc.open, "Check > EMC check opens the window");
    d.app.emc_turn(true);
    d.frame();
    d.frame();
    let emc: Vec<_> = d
        .app
        .shown_problems()
        .iter()
        .filter(|p| p.kind == novo_checks::ProblemKind::Emc)
        .cloned()
        .collect();
    assert!(
        emc.iter().any(|p| p.rule == Some("EMC-01")),
        "motor and analog wires share a duct: {emc:#?}"
    );
    // The Check menu lists it.
    let menu: Vec<&str> = novo_workflow::menus::entries("Check")
        .iter()
        .map(|(info, _)| info.id)
        .collect();
    assert!(menu.contains(&"check.emc"), "{menu:?}");
}

#[test]
fn applying_a_suggestion_is_one_undo_step_and_undo_restores_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let drive = cabinet(&mut d);
    d.frame();
    let devices = |d: &Drive| {
        d.app
            .backend
            .document()
            .unwrap()
            .project()
            .devices()
            .count()
    };
    let before = devices(&d);
    let depth = d.app.backend.document().unwrap().undo_depth();
    d.app.emc_apply(&Fix::AddFilter { drive: drive.raw() });
    d.frame();
    assert_eq!(devices(&d), before + 1);
    assert_eq!(d.app.backend.document().unwrap().undo_depth(), depth + 1);
    assert!(
        d.app
            .emc
            .done_text()
            .is_some_and(|t| t.contains("Filter -Z1")),
        "{:?}",
        d.app.emc.done_text()
    );
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(devices(&d), before);
}

#[test]
fn stating_a_current_is_one_undo_step_and_brings_the_field_estimate() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    cabinet(&mut d);
    d.frame();
    let report =
        |d: &Drive| novo_emc::project::analyse_project(d.app.backend.document().unwrap().project());
    let wire = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .wires()
        .next()
        .unwrap()
        .id;
    let st = |r: &novo_emc::Report| r.rules.iter().find(|x| x.rule == "EMC-19").unwrap().status;
    assert_eq!(st(&report(&d)), novo_emc::Status::NotEvaluated);
    let depth = d.app.backend.document().unwrap().undo_depth();
    d.app.emc_set_current(wire.raw(), Some(12.0));
    d.frame();
    assert_eq!(d.app.backend.document().unwrap().undo_depth(), depth + 1);
    let r = report(&d);
    let field = r.field.as_ref().unwrap();
    assert_eq!(field.currents.len(), 1);
    assert_eq!(field.currents[0].amps, 12.0);
    assert!(field.currents[0].declared);
    // A setting is one step too, and clearing it restores the default.
    d.app.emc_set_setting("field_hz", Some(Value::Float(150.0)));
    d.frame();
    assert_eq!(report(&d).field.unwrap().hz, 150.0);
    d.app.emc_set_setting("field_hz", None);
    d.frame();
    assert_eq!(report(&d).field.unwrap().hz, 50.0);
    // The window draws with the section in it.
    d.app.run_command(&d.ctx.clone(), "check.emc", None);
    d.frame();
    d.frame();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    d.app.run(&ctx, Action::Undo);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(st(&report(&d)), novo_emc::Status::NotEvaluated);
}

#[test]
fn a_preview_changes_nothing_and_apply_is_then_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    cabinet(&mut d);
    d.frame();
    let cab = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .structures()
        .next()
        .unwrap()
        .id;
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("painted", |tx| {
            tx.set_property(cab, "emc.finish", Some(Value::Text("painted".into())))
        })
        .unwrap();
    let fix = Fix::BarePlate { plate: cab.raw() };
    let revision = d.app.backend.document().unwrap().project().revision();
    let depth = d.app.backend.document().unwrap().undo_depth();
    d.app.emc_preview(&fix);
    d.frame();
    let preview = d.app.emc.previewed().expect("a preview").as_ref().unwrap();
    assert!(
        preview.cleared.iter().any(|(r, _)| *r == "EMC-11"),
        "{preview:#?}"
    );
    assert_eq!(
        d.app.backend.document().unwrap().project().revision(),
        revision
    );
    assert_eq!(d.app.backend.document().unwrap().undo_depth(), depth);
    // The window draws the preview card.
    d.app.run_command(&d.ctx.clone(), "check.emc", None);
    d.frame();
    d.frame();
    d.app.emc_apply(&fix);
    d.frame();
    assert_eq!(d.app.backend.document().unwrap().undo_depth(), depth + 1);
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    d.frame();
    let finish = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .get::<Structure>(cab)
        .unwrap()
        .properties
        .get("emc.finish")
        .cloned();
    assert_eq!(finish, Some(Value::Text("painted".into())));
}

#[test]
fn a_previewed_fix_reaches_both_views_and_the_banner_cancels_or_applies_it() {
    use crate::app::emc_view::WhatIfAction;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    cabinet(&mut d);
    d.frame();
    let cab = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .structures()
        .next()
        .unwrap()
        .id;
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("painted", |tx| {
            tx.set_property(cab, "emc.finish", Some(Value::Text("painted".into())))
        })
        .unwrap();
    let fix = Fix::BarePlate { plate: cab.raw() };
    let ctx = d.ctx.clone();
    d.app.emc_preview(&fix);
    d.app.emc_whatif_sync(&ctx);
    for view in [&d.app.view3d.emc, &d.app.cabinet.emc] {
        let w = view.whatif.as_ref().expect("both views get the preview");
        assert!(
            w.after && w.summary.ends_with("clears EMC-11"),
            "{}",
            w.summary
        );
    }
    // The picture of the copy has a better bond than the real project's.
    let bond = |p: &Project| {
        crate::app::emc_adapter::from_project(p, cab, [800.0, 400.0], &Default::default())
            .map(|pic| pic.bonds.iter().map(|b| b.quality).collect::<Vec<_>>())
    };
    let w = d.app.view3d.emc.whatif.as_ref().unwrap().project.clone();
    let real = d.app.backend.document().unwrap().project().clone();
    assert_ne!(bond(&real), bond(&w), "before and after differ");
    // Before / After switch.
    d.app.view3d.emc.whatif_action = Some(WhatIfAction::Show { after: false });
    d.app.emc_whatif_sync(&ctx);
    assert!(!d.app.cabinet.emc.whatif.as_ref().unwrap().after);
    // A change to the project drops the preview.
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("edit", |tx| tx.set_property(cab, "x", Some(Value::Int(1))))
        .unwrap();
    d.app.emc_whatif_sync(&ctx);
    assert!(d.app.view3d.emc.whatif.is_none() && d.app.emc.previewed().is_none());
    // Apply from the banner is one undo step.
    d.app.emc_preview(&fix);
    d.app.emc_whatif_sync(&ctx);
    let depth = d.app.backend.document().unwrap().undo_depth();
    d.app.cabinet.emc.whatif_action = Some(WhatIfAction::Apply);
    d.app.emc_whatif_sync(&ctx);
    assert_eq!(d.app.backend.document().unwrap().undo_depth(), depth + 1);
    d.app.emc_whatif_sync(&ctx);
    assert!(d.app.view3d.emc.whatif.is_none());
}

#[test]
fn the_emc_record_lists_findings_with_what_to_do_and_writes_both_files() {
    use super::emc_record::EmcRecordFormat;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    cabinet(&mut d);
    d.frame();
    let out = d
        .app
        .emc_record_output("Press", true)
        .expect("a project is open");
    let section = |key: &str| out.sections.iter().find(|s| s.key == key).unwrap();
    let text = |key: &str| -> String {
        section(key)
            .rows
            .iter()
            .flat_map(|r| r.cells.iter().map(|c| format!("{c:?}")))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let findings = text("findings");
    assert!(findings.contains("EMC-01"), "{findings}");
    assert!(
        findings.contains("Route"),
        "the fix hint is the bench step: {findings}"
    );
    assert!(text("not_checked").contains("Compliance with any standard"));
    assert!(text("rules").contains("EMC-12"));
    let summary = text("summary");
    assert!(
        summary.contains("Rules in the check") && summary.contains("KoperCAD"),
        "{summary}"
    );
    // A spreadsheet gets one row per finding, the rule in every row.
    let sheet = d.app.emc_record_output("Press", false).unwrap();
    let rows = |o: &novo_reports::ReportOutput| {
        o.sections
            .iter()
            .find(|s| s.key == "findings")
            .unwrap()
            .rows
            .len()
    };
    let first_cells: Vec<String> = sheet
        .sections
        .iter()
        .find(|s| s.key == "findings")
        .unwrap()
        .rows
        .iter()
        .map(|r| format!("{:?}", r.cells[0]))
        .collect();
    assert!(
        first_cells.iter().all(|c| c.contains("EMC-")),
        "the rule is in every row: {first_cells:?}"
    );
    assert!(
        rows(&out) > rows(&sheet),
        "the printed page wraps: {} against {}",
        rows(&out),
        rows(&sheet)
    );
    for (format, name, magic) in [
        (EmcRecordFormat::Pdf, "a.pdf", &b"%PDF"[..]),
        (EmcRecordFormat::Excel, "a.xlsx", &b"PK"[..]),
    ] {
        let path = dir.path().join(name);
        assert_eq!(d.app.write_emc_record(format, &path), Some(path.clone()));
        assert!(std::fs::read(&path).unwrap().starts_with(magic), "{name}");
    }
}
