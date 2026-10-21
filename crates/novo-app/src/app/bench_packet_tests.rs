//! Bench packet: one PDF in working order, with a cover whose page numbers
//! agree with the bookmarks, and the command that writes it.

use super::gate1_tests::Drive;
use super::route_all_tests::wired_cabinet;
use crate::actions::Action;
use novo_reports::packet::CoverLine;

fn app_with_wires() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    wired_cabinet(&mut d);
    let wires: Vec<_> = d.doc().project().wires().map(|w| w.id).collect();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Sizes", |tx| {
            for id in wires {
                tx.update::<novo_core::Wire>(id, |w| w.cross_section_mm2 = Some(1.5))?;
            }
            Ok(())
        })
        .unwrap();
    (dir, d)
}

fn page_texts(path: &std::path::Path) -> Vec<String> {
    let pdf = lopdf::Document::load_mem(&std::fs::read(path).unwrap()).unwrap();
    pdf.get_pages()
        .keys()
        .map(|n| pdf.extract_text(&[*n]).unwrap_or_default())
        .collect()
}

#[test]
fn an_empty_project_writes_no_packet() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let path = dir.path().join("packet.pdf");
    assert_eq!(d.app.export_bench_packet_to(&path, &[]), None);
    assert!(!path.exists());
}

#[test]
fn the_packet_has_a_cover_then_the_parts_and_the_cover_names_the_right_pages() {
    let (dir, mut d) = app_with_wires();
    let path = dir.path().join("packet.pdf");
    let separate = [CoverLine::new(
        "Labels",
        "2 Labels.pdf",
        "Print at 100 % on the label stock",
        "Texts from the drawing",
    )];
    assert_eq!(
        d.app.export_bench_packet_to(&path, &separate),
        Some(path.clone()),
        "{:?}",
        d.app.messages().last()
    );
    let pages = page_texts(&path);
    assert!(pages.len() >= 3, "{} pages", pages.len());
    let cover = &pages[0];
    assert!(cover.contains("Contents"), "{cover}");
    assert!(cover.contains("2 Labels.pdf"), "{cover}");
    assert!(cover.contains("Print at 100"), "{cover}");
    assert!(cover.contains("Read before you print"), "{cover}");
    // The wire counts are printed whole: a cell that is too long ends in "...".
    assert!(!cover.contains("..."), "{cover}");
    assert!(
        cover.contains("between pins +") && cover.contains("pieces on the sheets"),
        "{cover}"
    );
    // Every "p. N" on the cover points at a page that starts the part.
    let marks: Vec<usize> = cover
        .split("p. ")
        .skip(1)
        .filter_map(|t| t.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok())
        .collect();
    assert!(!marks.is_empty(), "no page numbers: {cover}");
    for n in &marks {
        assert!(*n >= 2 && *n <= pages.len(), "page {n} of {}", pages.len());
    }
    let circuits = marks.iter().copied().max().unwrap();
    assert!(
        pages[circuits - 1].contains("Tag / wire") || pages[circuits - 1].contains("Step"),
        "{}",
        pages[circuits - 1]
    );
}

#[test]
fn the_command_and_the_window_button_write_the_packet_next_to_the_project() {
    let (dir, mut d) = app_with_wires();
    let ctx = d.ctx.clone();
    assert!(d.app.save_to(&dir.path().join("Plant 9.novo")));
    d.frame();
    d.app.run_command(&ctx, "export.bench_packet", None);
    let file = dir.path().join("Plant 9 bench packet.pdf");
    assert!(file.exists(), "{:?}", d.app.messages().last());
    // A second run replaces it rather than writing a second file.
    d.app.run_command(&ctx, "export.bench_packet", None);
    assert_eq!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("bench packet"))
            .count(),
        1
    );
}

#[test]
fn the_workshop_pack_holds_the_packet_and_the_labels_and_no_longer_the_loose_files() {
    let (dir, mut d) = app_with_wires();
    let folder = dir.path().join("pack");
    std::fs::create_dir_all(&folder).unwrap();
    // A file of the first version of the pack is cleaned up.
    std::fs::write(folder.join("2 Cut list.pdf"), b"old").unwrap();
    assert_eq!(
        d.app.export_workshop_to(&folder).as_deref(),
        Some(folder.as_path()),
        "{:?}",
        d.app.messages().last()
    );
    assert!(folder.join("0 Bench packet.pdf").exists());
    assert!(folder.join("2 Labels.pdf").exists());
    assert!(!folder.join("2 Cut list.pdf").exists());
    let pages = page_texts(&folder.join("0 Bench packet.pdf"));
    assert!(pages[0].contains("2 Labels.pdf"), "{}", pages[0]);
    let last = &d.app.messages().last().unwrap().text;
    assert!(
        last.contains("2 Cut list.pdf") && last.contains("removed"),
        "{last}"
    );
    // No printed cell is cut off with an ellipsis (the tick lines are read at the bench).
    for (n, p) in pages.iter().enumerate() {
        assert!(!p.contains("..."), "page {} is cut off: {p}", n + 1);
    }
    assert!(
        pages
            .iter()
            .any(|p| p.contains("ferrules compared with the")),
        "the test lines are on a circuit sheet"
    );
}

#[test]
fn every_cover_note_stays_on_the_cover() {
    // The example has no wire sizes: the most notes, and the real command's contents lines.
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert!(d.app.save_to(&dir.path().join("Example.novo")));
    d.frame();
    let path = d.app.export_bench_packet().expect("the packet");
    let pages = page_texts(&path);
    let cover = &pages[0];
    assert!(
        cover.contains("workshop routine"),
        "the last note is on the cover: {cover}"
    );
    assert!(
        !pages[1].contains("workshop routine") && !pages[1].contains("Read before you print"),
        "page 2 starts a part, not a spilled note: {}",
        pages[1]
    );
}
