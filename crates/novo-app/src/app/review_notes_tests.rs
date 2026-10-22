//! Review notes through the window: writing one on the selection, the
//! bubbles on the canvas, going to a note, the panel, the project file, the
//! PDF and the handover package.

use novo_canvas::MarkKind;
use novo_core::AnyId;
use novo_revisions::{NoteFilter, NoteStatus, NoteTarget};

use super::gate1_tests::Drive;
use super::review_notes::Show;
use crate::actions::Action;

fn drive(dir: &tempfile::TempDir) -> Drive {
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d
}

fn send(d: &mut Drive, text: &str) {
    let compose = d.app.wb.notes.compose.as_mut().expect("the form is open");
    compose.text = text.to_owned();
    d.app.send_compose();
    d.frame();
}

#[test]
fn the_commands_exist_run_and_sit_in_the_check_menu() {
    for id in ["notes.panel", "notes.add", "notes.next"] {
        assert!(novo_workflow::commands::get(id).is_some(), "{id}");
        assert!(super::workbench::app_runs(id), "{id}");
    }
    let check = novo_workflow::menus::layout("Check");
    let ids: Vec<&str> = check
        .iter()
        .filter_map(|i| match i {
            novo_workflow::menus::MenuItem::Command(c) => Some(c.id),
            _ => None,
        })
        .collect();
    assert!(
        ids.contains(&"notes.panel") && ids.contains(&"notes.add"),
        "{ids:?}"
    );
}

#[test]
fn a_note_on_the_selection_is_about_the_device_and_shows_a_bubble() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let page = d.app.current_page.unwrap();
    let mcb = d.place("mcb3", 80.0, 48.0);
    d.select(mcb);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "notes.add", None);
    d.frame();
    assert!(d.app.wb.notes.open, "the panel opens with the form");
    send(&mut d, "  Is a 16 A breaker enough?  ");

    let store = d.app.wb.notes.store().unwrap();
    assert_eq!(store.len(), 1);
    let note = store.iter().next().unwrap();
    assert_eq!(note.text, "Is a 16 A breaker enough?");
    assert_eq!(note.status, NoteStatus::Open);
    let NoteTarget::Object {
        object: AnyId::Device(_),
    } = note.target
    else {
        panic!(
            "a symbol of a device stands for the device: {:?}",
            note.target
        );
    };
    assert_eq!(d.app.wb.notes.open_count(), 1);
    assert!(d.app.wb.notes.compose.is_none());

    let marks = d.app.notes_marks(page);
    assert_eq!(marks.len(), 1, "{marks:?}");
    assert_eq!(marks[0].kind, MarkKind::Note);
    assert!(marks[0].text.starts_with("N1 "), "{}", marks[0].text);
    // The notes are not part of the drawing: nothing to undo.
    assert!(
        d.app
            .backend
            .review_notes_json()
            .unwrap()
            .contains("16 A breaker")
    );
}

#[test]
fn an_empty_text_is_refused_and_the_form_stays() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "notes.add", None);
    d.frame();
    send(&mut d, "   ");
    assert!(
        d.app.wb.notes.compose.is_some(),
        "kept, so nothing typed is lost"
    );
    assert_eq!(d.app.wb.notes.store().unwrap().len(), 0);
}

#[test]
fn with_nothing_selected_a_note_is_about_the_page_or_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let page = d.app.current_page.unwrap();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "notes.add", None);
    d.frame();
    {
        let compose = d.app.wb.notes.compose.as_ref().unwrap();
        assert_eq!(
            compose.target(),
            NoteTarget::page(page),
            "the page comes first"
        );
    }
    send(&mut d, "Title block still says draft");
    d.app.run_command(&ctx, "notes.add", None);
    d.frame();
    d.app.wb.notes.compose.as_mut().unwrap().choose_last();
    send(&mut d, "Customer wants IEC symbols");
    let targets: Vec<_> = d
        .app
        .wb
        .notes
        .store()
        .unwrap()
        .iter()
        .map(|n| n.target)
        .collect();
    assert_eq!(targets, [NoteTarget::page(page), NoteTarget::Project]);
    // Neither has a bubble on the drawing.
    assert!(d.app.notes_marks(page).is_empty());
}

#[test]
fn a_right_click_spot_becomes_a_point_note_with_a_bubble_there() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let page = d.app.current_page.unwrap();
    d.app.canvas.schematic().unwrap().menu_world = Some(novo_core::Point::new(120.0, 70.0));
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "notes.add", None);
    d.frame();
    assert_eq!(
        d.app.wb.notes.compose.as_ref().unwrap().target(),
        NoteTarget::point(page, novo_core::Point::new(120.0, 70.0))
    );
    send(&mut d, "Room for a second contactor here");
    let marks = d.app.notes_marks(page);
    assert_eq!(marks.len(), 1);
    assert!(matches!(marks[0].target, novo_canvas::MarkTarget::Area(_)));
}

#[test]
fn going_to_a_note_opens_its_page_and_selects_the_object() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let first = d.app.current_page.unwrap();
    let mcb = d.place("mcb3", 80.0, 48.0);
    d.select(mcb);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "notes.add", None);
    d.frame();
    send(&mut d, "Check the rating");
    // Another page on screen.
    d.app.run_command(&ctx, "page.new", None);
    d.frame();
    assert_ne!(d.app.current_page, Some(first));
    d.app.jump_to_note(1);
    d.frame();
    d.frame();
    assert_eq!(d.app.current_page, Some(first));
    assert_eq!(d.app.selection(), vec![AnyId::Symbol(mcb)]);
    assert_eq!(d.app.wb.notes.focus, Some(1));
}

#[test]
fn next_note_walks_the_open_notes_and_wraps() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let page = d.app.current_page.unwrap();
    d.app.add_note(NoteTarget::page(page), "one");
    d.app.add_note(NoteTarget::Project, "two");
    d.app.add_note(NoteTarget::page(page), "three");
    let ctx = d.ctx.clone();
    let mut seen = Vec::new();
    for _ in 0..4 {
        d.app.run_command(&ctx, "notes.next", None);
        d.frame();
        seen.push(d.app.wb.notes.focus.unwrap());
    }
    // Page notes first, the project note last, then round again.
    assert_eq!(seen, [1, 3, 2, 1]);
}

#[test]
fn resolve_reply_reopen_and_delete_are_kept_in_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let page = d.app.current_page.unwrap();
    let id = d
        .app
        .add_note(NoteTarget::page(page), "Fix the title")
        .unwrap();
    d.app.resolve_note(id);
    assert_eq!(d.app.wb.notes.open_count(), 0);
    let json = d.app.backend.review_notes_json().unwrap();
    assert!(json.contains("resolved"), "{json}");
    d.app.reply_note(id, "Done in the template");
    assert_eq!(d.app.wb.notes.open_count(), 1, "a reply opens it again");
    d.app.delete_note(id);
    assert_eq!(d.app.wb.notes.store().unwrap().len(), 0);
    let again = d.app.add_note(NoteTarget::Project, "next").unwrap();
    assert_eq!(again, 2, "N1 is not given out twice");
}

#[test]
fn the_notes_come_back_when_the_project_is_saved_and_reopened() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let page = d.app.current_page.unwrap();
    d.app.add_note(NoteTarget::page(page), "Come back to this");
    let file = dir.path().join("Notes.novo");
    assert!(d.app.save_to(&file));
    d.frame();
    d.app.run(&d.ctx.clone(), Action::CloseProject);
    d.frame();
    let mut e = Drive::new(&dir.path().join("untitled2"));
    e.app.open_path(&file);
    e.frame();
    e.frame();
    e.app.ensure_notes_loaded();
    let store = e.app.wb.notes.store().expect("read from the file");
    assert_eq!(store.iter().next().unwrap().text, "Come back to this");
    assert_eq!(e.app.wb.notes.open_count(), 1);
}

#[test]
fn the_status_bar_counts_the_open_notes_and_the_chip_opens_the_panel() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let page = d.app.current_page.unwrap();
    assert!(
        d.app
            .mode_chips()
            .iter()
            .all(|c| c.command != "notes.panel")
    );
    d.app.add_note(NoteTarget::page(page), "a");
    d.app.add_note(NoteTarget::page(page), "b");
    let chips = d.app.mode_chips();
    let chip = chips
        .iter()
        .find(|c| c.command == "notes.panel")
        .expect("a chip");
    assert!(chip.label.contains("2 open notes"), "{}", chip.label);
}

#[test]
fn the_panel_draws_every_state_without_trouble() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let page = d.app.current_page.unwrap();
    let a = d.app.add_note(NoteTarget::page(page), "Open one").unwrap();
    let b = d.app.add_note(NoteTarget::Project, "Resolved one").unwrap();
    d.app.resolve_note(b);
    d.app.reply_note(a, "An answer");
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "notes.panel", None);
    for show in [Show::Open, Show::Resolved, Show::All] {
        d.app.wb.notes.show = show;
        d.frame();
    }
    d.app.run_command(&ctx, "notes.add", None);
    d.frame();
    d.app.run_command(&ctx, "notes.panel", None);
    assert!(!d.app.wb.notes.open);
}

#[test]
fn the_pdf_prints_open_notes_as_bubbles_and_a_list_only_when_asked() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let page = d.app.current_page.unwrap();
    let a = d
        .app
        .add_note(NoteTarget::page(page), "Check this page")
        .unwrap();
    let b = d
        .app
        .add_note(NoteTarget::page(page), "Already done")
        .unwrap();
    d.app.resolve_note(b);

    let plain = d
        .app
        .pdf_options_for(novo_pdf::PageSet::All, &Default::default(), None);
    assert!(plain.markers.is_empty() && plain.extra_appendices.is_empty());

    let choice = crate::settings::PdfChoice {
        review_notes: true,
        ..Default::default()
    };
    let options = d.app.pdf_options_for(novo_pdf::PageSet::All, &choice, None);
    let markers = &options.markers[&page];
    assert_eq!(markers.len(), 1, "open notes only");
    assert_eq!(markers[0].label, format!("N{a}"));
    assert_eq!(options.extra_appendices.len(), 1);
    assert_eq!(options.extra_appendices[0].title, "Review notes");
    let text: String = options.extra_appendices[0]
        .lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("Check this page") && !text.contains("Already done"),
        "{text}"
    );

    let out = dir.path().join("notes.pdf");
    assert_eq!(
        d.app
            .export_pdf_choice_to(&out, novo_pdf::PageSet::All, &choice),
        Some(out.clone())
    );
    let pages = lopdf::Document::load_mem(&std::fs::read(&out).unwrap())
        .unwrap()
        .get_pages()
        .len();
    let without = dir.path().join("plain.pdf");
    d.app.export_pdf_to(&without);
    let plain_pages = lopdf::Document::load_mem(&std::fs::read(&without).unwrap())
        .unwrap()
        .get_pages()
        .len();
    assert_eq!(pages, plain_pages + 1, "the list is one extra page");
}

#[test]
fn the_handover_package_leaves_the_notes_out_unless_they_are_ticked() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    d.place("mcb3", 80.0, 48.0);
    let page = d.app.current_page.unwrap();
    let none = dir.path().join("handover-none");
    assert!(d.app.export_handover_to(&none).is_some());
    assert!(!none.join("10 Review notes.pdf").exists());

    let id = d
        .app
        .add_note(NoteTarget::page(page), "Look at the cover")
        .unwrap();
    d.app.resolve_note(id);
    // Notes exist but were not ticked: still absent.
    let default = dir.path().join("handover-default");
    assert!(d.app.export_handover_to(&default).is_some());
    assert!(
        !default.join("10 Review notes.pdf").exists(),
        "internal remarks do not go to the customer by default"
    );
    // Ticked in the question before writing: present, and only for that run.
    d.app.wb.handover_notes = true;
    let with = dir.path().join("handover-with");
    assert!(d.app.export_handover_to(&with).is_some());
    assert!(
        std::fs::read(with.join("10 Review notes.pdf"))
            .unwrap()
            .starts_with(b"%PDF")
    );
    assert!(!d.app.wb.handover_notes, "the tick is used once");
    assert!(with.join("0 Contents.pdf").exists());
}

#[test]
fn the_handover_command_asks_about_the_notes_and_the_tick_starts_off() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    d.place("mcb3", 80.0, 48.0);
    let page = d.app.current_page.unwrap();
    d.app.add_note(NoteTarget::page(page), "Internal remark");
    d.app.wb.handover_notes = true;
    d.app.ask_handover();
    assert!(
        d.app.wb.handover_ask.is_some(),
        "notes make it ask even with no problems"
    );
    assert!(!d.app.wb.handover_notes, "the tick starts off every time");
    d.frame();
}

#[test]
fn a_project_without_the_notes_sidecar_opens_with_no_notes() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    assert_eq!(d.app.backend.review_notes_json(), None);
    let file = dir.path().join("Plain.novo");
    assert!(d.app.save_to(&file));
    d.app.run(&d.ctx.clone(), Action::CloseProject);
    d.frame();
    let mut e = Drive::new(&dir.path().join("untitled2"));
    e.app.open_path(&file);
    e.frame();
    e.app.ensure_notes_loaded();
    assert!(e.app.wb.notes.store().unwrap().is_empty());
    assert_eq!(e.app.wb.notes.open_count(), 0);
}

#[test]
fn a_note_on_a_deleted_object_is_still_listed_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive(&dir);
    let mcb = d.place("mcb3", 80.0, 48.0);
    d.select(mcb);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "notes.add", None);
    send(&mut d, "Replace this breaker");
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Delete", |tx| tx.remove(mcb))
        .unwrap();
    d.frame();
    let project = d.app.backend.document().unwrap().project();
    let lines = novo_revisions::notes_appendix_lines(
        project,
        d.app.wb.notes.store().unwrap(),
        &NoteFilter::default(),
    );
    assert!(
        lines.iter().any(|l| l.text.contains("target deleted"))
            || lines.iter().any(|l| l.text.contains("Device"))
    );
    d.app.jump_to_note(1);
    d.frame();
}
