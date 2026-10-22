//! The adapter on real projects: classes read from the drawing, routes from
//! the router, fixes applied as commands.

use novo_core::*;
use novo_emc::project::{analyse_project, apply, input_of};
use novo_emc::{EmcClass, Fix, Origin, Subject};

fn tag(t: &str) -> DeviceTag {
    DeviceTag::parse(t).unwrap()
}

struct Cab {
    project: Project,
    drive: DeviceId,
    plc: DeviceId,
    motor_wire: WireId,
    analog_wire: WireId,
    duct: RailId,
    cab: StructureId,
}

/// A 800 x 400 plate with a horizontal duct at y = 300. A drive at the left
/// feeds a motor at the right, and an analog module at the left reads a
/// transmitter at the right: both wires run the length of the duct.
fn cabinet() -> Cab {
    let mut project = Project::new("EMC");
    let (ids, _) = project
        .transact("build", |tx| {
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
            let mut place = |t: &str,
                             key: &str,
                             pins: &[&str],
                             x: f64,
                             y: f64|
             -> Result<(DeviceId, SymbolId)> {
                let d = tx.add_device(tag(t), "")?;
                tx.set_property(d, "width_mm", Some(Value::Float(60.0)))?;
                tx.set_property(d, "height_mm", Some(Value::Float(80.0)))?;
                tx.place_device(d, cab, Point::new(x, y), false, None)?;
                let s = tx.place_symbol(
                    page,
                    key,
                    Placement::at(Point::new(x, 10.0)),
                    pins.iter()
                        .enumerate()
                        .map(|(i, p)| Pin::new(*p, 8.0 * i as f64, 0.0))
                        .collect(),
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )?;
                Ok((d, s))
            };
            let (drive, s_drive) = place(
                "-T1",
                "iec/drives/frequency_converter",
                &["L1", "U"],
                40.0,
                190.0,
            )?;
            let (_m, s_motor) = place("-M1", "iec/motors/motor_3ph", &["U1"], 700.0, 190.0)?;
            let (plc, s_plc) = place("-A1", "iec/plc/analog_input", &["AI1"], 100.0, 190.0)?;
            let (_b, s_b) = place("-B1", "iec/sensors/transmitter", &["1"], 640.0, 190.0)?;
            let motor_wire = tx.connect(Endpoint::pin(s_drive, 1), Endpoint::pin(s_motor, 0))?;
            let analog_wire = tx.connect(Endpoint::pin(s_plc, 0), Endpoint::pin(s_b, 0))?;
            Ok((drive, plc, motor_wire, analog_wire, duct, cab))
        })
        .unwrap();
    Cab {
        project,
        drive: ids.0,
        plc: ids.1,
        motor_wire: ids.2,
        analog_wire: ids.3,
        duct: ids.4,
        cab: ids.5,
    }
}

#[test]
fn classes_come_from_the_drawing_and_say_why() {
    let c = cabinet();
    let input = input_of(&c.project, &c.project.netlist());
    let motor = input
        .runs
        .iter()
        .find(|r| r.id == c.motor_wire.raw())
        .unwrap();
    assert_eq!(motor.class, EmcClass::Motor);
    assert_eq!(motor.origin, Origin::Derived);
    assert!(
        motor.class_reason.contains("motor output"),
        "{}",
        motor.class_reason
    );
    assert_eq!(motor.drive, Some(c.drive.raw()));
    let analog = input
        .runs
        .iter()
        .find(|r| r.id == c.analog_wire.raw())
        .unwrap();
    assert_eq!(analog.class, EmcClass::Analog);
    // The routes lie in the duct.
    assert!(
        !motor.ducts.is_empty() && motor.ducts == analog.ducts,
        "{:?} {:?}",
        motor.ducts,
        analog.ducts
    );
    assert!(motor.path.len() >= 2);
}

#[test]
fn a_declared_class_wins_and_is_marked_so() {
    let mut c = cabinet();
    c.project
        .transact("declare", |tx| {
            tx.set_property(
                c.analog_wire,
                "emc.class",
                Some(Value::Text("control".into())),
            )
        })
        .unwrap();
    let input = input_of(&c.project, &c.project.netlist());
    let analog = input
        .runs
        .iter()
        .find(|r| r.id == c.analog_wire.raw())
        .unwrap();
    assert_eq!(
        (analog.class, analog.origin),
        (EmcClass::Control, Origin::Declared)
    );
}

#[test]
fn motor_and_analog_wires_in_one_duct_are_found_and_scored() {
    let c = cabinet();
    let report = analyse_project(&c.project);
    let f: Vec<_> = report.of_rule("EMC-01").collect();
    assert_eq!(f.len(), 1, "{:#?}", report.findings);
    assert!(f[0].subjects.contains(&Subject::Run(c.analog_wire.raw())));
    assert!(f[0].message.contains("same duct"), "{}", f[0].message);
    let score = report.score_of(Subject::Run(c.analog_wire.raw())).unwrap();
    assert!(score.points >= 50, "{score:#?}");
    // The drive has no filter and its motor wire is not a shielded cable.
    assert_eq!(report.of_rule("EMC-07").count(), 1);
    assert_eq!(report.of_rule("EMC-04").count(), 1);
    // The analog module sits 40 mm from the drive: too close.
    assert_eq!(
        report.of_rule("EMC-17").count(),
        1,
        "{:#?}",
        report.findings
    );
    assert!(report.nets.iter().any(|(_, s)| s.points == score.points));
}

#[test]
fn fixes_apply_as_commands_and_clear_their_findings() {
    let mut c = cabinet();
    // Put the motor wire in a cable first, then fix it by the suggestions.
    let (cable, _) = c
        .project
        .transact("cable", |tx| {
            let cable = tx.add_cable(tag("-W1"), vec![CableCore::new("BK")])?;
            tx.assign_core(cable, 0, Some(c.motor_wire))?;
            Ok(cable)
        })
        .unwrap();
    let before = analyse_project(&c.project);
    let fixes: Vec<Fix> = before
        .of_rule("EMC-04")
        .flat_map(|f| f.fixes.clone())
        .collect();
    assert_eq!(fixes, vec![Fix::AddShield { cable: cable.raw() }]);
    c.project
        .transact("shield", |tx| apply(tx, &fixes[0]).map(|_| ()))
        .unwrap();
    let mid = analyse_project(&c.project);
    // Shielded but its ends are not stated: an info, with the all-round fix.
    let f = mid.of_rule("EMC-04").next().unwrap();
    assert_eq!(f.severity, novo_emc::Severity::Info);
    c.project
        .transact("ends", |tx| apply(tx, &f.fixes[0].clone()).map(|_| ()))
        .unwrap();
    assert_eq!(analyse_project(&c.project).of_rule("EMC-04").count(), 0);

    // A filter for the drive: the device is created with its role and target.
    let add = Fix::AddFilter {
        drive: c.drive.raw(),
    };
    c.project
        .transact("filter", |tx| apply(tx, &add).map(|_| ()))
        .unwrap();
    let filter = c
        .project
        .devices()
        .find(|d| d.tag.to_string().contains("Z1"))
        .expect("a filter device");
    match filter.properties.get("emc.filter_for") {
        Some(Value::Text(t)) => assert!(t.contains("T1"), "{t}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        filter.properties.get("emc.role"),
        Some(&Value::Text("filter".into()))
    );

    // Fixes done by hand are refused rather than faked.
    let hand = Fix::BarePlate { plate: c.cab.raw() };
    assert!(
        c.project
            .transact("hand", |tx| apply(tx, &hand).map(|_| ()))
            .is_err()
    );
    let _ = (c.duct, c.plc);
}

#[test]
fn each_fix_is_one_undo_step() {
    let c = cabinet();
    let mut doc = Document::new(c.project);
    let devices = doc.project().devices().count();
    let depth = doc.undo_depth();
    let add = Fix::AddFilter {
        drive: c.drive.raw(),
    };
    doc.transact("EMC fix", |tx| apply(tx, &add).map(|_| ()))
        .unwrap();
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(doc.project().devices().count(), devices + 1);
    doc.undo().unwrap();
    assert_eq!(doc.project().devices().count(), devices);
    // A shield on a cable is one step as well.
    let wire = c.motor_wire;
    let (cable, _) = (
        doc.transact("cable", |tx| {
            let cable = tx.add_cable(tag("-W9"), vec![CableCore::new("BK")])?;
            tx.assign_core(cable, 0, Some(wire))?;
            Ok(cable)
        })
        .unwrap()
        .0,
        (),
    );
    let before = doc.undo_depth();
    doc.transact("EMC fix", |tx| {
        apply(tx, &Fix::AddShield { cable: cable.raw() }).map(|_| ())
    })
    .unwrap();
    assert_eq!(doc.undo_depth(), before + 1);
    assert!(
        doc.project()
            .cable(cable)
            .unwrap()
            .cores
            .iter()
            .any(|k| k.shield)
    );
    doc.undo().unwrap();
    assert!(
        !doc.project()
            .cable(cable)
            .unwrap()
            .cores
            .iter()
            .any(|k| k.shield)
    );
}
