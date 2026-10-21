//! Workshop pack: refuses without anything to print, then writes the
//! template and the checklist into one folder next to the project, and
//! replaces its own files on a second run.

use novo_core::{DeviceTag, Value};
use novo_layout::template::Paper;

use super::Level;
use super::cabinet::Ask;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn cabinet_with_devices(d: &mut Drive) {
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            tx.set_property(
                part,
                "hole_pattern",
                Some(Value::Text("4,4,4.2;41,86,4.2".into())),
            )?;
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-K2"] {
                let device = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(device, part)?;
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn the_pack_needs_something_to_print_then_fills_one_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let file = dir.path().join("Plant 2.novo");
    assert!(d.app.save_to(&file));
    d.frame();

    let folder = dir.path().join("Plant 2 workshop");
    assert_eq!(d.app.export_workshop(), None);
    assert_eq!(d.app.messages().last().unwrap().level, Level::Warning);
    assert!(!folder.exists(), "nothing written for an empty project");

    cabinet_with_devices(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .structures()
        .find(|s| s.enclosure)
        .unwrap()
        .id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    let project = d.app.backend.document().unwrap().project();
    let layout = novo_layout::model::load(project, cab);
    let candidates = novo_layout::model::candidates(project, cab);
    d.app
        .cabinet_ask(Ask::PlaceAll, Some(cab), layout, &candidates);
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();

    assert_eq!(d.app.export_workshop().as_deref(), Some(folder.as_path()));
    let template = folder.join("1 Drilling template +CAB1 A4.pdf");
    let bytes = std::fs::read(&template).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(text.contains("Drilling template 1:1"), "{text}");
    assert!(folder.join("5 Assembly checklist.pdf").exists());
    // No wires yet: the wire steps are left out and the summary says so.
    assert!(!folder.join("2 Cut list.pdf").exists());
    let last = &d.app.messages().last().unwrap().text;
    assert!(
        last.contains("Left out") && last.contains("cut list"),
        "{last}"
    );

    // The paper choice changes the template, and a second run replaces it.
    d.app.wb.workshop_paper = Paper::A3;
    d.app.export_workshop().expect("again");
    assert!(folder.join("1 Drilling template +CAB1 A3.pdf").exists());
}
