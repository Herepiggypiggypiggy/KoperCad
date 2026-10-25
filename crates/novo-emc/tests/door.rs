//! Devices on the door, read from a project.

use novo_core::*;
use novo_emc::project::{analyse_project, apply, input_of, preview};
use novo_emc::{Fix, Role, Subject};
use novo_layout::enclosure::{Door, Enclosure, Hinge};

fn tag(t: &str) -> DeviceTag {
    DeviceTag::parse(t).unwrap()
}

#[test]
fn a_door_device_is_read_without_a_plate_position_and_the_strap_fix_applies() {
    let mut project = Project::new("Door");
    let ((cab, plc, pb), _) = project
        .transact("build", |tx| {
            let cab = tx.add_structure(tag("+CAB"), "", true)?;
            novo_layout::model::set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 560.0,
                    height: 760.0,
                    margin: 10.0,
                },
            )?;
            novo_layout::model::set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 600.0,
                    height: 800.0,
                    depth: 250.0,
                    plate_setback: 20.0,
                    door: Some(Door {
                        hinge: Hinge::Left,
                        ..Door::default()
                    }),
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            let mut put = |t: &str, role: &str, x: f64, y: f64| -> Result<DeviceId> {
                let d = tx.add_device(tag(t), "")?;
                tx.set_property(d, "width_mm", Some(Value::Float(60.0)))?;
                tx.set_property(d, "height_mm", Some(Value::Float(80.0)))?;
                if !role.is_empty() {
                    tx.set_property(d, "emc.role", Some(Value::Text(role.to_owned())))?;
                }
                novo_layout::model::place_device(tx, cab, d.raw(), x, y, false)?;
                Ok(d)
            };
            let plc = put("-A1", "sensitive", 100.0, 100.0)?;
            // Same coordinates as the PLC, but on the door.
            let pb = put("-S1", "", 100.0, 100.0)?;
            Ok((cab, plc, pb))
        })
        .unwrap();
    let id = project
        .all::<CabinetPlacement>()
        .find(|p| p.device == pb)
        .unwrap()
        .id;
    project
        .transact("door", |tx| {
            tx.set_property(
                AnyId::CabinetPlacement(id),
                "surface",
                Some(Value::Text("door".into())),
            )
        })
        .unwrap();
    let input = input_of(&project, &project.netlist());
    let s1 = input.devices.iter().find(|d| d.id == pb.raw()).unwrap();
    assert!(s1.on_door && s1.rect.is_none() && s1.role == Role::Other);
    let a1 = input.devices.iter().find(|d| d.id == plc.raw()).unwrap();
    assert!(!a1.on_door && a1.rect.is_some());
    // The strap is not stated: one info with a fix, nothing else.
    let report = analyse_project(&project);
    let f: Vec<_> = report.of_rule("EMC-12").collect();
    assert_eq!(f.len(), 1);
    let fix = Fix::BondDoor { plate: cab.raw() };
    assert_eq!(f[0].fixes, vec![fix.clone()]);
    assert!(f[0].subjects.contains(&Subject::Device(pb.raw())));
    let pv = preview(&project, &fix).unwrap();
    assert!(pv.cleared.iter().any(|(r, _)| *r == "EMC-12"), "{pv:?}");
    assert!(pv.said.contains("strap"));
    project
        .transact("fix", |tx| apply(tx, &fix).map(|_| ()))
        .unwrap();
    assert_eq!(analyse_project(&project).of_rule("EMC-12").count(), 0);
}
