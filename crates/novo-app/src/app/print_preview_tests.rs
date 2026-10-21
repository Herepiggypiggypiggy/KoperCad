//! Print preview and plot sets: the command, the sheets drawn for the
//! ticked pages, the saved plot sets and the PDFs they write.

use super::gate1_tests::Drive;
use super::print_preview::setup_of;
use crate::actions::Action;
use crate::settings::PrintChoice;
use novo_pdf::print::{Orientation, PrintPaper, PrintScale};

fn saved_example(dir: &std::path::Path) -> Drive {
    let mut d = Drive::new(&dir.join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert!(d.app.save_to(&dir.join("Plant.novo")));
    d.app.wb.pdf_dir = Some(dir.to_owned());
    d
}

fn pdf(path: &std::path::Path) -> lopdf::Document {
    lopdf::Document::load_mem(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn the_choice_maps_to_a_print_setup() {
    let c = PrintChoice {
        paper: "a3".into(),
        orientation: "portrait".into(),
        fit: false,
        percent: 50,
        margin_mm: 12,
        centre: false,
        page_numbers: false,
    };
    let s = setup_of(&c);
    assert_eq!(s.paper, PrintPaper::A3);
    assert_eq!(s.orientation, Orientation::Portrait);
    assert_eq!(s.scale, PrintScale::Percent(50.0));
    assert!((s.margin_mm - 12.0).abs() < f32::EPSILON && !s.centre && !s.page_numbers);
    // Nonsense falls back to the defaults rather than failing.
    let bad = setup_of(&PrintChoice {
        paper: "b5".into(),
        orientation: "sideways".into(),
        ..Default::default()
    });
    assert_eq!(bad.paper, PrintPaper::AsSheet);
    assert_eq!(bad.orientation, Orientation::Auto);
}

#[test]
fn the_command_opens_the_preview_with_every_page_drawn() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = saved_example(dir.path());
    assert!(super::workbench::app_runs("export.print_preview"));
    d.app.wb.queued.push("export.print_preview");
    d.frame();
    d.frame();
    let pages = d.doc().project().pages_in_order().len();
    assert!(d.app.wb.print_preview.is_some(), "the window is open");
    assert!(pages >= 2);
    let (picked, sheets) = d.app.print_preview_counts().unwrap();
    assert_eq!(picked, pages);
    assert!(
        sheets >= pages,
        "a report page prints every one of its sheets"
    );
}

#[test]
fn save_pdf_writes_the_ticked_pages_on_the_chosen_paper() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = saved_example(dir.path());
    d.app.wb.queued.push("export.print_preview");
    d.frame();
    d.app.print_preview_pick(1);
    d.app.print_preview_set_paper("a4");
    let (_, drawn) = d.app.print_preview_counts().unwrap();
    let path = d.app.print_preview_save_pdf().expect("written");
    assert!(
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("print.pdf"),
        "{path:?}"
    );
    let doc = pdf(&path);
    assert_eq!(doc.get_pages().len(), drawn);
    let text = doc.extract_text(&[1]).unwrap();
    assert!(text.contains(&format!("Page 1 of {drawn}")), "{text}");
    // The one-click project PDF is untouched by the preview's setup.
    let all = d.app.export_pdf().expect("project pdf");
    assert_ne!(all, path);
}

#[test]
fn plot_sets_are_kept_per_project_and_each_writes_its_own_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = saved_example(dir.path());
    d.app.wb.queued.push("export.print_preview");
    d.frame();
    d.app.print_preview_pick(1);
    let (_, first) = d.app.print_preview_counts().unwrap();
    d.app.print_preview_name_set("Tender issue");
    d.app.print_preview_save_set();
    d.app.print_preview_pick(2);
    let (_, two) = d.app.print_preview_counts().unwrap();
    d.app.print_preview_set_paper("a3");
    d.app.print_preview_name_set("Workshop A3");
    d.app.print_preview_save_set();
    assert_eq!(d.app.settings.plot_sets.values().next().unwrap().len(), 2);

    let written = d.app.print_preview_export_all_sets();
    assert_eq!(written.len(), 2);
    let names: Vec<String> = written
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().any(|n| n.ends_with("plot Tender issue.pdf")),
        "{names:?}"
    );
    assert!(
        names.iter().any(|n| n.ends_with("plot Workshop A3.pdf")),
        "{names:?}"
    );
    assert_eq!(pdf(&written[0]).get_pages().len(), first);
    assert_eq!(pdf(&written[1]).get_pages().len(), two);

    // A project saved elsewhere does not see them.
    assert_eq!(d.app.plot_sets_for_test().len(), 2);
    let other = tempfile::tempdir().unwrap();
    assert!(d.app.save_to(&other.path().join("Copy.novo")));
    assert!(d.app.plot_sets_for_test().is_empty());
}

#[test]
fn the_window_and_both_tabs_draw_without_trouble() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = saved_example(dir.path());
    d.app.wb.queued.push("export.print_preview");
    for _ in 0..3 {
        d.frame();
    }
    d.app.print_preview_set_paper("sheet");
    d.frame();
    d.app.print_preview_show_workshop();
    for _ in 0..3 {
        d.frame();
    }
    assert!(d.app.wb.print_preview.is_some());
}
