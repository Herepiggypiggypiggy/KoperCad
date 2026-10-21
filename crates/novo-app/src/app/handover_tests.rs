//! Handover package: refuses an empty project, then writes the numbered
//! files, names what it left out, and replaces its own files on a second run.

use novo_core::{DeviceTag, Value};

use super::Level;
use super::gate1_tests::Drive;
use crate::actions::Action;

#[test]
fn the_package_fills_one_folder_and_says_what_it_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let file = dir.path().join("Plant.novo");
    assert!(d.app.save_to(&file));
    d.frame();

    let folder = dir.path().join("Plant handover");
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Parts", |tx| {
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            let device = tx.add_device(DeviceTag::parse("=A1+CAB1-K1").unwrap(), "")?;
            tx.assign_part(device, part)?;
            Ok(())
        })
        .unwrap();
    d.frame();
    let first_page = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .pages_in_order()
        .first()
        .map(|p| p.name.to_string())
        .unwrap();
    d.app.settings.plot_sets.insert(
        crate::app::pdf::owner_tag(&file),
        vec![crate::settings::PlotSetChoice {
            name: "Issue A3".into(),
            pages: vec![first_page],
            ..Default::default()
        }],
    );

    assert_eq!(
        d.app.export_handover_to(&folder).as_deref(),
        Some(folder.as_path())
    );
    for name in [
        "0 Contents.pdf",
        "1 Drawing set.pdf",
        "2 Revisions.pdf",
        "3 Parts list.pdf",
        "9 Lists.xlsx",
        "1 Plot set Issue A3.pdf",
    ] {
        assert!(folder.join(name).exists(), "{name}");
    }
    // Nothing wired, no strips, no cables, no safety function, no commissioning.
    for name in [
        "4 Wire list.pdf",
        "5 Terminal plan.pdf",
        "6 Cable schedule.pdf",
        "7 Commissioning record.pdf",
        "8 Safety functions.pdf",
    ] {
        assert!(!folder.join(name).exists(), "{name}");
    }
    // No plate and no wires: no workshop pack.
    assert!(!folder.join("Workshop").exists());
    let last = &d.app.messages().last().unwrap().text;
    assert!(
        last.contains("Left out") && last.contains("4 Wire list.pdf"),
        "{last}"
    );

    // The contents page carries the exact-or-estimate table.
    let bytes = std::fs::read(folder.join("0 Contents.pdf")).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let pages: Vec<u32> = pdf.get_pages().keys().copied().collect();
    let text = pdf.extract_text(&pages).unwrap();
    assert!(text.contains("exact or estimated"), "{text}");
    assert!(text.contains("Left out"), "{text}");

    // An old file this run no longer writes is removed, the user's own file stays.
    std::fs::write(folder.join("8 Safety functions.pdf"), b"old").unwrap();
    std::fs::write(folder.join("Customer notes.pdf"), b"theirs").unwrap();
    std::fs::create_dir_all(folder.join("Workshop")).unwrap();
    // A second run replaces its own files.
    assert!(d.app.export_handover_to(&folder).is_some());
    assert_eq!(d.app.messages().last().unwrap().level, Level::Info);
    assert!(
        !folder.join("8 Safety functions.pdf").exists(),
        "stale file removed"
    );
    assert!(folder.join("Customer notes.pdf").exists());
    let bytes = std::fs::read(folder.join("0 Contents.pdf")).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let pages: Vec<u32> = pdf.get_pages().keys().copied().collect();
    let text = pdf.extract_text(&pages).unwrap();
    assert!(text.contains("earlier package is still here"), "{text}");
    let last = &d.app.messages().last().unwrap().text;
    assert!(
        last.contains("replaced") && last.contains("removed"),
        "{last}"
    );
}
