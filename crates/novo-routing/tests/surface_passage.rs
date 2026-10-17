//! Wires to devices on the door: through the wiring opening that adds the
//! least wire, or straight when both ends are on the door.

use novo_core::{
    CabinetPlacement, DeviceTag, MountingPlate, PinRef, Placement, Point as CorePoint, Project,
    RailKind, StructureId, SymbolRole, Value,
};
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::set_enclosure;
use novo_routing::model::{Connection, route_connections};
use novo_routing::{Kind, Rules};

fn pin(tx: &mut novo_core::Tx<'_>, page: novo_core::PageId, device: novo_core::DeviceId) -> PinRef {
    let symbol = tx
        .place_symbol(
            page,
            "iec/contacts/no",
            Placement::at(CorePoint::new(0.0, 0.0)),
            vec![novo_core::Pin::new("1", 0.0, 0.0)],
            SymbolRole::Device { device, main: true },
        )
        .unwrap();
    PinRef::new(symbol, 0)
}

/// K1 on the plate, H1 and H2 on the door, optionally one opening.
fn project(opening: bool) -> (Project, StructureId, Vec<PinRef>) {
    let mut p = Project::new("t");
    let (out, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                }),
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 450.0,
                    height: 450.0,
                    depth: 200.0,
                    plate_setback: 20.0,
                    door: Some(Door {
                        hinge: Hinge::Left,
                        ..Door::default()
                    }),
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            let duct = tx.add_rail(cab, 150.0, 80.0, 250.0, RailKind::Duct)?;
            tx.update::<novo_core::Rail>(duct, |r| r.vertical = true)?;
            tx.set_rail_size(duct, 250.0, 40.0, 60.0)?;
            if opening {
                tx.set_property(
                    cab,
                    "opening.gland",
                    Some(Value::Text("plate round 25 150 300 to door 200 150".into())),
                )?;
            }
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            let page = tx.add_page("1")?;
            let mut pins = Vec::new();
            for (tag, x, y, door) in [
                ("=A1+CAB1-K1", 50.0, 50.0, false),
                ("=A1+CAB1-H1", 100.0, 100.0, true),
                ("=A1+CAB1-H2", 300.0, 100.0, true),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
                tx.place_device(d, cab, CorePoint::new(x, y), false, None)?;
                if door {
                    let id = tx
                        .project()
                        .all::<CabinetPlacement>()
                        .find(|c| c.device == d)
                        .unwrap()
                        .id;
                    tx.set_property(id, "surface", Some(Value::Text("door".into())))?;
                }
                pins.push(pin(tx, page, d));
            }
            Ok((cab, pins))
        })
        .unwrap();
    (p, out.0, out.1)
}

fn conn(id: u64, a: PinRef, b: PinRef) -> Connection {
    Connection {
        id,
        from: a,
        to: b,
        diameter_mm: Some(3.0),
    }
}

#[test]
fn a_wire_to_the_door_passes_through_the_opening() {
    let (p, _, pins) = project(true);
    let r = route_connections(&p, &[conn(7, pins[0], pins[1])], &Rules::default());
    assert!(r.unrouted.is_empty(), "{:?}", r.unrouted);
    let leg = &r.surface_legs[&7];
    assert_eq!(leg.surface, "door");
    assert_eq!(leg.opening.as_deref(), Some("gland"));
    // From the opening at (200, 150) on the door to the middle of H1 at
    // (122.5, 145), plus the loop at the terminal.
    assert_eq!(leg.length_mm, 77.5 + 5.0 + 50.0);
    let route = &r.routes[&7];
    assert_eq!(route.kind, Kind::Ducted);
    // The plate leg ends at the opening: K1's middle (72.5, 95) to (150, 300).
    let end = route.points.last().unwrap();
    assert_eq!((end.x, end.y), (150.0, 300.0));
    assert!(route.length_mm >= leg.length_mm + 77.5 + 205.0);
    // The same wire the other way round has the same length.
    let back = route_connections(&p, &[conn(8, pins[1], pins[0])], &Rules::default());
    assert_eq!(back.routes[&8].length_mm, route.length_mm);
    let start = back.routes[&8].points.first().unwrap();
    assert_eq!((start.x, start.y), (150.0, 300.0));
}

#[test]
fn two_door_devices_are_wired_straight_on_the_door() {
    let (p, _, pins) = project(false);
    let r = route_connections(&p, &[conn(3, pins[1], pins[2])], &Rules::default());
    let route = &r.routes[&3];
    assert_eq!(route.kind, Kind::FreeAir);
    // 200 across the door plus a loop at each end.
    assert_eq!(route.length_mm, 200.0 + 100.0);
    assert_eq!(r.surface_legs[&3].opening, None);
    assert!(r.plans.is_empty() && r.unrouted.is_empty());
}

#[test]
fn without_an_opening_a_door_wire_stays_unrouted() {
    let (p, _, pins) = project(false);
    let r = route_connections(&p, &[conn(5, pins[0], pins[1])], &Rules::default());
    assert_eq!(r.unrouted, vec![5]);
    assert!(r.routes.is_empty() && r.surface_legs.is_empty());
}
