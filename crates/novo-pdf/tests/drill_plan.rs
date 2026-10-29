use lopdf::Document;
use novo_core::{DeviceTag, MountingPlate, Project, Value};
use novo_layout::model::{apply_plan, candidates, drill_plan, load, set_plate};
use novo_pdf::drill_plan::export_drill_plan_pdf;

fn project(devices: usize, basis: Option<&str>) -> (Project, novo_core::StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(20.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(60)))?;
            tx.set_property(
                part,
                "hole_pattern",
                Some(Value::Text("3,3,4.2;17,57,4.2".into())),
            )?;
            if let Some(b) = basis {
                tx.set_property(part, "drill_basis", Some(Value::Text(b.into())))?;
            }
            for n in 1..=devices {
                let d = tx.add_device(DeviceTag::parse(&format!("=A1+CAB1-K{n}")).unwrap(), "")?;
                tx.assign_part(d, part)?;
            }
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 1200.0,
                    height: 1800.0,
                    margin: 10.0,
                },
            )?;
            Ok(cab)
        })
        .unwrap();
    let plan = load(&p, cab).unwrap().preview(&candidates(&p, cab));
    p.transact("place", |tx| apply_plan(tx, cab, &plan))
        .unwrap();
    (p, cab)
}

#[test]
fn the_plan_pdf_has_datum_tools_and_provenance() {
    let (p, cab) = project(3, Some("maker"));
    let plan = drill_plan(&p, cab, "Demo").unwrap();
    let bytes = export_drill_plan_pdf(&plan);
    assert_eq!(bytes, export_drill_plan_pdf(&plan));
    let pdf = Document::load_mem(&bytes).unwrap();
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(text.contains("Datum 0 / 0"), "{text}");
    assert!(text.contains("Drill dia 4.2 mm"), "{text}");
    assert!(text.contains("maker data"), "{text}");
    assert!(text.contains("1200") && text.contains("1800"), "{text}");
}

#[test]
fn a_part_without_a_recorded_origin_is_not_called_maker_data() {
    let (p, cab) = project(2, None);
    let plan = drill_plan(&p, cab, "Demo").unwrap();
    let steps: Vec<_> = plan.sheets[0]
        .groups
        .iter()
        .flat_map(|g| &g.steps)
        .filter(|s| s.source.starts_with("=A1"))
        .collect();
    assert!(!steps.is_empty());
    assert!(
        steps
            .iter()
            .all(|s| s.basis == novo_layout::drill_plan::Basis::PartData)
    );
}

#[test]
fn many_devices_flow_onto_further_pages() {
    let (p, cab) = project(60, Some("typed"));
    let plan = drill_plan(&p, cab, "Demo").unwrap();
    let pdf = Document::load_mem(&export_drill_plan_pdf(&plan)).unwrap();
    assert!(pdf.get_pages().len() >= 3);
}
