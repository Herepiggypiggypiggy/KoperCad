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
