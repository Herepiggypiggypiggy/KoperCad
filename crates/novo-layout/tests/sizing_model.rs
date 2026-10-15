use novo_core::{DeviceTag, Project, StructureId, Value};
use novo_layout::sizing_model::sizing_devices;

fn project() -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(90)))?;
            tx.set_property(part, "depth_mm", Some(Value::Int(70)))?;
            tx.set_property(part, "power_loss_w", Some(Value::Float(4.0)))?;
            tx.set_property(part, "clearance_above_mm", Some(Value::Int(30)))?;
            tx.set_property(part, "clearance_left_mm", Some(Value::Int(-5)))?;
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-K2", "=A1+FIELD-M1", "=A1+CAB1-K3"] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                if tag != "=A1+CAB1-K3" {
                    tx.assign_part(d, part)?;
                }
                if tag == "=A1+CAB1-K2" {
                    tx.set_property(d, "power_loss_w", Some(Value::Float(10.0)))?;
                    tx.set_property(d, "load_factor", Some(Value::Float(0.5)))?;
                }
            }
            Ok(cab)
        })
        .unwrap();
    (p, cab)
}

#[test]
fn reads_size_loss_and_clearance_with_device_overrides() {
    let (p, cab) = project();
    let mut d = sizing_devices(&p, cab);
    assert_eq!(d.len(), 3); // not the FIELD motor
    d.sort_by(|a, b| b.power_loss.total_cmp(&a.power_loss));
    assert_eq!(d[0].power_loss, 10.0); // device overrides part
    assert_eq!(d[0].load_factor, 0.5);
    assert_eq!(d[1].power_loss, 4.0);
    assert_eq!(d[1].depth, 70.0);
    assert_eq!(d[1].clearance, Some(30.0)); // negative left ignored
    assert!(!d[1].guessed);
    assert!(d[2].guessed && d[2].power_loss == 0.0 && d[2].clearance.is_none());
}
