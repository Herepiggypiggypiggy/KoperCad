//! The PDF options dialog: the options it maps to, the command that opens
//! it and the export it runs.

use super::gate1_tests::Drive;
use super::pdf_options::{page_set, parse_ranges, style_of};
use crate::actions::Action;
use crate::settings::PdfChoice;
use eframe::egui;
use novo_pdf::PageSet;

fn saved_example(dir: &std::path::Path) -> Drive {
    let mut d = Drive::new(&dir.join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert!(d.app.save_to(&dir.join("Plant.novo")));
    d
}

fn page_count(bytes: &[u8]) -> usize {
    lopdf::Document::load_mem(bytes).unwrap().get_pages().len()
}

#[test]
fn ranges_are_read_as_page_numbers() {
    assert_eq!(parse_ranges("1-3, 5", 6), Ok(vec![1, 2, 3, 5]));
    assert_eq!(parse_ranges("5;2;2", 6), Ok(vec![2, 5]));
    assert_eq!(parse_ranges("4-", 6), Ok(vec![4, 5, 6]));
    assert!(parse_ranges("", 6).is_err());
    assert!(parse_ranges("0", 6).is_err());
    assert!(parse_ranges("7", 6).unwrap_err().contains("outside"));
    assert!(parse_ranges("3-1", 6).unwrap_err().contains("backwards"));
    assert!(parse_ranges("a", 6).unwrap_err().contains("not a page"));
}

#[test]
fn the_choice_maps_to_page_sets_and_looks() {
    let dir = tempfile::tempdir().unwrap();
    let d = saved_example(dir.path());
    let pages: Vec<_> = d.doc().project().pages().map(|p| p.id).collect();
    assert!(pages.len() >= 2);
    let mut choice = PdfChoice::default();
    assert_eq!(page_set(&choice, &pages, None), Ok(PageSet::All));
    choice.scope = "current".into();
    assert!(page_set(&choice, &pages, None).is_err());
    assert_eq!(
        page_set(&choice, &pages, Some(pages[1])),
        Ok(PageSet::Pages(vec![pages[1]]))
    );
    choice.scope = "ranges".into();
    choice.ranges = "2".into();
    assert_eq!(
        page_set(&choice, &pages, Some(pages[0])),
        Ok(PageSet::Pages(vec![pages[1]]))
    );
    assert_eq!(style_of(&PdfChoice::default()), novo_pdf::print_style());
    let colour = PdfChoice {
        look: "colour".into(),
        ..Default::default()
    };
    assert_ne!(
        style_of(&colour).wire.color,
        novo_pdf::print_style().wire.color
    );
    // The default choice is the engine's default.
    let engine = novo_pdf::PdfOptions::default();
    let c = PdfChoice::default();
    assert!(engine.bookmarks == c.bookmarks && engine.links == c.links);
}

#[test]
fn the_command_opens_the_dialog_and_export_follows_the_choice() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = saved_example(dir.path());
    assert!(super::workbench::app_runs("export.pdf_options"));
    assert!(d.app.wb.pdf_options.is_none());
    d.app.wb.queued.push("export.pdf_options");
    d.frame();
    let open = d.app.wb.pdf_options.as_ref().expect("the dialog is open");
    assert_eq!(open.choice, PdfChoice::default());

    let all = d.app.export_pdf().expect("all pages");
    let all_pages = page_count(&std::fs::read(&all).unwrap());
    assert!(all_pages >= 2);

    let choice = PdfChoice {
        scope: "ranges".into(),
        ranges: "1-2".into(),
        bookmarks: false,
        links: false,
        checks_appendix: false,
        title: "Pump skid".into(),
        ..Default::default()
    };
    let path = d.app.export_pdf_with_choice(&choice).expect("exported");
    assert!(
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("pages 1-2.pdf")
    );
    let bytes = std::fs::read(&path).unwrap();
    let first_two = dir.path().join("first two.pdf");
    let pages: Vec<_> = d
        .doc()
        .project()
        .pages_in_order()
        .iter()
        .map(|p| p.id)
        .collect();
    d.app
        .export_pdf_choice_to(&first_two, PageSet::Pages(pages[..2].to_vec()), &choice)
        .unwrap();
    assert_eq!(
        page_count(&bytes),
        page_count(&std::fs::read(&first_two).unwrap())
    );
    assert!(!bytes.windows(9).any(|w| w == b"/Outlines"), "no bookmarks");
    assert!(bytes.windows(9).any(|w| w == b"Pump skid"));
    assert!(
        !std::fs::read(&all)
            .unwrap()
            .windows(9)
            .any(|w| w == b"Pump skid")
    );
    // Remembered for next time, per user.
    assert_eq!(d.app.settings.pdf_choice, choice);
    d.app.wb.queued.push("export.pdf_options");
    d.frame();
    assert_eq!(d.app.wb.pdf_options.as_ref().unwrap().choice, choice);

    // A range outside the project exports nothing and says why.
    let bad = PdfChoice {
        scope: "ranges".into(),
        ranges: "999".into(),
        ..Default::default()
    };
    assert!(d.app.export_pdf_with_choice(&bad).is_none());
    assert!(d.app.messages().last().unwrap().text.contains("outside"));
    assert_ne!(d.app.settings.pdf_choice, bad, "a bad choice is not kept");

    // The one-click commands still write everything with the defaults.
    assert_eq!(
        page_count(&std::fs::read(d.app.export_pdf().unwrap()).unwrap()),
        all_pages
    );
}

#[test]
fn the_choice_survives_the_settings_file() {
    let choice = PdfChoice {
        scope: "current".into(),
        ranges: "3".into(),
        links: false,
        look: "colour".into(),
        ..Default::default()
    };
    let settings = crate::settings::Settings {
        pdf_choice: choice.clone(),
        ..Default::default()
    };
    let text = ron::to_string(&settings).unwrap();
    let back: crate::settings::Settings = ron::from_str(&text).unwrap();
    assert_eq!(back.pdf_choice, choice);
    // An older settings file has no choice: the defaults.
    let old: crate::settings::Settings = ron::from_str("()").unwrap();
    assert_eq!(old.pdf_choice, PdfChoice::default());
}

#[test]
fn ranges_count_pages_in_the_order_the_outline_shows() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = saved_example(dir.path());
    let ctx = d.ctx.clone();
    // A copy is created last but placed before the page it copies.
    d.app.run_command(&ctx, "page.duplicate", None);
    d.frame();
    let copy = d.app.current_page.unwrap();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Place first", |tx| {
            tx.update::<novo_core::Page>(copy, |p| {
                p.properties
                    .insert(novo_core::PAGE_ORDER.into(), novo_core::Value::Int(-1));
            })?;
            Ok(())
        })
        .unwrap();
    d.frame();
    let ordered: Vec<_> = d
        .doc()
        .project()
        .pages_in_order()
        .iter()
        .map(|p| p.id)
        .collect();
    let by_creation: Vec<_> = d.doc().project().pages().map(|p| p.id).collect();
    let number = ordered.iter().position(|p| *p == copy).unwrap() + 1;
    assert_ne!(
        by_creation.iter().position(|p| *p == copy).unwrap() + 1,
        number,
        "the order differs from creation order"
    );

    let choice = PdfChoice {
        scope: "ranges".into(),
        ranges: number.to_string(),
        ..Default::default()
    };
    assert_eq!(
        page_set(&choice, &ordered, None),
        Ok(PageSet::Pages(vec![copy]))
    );
    let path = d.app.export_pdf_with_choice(&choice).expect("exported");
    let one = dir.path().join("copy only.pdf");
    d.app
        .export_pdf_pages_to(&one, PageSet::Pages(vec![copy]))
        .unwrap();
    assert_eq!(
        page_count(&std::fs::read(&path).unwrap()),
        page_count(&std::fs::read(&one).unwrap())
    );
}

#[test]
fn the_file_name_of_a_selection_is_safe_on_windows() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = saved_example(dir.path());
    let choice = PdfChoice {
        scope: "ranges".into(),
        ranges: "1 - 2 | *".into(),
        ..Default::default()
    };
    // The text is not valid pages, so nothing is written; a valid one with
    // odd spacing keeps a clean name.
    assert!(d.app.export_pdf_with_choice(&choice).is_none());
    let ok = PdfChoice {
        ranges: "1, 2".into(),
        ..choice
    };
    let path = d.app.export_pdf_with_choice(&ok).unwrap();
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(!name.contains(['*', '?', '"', '<', '>', '|']), "{name}");
    assert!(name.ends_with("pages 1,2.pdf"), "{name}");
}

#[test]
fn the_export_dialog_is_tall_enough_for_its_button_row_at_1280_by_768() {
    // The options need 398 points of content plus the title bar: egui's own
    // default of 420 left the Export button half hidden.
    let dir = tempfile::tempdir().unwrap();
    let mut d = saved_example(dir.path());
    d.app.wb.queued.push("export.pdf_options");
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 768.0));
    for _ in 0..5 {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        let app = &mut d.app;
        let mut out = d.ctx.run_ui(input, |ui| app.show(ui));
        out.textures_delta.clear();
    }
    assert!(d.app.wb.pdf_options.is_some(), "the dialog is open");
    // Window ids are made from the title; find the layer by name.
    let layer = d
        .ctx
        .memory(|m| m.areas().visible_layer_ids())
        .into_iter()
        .find(|l| format!("{l:?}").contains("Export PDF"))
        .expect("the dialog is shown");
    let rect = d
        .ctx
        .memory(|m| m.area_rect(layer.id))
        .expect("the dialog has a rect");
    assert!(rect.height() >= 440.0, "{rect:?}");
    assert!(screen.contains_rect(rect), "{rect:?}");
}
