//! Door swing hits of devices put on the door, as problems (PANEL-B3).

use novo_checks::{Problem, ProblemKind, Severity, check};
use novo_core::*;
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::{place_device, set_enclosure, set_plate};

fn swing(problems: &[Problem]) -> Vec<&Problem> {
    problems
        .iter()
        .filter(|p| p.kind == ProblemKind::DoorSwing)
        .collect()
}

/// A 600 x 800 x 250 cabinet with a left-hinged door and a 40 mm device -H1
/// placed at `x` on the plate and then put on the door.
fn cabinet(x: f64, on_door: bool) -> (Project, u64) {
    let mut p = Project::new("Swing");
    let (h1, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1")?, "Cabinet", true)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 560.0,
                    height: 760.0,
                    margin: 10.0,
                },
            )?;
            set_enclosure(
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
            let part = tx.add_part("ACME", "P1")?;
            for key in ["width_mm", "height_mm", "depth_mm"] {
                tx.set_property(part, key, Some(Value::Float(40.0)))?;
            }
            let h1 = tx.add_device(DeviceTag::parse("=A1+CAB1-H1")?, "")?;
            tx.assign_part(h1, part)?;
            place_device(tx, cab, h1.raw(), x, 100.0, false)?;
            Ok(h1.raw())
        })
        .unwrap();
    if on_door {
        let id = p
            .all::<CabinetPlacement>()
            .find(|c| c.device.raw() == h1)
            .unwrap()
            .id;
        p.transact("surface", |tx| {
            tx.set_property(id, "surface", Some(Value::Text("door".into())))
        })
        .unwrap();
    }
    (p, h1)
}

#[test]
fn a_door_device_that_hits_the_strike_wall_is_a_swing_problem_on_that_device() {
    let (p, h1) = cabinet(559.5, true);
    let problems = check(&p);
    let found = swing(&problems);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].severity, Severity::Warning);
    assert!(
        found[0].message.contains("H1") && found[0].message.contains("door"),
        "{}",
        found[0].message
    );
    assert_eq!(
        found[0].at,
        vec![AnyId::Device(DeviceId::from(h1))],
        "the problem points at the device so the Issues list can jump to it"
    );
}

#[test]
fn a_device_on_the_plate_or_clear_of_the_walls_gives_no_swing_problem() {
    let (p, _) = cabinet(559.5, false);
    assert!(
        swing(&check(&p)).is_empty(),
        "on the plate it is no door device"
    );
    let (p, _) = cabinet(100.0, true);
    assert!(swing(&check(&p)).is_empty(), "mid-door it clears the walls");
}
