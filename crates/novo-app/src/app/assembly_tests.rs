//! Export assembly checklist: refuses without a plate, then writes the
//! workshop PDF next to the project and replaces only its own file.

use novo_core::{DeviceTag, Value};

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
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-K2"] {
                let device = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(device, part)?;
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn the_checklist_needs_a_plate_then_lands_next_to_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let file = dir.path().join("Plant 2.novo");
    assert!(d.app.save_to(&file));
    d.frame();

    // Without a cabinet there is nothing to build: a warning, no file.
    assert_eq!(d.app.export_assembly(), None);
    let last = d.app.messages().last().unwrap();
    assert_eq!(last.level, Level::Warning);
    assert!(last.text.contains("mounting plate"), "{}", last.text);
    assert!(!dir.path().join("Plant 2 assembly.pdf").exists());

    // A cabinet with a plate and its devices placed.
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

    let pdf = d.app.export_assembly().expect("exported");
    assert_eq!(pdf, dir.path().join("Plant 2 assembly.pdf"));
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    // Six steps for the one cabinet, each starting its own page.
    let pages = lopdf::Document::load_mem(&bytes).unwrap().get_pages().len();
    assert!(pages >= 6, "{pages} pages");

    // A second export replaces our own file and says so.
    d.app.export_assembly().expect("exported again");
    let last = &d.app.messages().last().unwrap().text;
    assert!(last.starts_with("Replaced "), "{last}");
    assert_eq!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| {
                let n = e.as_ref().unwrap().file_name();
                n.to_string_lossy().contains("assembly")
            })
            .count(),
        1
    );
}

#[test]
fn a_pdf_the_user_made_is_never_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let file = dir.path().join("Plant.novo");
    assert!(d.app.save_to(&file));
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
    d.frame();
    std::fs::write(dir.path().join("Plant assembly.pdf"), b"%PDF mine").unwrap();
    let pdf = d.app.export_assembly().expect("exported");
    assert_eq!(pdf, dir.path().join("Plant assembly 2.pdf"));
    assert_eq!(
        std::fs::read(dir.path().join("Plant assembly.pdf")).unwrap(),
        b"%PDF mine"
    );
}
