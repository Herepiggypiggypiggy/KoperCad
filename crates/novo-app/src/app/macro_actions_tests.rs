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
    assert!(d.app.wb.macro_save.as_ref().unwrap().error.is_some());
    d.app.wb.macro_save = None;

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
