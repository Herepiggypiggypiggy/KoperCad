//! What the bench output lessons say about the example project's paper: the work cards, the workshop
//! pack and the kitting list, read back from the files the program writes.
//!
//! work-cards and workshop-pack run on the built-in example ("@example"), kitting on the lesson cabinet
//! of cooling-advice. A lesson that quotes a number or a line of a card must find it here.

use std::path::Path;

use novo_core::Project;
use novo_reports::kitting::{KitSettings, Phase, kit_plan};

use super::gate1_tests::Drive;
use crate::actions::Action;

/// All the text of a PDF, page after page, with a wrapped line read as one.
fn pdf_text(path: &Path) -> String {
    let pdf = lopdf::Document::load_mem(&std::fs::read(path).unwrap()).unwrap();
    pdf.get_pages()
        .keys()
        .map(|n| pdf.extract_text(&[*n]).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The example, saved, with its workshop pack written next to it.
fn example_pack() -> (tempfile::TempDir, Drive, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert!(d.app.save_to(&dir.path().join("Example.novo")));
    d.frame();
    let folder = dir.path().join("pack");
    std::fs::create_dir_all(&folder).unwrap();
    assert!(d.app.export_workshop_to(&folder).is_some());
    (dir, d, folder)
}

#[test]
fn the_work_cards_say_what_the_work_cards_lesson_quotes() {
    let (_dir, d, folder) = example_pack();
    let cards = pdf_text(&folder.join("1 Work cards.pdf"));
    for line in [
        // The step counts: 42 on the cards, 17 on the build order strip (checked below).
        "STEP 2 of 42",
        "Tools and materials",
        "Tick-off sheet",
        "Must measure 100 mm",
        // Rail card.
        "Holes, mm from the rail start: 10, 192.5, 375",
        "Pitch between holes, mm: 182.5 x 2",
        "Last hole to the rail end: 10 mm",
        "Clipped on this rail, mm from its start: F1 at 0, F2 at 64, F3 at 119",
        "Holes drilled",
        "Rail fitted",
        "Checked by",
        // A rail longer than the strip, and a duct template, are drawn smaller.
        "Rail strip, scale 1:2.5, NOT true size. Measure from the list; the ruler below is true size",
        "Template, scale 1:5, NOT true size",
        // Device card of a device on a rail.
        "Nothing to drill: the rail holds it",
        // Duct card: the planning rule for the fixings.
        "Top left corner: 20 mm from the left edge, 10 mm from the top edge",
        "Fixing (a planning rule, not a datasheet",
        "25, 185, 345, 505 mm from the start",
        "These holes are not in the drilling list",
        // Wire cards and cut strips.
        "Wires Control",
        "Per end",
        "NOT A STEP",
        "cut strip",
        "Wires outside the plate",
        "Duct fitted",
    ] {
        assert!(cards.contains(line), "the work cards lack: {line}");
    }
    // The strip of the build order counts the parts and one closing step for the wires.
    let project = d.app.backend.document().unwrap().project();
    let cab = project.structures().find(|s| s.enclosure).unwrap().id;
    assert_eq!(
        super::view3d_build_order::sequence(project, cab).len(),
        17,
        "Step 2 of 17 on the strip"
    );
    // Wire cards hold nine wires: the group Control has 19 rows on three cards, 1 of 3 to 3 of 3.
    assert!(cards.contains("Control (1 of 3)") && cards.contains("Control (3 of 3)"));
}

#[test]
fn the_workshop_pack_holds_the_files_the_workshop_lessons_name() {
    let (_dir, _d, folder) = example_pack();
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for wanted in [
        "0 Bench packet.pdf",
        "0 Print scale check.pdf",
        "1 Drilling template +CAB A4.pdf",
        "1 Plate full size +CAB.pdf",
        "1 Work cards.pdf",
        "2 Labels.pdf",
        "3 Kitting list.pdf",
    ] {
        assert!(names.iter().any(|n| n == wanted), "{wanted} in {names:?}");
    }
    // The cover of the packet says which files to print at 100 %.
    let cover = pdf_text(&folder.join("0 Bench packet.pdf"));
    for file in [
        "0 Print scale check.pdf",
        "2 Labels.pdf",
        "3 Kitting list.pdf",
        "1 Work cards.pdf",
    ] {
        assert!(
            cover.contains(file),
            "the packet's contents page names {file}"
        );
    }
    assert!(cover.contains("Print at 100 % (actual size)"));
}

#[test]
fn the_bench_window_says_what_is_missing_before_printing() {
    // No plate: the strip offers Open Cabinet, as the workshop-pack lesson says.
    let gaps = super::bench::readiness(&[], false);
    assert_eq!(gaps.len(), 1);
    assert!(gaps[0].text.starts_with("No mounting plate"));
    assert_eq!(gaps[0].fix, Some(("Open Cabinet", "view.cabinet")));
}

#[test]
fn the_kitting_list_of_the_lesson_cabinet_reads_as_the_kitting_lesson_says() {
    let file = format!(
        "{}/../../assets/lessons/cooling-advice/project.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let project = Project::from_json(&std::fs::read_to_string(file).unwrap()).unwrap();
    let plan = kit_plan(&project, &KitSettings::default());
    // Pick lists keep the checklist's step numbers: Drill (step 2) has none.
    assert_eq!(
        Phase::ALL.map(|p| p.name()),
        ["Prepare", "Mount", "Wire", "Label"]
    );
    assert_eq!(Phase::Prepare.step(), 1);
    assert_eq!(Phase::Mount.step(), 3);
    let prepare: Vec<_> = plan.picks_in(Phase::Prepare).collect();
    assert_eq!(prepare.len(), 2, "a bar of rail and a bar of duct");
    assert!(prepare.iter().all(|p| p.from == "New bar, to buy"));
    assert!(prepare[0].for_.contains("+CAB1 R1"));
    // Two saw plans, three bars each, 6 bars to buy and offcuts to keep.
    assert_eq!(plan.saw.len(), 2);
    assert_eq!(plan.bars_to_buy(), 6);
    assert!(plan.saw.iter().all(|s| s.plan.bars_bought() == 3));
    // The headline's orange line: the devices have no part.
    assert!(
        plan.problems.iter().any(|p| p.contains(
            "have no part: they cannot be on the shopping list until a part is assigned"
        )),
        "{:?}",
        plan.problems
    );
}
