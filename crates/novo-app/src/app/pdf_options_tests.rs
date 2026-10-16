//! The PDF options dialog: the options it maps to, the command that opens
//! it and the export it runs.

use super::gate1_tests::Drive;
use super::pdf_options::{page_set, parse_ranges, style_of};
use crate::actions::Action;
use crate::settings::PdfChoice;
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
    assert_eq!(page_count(&bytes), 2);
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
