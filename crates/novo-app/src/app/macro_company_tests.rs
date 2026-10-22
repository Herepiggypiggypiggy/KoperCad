//! Company macro folder, update offers, setting an old copy aside, and the
//! picture of a macro in the browser.

use novo_core::AnyId;
use novo_parts::MacroLibrary;

use super::gate1_tests::Drive;
use super::macro_actions::PlaceWindow;
use super::macro_browser::MacroBrowser;

#[test]
fn a_company_folder_is_listed_and_a_macro_is_copied_to_my_macros() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.macros =
        MacroBrowser::with_library(MacroLibrary::new(dir.path().join("Mine"), "user"));
    let company = dir.path().join("Company");
    let theirs = MacroLibrary::new(&company, "company");
    theirs.install_starters().unwrap();

    let b = &mut d.app.wb.macros;
    assert!(!b.has_company());
    b.set_company_folder(Some(&company));
    assert!(b.has_company());
    assert!(b.entries().is_empty(), "still showing my (empty) library");
    b.show_company(true);
    assert_eq!(b.entries().len(), 17);

    b.selected = Some("Starter/E-stop chain.kcmacro".into());
    let thumb = b.thumbnail().expect("a picture of the selected macro");
    assert!(!thumb.symbols.is_empty());
    assert!(thumb.bounds.is_some());
    b.copy_selected_to_user();
    b.show_company(false);
    assert_eq!(b.entries().len(), 1, "{}", b.status);

    // Clearing the setting returns to my macros and drops the company one.
    b.show_company(true);
    b.set_company_folder(None);
    assert!(!b.company && !b.has_company());
}

#[test]
fn an_update_is_offered_and_the_old_copy_can_be_set_aside() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let library = MacroLibrary::new(dir.path().join("Macros"), "user");
    d.app.wb.macro_place = PlaceWindow::new(Some(library.clone()));
    d.app.wb.macros = MacroBrowser::with_library(library.clone());
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
    d.app.run_command(&ctx, "macro.place", Some("Breaker"));
    d.app.wb.macro_place.select("Breaker.kcmacro");
    d.frame();
    d.app.place_picked_macro();

    let none = d.app.wb.macros.updates_for(d.doc().project());
    assert!(none.is_empty(), "the project is current: {none:?}");

    // Someone replaces the macro: revision 2.
    let file = library.load("Breaker.kcmacro").unwrap();
    library.replace("Breaker.kcmacro", &file).unwrap();
    let offers = d.app.wb.macros.updates_for(d.doc().project());
    assert_eq!(offers.len(), 1, "{offers:?}");
    assert_eq!(offers[0].definition, "Breaker");
    assert_eq!(
        (offers[0].project_revision, offers[0].library_revision),
        (1, 2)
    );

    d.app.set_old_macro_aside("Breaker", 1);
    let project = d.doc().project();
    assert!(project.block_definition_by_name("Breaker").is_none());
    assert!(
        project
            .block_definition_by_name("Breaker (rev 1)")
            .is_some()
    );
    assert_eq!(project.block_instances().count(), 1);
}
