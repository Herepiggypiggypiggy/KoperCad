//! Project data safety: a big project saves, autosaves, survives a crash
//! and a damaged file; undo across a save is what reaches the disk.
//!
//! Time limits are generous (debug build, shared CI machines): they catch
//! autosave going quadratic or rewriting everything per edit, not a slow
//! afternoon.

use std::time::{Duration, Instant};

use novo_core::{DeviceTag, Document, Placement, Point};
use novo_storage::{ProjectFile, Session, salvage};
use novo_tests::{BigSpec, big_project, motor_starter};

fn big_600_pages() -> Document {
    let mut doc = Document::default();
    big_project(&mut doc, BigSpec::LARGE).expect("generator");
    assert!(doc.project().pages().count() >= 600, "600 pages");
    doc
}

#[test]
fn a_600_page_project_saves_autosaves_incrementally_and_survives_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.novo");
    let mut doc = big_600_pages();

    let started = Instant::now();
    let mut session = Session::create(&path, doc.project()).unwrap();
    let full = started.elapsed();
    assert!(full < Duration::from_secs(60), "full save took {full:?}");

    // A small edit is autosaved by writing only that edit.
    let device = doc.project().devices().next().unwrap().id;
    let (_, c) = doc
        .transact("Rename", |tx| {
            tx.set_tag(device, DeviceTag::parse("=A1+X1-ZZ99").unwrap())
        })
        .unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap(); // first one writes the file
    let (_, c) = doc
        .transact("Rename again", |tx| {
            tx.set_tag(device, DeviceTag::parse("=A1+X1-ZZ98").unwrap())
        })
        .unwrap();
    session.record(&c);
    let started = Instant::now();
    session.autosave(doc.project()).unwrap();
    let incremental = started.elapsed();
    assert!(
        incremental < Duration::from_secs(5) && incremental < full,
        "an incremental autosave ({incremental:?}) is far cheaper than a full save ({full:?})"
    );

    // The app dies without closing: the work is offered back, whole.
    drop(session);
    let started = Instant::now();
    let opened = Session::open(&path).unwrap();
    let open = started.elapsed();
    assert!(open < Duration::from_secs(60), "opening took {open:?}");
    let recovered = opened
        .recovered
        .expect("the unsaved edits are offered back");
    let (left, right) = (recovered.project.to_json(), doc.project().to_json());
    if left != right {
        let first = left.lines().zip(right.lines()).position(|(a, b)| a != b);
        let at = first.unwrap_or(0);
        panic!(
            "recovered differs at line {at} (lines {} vs {}):\n{:?}\n{:?}",
            left.lines().count(),
            right.lines().count(),
            left.lines()
                .skip(at.saturating_sub(3))
                .take(8)
                .collect::<Vec<_>>(),
            right
                .lines()
                .skip(at.saturating_sub(3))
                .take(8)
                .collect::<Vec<_>>()
        );
    }
    assert_ne!(
        opened.project.to_json(),
        doc.project().to_json(),
        "the saved file is the state before the edits"
    );
}

#[test]
fn a_600_page_project_cut_short_is_salvaged_into_a_copy_that_opens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.novo");
    let doc = big_600_pages();
    Session::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() * 7 / 10]).unwrap();

    assert!(
        Session::open(&path).is_err(),
        "a cut file is not opened for editing"
    );
    let salvaged = salvage(&path).unwrap();
    assert!(salvaged.cut_short);
    assert!(salvaged.recovered > 0);
    let total = doc.project().records().count();
    assert!(
        salvaged.recovered < total,
        "the lost end is reported, not invented"
    );
    let reopened = ProjectFile::read(&salvaged.copy).unwrap();
    assert_eq!(reopened.records().count(), salvaged.recovered);
}

/// The unsaved work after an undo that crosses a save is the undone state,
/// not the state that was saved.
#[test]
fn a_crash_after_undoing_past_a_save_offers_the_undone_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("starter.novo");
    let mut doc = Document::default();
    let s = motor_starter(&mut doc).unwrap();
    let mut session = Session::create(&path, doc.project()).unwrap();

    let (_, c) = doc
        .transact("Move coil", |tx| {
            tx.move_symbol(s.k1_coil, Placement::at(Point::new(80.0, 168.0)))
        })
        .unwrap();
    session.record(&c);
    session.save(doc.project()).unwrap();
    let saved = doc.project().to_json();

    let c = doc.undo().unwrap().unwrap();
    session.record(&c);
    assert!(
        session.has_unsaved_changes(),
        "undoing past a save is a change"
    );
    session.autosave(doc.project()).unwrap();
    let undone = doc.project().to_json();
    assert_ne!(undone, saved);
    drop(session);

    let opened = Session::open(&path).unwrap();
    assert_eq!(
        opened.project.to_json(),
        saved,
        "the file still holds the save"
    );
    assert_eq!(opened.recovered.unwrap().project.to_json(), undone);
}

/// Save, undo, save, redo, then a crash with an autosave in between: the
/// file after each save is the state at that save, and the recovery is the
/// state after the redo.
#[test]
fn undo_redo_and_saves_in_any_order_reach_the_file_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("starter.novo");
    let mut doc = Document::default();
    let s = motor_starter(&mut doc).unwrap();
    let mut session = Session::create(&path, doc.project()).unwrap();

    let (_, c) = doc.transact("Delete -S1", |tx| tx.remove(s.s1)).unwrap();
    session.record(&c);
    session.save(doc.project()).unwrap();
    let after_delete = doc.project().to_json();

    let c = doc.undo().unwrap().unwrap();
    session.record(&c);
    session.save(doc.project()).unwrap();
    let after_undo = doc.project().to_json();
    assert_ne!(after_delete, after_undo);
    // Read a copy: the open session holds the file's lock.
    let copy = dir.path().join("copy.novo");
    std::fs::copy(&path, &copy).unwrap();
    assert_eq!(
        ProjectFile::read(&copy).unwrap().to_json(),
        after_undo,
        "the file follows the undo"
    );

    let c = doc.redo().unwrap().unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();
    drop(session);

    let opened = Session::open(&path).unwrap();
    assert_eq!(opened.project.to_json(), after_undo);
    assert_eq!(opened.recovered.unwrap().project.to_json(), after_delete);
}
