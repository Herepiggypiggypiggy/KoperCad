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

#[test]
fn a_contactor_part_with_coil_and_main_contacts_stays_quiet() {
    let mut project = Project::default();
    project
        .transact("build", |tx| {
            let page = tx.add_page("1")?;
            let device = tx.add_device(tag("-K1"), "")?;
            let coil = ["A1", "A2"].map(|n| Pin::new(n, 0.0, 0.0)).to_vec();
            let main = ["1L1", "2T1", "3L2", "4T2", "5L3", "6T3"]
                .map(|n| Pin::new(n, 0.0, 0.0))
                .to_vec();
            for (key, pins, is_main) in [
                ("iec/coils/contactor", coil, true),
                ("iec/contactors/main_3p", main, false),
            ] {
                tx.place_symbol(
                    page,
                    key,
                    Placement::at(Point::new(10.0, 10.0)),
                    pins,
                    SymbolRole::Device {
                        device,
                        main: is_main,
                    },
                )?;
            }
            let part = tx.add_part("Acme", "LC1")?;
            for (i, name) in ["A1", "A2", "1L1", "2T1", "3L2", "4T2", "5L3", "6T3"]
                .iter()
                .enumerate()
            {
                for axis in ["x", "y", "z"] {
                    tx.set_property(
                        part,
                        format!("pin.{name}.{axis}_mm"),
                        Some(Value::Float(i as f64)),
                    )?;
                }
            }
            tx.assign_part(device, part)?;
            Ok(())
        })
        .unwrap();
    assert!(pin_names(&project).is_empty(), "{:#?}", pin_names(&project));
}

fn apply(project: &mut Project, command: novo_core::Command) {
    project.transact("fix", |tx| tx.execute(command)).unwrap();
}

#[test]
fn the_fix_renames_symbol_pins_and_one_undo_step_covers_it() {
    let mut project = one("iec/motors/motor_3ph", "-M1", &["U", "V", "W", "PE"], &[]);
    let problem = pin_names(&project).remove(0);
    let fix = novo_checks::pin_fix(&project, &problem).expect("a clear fix");
    apply(&mut project, fix.command);
    let names: Vec<_> = project
        .symbols()
        .flat_map(|s| s.pins.iter().map(|p| p.name.clone()))
        .collect();
    assert_eq!(names, ["U1", "V1", "W1", "PE"]);
    assert!(pin_names(&project).is_empty());
}

#[test]
fn the_fix_renames_part_terminals_when_the_pins_are_fine() {
    let mut project = one("iec/coils/contactor", "-K1", &["A1", "A2"], &["a1", "A2"]);
    let problem = pin_names(&project).remove(0);
    let fix = novo_checks::pin_fix(&project, &problem).expect("a clear fix");
    assert!(fix.label.contains("part terminal"));
    apply(&mut project, fix.command);
    assert!(pin_names(&project).is_empty());
    let part = project.parts().next().unwrap();
    assert!(part.properties.contains_key("pin.A1.x_mm"));
    assert!(!part.properties.contains_key("pin.a1.x_mm"));
}

#[test]
fn a_terminal_keeps_its_wire_range_when_renamed() {
    let mut project = one("iec/coils/contactor", "-K1", &["A1", "A2"], &["a1", "A2"]);
    let part = project.parts().next().unwrap().id;
    project
        .transact("range", |tx| {
            tx.set_property(part, "pin.a1.wire_min_mm2", Some(Value::Float(0.5)))?;
            tx.set_property(part, "pin.a1.wire_max_mm2", Some(Value::Float(2.5)))?;
            tx.set_property(part, "pin.a1.torque_nm", Some(Value::Float(0.6)))
        })
        .unwrap();
    let problem = pin_names(&project).remove(0);
    let fix = novo_checks::pin_fix(&project, &problem).unwrap();
    assert!(fix.note.is_some());
    apply(&mut project, fix.command);
    let part = project.parts().next().unwrap();
    for key in ["wire_min_mm2", "wire_max_mm2", "torque_nm", "x_mm"] {
        assert!(
            part.properties.contains_key(&format!("pin.A1.{key}")),
            "{key}"
        );
        assert!(
            !part.properties.contains_key(&format!("pin.a1.{key}")),
            "{key}"
        );
    }
}

#[test]
fn no_fix_when_the_names_are_a_guess() {
    let project = one(
        "iec/transformers/transformer_1ph",
        "-T1",
        &["P", "Q", "R", "S"],
        &[],
    );
    let found = pin_names(&project);
    assert_eq!(found.len(), 1);
    assert!(novo_checks::pin_fix(&project, &found[0]).is_none());
}

const BRAKE_MOTOR: &str = r#"
[[class]]
id = "motor.brake"
label = "brake motor"
functions = ["motor.ac3"]
[[class.set]]
name = "three terminals and brake"
required = ["U1", "V1", "W1", "BD1", "BD2"]
optional = ["PE"]
"#;

fn with_company(mut project: Project, text: &str) -> Project {
    project
        .transact("company", |tx| {
            tx.set_info(|i| {
                i.properties.insert(
                    novo_checks::PIN_TEMPLATES_PROPERTY.into(),
                    Value::Text(text.into()),
                );
            })
        })
        .unwrap();
    project
}

#[test]
fn company_templates_change_what_is_usual() {
    let plain = one(
        "iec/motors/motor_3ph",
        "-M1",
        &["U1", "V1", "W1", "PE"],
        &[],
    );
    assert!(pin_names(&plain).is_empty());
    let company = with_company(plain, BRAKE_MOTOR);
    let found = pin_names(&company);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.contains("brake motor"));
    assert!(found[0].message.contains("BD1"));
    // Other classes keep the built-in names.
    let coil = with_company(
        one("iec/coils/relay", "-K1", &["A1", "A2"], &[]),
        BRAKE_MOTOR,
    );
    assert!(pin_names(&coil).is_empty());
}

#[test]
fn unreadable_company_text_is_one_note_and_the_built_ins_stay() {
    let project = with_company(
        one("iec/motors/motor_3ph", "-M1", &["U1", "V1", "W1"], &[]),
        "[[class]]\nid = \"x\"\nfunctions = []",
    );
    let found = pin_names(&project);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.contains("cannot be read"));
}
