//! The macro editor and the macro pack export: reached by command, parameters
//! added, changed and removed and written to the library, packs exported and
//! imported again.

use novo_core::Value;
use novo_parts::{MacroLibrary, ParamKind};

use super::gate1_tests::Drive;
use super::macro_browser::MacroBrowser;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.macros =
        MacroBrowser::with_library(MacroLibrary::new(dir.path().join("Macros"), "user"));
    (dir, d)
}

fn params(d: &Drive, path: &str) -> Vec<(String, Value)> {
    d.app
        .wb
        .macros
        .library
        .as_ref()
        .unwrap()
        .load(path)
        .unwrap()
        .definition
        .parameters
        .into_iter()
        .map(|p| (p.name, p.default))
        .collect()
}

#[test]
fn the_commands_exist_run_and_have_menu_homes() {
    for id in ["macro.edit", "macro.export_pack"] {
        assert!(super::workbench::app_runs(id), "{id}");
        assert!(novo_workflow::commands::get(id).is_some(), "{id}");
    }
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app
        .run_command(&ctx, "macro.edit", Some("Power supply 24 V"));
    assert!(d.app.wb.macro_editor.open);
    assert_eq!(d.app.wb.macro_editor.name, "Power supply 24 V");
    d.frame();
    d.frame();
}

#[test]
fn parameters_are_added_changed_and_removed_and_the_revision_grows() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    let path = "Starter/Power supply 24 V.kcmacro";
    d.app
        .run_command(&ctx, "macro.edit", Some("Power supply 24 V"));
    let lib = d.app.wb.macros.library.clone().unwrap();
    let before = lib.load(path).unwrap();
    let ed = &mut d.app.wb.macro_editor;
    assert_eq!(ed.path.as_deref(), Some(path));
    let first = ed.drafts.len();
    assert!(!ed.changed());

    ed.add();
    ed.add();
    assert_eq!(ed.drafts.len(), first + 2);
    let n = ed.drafts.len();
    ed.drafts[n - 2].name = "amps".into();
    ed.drafts[n - 2].set_kind(ParamKind::Integer);
    ed.drafts[n - 2].default = "10".into();
    ed.drafts[n - 1].name = "fused".into();
    ed.drafts[n - 1].set_kind(ParamKind::Switch);
    ed.drafts[n - 1].default = "yes".into();
    assert!(ed.changed());
    assert!(ed.save(&lib));
    assert!(!ed.changed());

    let got = params(&d, path);
    assert_eq!(got.len(), first + 2);
    assert!(got.contains(&("amps".into(), Value::Int(10))));
    assert!(got.contains(&("fused".into(), Value::Bool(true))));
    assert_eq!(lib.load(path).unwrap().revision, before.revision + 1);

    // Remove one again.
    let ed = &mut d.app.wb.macro_editor;
    ed.remove(ed.drafts.len() - 1);
    assert!(ed.save(&lib));
    assert_eq!(params(&d, path).len(), first + 1);

    // The Place window offers the new parameter with its default.
    d.app.open_macro_place(None);
    d.app.wb.macro_place.select(path);
    d.frame();
}

#[test]
fn a_bad_list_is_refused_and_nothing_is_written() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    let path = "Starter/Power supply 24 V.kcmacro";
    d.app
        .run_command(&ctx, "macro.edit", Some("Power supply 24 V"));
    let lib = d.app.wb.macros.library.clone().unwrap();
    let before = lib.load(path).unwrap();
    let ed = &mut d.app.wb.macro_editor;
    ed.add();
    ed.add();
    let n = ed.drafts.len();
    ed.drafts[n - 1].name = ed.drafts[n - 2].name.clone();
    assert!(!ed.save(&lib));
    assert!(ed.error.as_deref().unwrap().contains("twice"));
    ed.drafts[n - 1].name = "x".into();
    ed.drafts[n - 1].set_kind(ParamKind::Number);
    ed.drafts[n - 1].default = "abc".into();
    assert!(!ed.save(&lib));
    assert!(ed.error.as_deref().unwrap().contains("not a number"));
    assert_eq!(lib.load(path).unwrap().revision, before.revision);
}

#[test]
fn a_company_macro_cannot_be_changed() {
    let (dir, mut d) = drive();
    let ctx = d.ctx.clone();
    let company = dir.path().join("Company");
    let lib = MacroLibrary::new(&company, "company");
    lib.install_starters().unwrap();
    d.app.wb.macros.set_company_folder(Some(&company));
    d.app.wb.macros.show_company(true);
    d.app
        .run_command(&ctx, "macro.edit", Some("Power supply 24 V"));
    let ed = &mut d.app.wb.macro_editor;
    assert!(ed.read_only);
    ed.add();
    assert!(!ed.save(&lib));
    d.frame();
}

#[test]
fn a_pack_is_exported_from_the_browser_and_imported_into_another_library() {
    let (dir, mut d) = drive();
    let b = &mut d.app.wb.macros;
    b.show();
    let total = b.entries().len();
    assert!(total >= 4);
    let out = dir.path().join("share").join("team.kcmacropack");
    b.export_pack(&out);
    assert!(out.exists(), "{}", b.status);
    assert!(b.status.contains("Exported"), "{}", b.status);

    // A search narrows the pack.
    b.query = "24 V".into();
    let small = dir.path().join("small.kcmacropack");
    b.export_pack(&small);
    b.query.clear();

    // Another person's library takes the pack, folders and all.
    let mut other = MacroBrowser::with_library(MacroLibrary::new(dir.path().join("Other"), "user"));
    other.import(&out);
    assert_eq!(other.entries().len(), total, "{}", other.status);
    assert!(other.entries().iter().any(|e| e.folder == "Starter"));
    let mut third = MacroBrowser::with_library(MacroLibrary::new(dir.path().join("Third"), "user"));
    third.import(&small);
    assert_eq!(third.entries().len(), 1);

    // An empty match exports nothing and says so.
    d.app.wb.macros.query = "no such macro".into();
    let none = dir.path().join("none.kcmacropack");
    d.app.wb.macros.export_pack(&none);
    assert!(!none.exists());
    assert!(d.app.wb.macros.status.contains("Nothing"));
}
