//! Save as macro and Place macro: reached by command, the preview names the
//! tags before anything changes, and a placement is one undo step.

use novo_core::AnyId;
use novo_parts::MacroLibrary;

use super::gate1_tests::Drive;
use super::macro_actions::PlaceWindow;

#[test]
fn a_saved_selection_is_previewed_and_placed_with_the_next_free_tag() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.macro_place =
        PlaceWindow::new(Some(MacroLibrary::new(dir.path().join("Macros"), "user")));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    let breaker = d.place("mcb3", 80.0, 48.0);
    d.frame();

    // Both commands are reached from the command field and run in the app.
    for id in ["macro.save", "macro.place"] {
        assert!(super::workbench::app_runs(id));
        assert!(novo_workflow::commands::get(id).is_some());
    }

    d.app
        .canvas
        .schematic()
        .unwrap()
        .select_ids(&[AnyId::Symbol(breaker)]);
    d.app.run_command(&ctx, "macro.save", Some("Breaker"));
    let draft = d.app.wb.macro_save.take().expect("the window opens");
    assert_eq!(draft.name, "Breaker");
    d.app.submit_macro_save(draft);
    assert!(d.app.wb.macro_save.is_none(), "saved without a complaint");
    assert!(dir.path().join("Macros").join("Breaker.kcmacro").is_file());

    // Saving the same name again says so and keeps the window.
    d.app.run_command(&ctx, "macro.save", Some("Breaker"));
    let again = d.app.wb.macro_save.take().unwrap();
    d.app.submit_macro_save(again);
    let refused = d.app.wb.macro_save.take().unwrap();
    assert!(refused.error.is_some() && refused.exists);

    // Replace writes over it and bumps the revision.
    d.app.submit_macro_replace(refused);
    assert!(
        d.app.wb.macro_save.is_none(),
        "replaced without a complaint"
    );
    let library = MacroLibrary::new(dir.path().join("Macros"), "user");
    assert_eq!(library.load("Breaker.kcmacro").unwrap().revision, 2);

    // Place: the preview names the tag it would take.
    let devices = d.doc().project().devices().count();
    d.app.run_command(&ctx, "macro.place", Some("Breaker"));
    d.app.wb.macro_place.select("Breaker.kcmacro");
    d.frame();
    let text = d.app.wb.macro_place.preview_text().expect("a preview");
    assert!(text.starts_with("Takes -F"), "{text}");
    assert_eq!(d.doc().project().devices().count(), devices, "preview only");

    let depth = d.doc().undo_depth();
    d.app.place_picked_macro();
    assert_eq!(d.doc().project().devices().count(), devices + 1);
    assert_eq!(d.doc().undo_depth(), depth + 1, "one undo step");
    assert!(!d.app.wb.macro_place.open);
    assert_eq!(d.app.selection().len(), 1, "the placed symbol is selected");
}

#[test]
fn replace_overwrites_the_macro_save_refused_even_with_a_slash_in_the_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let library = MacroLibrary::new(dir.path().join("Macros"), "user");
    d.app.wb.macro_place = PlaceWindow::new(Some(library.clone()));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    let breaker = d.place("mcb3", 80.0, 48.0);
    d.frame();
    d.app
        .canvas
        .schematic()
        .unwrap()
        .select_ids(&[AnyId::Symbol(breaker)]);

    // An unrelated macro in a folder, and ours with a slash in its name.
    d.app.run_command(&ctx, "macro.save", Some("sub/x"));
    let ours = d.app.wb.macro_save.take().unwrap();
    d.app.submit_macro_save(ours);
    let other = {
        let mut file = library.load("sub_x.kcmacro").unwrap();
        file.name = "x".into();
        file.definition.name = "x".into();
        file.description = "unrelated".into();
        file
    };
    library.save("sub", &other).unwrap();

    d.app.run_command(&ctx, "macro.save", Some("sub/x"));
    let again = d.app.wb.macro_save.take().unwrap();
    d.app.submit_macro_save(again);
    let refused = d.app.wb.macro_save.take().unwrap();
    assert!(refused.exists);
    d.app.submit_macro_replace(refused);
    assert!(d.app.wb.macro_save.is_none());
    assert_eq!(library.load("sub_x.kcmacro").unwrap().revision, 2);
    let untouched = library.load("sub/x.kcmacro").unwrap();
    assert_eq!(untouched.description, "unrelated");
    assert_eq!(untouched.revision, 1);
}

#[test]
fn a_device_part_is_saved_with_the_macro_and_assigned_on_placement() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let library = MacroLibrary::new(dir.path().join("Macros"), "user");
    d.app.wb.macro_place = PlaceWindow::new(Some(library.clone()));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    let breaker = d.place("mcb3", 80.0, 48.0);
    d.frame();
    let device = d.doc().project().devices().next().unwrap().id;
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Part", |tx| {
            let part = tx.add_part("ABB", "S201-C16")?;
            tx.assign_part(device, part)
        })
        .unwrap();
    d.app
        .canvas
        .schematic()
        .unwrap()
        .select_ids(&[AnyId::Symbol(breaker)]);
    d.app.run_command(&ctx, "macro.save", Some("With part"));
    let draft = d.app.wb.macro_save.take().unwrap();
    d.app.submit_macro_save(draft);
    let saved = library.load("With part.kcmacro").unwrap();
    assert!(saved.extra.contains_key("parts"));

    d.app.run_command(&ctx, "macro.place", None);
    d.app.wb.macro_place.select("With part.kcmacro");
    d.frame();
    let text = d.app.wb.macro_place.preview_text().unwrap();
    assert!(text.contains("1 part assigned"), "{text}");
    d.app.place_picked_macro();
    let with_part = d
        .doc()
        .project()
        .devices()
        .filter(|dev| !dev.parts.is_empty())
        .count();
    assert_eq!(with_part, 2);
    assert_eq!(d.doc().project().parts().count(), 1, "the part is reused");
}

#[test]
fn the_command_field_finds_a_starter_circuit_and_puts_it_on_the_pointer() {
    use novo_workflow::palette::{Mode, Target};
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.macro_place =
        PlaceWindow::new(Some(MacroLibrary::new(dir.path().join("Macros"), "user")));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    d.app.wb.field.open(Mode::All, None);
    d.app.wb.field.type_text("motor starter");
    let rows = d.app.with_field(|field, sources| field.rows(sources));
    assert_eq!(
        rows[0].target,
        Target::Macro("Motor starter DOL".into()),
        "{:?}",
        rows.iter()
            .take(4)
            .map(|r| (r.label.clone(), r.score))
            .collect::<Vec<_>>()
    );
    d.app.run_target(&ctx, rows[0].target.clone());
    // The circuit follows the pointer; the form is not in the way.
    assert!(!d.app.wb.macro_place.open);
    assert!(d.app.wb.macro_place.ghost.is_some());
    assert!(d.app.canvas.schematic_ref().unwrap().editor.pasting());
}

#[test]
fn a_circuit_ghost_is_placed_by_a_click_and_cancelled_by_escape() {
    use novo_workflow::palette::Target;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.macro_place =
        PlaceWindow::new(Some(MacroLibrary::new(dir.path().join("Macros"), "user")));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    let target = Target::Macro("Motor starter DOL".into());
    // Esc leaves nothing behind.
    d.app.run_target(&ctx, target.clone());
    d.frame();
    d.editor_leave();
    d.frame();
    assert!(d.app.wb.macro_place.ghost.is_none());
    assert_eq!(d.doc().project().symbols().count(), 0);
    // A click places it at the click, as one undo step, and the ghost sits
    // under the pointer: what it showed is where the circuit lands.
    d.app.run_target(&ctx, target.clone());
    d.frame();
    let page = d.app.current_page.unwrap();
    let ghost_left = {
        let app = &mut d.app;
        let view = app.canvas.schematic().unwrap();
        let doc = app.backend.document_mut().unwrap();
        view.editor
            .hover(doc, page, novo_core::Point::new(200.0, 60.0), 2.0);
        let preview = view.editor.preview(doc, page);
        preview
            .symbols
            .iter()
            .map(|s| s.placement.origin.x)
            .fold(f64::INFINITY, f64::min)
    };
    assert!(
        (200.0..=225.0).contains(&ghost_left),
        "the ghost starts at the pointer, not at the circuit's own origin: {ghost_left}"
    );
    d.click(novo_core::Point::new(200.0, 60.0));
    d.frame();
    assert!(d.app.wb.macro_place.ghost.is_none());
    let left = d
        .doc()
        .project()
        .symbols()
        .map(|s| s.placement.origin.x)
        .fold(f64::INFINITY, f64::min);
    assert!(
        d.doc().project().symbols().count() > 1 && (left - ghost_left).abs() < 0.01,
        "the circuit lands where the ghost was: {left} against {ghost_left}"
    );
    d.app.undo_redo(false);
    d.frame();
    assert_eq!(d.doc().project().symbols().count(), 0, "one undo step");
    // A click outside the sheet changes nothing and drops the ghost.
    d.app.run_target(&ctx, target.clone());
    d.frame();
    d.click(novo_core::Point::new(-500.0, -500.0));
    d.frame();
    assert!(d.app.wb.macro_place.ghost.is_none());
    assert_eq!(d.doc().project().symbols().count(), 0, "nothing placed");
    // Options hands the circuit to the form.
    d.app.run_target(&ctx, target);
    d.frame();
    d.frame();
    assert!(d.app.wb.macro_place.ghost.is_some());
    assert_eq!(
        d.app.macro_ghost_name().map(|(n, _)| n),
        Some("Motor starter DOL")
    );
    assert!(
        d.app
            .mode_chips()
            .iter()
            .any(|c| c.label.contains("Options") && c.command == "macro.place"),
        "the status bar chip offers the form"
    );
    // The keyboard reaches the form too: O.
    d.key(eframe::egui::Key::O, eframe::egui::Modifiers::NONE);
    assert!(d.app.wb.macro_place.ghost.is_none(), "O opens the form");
    assert!(d.app.wb.macro_place.open);
    d.app.wb.macro_place.open = false;
    d.app.run_target(
        &ctx,
        novo_workflow::palette::Target::Macro("Motor starter DOL".into()),
    );
    d.frame();
    d.app.run_command(&ctx, "macro.place", None);
    d.frame();
    assert!(d.app.wb.macro_place.ghost.is_none());
    assert!(d.app.wb.macro_place.open, "the form opens on the circuit");
    assert!(d.app.wb.macro_place.selected.is_some());
    assert!(!d.app.canvas.schematic_ref().unwrap().editor.pasting());
}

/// Audit C-59: the PLC input card macro only draws symbols, so placing it says
/// where to get a real card; a macro without PLC channels says nothing.
#[test]
fn only_a_macro_with_plc_channels_points_to_add_plc_card() {
    let starters = novo_parts::starter_macros();
    let draws = |name: &str| {
        let (_, file) = starters
            .iter()
            .find(|(_, f)| f.name == name)
            .unwrap_or_else(|| panic!("starter {name}"));
        super::macro_actions::draws_plc_channels(&file.definition)
    };
    assert!(draws("PLC input card"));
    assert!(!draws("Frequency converter drive"));
}

#[test]
fn a_macro_never_lands_on_a_generated_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.macro_place =
        PlaceWindow::new(Some(MacroLibrary::new(dir.path().join("Macros"), "user")));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    let breaker = d.place("mcb3", 80.0, 48.0);
    d.frame();
    d.app
        .canvas
        .schematic()
        .unwrap()
        .select_ids(&[AnyId::Symbol(breaker)]);
    d.app.run_command(&ctx, "macro.save", Some("Breaker"));
    let draft = d.app.wb.macro_save.take().unwrap();
    d.app.submit_macro_save(draft);
    let drawing = d.app.current_page.unwrap();

    // The contents page is generated; open it and place from there.
    d.app.add_report_page(novo_reports::TableOfContents::ID);
    d.frame();
    let contents = d
        .doc()
        .project()
        .pages()
        .find(|p| matches!(p.kind, novo_core::PageKind::Generated { .. }))
        .expect("a contents page")
        .id;
    d.app.show_page(contents);
    d.frame();
    d.app.run_command(&ctx, "macro.place", Some("Breaker"));
    d.app.wb.macro_place.select("Breaker.kcmacro");
    d.frame();
    assert_eq!(d.app.macro_target_page(), Some(drawing));
    d.app.place_picked_macro();
    assert_eq!(
        d.app.current_page,
        Some(drawing),
        "the canvas goes to the drawing page"
    );
    let on = |page| {
        d.doc()
            .project()
            .symbols()
            .filter(|s| s.page == page)
            .count()
    };
    assert_eq!(on(contents), 0, "nothing was drawn on the contents page");
    assert_eq!(
        on(drawing),
        2,
        "the macro joined the breaker on the drawing page"
    );
}

#[test]
fn a_circuit_lands_where_the_pointer_last_was_and_says_what_is_left_to_connect() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.macro_place =
        PlaceWindow::new(Some(MacroLibrary::new(dir.path().join("Macros"), "user")));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    let breaker = d.place("mcb3", 20.0, 20.0);
    d.frame();
    d.app
        .canvas
        .schematic()
        .unwrap()
        .select_ids(&[AnyId::Symbol(breaker)]);
    d.app.run_command(&ctx, "macro.save", Some("Breaker"));
    let draft = d.app.wb.macro_save.take().unwrap();
    d.app.submit_macro_save(draft);
    let page = d.app.current_page.unwrap();
    // The pointer rested on the drawing, then moved to the palette.
    d.app.wb.last_pointer = Some((page, novo_core::Point::new(200.0, 60.0)));
    let before = d.doc().project().symbols().count();
    d.app.run_command(&ctx, "macro.place", Some("Breaker"));
    d.app.wb.macro_place.select("Breaker.kcmacro");
    d.frame();
    d.app.place_picked_macro();
    assert!(d.doc().project().symbols().count() > before, "placed");
    let left = d
        .doc()
        .project()
        .symbols()
        .filter(|s| s.id != breaker)
        .map(|s| s.placement.origin.x)
        .fold(f64::INFINITY, f64::min);
    assert!(
        (190.0..=225.0).contains(&left),
        "the circuit starts at the pointer, not the view corner: {left}"
    );
    let said = d.app.messages.iter().any(|m| m.text.contains("to connect"));
    assert!(
        said,
        "{:?}",
        d.app.messages.iter().map(|m| &m.text).collect::<Vec<_>>()
    );
}

#[test]
fn a_pointer_beside_the_sheet_still_puts_the_circuit_on_the_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.macro_place =
        PlaceWindow::new(Some(MacroLibrary::new(dir.path().join("Macros"), "user")));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    let breaker = d.place("mcb3", 20.0, 20.0);
    d.frame();
    d.app
        .canvas
        .schematic()
        .unwrap()
        .select_ids(&[AnyId::Symbol(breaker)]);
    d.app.run_command(&ctx, "macro.save", Some("Breaker"));
    let draft = d.app.wb.macro_save.take().unwrap();
    d.app.submit_macro_save(draft);
    let page = d.app.current_page.unwrap();
    let (w, h) = d.doc().project().page(page).unwrap().format.size_mm();
    for spot in [(-300.0, -300.0), (w + 300.0, h + 300.0), (-300.0, 100.0)] {
        d.app.wb.last_pointer = Some((page, novo_core::Point::new(spot.0, spot.1)));
        let before: Vec<_> = d.doc().project().symbols().map(|s| s.id).collect();
        d.app.run_command(&ctx, "macro.place", Some("Breaker"));
        d.app.wb.macro_place.select("Breaker.kcmacro");
        d.frame();
        d.app.place_picked_macro();
        let placed: Vec<_> = d
            .doc()
            .project()
            .symbols()
            .filter(|s| !before.contains(&s.id))
            .map(|s| s.placement.origin)
            .collect();
        assert!(!placed.is_empty(), "placed from {spot:?}");
        assert!(
            placed
                .iter()
                .all(|p| (0.0..=w).contains(&p.x) && (0.0..=h).contains(&p.y)),
            "on the sheet for a pointer at {spot:?}: {placed:?}"
        );
    }
}

#[test]
fn a_circuit_ghost_near_the_sheet_edge_is_drawn_where_it_will_land() {
    use novo_workflow::palette::Target;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.macro_place =
        PlaceWindow::new(Some(MacroLibrary::new(dir.path().join("Macros"), "user")));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    d.app
        .run_target(&ctx, Target::Macro("Motor starter DOL".into()));
    d.frame();
    let page = d.app.current_page.unwrap();
    let (w, h) = d.doc().project().page(page).unwrap().format.size_mm();
    // The pointer low and to the right: over the title block.
    let at = novo_core::Point::new(w - 30.0, h - 20.0);
    {
        let app = &mut d.app;
        let view = app.canvas.schematic().unwrap();
        let doc = app.backend.document_mut().unwrap();
        view.editor.hover(doc, page, at, 2.0);
    }
    // The frame loop does this each frame; a frame with no real pointer
    // would clear the pointer, so call it directly.
    d.app.keep_macro_ghost_on_sheet();
    let ghost: Vec<novo_core::Point> = {
        let app = &mut d.app;
        let view = app.canvas.schematic().unwrap();
        let doc = app.backend.document_mut().unwrap();
        view.editor
            .preview(doc, page)
            .symbols
            .iter()
            .map(|s| s.placement.origin)
            .collect()
    };
    assert!(!ghost.is_empty());
    assert!(
        ghost
            .iter()
            .all(|p| p.x > 0.0 && p.x < w - 10.0 && p.y > 0.0 && p.y < h - 10.0),
        "the ghost stays on the sheet: {ghost:?}"
    );
    assert!(
        d.app.macro_ghost_name().is_some_and(|(_, moved)| moved),
        "the chip says it was moved"
    );
    let ghost_left = ghost.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let ghost_top = ghost.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    d.click(at);
    d.frame();
    let placed: Vec<novo_core::Point> = d
        .doc()
        .project()
        .symbols()
        .map(|s| s.placement.origin)
        .collect();
    let left = placed.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let top = placed.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    assert!(
        (left - ghost_left).abs() < 0.01 && (top - ghost_top).abs() < 0.01,
        "it lands where the ghost was drawn: ({left}, {top}) against ({ghost_left}, {ghost_top})"
    );
}
