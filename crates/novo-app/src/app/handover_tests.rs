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

#[test]
fn open_problems_are_asked_about_before_anything_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let summary = novo_checks::Summary::of(d.app.problems());
    assert!(
        summary.errors + summary.warnings > 0,
        "the example carries a warning"
    );
    d.app.ask_handover();
    assert!(
        d.app.wb.handover_ask.is_some(),
        "asked, nothing written yet"
    );
    assert!(d.app.wb.last_export.is_none());
}

#[test]
fn the_question_lists_errors_first_and_a_row_jumps_to_its_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    // A lone contactor: its pins are open, so the checks warn about a symbol
    // that is drawn on the page.
    let symbol = d.place("contactor", 100.0, 100.0);
    d.frame();
    d.app.ask_handover();
    let ask = d.app.wb.handover_ask.as_ref().expect("asked");
    assert!(!ask.lines.is_empty());
    let first_warning = ask.lines.iter().position(|l| !l.error);
    let last_error = ask.lines.iter().rposition(|l| l.error);
    if let (Some(w), Some(e)) = (first_warning, last_error) {
        assert!(e < w, "errors come before warnings");
    }
    let place = ask.lines.iter().find_map(|l| l.place).unwrap_or_else(|| {
        let all: Vec<_> = ask.lines.iter().map(|l| l.message.clone()).collect();
        panic!("no problem is drawn: {all:?}")
    });
    assert_eq!(place.symbol, Some(symbol));
    // A frame with the question open must not write anything or lose it.
    d.frame();
    assert!(d.app.wb.handover_ask.is_some());
    assert!(d.app.wb.last_export.is_none());
    d.app.jump(place, true);
    d.frame();
    assert_eq!(d.app.current_page, Some(place.page));
}

#[test]
fn a_problem_that_is_not_drawn_has_no_place_to_jump_to() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.app.ask_handover();
    let ask = d.app.wb.handover_ask.as_ref().expect("asked");
    // The example's one warning is the cabinet door: it is on no page, so
    // its row offers the Issues list instead of a jump.
    assert!(ask.lines.iter().all(|l| l.place.is_none()));
}

#[test]
fn a_package_handed_over_with_open_problems_says_so_in_its_notes() {
    use super::handover::open_problems_note;
    let s = |errors, warnings| novo_checks::Summary {
        errors,
        warnings,
        infos: 3,
    };
    assert_eq!(open_problems_note(&s(0, 0)), None);
    let note = open_problems_note(&s(1, 2)).unwrap();
    assert!(note.contains("1 error · 2 warnings"), "{note}");
    assert!(note.starts_with("Handed over with open problems"));
}
