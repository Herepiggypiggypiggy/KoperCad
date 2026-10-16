//! Door and depth checks as problems (DOOR-1).

use novo_checks::{Problem, ProblemKind, Severity, check};
use novo_core::*;
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::{device_depth, set_enclosure};

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn of_kind(problems: &[Problem], kind: ProblemKind) -> Vec<&Problem> {
    problems.iter().filter(|p| p.kind == kind).collect()
}

/// A 300 x 300 plate with -K1 (part depth 100), -K2 (part depth 100, device
/// override 250), -K3 (no depth); interior 300 x 300 x `depth`.
fn cabinet(enclosure: Option<Enclosure>, plate_w: f64) -> Project {
    let mut project = Project::new("Door");
    project
        .transact("build", |tx| {
            let page = tx.add_page("1")?;
            let cab = tx.add_structure(tag("+CAB1"), "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: plate_w,
                    height: 300.0,
                    margin: 10.0,
                }),
            )?;
            let depths = [Some(100.0), Some(100.0), None];
            for (i, depth) in depths.into_iter().enumerate() {
                let d = tx.add_device(tag(&format!("-K{}", i + 1)), "")?;
                tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    Placement::at(Point::new(10.0, 10.0 + 20.0 * i as f64)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )?;
                tx.place_device(
                    d,
                    cab,
                    Point::new(20.0 + 90.0 * i as f64, 20.0),
                    false,
                    None,
                )?;
                if let Some(depth) = depth {
                    let part = tx.add_part("ACME", format!("C{i}"))?;
                    tx.set_property(part, "depth_mm", Some(Value::Float(depth)))?;
                    tx.assign_part(d, part)?;
                }
                if i == 1 {
                    tx.set_property(d, "depth_mm", Some(Value::Float(250.0)))?;
                }
            }
            if let Some(e) = &enclosure {
                set_enclosure(tx, cab, e)?;
            }
            Ok(())
        })
        .unwrap();
    project
}

fn enclosure(depth: f64) -> Enclosure {
    Enclosure {
        width: 400.0,
        height: 400.0,
        depth,
        plate_setback: 0.0,
        door: Some(Door {
            hinge: Hinge::default(),
            max_angle: 120.0,
            inner_depth: 0.0,
        }),
        front_clearance: None,
        hinge_side_clearance: None,
    }
}

#[test]
fn no_enclosure_means_no_door_problems() {
    let problems = check(&cabinet(None, 300.0));
    for kind in [
        ProblemKind::DeviceTooDeep,
        ProblemKind::DoorGapTight,
        ProblemKind::DepthUnknown,
        ProblemKind::PlateTooBig,
        ProblemKind::DoorSwing,
    ] {
        assert!(of_kind(&problems, kind).is_empty(), "{kind:?}");
    }
}

#[test]
fn a_device_deeper_than_the_room_is_an_error_at_the_device() {
    // Usable 200: -K2 (250 by device override) is 50 too deep.
    let problems = check(&cabinet(Some(enclosure(200.0)), 300.0));
    let found = of_kind(&problems, ProblemKind::DeviceTooDeep);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind.severity(), Severity::Error);
    assert!(found[0].message.contains("-K2") && found[0].message.contains("50 mm"));
    assert!(matches!(found[0].at[..], [AnyId::Device(_)]));
}

#[test]
fn a_device_close_to_the_door_is_a_warning() {
    // Usable 105: -K1 leaves 5 mm; -K2 (250) is too deep.
    let problems = check(&cabinet(Some(enclosure(105.0)), 300.0));
    let found = of_kind(&problems, ProblemKind::DoorGapTight);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind.severity(), Severity::Warning);
}

#[test]
fn a_device_without_depth_is_info() {
    let problems = check(&cabinet(Some(enclosure(300.0)), 300.0));
    let found = of_kind(&problems, ProblemKind::DepthUnknown);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind.severity(), Severity::Info);
    assert!(found[0].message.contains("-K3"));
}

#[test]
fn a_plate_larger_than_the_interior_is_an_error_at_the_structure() {
    let mut e = enclosure(300.0);
    e.width = 250.0;
    let problems = check(&cabinet(Some(e), 300.0));
    let found = of_kind(&problems, ProblemKind::PlateTooBig);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind.severity(), Severity::Error);
    assert!(matches!(found[0].at[..], [AnyId::Structure(_)]));
}

#[test]
fn a_door_that_needs_more_room_than_given_is_a_warning() {
    let mut e = enclosure(300.0);
    e.front_clearance = Some(10.0);
    e.hinge_side_clearance = Some(0.0);
    let problems = check(&cabinet(Some(e), 300.0));
    let found = of_kind(&problems, ProblemKind::DoorSwing);
    assert_eq!(found.len(), 2, "front and hinge side");
    assert!(found.iter().all(|p| p.kind.severity() == Severity::Warning));
}

#[test]
fn the_device_property_overrides_the_part_depth() {
    let project = cabinet(None, 300.0);
    let depth = |tag_text: &str| {
        let d = project.devices().find(|d| d.tag == tag(tag_text)).unwrap();
        device_depth(&project, d)
    };
    assert_eq!(depth("-K1"), Some(100.0));
    assert_eq!(depth("-K2"), Some(250.0));
    assert_eq!(depth("-K3"), None);
}

#[test]
fn keys_and_labels_are_unique() {
    let keys: std::collections::HashSet<_> = ProblemKind::ALL.iter().map(|k| k.key()).collect();
    assert_eq!(keys.len(), ProblemKind::ALL.len());
}
