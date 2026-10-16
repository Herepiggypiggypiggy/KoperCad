use lopdf::Document;
use novo_core::{DeviceTag, MountingPlate, Project, Value};
use novo_layout::model::{apply_plan, candidates, load, plate_documents, set_plate};
use novo_pdf::plate::export_plate_pdf;

fn docs(devices: usize) -> Vec<novo_layout::plate_drawing::PlateDocument> {
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
    plate_documents(&p, cab).unwrap().0
}

#[test]
fn a_small_layout_is_one_sheet_and_deterministic() {
    let docs = docs(3);
    let a = export_plate_pdf(&docs, "Demo");
    assert_eq!(a, export_plate_pdf(&docs, "Demo"));
    let pdf = Document::load_mem(&a).unwrap();
    assert_eq!(pdf.get_pages().len(), 1);
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(text.contains("Drill holes"), "{text}");
    assert!(text.contains("1200") && text.contains("1800"), "{text}");
    assert!(text.contains("Scale 1:10"), "{text}");
}

#[test]
fn a_long_table_continues_on_further_sheets() {
    let docs = docs(60);
    let pdf = Document::load_mem(&export_plate_pdf(&docs, "Demo")).unwrap();
    let pages = pdf.get_pages().len();
    assert!(pages >= 2, "{pages}");
    let last = pdf.extract_text(&[pages as u32]).unwrap();
    assert!(last.contains("(continued)"), "{last}");
    assert!(
        last.contains(&format!("Sheet {pages} of {pages}")),
        "{last}"
    );
}
