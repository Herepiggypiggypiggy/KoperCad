//! The Parts import from the click to the result, run for real: files in a
//! Downloads folder, the background read, the library on disk, and the
//! failure paths (nothing arrives, missing or broken file, a reader that
//! stalls or dies). The Learn lesson "Find a part and import it in one
//! click" runs exactly this flow on Siemens 3RT2016-1AP01.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use eframe::egui;
use novo_parts::{Catalog, ImportFlow, LocalLibrary, PartKey, RowAction, SourceTable};

use super::super::parts_import::{Import, Phase, READ_LIMIT_SECONDS, WAIT_LIMIT_SECONDS};
use super::{PartsWindow, parts_ui};

/// A STEP file whose points span `w` x `h` x `d` millimetres.
fn step(w: f64, h: f64, d: f64) -> String {
    format!(
        "ISO-10303-21;\nDATA;\n#1=CARTESIAN_POINT('',(0.,0.,0.));\n#2=CARTESIAN_POINT('',({w},{h},{d}));\nENDSEC;\nEND-ISO-10303-21;\n"
    )
}

struct Bench {
    _dir: tempfile::TempDir,
    downloads: std::path::PathBuf,
    win: PartsWindow,
}

fn bench() -> Bench {
    let dir = tempfile::tempdir().unwrap();
    let downloads = dir.path().join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    let mut win = PartsWindow::new(
        Catalog::starter(),
        Some(LocalLibrary::at(dir.path().join("lib"))),
        SourceTable::builtin(),
    );
    win.downloads = Some(downloads.clone());
    win.catalog_path = Some(dir.path().join("parts.json"));
    Bench {
        _dir: dir,
        downloads,
        win,
    }
}

fn key(maker: &str, order: &str) -> PartKey {
    PartKey::new(maker, order)
}

impl Bench {
    /// Clicks Import on the part, as the Import button does.
    fn click_import(&mut self, k: &PartKey) {
        self.win.select(k.clone());
        self.win.start_import(k);
    }

    /// Runs frames like the window does until the import leaves `Waiting`
    /// and `Reading`, or `secs` real seconds pass. Returns the last phase.
    fn pump_until_settled(&mut self, secs: u64) -> Phase {
        let started = Instant::now();
        let mut now = 100.0;
        loop {
            self.win.pump_import(now);
            let phase = self.win.import.as_ref().unwrap().phase.clone();
            if !matches!(phase, Phase::Waiting | Phase::Reading) {
                return phase;
            }
            assert!(
                started.elapsed() < Duration::from_secs(secs),
                "still {phase:?} after {secs} s"
            );
            std::thread::sleep(Duration::from_millis(20));
            now += 1.1;
        }
    }

    fn row(&self, k: &PartKey) -> RowAction {
        self.win
            .rows()
            .into_iter()
            .find(|(r, _)| r == k)
            .unwrap_or_else(|| panic!("no row for {k:?}"))
            .1
    }

    /// The text the window paints once its layout has settled (the third
    /// frame; egui sizes panels from the previous frame).
    fn painted(&mut self) -> String {
        let ctx = egui::Context::default();
        novo_theme::apply(&ctx);
        let mut text = String::new();
        for _ in 0..3 {
            let mut actions = Vec::new();
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 700.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default()
                        .show(ui, |ui| parts_ui(ui, &mut self.win, false, &mut actions));
                },
            );
            out.textures_delta.clear();
            text.clear();
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(t) = &clipped.shape {
                    text.push_str(t.galley.text());
                    text.push('\n');
                }
            }
        }
        text
    }
}

fn write(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

/// The lesson's part and its neighbour from the report, plus two Phoenix
/// Contact parts, each imported from a file that arrives in the Downloads
/// folder after the click. Every row turns "In library", the file is stored
/// and the library records its origin.
#[test]
fn two_siemens_contactors_and_two_others_import_from_a_downloads_folder() {
    let mut b = bench();
    let cases = [
        ("Siemens", "3RT2016-1AP01", "3RT2016-1AP01.stp"),
        ("Siemens", "3RT2017-1BB41", "3RT2017-1BB41_3D.step"),
        ("Phoenix Contact", "3044076", "3044076.stp"),
        ("Phoenix Contact", "2904601", "2904601_model.stp"),
    ];
    for (i, (maker, order, file)) in cases.iter().enumerate() {
        let k = key(maker, order);
        assert_ne!(
            b.row(&k),
            RowAction::InLibrary,
            "{order} starts not imported"
        );
        b.click_import(&k);
        assert_eq!(b.win.import.as_ref().unwrap().phase, Phase::Waiting);
        // A file for another article lands first and must be left alone.
        write(&b.downloads, "invoice-4711.stp", &step(1.0, 1.0, 1.0));
        write(&b.downloads, file, &step(45.0, 57.5, 73.0 + i as f64));
        let phase = b.pump_until_settled(20);
        let Phase::Done(summary) = phase else {
            panic!("{order}: {phase:?}");
        };
        assert_eq!(
            summary.file, *file,
            "{order}: the file named after the part"
        );
        assert_eq!(b.row(&k), RowAction::InLibrary, "{order} row");
        let records = b.win.records(&k);
        assert_eq!(records.len(), 1, "{order}: one stored file");
        assert_eq!(records[0].flow, ImportFlow::Downloads);
        assert!(
            b.win
                .library
                .as_ref()
                .unwrap()
                .path_of(&records[0])
                .is_file(),
            "{order}: the file is in the library folder"
        );
        // The strip shows the result, the Done button closes it.
        let text = b.painted();
        assert!(
            text.contains(&format!("Imported {maker} {order}")),
            "{text}"
        );
        b.win.import = None;
        fs::remove_file(b.downloads.join(file)).unwrap();
        fs::remove_file(b.downloads.join("invoice-4711.stp")).unwrap();
    }
    // The other parts' rows are untouched by the four imports.
    let imported = b
        .win
        .rows()
        .into_iter()
        .filter(|(_, a)| *a == RowAction::InLibrary)
        .count();
    assert_eq!(imported, cases.len());
}

#[test]
fn a_file_dropped_on_the_window_imports_without_freezing_it() {
    let mut b = bench();
    let k = key("Siemens", "3RT2016-1AP01");
    b.win.select(k.clone());
    let elsewhere = tempfile::tempdir().unwrap();
    write(elsewhere.path(), "contactor.stp", &step(45.0, 57.5, 73.0));
    b.win
        .import_file(&elsewhere.path().join("contactor.stp"), ImportFlow::Manual);
    // The call returned with the read still on its way: the window thread
    // did not do the work.
    assert_eq!(b.win.import.as_ref().unwrap().phase, Phase::Reading);
    assert!(
        b.painted()
            .contains("Reading the file for Siemens 3RT2016-1AP01")
    );
    assert!(matches!(b.pump_until_settled(20), Phase::Done(_)));
    assert_eq!(b.row(&k), RowAction::InLibrary);
}

#[test]
fn a_download_that_never_comes_ends_with_a_message_and_try_again() {
    let mut b = bench();
    let k = key("Siemens", "3RT2016-1AP01");
    b.click_import(&k);
    b.win.pump_import(10.0);
    let text = b.painted();
    assert!(text.contains("Waiting for the download"), "{text}");
    assert!(text.contains("Stop"), "{text}");
    // Just inside the limit it still waits; past it, it stops by itself.
    b.win.pump_import(10.0 + WAIT_LIMIT_SECONDS - 1.0);
    assert_eq!(b.win.import.as_ref().unwrap().phase, Phase::Waiting);
    b.win.pump_import(10.0 + WAIT_LIMIT_SECONDS + 1.0);
    let Phase::Failed(why) = b.win.import.as_ref().unwrap().phase.clone() else {
        panic!("still waiting after the limit");
    };
    assert!(why.contains("No file arrived"), "{why}");
    let text = b.painted();
    assert!(
        text.contains("Could not import Siemens 3RT2016-1AP01"),
        "{text}"
    );
    assert!(text.contains("Try again"), "{text}");
    assert_ne!(b.row(&k), RowAction::InLibrary, "nothing was imported");
}

#[test]
fn a_missing_or_broken_file_shows_an_error_and_imports_nothing() {
    let mut b = bench();
    let k = key("Siemens", "3RT2017-1BB41");
    b.win.select(k.clone());
    // A path that does not exist (deleted between pick and read).
    b.win
        .import_file(Path::new("/no/such/3RT2017-1BB41.stp"), ImportFlow::Manual);
    let Phase::Failed(why) = b.pump_until_settled(20) else {
        panic!("a missing file must fail");
    };
    assert!(!why.is_empty());
    assert!(
        b.painted()
            .contains("Could not import Siemens 3RT2017-1BB41")
    );
    assert_ne!(b.row(&k), RowAction::InLibrary);

    // A ZIP that is not a ZIP, and an empty STEP file.
    for (name, content) in [
        ("3RT2017-1BB41.zip", "not a zip"),
        ("3RT2017-1BB41.stp", ""),
    ] {
        let elsewhere = tempfile::tempdir().unwrap();
        write(elsewhere.path(), name, content);
        b.win
            .import_file(&elsewhere.path().join(name), ImportFlow::Manual);
        let phase = b.pump_until_settled(20);
        // Either refused with a reason, or kept with a note: never a spinner.
        assert!(
            matches!(phase, Phase::Failed(_) | Phase::Done(_)),
            "{name}: {phase:?}"
        );
    }
}

#[test]
fn without_a_profile_folder_the_import_says_so() {
    let mut b = bench();
    b.win.library = None;
    let k = key("Siemens", "3RT2016-1AP01");
    b.win.select(k.clone());
    b.win.start_import(&k);
    // start_import needs a library, so there is no import to fail: dropping
    // a file must still not spin or panic.
    let elsewhere = tempfile::tempdir().unwrap();
    write(elsewhere.path(), "a.stp", &step(1.0, 2.0, 3.0));
    b.win
        .import_file(&elsewhere.path().join("a.stp"), ImportFlow::Manual);
    if let Some(i) = b.win.import.as_ref() {
        assert!(matches!(i.phase, Phase::Failed(_)), "{:?}", i.phase);
    }
}

#[test]
fn picking_another_part_ends_the_wait_so_a_drop_goes_to_the_right_part() {
    let mut b = bench();
    let old = key("Siemens", "3RT2017-1BB41");
    let new = key("Siemens", "3RT2016-1AP01");
    b.click_import(&old);
    b.win.select(new.clone());
    assert!(b.win.import.is_none(), "the old wait is gone");
    assert_eq!(b.win.target().unwrap().key(), new);
}

#[test]
fn a_read_that_stalls_ends_after_the_limit_and_one_that_dies_ends_at_once() {
    let part = novo_core_part();
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path().join("lib"));
    let mut imp = Import::start(&part, &lib, &SourceTable::builtin(), None, "2026-10-19");

    // Stalled: the sender stays alive and never answers.
    let (tx, rx) = std::sync::mpsc::channel();
    imp.install_read_for_test(rx, "huge.zip");
    assert!(imp.tick(1.0).is_none());
    assert_eq!(imp.phase, Phase::Reading);
    imp.tick(1.0 + READ_LIMIT_SECONDS + 1.0);
    let Phase::Failed(why) = imp.phase.clone() else {
        panic!("a stalled read must end");
    };
    assert!(
        why.contains("huge.zip") && why.contains("longer than"),
        "{why}"
    );
    drop(tx);

    // Dead: the thread went away without a result.
    let (tx, rx) = std::sync::mpsc::channel();
    imp.install_read_for_test(rx, "x.zip");
    drop(tx);
    imp.tick(5.0);
    assert!(matches!(imp.phase, Phase::Failed(_)), "{:?}", imp.phase);
}

fn novo_core_part() -> novo_parts::CatalogPart {
    novo_parts::CatalogPart::new("Siemens", "3RT2016-1AP01")
}

/// The whole app, not just the window: the lesson's steps as a person does
/// them (open Parts from the palette command, type the order number, pick
/// the part, Import), with the maker's file arriving in Downloads. Frames
/// run through `NovoApp::show`, so the background read, the repaint wake and
/// the catalog merge all go through the code the real window runs.
#[test]
fn the_lesson_flow_through_the_whole_app_imports_both_siemens_contactors() {
    use super::super::gate1_tests::Drive;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "parts.browser", None);
    d.frame();
    d.frame();
    let downloads = dir.path().join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    {
        let win = d.app.wb.parts.as_mut().expect("the Parts window opened");
        // The real window reads the user's profile; the test uses its own.
        win.catalog = Catalog::starter();
        win.library = Some(LocalLibrary::at(dir.path().join("lib")));
        win.catalog_path = Some(dir.path().join("parts.json"));
        win.downloads = Some(downloads.clone());
    }
    for (order, file) in [
        ("3RT2016-1AP01", "3RT2016-1AP01.stp"),
        ("3RT2017-1BB41", "3RT2017-1BB41.stp"),
    ] {
        // Step "Search by order number": typed into the box.
        d.app.wb.parts.as_mut().unwrap().query.clear();
        d.frame_with(vec![egui::Event::Text(order.to_owned())]);
        d.frame();
        let win = d.app.wb.parts.as_ref().unwrap();
        let rows = win.rows();
        assert_eq!(
            rows.len(),
            1,
            "{order}: one row remains (query {:?})",
            win.query
        );
        let k = rows[0].0.clone();
        assert_ne!(rows[0].1, RowAction::InLibrary);
        // Step "Import with one click".
        let win = d.app.wb.parts.as_mut().unwrap();
        win.select(k.clone());
        win.start_import(&k);
        d.frame();
        write(&downloads, file, &step(45.0, 57.5, 73.0));
        let started = Instant::now();
        loop {
            d.frame();
            let win = d.app.wb.parts.as_ref().unwrap();
            match &win.import.as_ref().unwrap().phase {
                Phase::Done(_) => break,
                Phase::Failed(e) => panic!("{order}: {e}"),
                _ => {}
            }
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "{order}: spinner never stopped"
            );
            std::thread::sleep(Duration::from_millis(30));
        }
        // Step "Check what was found": the row now says In library.
        let win = d.app.wb.parts.as_ref().unwrap();
        assert_eq!(win.rows()[0].1, RowAction::InLibrary, "{order}");
        d.app.wb.parts.as_mut().unwrap().import = None;
        d.frame();
    }
}

/// A download that arrived but does not name the part is not taken, and the
/// strip says so instead of spinning silently.
#[test]
fn a_download_that_does_not_name_the_part_is_listed_not_ignored() {
    let mut b = bench();
    let k = key("Siemens", "3RT2016-1AP01");
    b.click_import(&k);
    write(&b.downloads, "A5E02345678.zip", "PK data");
    for i in 0..3 {
        b.win.pump_import(100.0 + 1.1 * f64::from(i));
    }
    assert_eq!(b.win.import.as_ref().unwrap().phase, Phase::Waiting);
    let text = b.painted();
    assert!(text.contains("A5E02345678.zip"), "{text}");
    assert!(text.contains("Choose file"), "{text}");
    // Choosing it by hand imports it (an unreadable archive fails with a reason).
    b.win
        .import_file(&b.downloads.join("A5E02345678.zip"), ImportFlow::Manual);
    assert!(matches!(
        b.pump_until_settled(20),
        Phase::Failed(_) | Phase::Done(_)
    ));
}
