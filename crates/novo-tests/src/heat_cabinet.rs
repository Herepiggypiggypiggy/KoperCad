//! A small control cabinet in which where the devices sit decides whether
//! they stay inside their temperature limits (fixture of the heat-aware
//! Place all). All figures are planning values made up for the test, not
//! maker data.

/// A cabinet where position decides the outcome: three 45 W drives (limit
/// 55 C) and a row of PLC modules with a 45 C limit. The rules alone put the
/// modules in the middle zone above the drives; hot air rises into them.
pub fn heat_cabinet() -> novo_core::Document {
    use novo_core::{DeviceTag, MountingPlate, Value};
    use novo_layout::enclosure::Enclosure;
    use novo_layout::model::{set_enclosure, set_plate};
    let mut project = novo_core::Project::new("heat-place-all-synthetic");
    project
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 800.0,
                    height: 1300.0,
                    margin: 10.0,
                },
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 900.0,
                    height: 1400.0,
                    depth: 300.0,
                    plate_setback: 0.0,
                    door: None,
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            // (kind, count, w, h, loss W, limit C)
            let kinds: [(&str, usize, f64, f64, f64, i64); 4] = [
                ("Q", 6, 18.0, 90.0, 2.0, 60),
                ("K", 8, 45.0, 90.0, 4.0, 60),
                ("A", 8, 45.0, 120.0, 3.0, 45),
                ("T", 3, 140.0, 260.0, 45.0, 55),
            ];
            for (kind, count, w, h, loss, limit) in kinds {
                let part = tx.add_part("ACME", format!("{kind}-part"))?;
                tx.set_property(part, "width_mm", Some(Value::Float(w)))?;
                tx.set_property(part, "height_mm", Some(Value::Float(h)))?;
                tx.set_property(part, "depth_mm", Some(Value::Int(100)))?;
                tx.set_property(part, "power_loss_w", Some(Value::Float(loss)))?;
                tx.set_property(part, "max_ambient_c", Some(Value::Int(limit)))?;
                for n in 1..=count {
                    let d = tx.add_device(
                        DeviceTag::parse(&format!("=A1+CAB1-{kind}{n}")).unwrap(),
                        "",
                    )?;
                    tx.assign_part(d, part)?;
                }
            }
            Ok(())
        })
        .expect("synthetic project");
    novo_core::Document::new(project)
}
