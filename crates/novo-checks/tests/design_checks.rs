//! Voltage drop, short-circuit rating, unplaced devices, routing and waivers.

use novo_checks::{Problem, ProblemKind, check, check_open, waive, waived_problems};
use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn build<R>(edit: impl FnOnce(&mut Tx<'_>) -> Result<R>) -> (Project, R) {
    let mut project = Project::default();
    let (r, _) = project.transact("build", edit).unwrap();
    (project, r)
}

fn of_kind(problems: &[Problem], kind: ProblemKind) -> Vec<&Problem> {
    problems.iter().filter(|p| p.kind == kind).collect()
}

/// A project where the layout has started: -K1 is on the plate, -K2 is not.
fn one_unplaced() -> Project {
    build(|tx| {
        let page = tx.add_page("1")?;
        let cab = tx.add_structure(tag("+CAB1"), "", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 400.0,
                height: 400.0,
                margin: 10.0,
            }),
        )?;
        for (t, placed) in [("-K1", true), ("-K2", false)] {
            let d = tx.add_device(tag(t), "")?;
            tx.place_symbol(
                page,
                "iec/contacts/no",
                Placement::at(Point::new(10.0, 10.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Device {
                    device: d,
                    main: true,
                },
            )?;
            if placed {
                tx.place_device(d, cab, Point::new(20.0, 20.0), false, None)?;
            }
        }
        Ok(())
    })
    .0
}

#[test]
fn devices_off_the_plate_are_listed_once_the_layout_started() {
    let (mut project, (cab, k1)) = build(|tx| {
        let page = tx.add_page("1")?;
        let cab = tx.add_structure(tag("+CAB1"), "", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 400.0,
                height: 400.0,
                margin: 10.0,
            }),
        )?;
        let mut devices = Vec::new();
        for t in ["-K1", "-K2"] {
            let d = tx.add_device(tag(t), "")?;
            tx.place_symbol(
                page,
                "iec/contacts/no",
                Placement::at(Point::new(10.0, 10.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Device {
                    device: d,
                    main: true,
                },
            )?;
            devices.push(d);
        }
        Ok((cab, devices[0]))
    });
    assert!(of_kind(&check(&project), ProblemKind::UnplacedDevice).is_empty());
    project
        .transact("place", |tx| {
            tx.place_device(k1, cab, Point::new(20.0, 20.0), false, None)
        })
        .unwrap();
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::UnplacedDevice);
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("-K2"));
}

fn unplaced_problem(project: &Project) -> Problem {
    check(project)
        .into_iter()
        .find(|p| p.kind == ProblemKind::UnplacedDevice)
        .unwrap()
}

#[test]
fn a_waived_problem_leaves_the_open_list_with_its_reason() {
    let mut project = one_unplaced();
    let problem = unplaced_problem(&project);
    project
        .transact("waive", |tx| {
            waive(tx, &problem, "  Pump runs 5 s only ").map(|_| ())
        })
        .unwrap();
    assert!(of_kind(&check_open(&project), ProblemKind::UnplacedDevice).is_empty());
    let waived = waived_problems(&project);
    assert_eq!(waived.len(), 1);
    assert_eq!(waived[0].reason, "Pump runs 5 s only");
    // Waiving again replaces the reason.
    project
        .transact("again", |tx| waive(tx, &problem, "Short start").map(|_| ()))
        .unwrap();
    assert_eq!(waived_problems(&project)[0].reason, "Short start");
}

#[test]
fn a_wire_between_placed_devices_without_a_duct_is_found() {
    let (project, _) = build(|tx| {
        let page = tx.add_page("1")?;
        let cab = tx.add_structure(tag("+CAB1"), "", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 400.0,
                height: 400.0,
                margin: 10.0,
            }),
        )?;
        let mut pins = Vec::new();
        for (t, y) in [("-K1", 20.0), ("-K2", 200.0)] {
            let d = tx.add_device(tag(t), "")?;
            tx.place_device(d, cab, Point::new(50.0, y), false, None)?;
            let s = tx.place_symbol(
                page,
                "iec/contacts/no",
                Placement::at(Point::new(10.0, y)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Device {
                    device: d,
                    main: true,
                },
            )?;
            pins.push(Endpoint::pin(s, 0));
        }
        tx.connect(pins[0], pins[1])?;
        Ok(())
    });
    let problems = check(&project);
    assert_eq!(
        of_kind(&problems, ProblemKind::WireWithoutDuct).len(),
        1,
        "{problems:#?}"
    );
}

/// -K1 and -K2 on a 400 mm plate with a vertical duct between them and a
/// wire from one to the other. The duct has no size and the wire no
/// cross-section until the test says so.
fn wire_through_a_duct() -> (Project, RailId, WireId) {
    let (project, ids) = build(|tx| {
        let page = tx.add_page("1")?;
        let cab = tx.add_structure(tag("+CAB1"), "", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 400.0,
                height: 400.0,
                margin: 10.0,
            }),
        )?;
        let duct = tx.add_rail(cab, 200.0, 20.0, 360.0, RailKind::Duct)?;
        tx.update::<Rail>(duct, |r| r.vertical = true)?;
        let mut pins = Vec::new();
        for (t, x) in [("-K1", 50.0), ("-K2", 300.0)] {
            let d = tx.add_device(tag(t), "")?;
            tx.place_device(d, cab, Point::new(x, 100.0), false, None)?;
            let s = tx.place_symbol(
                page,
                "iec/contacts/no",
                Placement::at(Point::new(x, 10.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Device {
                    device: d,
                    main: true,
                },
            )?;
            pins.push(Endpoint::pin(s, 0));
        }
        let wire = tx.connect(pins[0], pins[1])?;
        Ok((duct, wire))
    });
    (project, ids.0, ids.1)
}

#[test]
fn a_duct_without_a_size_is_an_info_on_the_wires_through_it() {
    let (mut project, duct, wire) = wire_through_a_duct();
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::UnsizedDuct);
    assert_eq!(found.len(), 1, "{problems:#?}");
    assert_eq!(found[0].at, vec![AnyId::Wire(wire)]);
    assert_eq!(found[0].kind.severity(), novo_checks::Severity::Info);
    assert!(of_kind(&problems, ProblemKind::WireWithoutDuct).is_empty());

    project
        .transact("size", |tx| tx.set_rail_size(duct, 360.0, 40.0, 60.0))
        .unwrap();
    assert!(of_kind(&check(&project), ProblemKind::UnsizedDuct).is_empty());
}

#[test]
fn wires_without_a_cross_section_are_one_aggregated_info() {
    let (mut project, _, wire) = wire_through_a_duct();
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::GuessedWire);
    assert_eq!(found.len(), 1, "{problems:#?}");
    assert_eq!(found[0].at, vec![AnyId::Wire(wire)]);
    assert_eq!(found[0].kind.severity(), novo_checks::Severity::Info);
    assert!(found[0].message.starts_with("1 wire has no cross-section"));

    project
        .transact("size", |tx| {
            tx.update::<Wire>(wire, |w| w.cross_section_mm2 = Some(1.5))
        })
        .unwrap();
    assert!(of_kind(&check(&project), ProblemKind::GuessedWire).is_empty());
}

#[test]
fn the_routing_infos_wait_until_a_layout_exists() {
    let project = Project::default();
    let problems = check(&project);
    assert!(of_kind(&problems, ProblemKind::UnsizedDuct).is_empty());
    assert!(of_kind(&problems, ProblemKind::GuessedWire).is_empty());
}

#[test]
fn the_report_lists_open_and_waived_problems() {
    let mut project = one_unplaced();
    let lines = novo_checks::report_lines(&project);
    assert!(lines.iter().any(|(_, t)| t.starts_with("Unplaced device:")));
    let problem = unplaced_problem(&project);
    project
        .transact("w", |tx| waive(tx, &problem, "Short start").map(|_| ()))
        .unwrap();
    let lines = novo_checks::report_lines(&project);
    assert!(lines.iter().any(|(_, t)| t.contains("Reason: Short start")));
}

/// A 300 x 300 mm plate with -K1 .. -K3 placed at the given points (45 x 90
/// mm each, the size of a device without a part).
fn plate_with(points: [(f64, f64); 3]) -> Project {
    build(|tx| {
        let page = tx.add_page("1")?;
        let cab = tx.add_structure(tag("+CAB1"), "", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 300.0,
                height: 300.0,
                margin: 10.0,
            }),
        )?;
        for (i, (x, y)) in points.into_iter().enumerate() {
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
            tx.place_device(d, cab, Point::new(x, y), false, None)?;
        }
        Ok(())
    })
    .0
}

#[test]
fn devices_apart_on_the_plate_are_quiet() {
    let project = plate_with([(20.0, 20.0), (100.0, 20.0), (180.0, 20.0)]);
    let problems = check(&project);
    for kind in [
        ProblemKind::DeviceCollision,
        ProblemKind::DeviceOutsidePlate,
        ProblemKind::DevicesTooClose,
    ] {
        assert!(of_kind(&problems, kind).is_empty(), "{kind:?}");
    }
}

#[test]
fn overlapping_off_plate_and_tight_devices_are_found() {
    // -K1 and -K2 overlap, -K3 hangs over the right edge; -K1 and -K3 are far.
    let project = plate_with([(20.0, 20.0), (40.0, 20.0), (270.0, 20.0)]);
    let problems = check(&project);
    assert_eq!(of_kind(&problems, ProblemKind::DeviceCollision).len(), 1);
    assert_eq!(of_kind(&problems, ProblemKind::DeviceOutsidePlate).len(), 1);

    // 5 mm between -K1 and -K2 is under the 10 mm default clearance.
    let project = plate_with([(20.0, 20.0), (70.0, 20.0), (200.0, 20.0)]);
    let problems = check(&project);
    assert_eq!(of_kind(&problems, ProblemKind::DevicesTooClose).len(), 1);
    assert!(of_kind(&problems, ProblemKind::DeviceCollision).is_empty());
}
