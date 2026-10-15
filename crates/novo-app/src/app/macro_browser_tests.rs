//! The macro browser: reached by command, starter set on first open,
//! search, rename, trash and restore, import and export.

use novo_parts::MacroLibrary;

use super::gate1_tests::Drive;
use super::macro_browser::MacroBrowser;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.macros =
        MacroBrowser::with_library(MacroLibrary::new(dir.path().join("Macros"), "user"));
    (dir, d)
}

#[test]
fn the_command_opens_the_browser_without_a_project_and_installs_the_starters() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    assert!(super::workbench::app_runs("macro.browser"));
    assert!(super::workbench::app_runs("macro.import"));
    assert!(novo_workflow::commands::get("macro.browser").is_some());
    assert!(d.app.document().is_none());
    d.app.run_command(&ctx, "macro.browser", None);
    assert!(d.app.wb.macros.open);
    d.frame();
    assert_eq!(d.app.wb.macros.entries().len(), 4);
    // Opening again does not bring back one the user deleted.
    d.app.wb.macros.selected = Some("Starter/E-stop chain.kcmacro".into());
    d.app.wb.macros.trash_selected();
    d.app.run_command(&ctx, "macro.browser", None);
    d.frame();
    assert_eq!(d.app.wb.macros.entries().len(), 3);
}

#[test]
fn search_rename_trash_restore_export_and_import() {
    let (dir, mut d) = drive();
    let b = &mut d.app.wb.macros;
    b.show();
    b.query = "24 V".into();
    assert_eq!(b.entries().len(), 1);
    b.query.clear();

    b.selected = Some("Starter/Power supply 24 V.kcmacro".into());
    b.rename_selected("Supply 24 V");
    assert_eq!(b.selected.as_deref(), Some("Starter/Supply 24 V.kcmacro"));
    b.move_selected("");
    assert_eq!(b.selected.as_deref(), Some("Supply 24 V.kcmacro"));

    let out = dir.path().join("share").join("supply.kcmacro");
    b.export_selected(&out);
    assert!(out.exists(), "{}", b.status);

    b.trash_selected();
    assert_eq!(b.entries().len(), 3);
    assert!(b.selected.is_none());
    b.restore(".trash/Supply 24 V.kcmacro");
    assert_eq!(b.entries().len(), 4);

    // Importing the exported file keeps both: the name is taken.
    b.import(&out);
    assert_eq!(b.entries().len(), 5, "{}", b.status);
    assert!(b.status.contains("Imported 1"));
}

#[test]
fn the_window_draws_with_a_selection() {
    let (_dir, mut d) = drive();
    d.app.wb.macros.show();
    d.app.wb.macros.selected = Some("Starter/Star-delta starter.kcmacro".into());
    d.frame();
    d.frame();
}
