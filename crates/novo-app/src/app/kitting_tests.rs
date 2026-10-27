//! Kitting from the app: the Bench output tab, the export command, the
//! Workshop pack file and the saved settings.

use novo_reports::KitSettings;
use novo_reports::kitting::kit_plan;

use super::bench::Tab;
use super::gate1_tests::Drive;
use super::route_all_tests::wired_cabinet;
use crate::actions::Action;

fn app_with_cabinet() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    wired_cabinet(&mut d);
    (dir, d)
}

fn is_pdf(path: &std::path::Path) -> bool {
    std::fs::read(path).is_ok_and(|b| b.starts_with(b"%PDF"))
}

#[test]
fn the_kitting_tab_renders_and_works_out_a_plan() {
    let (_dir, mut d) = app_with_cabinet();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.bench", None);
    d.frame();
    d.app.wb.bench.as_mut().expect("open").tab = Tab::Kitting;
    d.frame();
    d.frame();
    let state = d.app.wb.bench.as_ref().expect("open");
    let draft = state.kit.current().expect("the defaults read");
    assert_eq!(draft, KitSettings::default());
}

#[test]
fn the_command_writes_a_pdf_next_to_the_project() {
    let (dir, mut d) = app_with_cabinet();
    let ctx = d.ctx.clone();
    let file = dir.path().join("Plant 3.novo");
    assert!(d.app.save_to(&file));
    d.frame();
    d.app.wb.last_export = None;
    d.app.run_command(&ctx, "export.kitting", None);
    d.frame();
    let path = d.app.wb.last_export.clone().expect("exported");
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.contains("kitting list") && name.ends_with(".pdf"),
        "{name}"
    );
    assert!(is_pdf(&path));
    let msg = d.app.messages().last().unwrap().text.clone();
    assert!(
        msg.contains("to buy") && msg.contains("on the shopping list"),
        "{msg}"
    );
}

#[test]
fn nothing_to_kit_writes_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let path = dir.path().join("k.pdf");
    assert_eq!(d.app.export_kitting_to(&path), None);
    assert!(!path.exists());
}

#[test]
fn the_export_follows_the_draft_and_falls_back_when_it_does_not_read() {
    let (dir, mut d) = app_with_cabinet();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.bench", None);
    d.frame();
    d.app.wb.bench.as_mut().unwrap().tab = Tab::Kitting;
    d.frame();
    // Short bars in the draft: the 200 mm duct is cut from a 1000 mm bar.
    {
        let kit = &mut d.app.wb.bench.as_mut().unwrap().kit;
        kit.set_text_for_test("1000", "1000");
    }
    let a = dir.path().join("a.pdf");
    assert_eq!(d.app.export_kitting_to(&a), Some(a.clone()));
    let draft = d.app.wb.bench.as_ref().unwrap().kit.current().unwrap();
    assert_eq!(draft.duct_stock_mm, vec![1000.0]);
    let plan = kit_plan(d.doc().project(), &draft);
    assert_eq!(plan.saw[0].plan.bought, vec![(1000.0, 1)]);
    // Text that is not a length: the draft does not read, the project's settings are used.
    {
        let kit = &mut d.app.wb.bench.as_mut().unwrap().kit;
        kit.set_text_for_test("tall", "1000");
        assert!(kit.current().is_none());
    }
    let b = dir.path().join("b.pdf");
    assert_eq!(d.app.export_kitting_to(&b), Some(b.clone()));
    assert!(is_pdf(&b));
}

#[test]
fn settings_saved_to_the_project_come_back_when_the_window_reopens() {
    let (_dir, mut d) = app_with_cabinet();
    let ctx = d.ctx.clone();
    let s = KitSettings {
        duct_stock_mm: vec![1500.0],
        on_hand: "duct 40x60 700\n".into(),
        ..KitSettings::default()
    };
    let stored = s.clone();
    assert!(
        d.app
            .cabinet_edit("Kitting settings", move |tx| KitSettings::store(
                tx, &stored
            ))
    );
    d.app.run_command(&ctx, "view.bench", None);
    d.frame();
    d.app.wb.bench.as_mut().unwrap().tab = Tab::Kitting;
    d.frame();
    d.frame();
    assert_eq!(d.app.wb.bench.as_ref().unwrap().kit.current().unwrap(), s);
    // The held 40 x 60 duct piece of 700 mm covers the 200 mm duct of the cabinet.
    let plan = kit_plan(d.doc().project(), &s);
    assert_eq!(plan.saw[0].plan.bars_bought(), 0);
}

#[test]
fn the_workshop_pack_carries_the_kitting_list() {
    let (dir, mut d) = app_with_cabinet();
    let file = dir.path().join("Plant 4.novo");
    assert!(d.app.save_to(&file));
    d.frame();
    let folder = d.app.export_workshop().expect("folder");
    let list = folder.join("3 Kitting list.pdf");
    assert!(
        is_pdf(&list),
        "{:?}",
        std::fs::read_dir(&folder).map(|r| r.count())
    );
}
