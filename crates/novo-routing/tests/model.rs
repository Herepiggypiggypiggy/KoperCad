use novo_core::{DeviceTag, MountingPlate, Project, Rail, RailKind, StructureId};
use novo_routing::model::{ducts, network};
use novo_routing::{Anchor, Facing, Job, Point, Rules, plan};

fn project() -> (Project, StructureId, StructureId) {
    let mut p = Project::new("t");
    let (ids, _) = p
        .transact("setup", |tx| {
            let a = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "A", true)?;
            let b = tx.add_structure(DeviceTag::parse("+CAB2").unwrap(), "B", true)?;
            let plate = MountingPlate {
                width: 600.0,
                height: 800.0,
                margin: 10.0,
            };
            tx.set_plate(a, Some(plate))?;
            tx.set_plate(b, Some(plate))?;
            let h = tx.add_rail(a, 20.0, 100.0, 300.0, RailKind::Duct)?;
            tx.set_rail_size(h, 300.0, 40.0, 60.0)?;
            let v = tx.add_rail(a, 200.0, 20.0, 250.0, RailKind::Duct)?;
            tx.update::<Rail>(v, |r| r.vertical = true)?;
            tx.add_rail(a, 20.0, 200.0, 300.0, RailKind::DinRail)?;
            tx.add_rail(b, 0.0, 0.0, 100.0, RailKind::Duct)?;
            Ok((a, b))
        })
        .unwrap();
    (p, ids.0, ids.1)
}

#[test]
fn only_the_ducts_of_the_enclosure_are_read_with_their_direction_and_size() {
    let (p, a, b) = project();
    let d = ducts(&p, a);
    assert_eq!(d.len(), 2); // the DIN rail and the other cabinet's duct are left out
    assert_eq!(
        (d[0].a, d[0].b),
        (Point::new(20.0, 100.0), Point::new(320.0, 100.0))
    );
    assert_eq!((d[0].width, d[0].height), (40.0, 60.0));
    assert_eq!(
        (d[1].a, d[1].b),
        (Point::new(200.0, 20.0), Point::new(200.0, 270.0))
    );
    assert_eq!((d[1].width, d[1].height), (0.0, 0.0));
    assert_eq!(ducts(&p, b).len(), 1);
}

#[test]
fn the_ducts_from_the_model_route_a_wire() {
    let (p, a, _) = project();
    let net = network(&p, a);
    let anchor = |x, y| Anchor {
        pos: Point::new(x, y),
        facing: Facing::Any,
    };
    let job = Job {
        id: 1,
        from: anchor(60.0, 90.0),
        to: anchor(210.0, 250.0),
        diameter_mm: Some(3.0),
    };
    let plan = plan(&net, &[job], &Rules::default());
    // Down the horizontal duct to the crossing, then down the vertical one.
    assert_eq!(plan.routes[0].ducts.len(), 2);
    assert_eq!(plan.routes[0].kind, novo_routing::Kind::Ducted);
}
