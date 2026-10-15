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
