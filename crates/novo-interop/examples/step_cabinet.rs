//! Writes a sample cabinet as STEP: `cargo run -p novo-interop --example step_cabinet -- out.stp`.
use novo_core::{DeviceTag, MountingPlate, Project, Value};
use novo_interop::step::{StepOptions, export_enclosure};
use novo_layout::model::{apply_plan, candidates, load, set_plate};
use novo_view3d::build::Options;

fn main() {
    let out = std::env::args().nth(1).expect("output path");
    let mut p = Project::new("sample");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(90)))?;
            tx.set_property(part, "depth_mm", Some(Value::Int(70)))?;
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-K2", "=A1+CAB1-K3"] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
            }
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 600.0,
                    height: 800.0,
                    margin: 10.0,
                },
            )?;
            Ok(cab)
        })
        .unwrap();
    let plan = load(&p, cab).unwrap().preview(&candidates(&p, cab));
    p.transact("place", |tx| apply_plan(tx, cab, &plan))
        .unwrap();
    let step = export_enclosure(&p, cab, &Options::default(), &StepOptions::default()).unwrap();
    std::fs::write(&out, &step.text).unwrap();
    println!("{}", step.report.text());
}
