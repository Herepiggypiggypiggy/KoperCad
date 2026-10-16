use novo_core::{Command, DeviceTag, MountingPlate, Project, RailKind, StructureId, Value};
use novo_layout::enclosure::Enclosure;
use novo_layout::model::{place_device, set_enclosure, set_plate};
use novo_thermal::bridge::{BridgeError, BridgeOptions, scenario_from_project};

fn fixture() -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 400.0,
                    height: 600.0,
                    margin: 10.0,
                },
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 500.0,
                    height: 700.0,
                    depth: 250.0,
                    plate_setback: 0.0,
                    door: None,
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(90)))?;
            tx.set_property(part, "depth_mm", Some(Value::Int(70)))?;
            tx.set_property(part, "power_loss_w", Some(Value::Float(4.0)))?;
            tx.set_property(part, "max_ambient_c", Some(Value::Int(55)))?;
            tx.set_property(part, "max_surface_c", Some(Value::Int(80)))?;
            let silent = tx.add_part("ACME", "NOLOSS")?;
            tx.set_property(silent, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(silent, "height_mm", Some(Value::Int(90)))?;
            for (i, (tag, with)) in [
                ("=A1+CAB1-K1", part),
                ("=A1+CAB1-K2", part),
                ("=A1+CAB1-K3", silent),
                ("=A1+CAB1-K4", silent),
                ("=A1+CAB1-K5", part),
                ("=A1+FIELD-M1", part),
            ]
            .into_iter()
            .enumerate()
            {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, with)?;
                if tag == "=A1+CAB1-K2" {
                    tx.set_property(d, "power_loss_w", Some(Value::Float(10.0)))?;
                    tx.set_property(d, "load_factor", Some(Value::Float(0.5)))?;
                    tx.set_property(d, "depth_mm", Some(Value::Int(120)))?;
                    tx.set_property(d, "max_ambient_c", Some(Value::Int(40)))?;
                }
                if tag != "=A1+CAB1-K5" && tag != "=A1+FIELD-M1" {
                    place_device(tx, cab, d.raw(), 20.0 + 60.0 * i as f64, 100.0, i == 1)?;
                }
            }
            tx.execute(Command::AddRail {
                structure: cab,
                x: 10.0,
                y: 50.0,
                length: 300.0,
                kind: RailKind::DinRail,
                vertical: false,
                width: 0.0,
                height: 0.0,
            })?;
            tx.execute(Command::AddRail {
                structure: cab,
                x: 380.0,
                y: 20.0,
                length: 500.0,
                kind: RailKind::Duct,
                vertical: true,
                width: 60.0,
                height: 0.0,
            })?;
            Ok(cab)
        })
        .unwrap();
    (p, cab)
}

#[test]
fn builds_devices_with_loss_depth_limits_and_interior_frame() {
    let (p, cab) = fixture();
    let b = scenario_from_project(&p, cab, &BridgeOptions::default()).unwrap();
    let s = &b.scenario;
    assert_eq!(s.devices.len(), 4);
    // Interior 500 x 700 + 2 mm walls; plate 400 x 600 centred: origin (50, 50).
    assert_eq!(s.enclosure.width_mm, 504.0);
    assert_eq!(s.enclosure.plate.x, 50.0);
    assert_eq!(s.enclosure.plate.y, 50.0);
    let k1 = &s.devices[0];
    assert_eq!(k1.loss_w, 4.0);
    assert_eq!(k1.height_mm, Some(70.0));
    assert_eq!(k1.rated_max_ambient_c, Some(55.0));
    assert_eq!(k1.rated_max_surface_c, Some(80.0));
    assert_eq!((k1.footprint.x, k1.footprint.y), (70.0, 150.0));
    assert_eq!((k1.footprint.w, k1.footprint.h), (45.0, 90.0));
    // Device overrides part: loss 10 W at load factor 0.5, depth 120, ambient 40.
    let k2 = &s.devices[1];
    assert_eq!(k2.loss_w, 5.0);
    assert_eq!(k2.height_mm, Some(120.0));
    assert_eq!(k2.rated_max_ambient_c, Some(40.0));
    assert_eq!(k2.rated_max_surface_c, Some(80.0));
    // Rotated: width and height swap.
    assert_eq!((k2.footprint.w, k2.footprint.h), (90.0, 45.0));
    assert_eq!(b.report.total_loss_w, 4.0 + 5.0);
}

#[test]
fn reports_missing_data_instead_of_inventing_it() {
    let (p, cab) = fixture();
    let b = scenario_from_project(&p, cab, &BridgeOptions::default()).unwrap();
    let r = &b.report;
    assert_eq!(r.placed, 4);
    let tags: Vec<&str> = r.without_loss.iter().map(|d| d.tag.as_str()).collect();
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().all(|t| t.contains("K3") || t.contains("K4")));
    assert_eq!(r.without_depth.len(), 2);
    assert_eq!(r.guessed_size.len(), 0);
    assert_eq!(r.unplaced.len(), 1); // K5, not the FIELD motor
    assert!(r.unplaced[0].tag.contains("K5"));
    let msgs = r.messages();
    assert_eq!(msgs[0], "2 devices have no loss data");
    assert!(
        msgs.iter()
            .any(|m| m == "1 device has not been placed on the plate")
    );
    // Unknown loss is placed as 0 W, never a guess.
    assert!(
        b.scenario
            .devices
            .iter()
            .filter(|d| d.loss_w == 0.0)
            .count()
            == 2
    );
}

#[test]
fn one_missing_loss_reads_in_the_singular() {
    let (mut p, cab) = fixture();
    let k4 = p
        .all::<novo_core::Device>()
        .find(|d| d.tag.to_string().contains("K4"))
        .unwrap()
        .id;
    p.transact("loss", |tx| {
        tx.set_property(k4, "power_loss_w", Some(Value::Float(2.0)))?;
        Ok(())
    })
    .unwrap();
    let b = scenario_from_project(&p, cab, &BridgeOptions::default()).unwrap();
    assert_eq!(b.report.messages()[0], "1 device has no loss data");
}

#[test]
fn rails_and_ducts_become_obstacles_in_the_interior_frame() {
    let (p, cab) = fixture();
    let b = scenario_from_project(&p, cab, &BridgeOptions::default()).unwrap();
    let o = &b.scenario.obstacles;
    assert_eq!(o.len(), 2);
    use novo_thermal::ObstacleKind::*;
    let duct = o.iter().find(|o| o.kind == Duct).unwrap();
    // Vertical duct at plate x 380, 60 wide, centred on that line, 500 long.
    assert_eq!(
        (duct.rect.x, duct.rect.y, duct.rect.w, duct.rect.h),
        (400.0, 70.0, 60.0, 500.0)
    );
    let rail = o.iter().find(|o| o.kind == Rail).unwrap();
    assert_eq!((rail.rect.w, rail.rect.h), (300.0, 35.0));
}

#[test]
fn the_scenario_runs_in_the_engine() {
    let (p, cab) = fixture();
    let b = scenario_from_project(&p, cab, &BridgeOptions::default()).unwrap();
    let r = novo_thermal::simulate_steady_with(
        &b.scenario,
        novo_thermal::Options {
            bands: false,
            advice: false,
        },
    )
    .unwrap();
    assert_eq!(r.devices.len(), 4);
}

#[test]
fn refuses_what_is_not_an_enclosure_with_a_plate_and_interior() {
    let mut p = Project::new("t");
    let ((room, bare, plated), _) = p
        .transact("s", |tx| {
            let room = tx.add_structure(DeviceTag::parse("+ROOM").unwrap(), "Room", false)?;
            let bare = tx.add_structure(DeviceTag::parse("+C1").unwrap(), "Box", true)?;
            let plated = tx.add_structure(DeviceTag::parse("+C2").unwrap(), "Box", true)?;
            set_plate(
                tx,
                plated,
                MountingPlate {
                    width: 300.0,
                    height: 300.0,
                    margin: 5.0,
                },
            )?;
            Ok((room, bare, plated))
        })
        .unwrap();
    let o = BridgeOptions::default();
    assert_eq!(
        scenario_from_project(&p, room, &o).unwrap_err(),
        BridgeError::NotAnEnclosure
    );
    assert_eq!(
        scenario_from_project(&p, bare, &o).unwrap_err(),
        BridgeError::NoPlate
    );
    assert_eq!(
        scenario_from_project(&p, plated, &o).unwrap_err(),
        BridgeError::NoInterior
    );
}

#[test]
fn keys_match_novo_parts() {
    use novo_parts::keys as part;
    use novo_thermal::bridge::keys as mine;
    assert_eq!(mine::POWER_LOSS_W, part::POWER_LOSS_W);
    assert_eq!(mine::LOAD_FACTOR, part::LOAD_FACTOR);
    assert_eq!(mine::MAX_AMBIENT_C, part::MAX_AMBIENT_C);
    assert_eq!(mine::MAX_SURFACE_C, part::MAX_SURFACE_C);
    assert_eq!(mine::DEPTH_MM, part::DEPTH_MM);
}
