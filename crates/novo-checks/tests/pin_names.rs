//! Pin names against the pin template of the device class (`PinNames`).

use novo_checks::{Problem, ProblemKind, Severity, check};
use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn pin_names(project: &Project) -> Vec<Problem> {
    check(project)
        .into_iter()
        .filter(|p| p.kind == ProblemKind::PinNames)
        .collect()
}

/// A device with one symbol of `key` and the given pin names, and an optional
/// part whose terminals carry positions (so they have names).
fn one(key: &str, tagname: &str, pins: &[&str], terminals: &[&str]) -> Project {
    let mut project = Project::default();
    project
        .transact("build", |tx| {
            let page = tx.add_page("1")?;
            let device = tx.add_device(tag(tagname), "")?;
            let pins = pins
                .iter()
                .enumerate()
                .map(|(i, n)| Pin::new(*n, 0.0, i as f64 * 8.0))
                .collect();
            tx.place_symbol(
                page,
                key,
                Placement::at(Point::new(10.0, 10.0)),
                pins,
                SymbolRole::Device { device, main: true },
            )?;
            if !terminals.is_empty() {
                let part = tx.add_part("Acme", "P1")?;
                for (i, name) in terminals.iter().enumerate() {
                    for axis in ["x", "y", "z"] {
                        tx.set_property(
                            part,
                            format!("pin.{name}.{axis}_mm"),
                            Some(Value::Float(i as f64)),
                        )?;
                    }
                }
                tx.assign_part(device, part)?;
            }
            Ok(())
        })
        .unwrap();
    project
}

#[test]
fn usual_names_are_quiet() {
    assert!(pin_names(&one("iec/coils/contactor", "-K1", &["A1", "A2"], &[])).is_empty());
    assert!(
        pin_names(&one(
            "iec/motors/motor_3ph",
            "-M1",
            &["U1", "V1", "W1", "PE"],
            &[]
        ))
        .is_empty()
    );
}

#[test]
fn a_motor_named_u_v_w_is_reported_as_information() {
    let found = pin_names(&one(
        "iec/motors/motor_3ph",
        "-M1",
        &["U", "V", "W", "PE"],
        &[],
    ));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].severity, Severity::Info);
    assert!(found[0].message.contains("-M1"));
    assert!(found[0].message.contains("U1"));
}

#[test]
fn part_terminals_are_compared_with_the_class() {
    let ok = one("iec/coils/contactor", "-K1", &["A1", "A2"], &["A1", "A2"]);
    assert!(pin_names(&ok).is_empty());
    let odd = one("iec/coils/contactor", "-K1", &["A1", "A2"], &["1", "2"]);
    let found = pin_names(&odd);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.contains("part terminals"));
}

#[test]
fn symbols_without_a_template_are_left_alone() {
    assert!(pin_names(&one("iec/contacts/no", "-K1", &["X", "Y"], &[])).is_empty());
}
