//! Project data safety: a big project saves, autosaves, survives a crash
//! and a damaged file; undo across a save is what reaches the disk.
//!
//! Time limits are generous (debug build, shared CI machines): they catch
//! autosave going quadratic or rewriting everything per edit, not a slow
//! afternoon.

use std::path::{Path, PathBuf};
use std::process::Stdio;
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

// ------------------------------------------------- a real kill during a save

/// The 600-page project and the same project with one more page: the two
/// states the saving child alternates between.
fn big_pair() -> (novo_core::Project, novo_core::Project) {
    let mut doc = big_600_pages();
    let a = doc.project().clone();
    doc.transact("One more page", |tx| tx.add_page("LAST").map(|_| ()))
        .unwrap();
    (a, doc.project().clone())
}

/// The file the child touches after each finished save.
fn marker(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap().to_owned();
    name.push(".saved");
    path.with_file_name(name)
}

/// Child: saves the two 600-page states over the file for as long as it lives.
#[test]
fn child_big_save_loop() {
    let Some(path) = std::env::var_os("KOPERCAD_BIG_CHILD_FILE") else {
        return;
    };
    let (a, b) = big_pair();
    let mut file = ProjectFile::open(&path).unwrap();
    let marker = marker(Path::new(&path));
    let mut next_is_b = true;
    loop {
        file.save(if next_is_b { &b } else { &a }).unwrap();
        let _ = std::fs::write(&marker, b"saved");
        next_is_b = !next_is_b;
    }
}

/// Child: creates a new 600-page project file (the first save of a project).
#[test]
fn child_big_create() {
    let Some(path) = std::env::var_os("KOPERCAD_BIG_CHILD_FILE") else {
        return;
    };
    let (a, _) = big_pair();
    // The parent kills this process at a staggered moment after it starts.
    Session::create(&path, &a).unwrap().close().unwrap();
    let _ = std::fs::write(marker(Path::new(&path)), b"saved");
}

/// A child process that is killed when this goes out of scope, so a test that
/// fails between spawning and killing does not leave it saving forever.
struct Child(std::process::Child);

impl Child {
    /// Kills the child and waits for it; killing one that already ended is fine.
    fn kill(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        self.kill();
    }
}

fn spawn_child(test: &str, path: &Path) -> Child {
    Child(spawn_child_process(test, path))
}

fn spawn_child_process(test: &str, path: &Path) -> std::process::Child {
    std::process::Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--nocapture", "--test-threads=1"])
        .env("KOPERCAD_BIG_CHILD_FILE", path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// A process killed during a full save of a 600-page project leaves the last
/// good save or the new one, never a mix, and never a file that needs repair.
#[test]
fn a_process_killed_during_a_600_page_save_leaves_a_whole_project() {
    let (a, b) = big_pair();
    let (json_a, json_b) = (a.to_json(), b.to_json());
    let (mut last_good, mut new_one, mut killed_inside) = (0, 0, 0);
    for round in 0..6u64 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.novo");
        Session::create(&path, &a).unwrap().close().unwrap();

        let mut child = spawn_child("child_big_save_loop", &path);
        // Once the child has finished a save of its own, the kill lands
        // somewhere in the saves that follow, at a different point per round.
        let marker = marker(&path);
        let deadline = Instant::now() + Duration::from_secs(300);
        while !marker.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            marker.exists(),
            "round {round}: the child never finished a save"
        );
        // The write transaction keeps a rollback journal beside the file
        // while it runs, so waiting for it puts the kill inside a save.
        let mut journal = path.as_os_str().to_owned();
        journal.push("-journal");
        let journal = PathBuf::from(journal);
        while !journal.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        std::thread::sleep(Duration::from_millis(round * 60));
        child.kill();
        if journal.exists() {
            killed_inside += 1;
        }

        // The relaunch: the file opens for editing as it is.
        let opened = Session::open(&path)
            .unwrap_or_else(|e| panic!("round {round}: the file after the kill: {e}"));
        let json = opened.project.to_json();
        if json == json_a {
            last_good += 1;
        } else if json == json_b {
            new_one += 1;
        } else {
            panic!("round {round}: neither the last good save nor the new one");
        }
        opened.session.close().unwrap();
    }
    eprintln!(
        "kill rounds: {last_good} reopened as the first state, {new_one} as the second, \
         {killed_inside} killed with a save unfinished"
    );
    assert!(
        killed_inside >= 3,
        "most kills must land inside a save ({killed_inside} of 6)"
    );
}

/// A process killed during the first save of a new 600-page project leaves
/// no file, or a whole one: never a half-written project under its name.
#[test]
fn a_process_killed_during_the_first_600_page_save_leaves_nothing_or_all() {
    let (a, _) = big_pair();
    let json_a = a.to_json();
    let (mut none, mut whole) = (0, 0);
    for round in 0..6u64 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.novo");
        let mut child = spawn_child("child_big_create", &path);
        // Rounds 0 to 3 kill early, at staggered moments; the later rounds
        // wait for the child to finish, so both ends are covered.
        let marker = marker(&path);
        if round < 4 {
            std::thread::sleep(Duration::from_millis(200 + round * 400));
        } else {
            let deadline = Instant::now() + Duration::from_secs(300);
            while !marker.exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        child.kill();

        if path.exists() {
            let opened = Session::open(&path)
                .unwrap_or_else(|e| panic!("round {round}: a file under the name, not whole: {e}"));
            assert_eq!(opened.project.to_json(), json_a, "round {round}");
            opened.session.close().unwrap();
            whole += 1;
        } else {
            none += 1;
        }
    }
    eprintln!("first-save rounds: {none} no file, {whole} whole file");
    assert!(whole >= 2, "the late rounds must finish a save");
}
