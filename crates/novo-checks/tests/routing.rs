//! Wire routing problems (novo-routing) in the Issues list.

use novo_checks::{ProblemKind, Severity, check_routing};
use novo_core::{
    AnyId, DeviceTag, Direction, MountingPlate, Pin, PinRef, Placement, Point, Project, RailKind,
    SymbolRole, Value,
};
use novo_routing::Rules;
use novo_routing::model::Connection;

/// Two placed devices on a plate; a vertical duct between them of the given
/// size (none when `duct` is `None`).
fn cabinet(duct: Option<(f64, f64)>) -> (Project, Vec<PinRef>, Option<AnyId>) {
    let mut p = Project::new("t");
    let (out, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1")?, "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                }),
            )?;
            let rail = match duct {
                Some((w, h)) => {
                    let d = tx.add_rail(cab, 150.0, 100.0, 200.0, RailKind::Duct)?;
                    tx.update::<novo_core::Rail>(d, |r| r.vertical = true)?;
                    tx.set_rail_size(d, 200.0, w, h)?;
                    Some(AnyId::from(d))
                }
                None => None,
            };
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            let page = tx.add_page("1")?;
            let mut pins = Vec::new();
            for (tag, y, dir) in [
                ("=A1+CAB1-K1", 50.0, Direction::Down),
                ("=A1+CAB1-K2", 250.0, Direction::Up),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag)?, "")?;
                tx.assign_part(d, part)?;
                tx.place_device(d, cab, Point::new(50.0, y), false, None)?;
                let symbol = tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    Placement::at(Point::new(0.0, 0.0)),
                    vec![Pin::new("1", 0.0, 0.0).with_direction(dir)],
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )?;
                pins.push(PinRef::new(symbol, 0));
            }
            Ok((pins, rail))
        })
        .unwrap();
    (p, out.0, out.1)
}

fn wire(pins: &[PinRef], diameter_mm: Option<f64>) -> [Connection; 1] {
    [Connection {
        id: 1,
        from: pins[0],
        to: pins[1],
        diameter_mm,
    }]
}

#[test]
fn a_wire_no_duct_reaches_is_a_warning_on_both_devices() {
    let (p, pins, _) = cabinet(None);
    let problems = check_routing(&p, &wire(&pins, Some(3.0)), &Rules::default());
    assert_eq!(problems.len(), 1, "{problems:#?}");
    let f = &problems[0];
    assert_eq!(f.kind, ProblemKind::WireRouting);
    assert_eq!(f.severity, Severity::Warning);
    assert_eq!(f.at.len(), 2);
    assert!(f.message.contains("free air"), "{}", f.message);
    assert!(f.page.is_some());
}

#[test]
fn a_duct_over_its_fill_limit_is_a_warning_marking_the_duct() {
    // 40 x 60 mm at the 0.5 limit allows 1200 mm2; one 40 mm wire is 1257.
    let (p, pins, rail) = cabinet(Some((40.0, 60.0)));
    let problems = check_routing(&p, &wire(&pins, Some(40.0)), &Rules::default());
    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert_eq!(problems[0].kind, ProblemKind::WireRouting);
    assert_eq!(problems[0].at[0], rail.unwrap());
    assert!(
        problems[0].message.contains("105%"),
        "{}",
        problems[0].message
    );
}

#[test]
fn a_duct_with_room_and_known_wires_is_clean() {
    let (p, pins, _) = cabinet(Some((40.0, 60.0)));
    assert!(check_routing(&p, &wire(&pins, Some(3.0)), &Rules::default()).is_empty());
}

#[test]
fn an_unsized_duct_and_an_unsized_wire_are_infos() {
    let (p, pins, rail) = cabinet(Some((0.0, 0.0)));
    let problems = check_routing(&p, &wire(&pins, None), &Rules::default());
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(problems.iter().all(|f| f.kind == ProblemKind::RoutingData));
    assert!(problems.iter().all(|f| f.severity == Severity::Info));
    assert!(problems.iter().any(|f| f.at == vec![rail.unwrap()]));
    assert!(problems.iter().any(|f| f.message.contains("1 wire ")));
}
