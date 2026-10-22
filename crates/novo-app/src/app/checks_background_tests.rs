//! The design check on a big project runs on a worker thread, so an edit does
//! not freeze the window (`responsiveness` handoff).

use crate::actions::Action;
use crate::app::gate1_tests::Drive;

fn big_project() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_symbols(&mut d, 1600);
    (dir, d)
}

fn add_symbols(d: &mut Drive, n: usize) {
    let page = d.app.current_page().expect("a page is shown");
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Many symbols", |tx| {
        for i in 0..n {
            tx.place_symbol(
                page,
                "iec/contacts/no",
                novo_core::Placement::at(novo_core::Point::new(10.0 * (i % 100) as f64, 20.0)),
                vec![
                    novo_core::Pin::new("1", 0.0, 0.0),
                    novo_core::Pin::new("2", 0.0, 10.0),
                ],
                novo_core::SymbolRole::Unassigned,
            )?;
        }
        Ok(())
    })
    .unwrap();
}

/// Like [`add_symbols`] but tells the backend, as the app's edit path does, so
/// saves and autosaves include the edit.
fn add_symbols_recorded(d: &mut Drive, n: usize) {
    let page = d.app.current_page().expect("a page is shown");
    let doc = d.app.backend.document_mut().unwrap();
    let (_, changes) = doc
        .transact("Many symbols", |tx| {
            for i in 0..n {
                tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    novo_core::Placement::at(novo_core::Point::new(10.0 * (i % 100) as f64, 60.0)),
                    vec![
                        novo_core::Pin::new("1", 0.0, 0.0),
                        novo_core::Pin::new("2", 0.0, 10.0),
                    ],
                    novo_core::SymbolRole::Unassigned,
                )?;
            }
            Ok(())
        })
        .unwrap();
    d.app.backend.record(&changes);
}

fn wait_for_checks(d: &mut Drive) {
    for _ in 0..400 {
        d.frame();
        if !d.app.checks_pending() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the background check never finished");
}

#[test]
fn a_big_project_is_checked_in_the_background_and_the_old_answer_stays() {
    let (_dir, mut d) = big_project();
    // The first frame after the edit starts a worker; it does not wait.
    assert!(d.app.shown_problems().is_empty());
    assert!(d.app.checks_pending());
    assert!(d.app.wb.check_job.is_some());
    wait_for_checks(&mut d);
    let finished = d.app.wb.problems.as_ref().map(|(r, _)| *r);
    assert_eq!(
        finished,
        d.app.document().map(|doc| doc.project().revision())
    );
    let before = d.app.shown_problems().len();
    assert!(before > 0, "1600 unconnected symbols leave open ends");

    // An edit makes the shown list one revision old until the worker lands;
    // the old list stays visible meanwhile.
    add_symbols(&mut d, 4);
    assert_eq!(d.app.shown_problems().len(), before);
    assert!(d.app.checks_pending());
    wait_for_checks(&mut d);
    assert!(d.app.shown_problems().len() > before);
}

#[test]
fn exports_and_reports_wait_for_the_current_answer() {
    let (_dir, mut d) = big_project();
    let now = d.app.problems().len();
    assert!(now > 0);
    assert!(!d.app.checks_pending());
}

#[test]
fn a_small_project_is_checked_in_the_same_frame() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_symbols(&mut d, 20);
    let _ = d.app.shown_problems();
    assert!(!d.app.checks_pending());
    assert!(d.app.wb.check_job.is_none());
}

/// Not a test: times the frames of a big project file (set `KOPERCAD_SCALE_FILE`)
/// in the egui test harness, no GPU. Run in release:
/// `KOPERCAD_SCALE_FILE=target/scale600.novo cargo test --release -p novo-app --lib frame_cost -- --ignored --nocapture`
#[test]
#[ignore]
fn frame_cost_of_a_big_project() {
    let Some(file) = std::env::var_os("KOPERCAD_SCALE_FILE") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.open_path(std::path::Path::new(&file));
    let frames: usize = std::env::var("KOPERCAD_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    for i in 0..frames {
        let t = std::time::Instant::now();
        d.frame();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        if i < 6 || i + 1 == frames {
            eprintln!("frame {i}: {ms:.1} ms");
        }
        if i == 5 {
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
    }
}

#[test]
fn a_burst_of_edits_runs_one_check_at_a_time() {
    let (_dir, mut d) = big_project();
    let _ = d.app.shown_problems();
    assert_eq!(d.app.wb.check_started, 1);
    // Three quick edits while that run is in flight start nothing more.
    for _ in 0..3 {
        add_symbols(&mut d, 1);
        let _ = d.app.shown_problems();
    }
    assert!(
        d.app.wb.check_started <= 2,
        "one worker in flight, then at most one for the latest revision, saw {}",
        d.app.wb.check_started
    );
    wait_for_checks(&mut d);
    // Whatever order they landed in, the shown answer is for the last revision
    // and no more than one run per landing was needed.
    assert!(d.app.wb.check_started <= 3);
    assert!(!d.app.checks_pending());
    assert!(d.app.wb.check_job.is_none());
}

#[test]
fn a_dead_worker_falls_back_to_the_foreground_check() {
    let (_dir, mut d) = big_project();
    // Replace the running worker with one that died without an answer.
    let revision = d.app.document().unwrap().project().revision();
    let (tx, rx) = std::sync::mpsc::channel();
    drop(tx);
    d.app.wb.check_job = Some(super::checks::CheckJob { revision, rx });
    let started = d.app.wb.check_started;
    let _ = d.app.shown_problems();
    assert_eq!(d.app.wb.check_failed, Some(revision));
    // The next frame checks in the foreground: nothing keeps pulsing.
    let _ = d.app.shown_problems();
    assert!(!d.app.checks_pending());
    assert!(d.app.wb.check_job.is_none());
    assert_eq!(
        d.app.wb.check_started, started,
        "no new worker for a failed revision"
    );
}

/// The project file as the next start would read it, while the app still has
/// it open: a copy of the bytes (an open project holds the file's lock, and
/// on Windows a locked file cannot be copied, so this is Unix only).
#[cfg(unix)]
fn on_disk_while_open(path: &std::path::Path) -> String {
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("copy.novo");
    std::fs::copy(path, &copy).unwrap();
    novo_storage::ProjectFile::read(&copy).unwrap().to_json()
}

/// Gives the backend its chance to write, as the app loop does each frame.
fn write_tick(d: &mut Drive) {
    d.app.next_tick = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let ctx = d.ctx.clone();
    d.app.tick(&ctx);
}

/// A saved big project keeps writing its edits to the file while the design
/// check runs on a worker: the worker reads a snapshot, so it neither holds
/// the file nor sees half an edit, and what lands on disk is the drawing.
#[test]
fn a_saved_big_project_keeps_writing_while_a_check_runs() {
    let (dir, mut d) = big_project();
    let path = dir.path().join("plant.novo");
    assert!(d.app.save_to(&path));

    // A check is in flight for the saved revision.
    add_symbols_recorded(&mut d, 3);
    let _ = d.app.shown_problems();
    assert!(d.app.wb.check_job.is_some(), "a worker is running");

    // Edits and writes alternate while it runs, and after every write the
    // file is the drawing as it stands.
    for round in 0..4 {
        add_symbols_recorded(&mut d, 1);
        let _ = d.app.shown_problems();
        assert!(d.app.is_dirty(), "round {round}: the edit is unsaved");
        write_tick(&mut d);
        assert!(!d.app.autosave_failing, "round {round}: the write failed");
        assert!(!d.app.is_dirty(), "round {round}: written, so not unsaved");
        #[cfg(unix)]
        assert_eq!(
            on_disk_while_open(&path),
            d.app.document().unwrap().project().to_json(),
            "round {round}"
        );
    }

    // The checks catch up without having changed the drawing.
    let before = d.app.document().unwrap().project().to_json();
    wait_for_checks(&mut d);
    assert_eq!(d.app.document().unwrap().project().to_json(), before);
    assert!(!d.app.checks_pending());

    // Closed, the file is the drawing, on every platform.
    d.app.backend.close().unwrap();
    assert_eq!(
        novo_storage::ProjectFile::read(&path).unwrap().to_json(),
        before
    );
}

/// An untitled big project autosaves to its recovery file while a check
/// runs, and a crash then offers exactly the drawing as it stood.
#[test]
fn an_untitled_big_project_autosaves_while_a_check_runs() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    add_symbols_recorded(&mut d, 1600);
    let _ = d.app.shown_problems();
    assert!(d.app.wb.check_job.is_some(), "a worker is running");
    add_symbols_recorded(&mut d, 2);
    let _ = d.app.shown_problems();
    let project_json = d.app.document().unwrap().project().to_json();

    // The autosave interval has passed.
    d.app
        .backend
        .set_autosave_interval(std::time::Duration::ZERO);
    write_tick(&mut d);
    assert!(!d.app.autosave_failing);
    wait_for_checks(&mut d);

    // Closing with work keeps the recovery file; it holds the drawing.
    d.app.backend.close().unwrap();
    let kept = d.app.backend.unsaved_projects();
    assert_eq!(kept.len(), 1, "one project with unsaved work is kept");
    assert_eq!(
        novo_storage::ProjectFile::read(&kept[0].0)
            .unwrap()
            .to_json(),
        project_json
    );
}
