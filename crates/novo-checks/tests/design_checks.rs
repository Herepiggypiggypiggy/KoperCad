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

/// A 230 V wire between two junctions, with the given length and section.
fn run(length: Option<f64>, section: f64, current: f64) -> Project {
    build(|tx| {
        let page = tx.add_page("1")?;
        let l1 = tx.add_potential("L1", PotentialKind::Line)?;
        tx.update::<Potential>(l1, |p| p.voltage = Some(230.0))?;
        let a = tx.add_junction(page, Point::new(0.0, 0.0))?;
        let b = tx.add_junction(page, Point::new(50.0, 0.0))?;
        let wire = tx.connect(Endpoint::junction(a), Endpoint::junction(b))?;
        tx.set_wire_potential(wire, Some(l1))?;
        tx.update::<Wire>(wire, |w| w.cross_section_mm2 = Some(section))?;
        tx.set_property(wire, "load_current_a", Some(Value::Float(current)))?;
        if let Some(l) = length {
            tx.set_property(wire, "length_m", Some(Value::Float(l)))?;
        }
        Ok(())
    })
    .0
}

#[test]
fn a_long_thin_run_drops_too_much_voltage() {
    // 2 * 0.0225 * 100 m * 10 A / 1.5 mm2 = 30 V, 13 % of 230 V.
    let problems = check(&run(Some(100.0), 1.5, 10.0));
    let found = of_kind(&problems, ProblemKind::VoltageDrop);
    assert_eq!(found.len(), 1, "{problems:#?}");
    assert!(found[0].message.contains("13%"), "{}", found[0].message);
    assert!(
        of_kind(
            &check(&run(Some(100.0), 16.0, 10.0)),
            ProblemKind::VoltageDrop
        )
        .is_empty()
    );
}

#[test]
fn voltage_drop_needs_a_length() {
    assert!(of_kind(&check(&run(None, 1.5, 10.0)), ProblemKind::VoltageDrop).is_empty());
}

#[test]
fn a_breaker_below_the_supply_fault_current_is_an_error() {
    let (mut project, _) = build(|tx| {
        let page = tx.add_page("1")?;
        let q1 = tx.add_device(tag("-Q1"), "Breaker")?;
        tx.set_property(q1, "breaking_capacity_ka", Some(Value::Float(6.0)))?;
        tx.place_symbol(
            page,
            "iec/protection/mcb",
            Placement::at(Point::new(10.0, 10.0)),
            vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 16.0)],
            SymbolRole::Device {
                device: q1,
                main: true,
            },
        )?;
        Ok(())
    });
    assert!(of_kind(&check(&project), ProblemKind::ShortCircuit).is_empty());
    project
        .transact("supply", |tx| {
            tx.set_info(|i| {
                i.properties
                    .insert("supply.short_circuit_ka".into(), Value::Float(10.0));
            })
        })
        .unwrap();
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::ShortCircuit);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, novo_checks::Severity::Error);
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

#[test]
fn a_waived_problem_leaves_the_open_list_with_its_reason() {
    let mut project = run(Some(100.0), 1.5, 10.0);
    let problem = check(&project).into_iter().next().unwrap();
    project
        .transact("waive", |tx| {
            waive(tx, &problem, "  Pump runs 5 s only ").map(|_| ())
        })
        .unwrap();
    assert!(check_open(&project).is_empty());
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
    let mut project = run(Some(100.0), 1.5, 10.0);
    let lines = novo_checks::report_lines(&project);
    assert!(lines.iter().any(|(_, t)| t == "1 warning"), "{lines:?}");
    assert!(lines.iter().any(|(_, t)| t.starts_with("Voltage drop:")));
    let problem = check(&project).remove(0);
    project
        .transact("w", |tx| waive(tx, &problem, "Short start").map(|_| ()))
        .unwrap();
    let lines = novo_checks::report_lines(&project);
    assert!(lines.iter().any(|(_, t)| t == "No issues"));
    assert!(lines.iter().any(|(_, t)| t.contains("Reason: Short start")));
}
