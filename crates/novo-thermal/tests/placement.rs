//! The project side of the heat-aware placement: rails as rows, the search on
//! a cabinet built in a project, and the moves applied as one transaction.

use novo_core::{Command, DeviceTag, MountingPlate, Project, RailKind, StructureId, Value};
use novo_layout::enclosure::Enclosure;
use novo_layout::model::{load, set_enclosure};
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};
use novo_thermal::placement::{Settings, suggest};
use novo_thermal::placement_project::{apply_moves, plate_origin, rows};

/// Three rails 350 mm apart. A 150 W drive sits at the left of the lowest
/// rail with a cool 45 C-limit device on the rail above it; the right of the
/// lowest rail is free.
fn cabinet() -> (Project, StructureId, u64, u64, Vec<u64>) {
    let mut p = Project::new("placement");
    let (mut hot, mut cool, mut rails) = (0, 0, Vec::new());
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 1100.0,
                    margin: 10.0,
                }),
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 900.0,
                    height: 1200.0,
                    depth: 300.0,
                    plate_setback: 0.0,
                    door: None,
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            for row in 0..3 {
                let id = tx.add_rail(
                    cab,
                    20.0,
                    40.0 + 350.0 * f64::from(row),
                    760.0,
                    RailKind::DinRail,
                )?;
                rails.push(id.raw());
            }
            let put = |tx: &mut novo_core::Tx<'_>,
                       tag: &str,
                       x: f64,
                       row: usize,
                       w,
                       h,
                       loss,
                       limit: Option<f64>| {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                for (k, v) in [
                    ("width_mm", w),
                    ("height_mm", h),
                    ("depth_mm", 80.0),
                    ("power_loss_w", loss),
                ] {
                    tx.set_property(d, k, Some(Value::Float(v)))?;
                }
                if let Some(l) = limit {
                    tx.set_property(d, "max_ambient_c", Some(Value::Float(l)))?;
                }
                let rail = novo_core::RailId::from(rails[row]);
                tx.execute(Command::PlaceDevice {
                    device: d,
                    structure: cab,
                    x,
                    y: 40.0 + 350.0 * row as f64,
                    rotated: false,
                    rail: Some((rail, x - 20.0)),
                })?;
                Ok::<_, novo_core::ModelError>(d.raw())
            };
            hot = put(tx, "=A1+CAB1-T1", 30.0, 2, 120.0, 220.0, 150.0, Some(40.0))?;
            cool = put(tx, "=A1+CAB1-K1", 40.0, 1, 50.0, 90.0, 4.0, Some(45.0))?;
            Ok(cab)
        })
        .unwrap();
    (p, cab, hot, cool, rails)
}

#[test]
fn the_rails_become_rows_and_each_device_knows_its_row() {
    let (p, cab, hot, cool, rails) = cabinet();
    let b = scenario_from_project(&p, cab, &BridgeOptions::default()).unwrap();
    let r = rows(&p, cab, &b.scenario).unwrap();
    assert_eq!(r.rows.len(), 3);
    assert_eq!(r.rows.iter().map(|x| x.id).collect::<Vec<_>>(), rails);
    let at = |id: u64| b.scenario.devices.iter().position(|d| d.id == id).unwrap();
    assert_eq!(r.row_of[at(hot)], Some(2));
    assert_eq!(r.row_of[at(cool)], Some(1));
    // Rows start at the rail and end where it ends, in the interior frame.
    let (ox, oy) = plate_origin(&p, cab).unwrap();
    assert!((r.rows[0].x_min - (20.0 + ox)).abs() < 1e-6);
    assert!((r.rows[0].x_max - (780.0 + ox)).abs() < 1e-6);
    assert!((r.rows[0].y_top - (40.0 + oy)).abs() < 1e-6);
    // Room under a row: up to the next rail row less the clearance.
    assert!((r.rows[0].height_mm - (350.0 - 5.0)).abs() < 1e-6);
}

#[test]
fn a_search_then_apply_puts_the_device_on_a_rail_in_one_undo_step() {
    let (mut p, cab, hot, _cool, rails) = cabinet();
    let b = scenario_from_project(&p, cab, &BridgeOptions::default()).unwrap();
    let r = rows(&p, cab, &b.scenario).unwrap();
    let s = suggest(&b.scenario, &r.rows, &r.row_of, &Settings::default()).unwrap();
    assert!(!s.moves.is_empty(), "{:?}", s.notes);
    assert!(s.after.cost < s.before.cost);

    let (_, tx) = p
        .transact("Cooler layout", |tx| apply_moves(tx, cab, &s.moves))
        .unwrap();
    assert!(!tx.changes().is_empty());
    let layout = load(&p, cab).unwrap();
    for m in &s.moves {
        let placed = layout.placements.iter().find(|q| q.id == m.id).unwrap();
        assert!(rails.contains(&placed.rail.expect("still on a rail")));
        assert_eq!(placed.rail, Some(m.to_row));
    }
    // The scenario built again from the project shows the search's layout.
    let again = scenario_from_project(&p, cab, &BridgeOptions::default()).unwrap();
    for d in &s.scenario.devices {
        let q = again
            .scenario
            .devices
            .iter()
            .find(|x| x.id == d.id)
            .unwrap();
        assert!((q.footprint.x - d.footprint.x).abs() < 1e-6, "{}", d.id);
        assert!((q.footprint.y - d.footprint.y).abs() < 1e-6, "{}", d.id);
    }
    let _ = hot;
}
