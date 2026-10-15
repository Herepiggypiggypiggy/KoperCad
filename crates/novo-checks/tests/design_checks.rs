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
