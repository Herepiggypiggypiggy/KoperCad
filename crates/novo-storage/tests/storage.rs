//! End-to-end tests of the project file: round trips, incremental saves,
//! locking, autosave and crash recovery, and refusing files we cannot read.

use std::path::{Path, PathBuf};
use std::time::Duration;

use novo_core::*;
use novo_storage::{ProjectFile, Session, StorageError, autosave_path, schema};
use tempfile::TempDir;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn at(x: f64, y: f64) -> Placement {
    Placement::at(Point::new(x, y))
}

fn two_pins(a: &str, b: &str) -> Vec<Pin> {
    vec![Pin::new(a, 0.0, -5.0), Pin::new(b, 0.0, 5.0)]
}

fn temp() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("motor starter.novo");
    (dir, path)
}

/// Asserts two projects are identical, including the id counter.
fn assert_same(a: &Project, b: &Project) {
    assert_eq!(a.to_json(), b.to_json());
    assert_eq!(a.next_id(), b.next_id());
}

/// A two-page DOL starter using every kind of object: parts, potentials,
/// pages, devices, a terminal strip, symbols, junctions, wires, a cable, and
/// properties of every value type.
fn build_starter(doc: &mut Document) -> Vec<AnyId> {
    let (ids, _) = doc
        .transact("Draw starter", |tx| {
            tx.set_info(|info| {
                info.name = "Pump station P-101 — Förderpumpe".into();
                info.description = "DOL starter\nwith self-hold".into();
                info.properties
                    .insert("customer".into(), Value::Text("ACME".into()));
            })?;
            let page1 = tx.add_page("1")?;
            let page2 = tx.add_page("2")?;
            let l = tx.add_potential("L1", PotentialKind::Line)?;
            let n = tx.add_potential("N", PotentialKind::Neutral)?;
            let part = tx.add_part("Siemens", "3RT2015-1BB41")?;
            let k1 = tx.add_device(tag("=A1+CAB1-K1"), "Main contactor")?;
            tx.assign_part(k1, part)?;
            tx.set_property(k1, "coil_voltage", Some(Value::Float(24.0)))?;
            tx.set_property(k1, "poles", Some(Value::Int(3)))?;
            tx.set_property(k1, "spare", Some(Value::Bool(false)))?;
            tx.set_property(k1, "power_kw", Some(Value::Float(0.1 + 0.2)))?;
            let s0 = tx.add_device(tag("=A1+CAB1-S0"), "Stop")?;
            let s1 = tx.add_device(tag("=A1+CAB1-S1"), "Start")?;
            let x1 = tx.add_terminal_strip(tag("=A1+CAB1-X1"))?;
            let t1 = tx.add_terminal(x1, "1")?;

            let feed_l = tx.place_symbol(
                page1,
                "iec/potential",
                at(10.0, 10.0),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::PotentialFeed { potential: l },
            )?;
            let x1_1 = tx.place_symbol(
                page1,
                "iec/terminal",
                at(10.0, 30.0),
                two_pins("1", "2"),
                SymbolRole::Terminal {
                    terminals: vec![t1],
                },
            )?;
            let stop = tx.place_symbol(
                page1,
                "iec/contact_nc",
                Placement {
                    origin: Point::new(10.0, 60.0),
                    rotation: Rotation::R90,
                    mirrored: true,
                },
                two_pins("11", "12"),
                SymbolRole::Device {
                    device: s0,
                    main: true,
                },
            )?;
            let start = tx.place_symbol(
                page1,
                "iec/contact_no",
                at(10.0, 90.0),
                two_pins("13", "14"),
                SymbolRole::Device {
                    device: s1,
                    main: true,
                },
            )?;
            let hold = tx.place_symbol(
                page1,
                "iec/contact_no",
                at(30.0, 90.0),
                two_pins("13", "14"),
                SymbolRole::Device {
                    device: k1,
                    main: false,
                },
            )?;
            let jump = tx.place_symbol(
                page1,
                "iec/interruption",
                at(10.0, 120.0),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Interruption {
                    name: "K1.A1".into(),
                },
            )?;
            let land = tx.place_symbol(
                page2,
                "iec/interruption",
                at(10.0, 10.0),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Interruption {
                    name: "K1.A1".into(),
                },
            )?;
            let coil = tx.place_symbol(
                page2,
                "iec/coil",
                at(10.0, 40.0),
                two_pins("A1", "A2"),
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            )?;
            let feed_n = tx.place_symbol(
                page2,
                "iec/potential",
                at(10.0, 70.0),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::PotentialFeed { potential: n },
            )?;
            let w1 = tx.connect(Endpoint::pin(feed_l, 0), Endpoint::pin(x1_1, 0))?;
            let w2 = tx.connect(Endpoint::pin(x1_1, 1), Endpoint::pin(stop, 0))?;
            let split = tx.add_junction(page1, Point::new(10.0, 75.0))?;
            tx.connect(Endpoint::pin(stop, 1), split)?;
            tx.connect(split, Endpoint::pin(start, 0))?;
            tx.connect(split, Endpoint::pin(hold, 0))?;
            let join = tx.add_junction(page1, Point::new(10.0, 105.0))?;
            tx.connect(Endpoint::pin(start, 1), join)?;
            tx.connect(Endpoint::pin(hold, 1), join)?;
            tx.connect(join, Endpoint::pin(jump, 0))?;
            tx.connect(Endpoint::pin(land, 0), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(feed_n, 0))?;
            let w1c = tx.add_cable(
                tag("=A1-W1"),
                vec![CableCore::new("BK"), CableCore::new("GNYE")],
            )?;
            tx.assign_core(w1c, 0, Some(w1))?;
            tx.assign_core(w1c, 1, Some(w2))?;
            Ok(vec![
                page1.into(),
                k1.into(),
                coil.into(),
                hold.into(),
                w1.into(),
                w1c.into(),
                part.into(),
            ])
        })
        .unwrap();
    ids
}

#[test]
fn save_close_reopen_is_identical() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    build_starter(&mut doc);

    ProjectFile::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();
    let loaded = ProjectFile::read(&path).unwrap();
    assert_same(&loaded, doc.project());

    // The reloaded project behaves the same, not just looks the same.
    let a = doc.project().netlist();
    let b = loaded.netlist();
    assert_eq!(a.nets().len(), b.nets().len());
    assert_eq!(loaded.unconnected_pins(), doc.project().unconnected_pins());

    // And saving it again changes nothing.
    let again = temp_path_in(&path, "again.novo");
    ProjectFile::create(&again, &loaded)
        .unwrap()
        .close()
        .unwrap();
    assert_same(&ProjectFile::read(&again).unwrap(), doc.project());
}

fn temp_path_in(sibling: &Path, name: &str) -> PathBuf {
    sibling.with_file_name(name)
}

#[test]
fn empty_project_round_trips() {
    let (_dir, path) = temp();
    let project = Project::new("Empty");
    ProjectFile::create(&path, &project)
        .unwrap()
        .close()
        .unwrap();
    assert_same(&ProjectFile::read(&path).unwrap(), &project);
}

#[test]
fn incremental_saves_track_edits_undo_and_redo() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project()).unwrap();
    assert!(!session.has_unsaved_changes());

    let ids = build_starter(&mut doc);
    // build_starter does not report its change set; record a full one.
    let mut all = ChangeSet::default();
    all.added.extend(doc.project().records().map(|r| r.id()));
    all.info = true;
    session.record(&all);
    assert!(session.has_unsaved_changes());
    session.save(doc.project()).unwrap();
    assert!(!session.has_unsaved_changes());

    let AnyId::Symbol(hold) = ids[3] else {
        panic!()
    };
    let AnyId::Device(k1) = ids[1] else { panic!() };

    // Move, rename, delete (with its wires), undo one, redo it, add a page.
    let (_, c) = doc
        .transact("Move", |tx| tx.move_symbol(hold, at(50.0, 90.0)))
        .unwrap();
    session.record(&c);
    let (_, c) = doc
        .transact("Rename", |tx| tx.set_tag(k1, tag("=A1+CAB1-K2")))
        .unwrap();
    session.record(&c);
    let (_, c) = doc
        .transact("Delete hold", |tx| {
            let wires: Vec<WireId> = tx.project().wires_at(Endpoint::pin(hold, 0)).collect();
            let wires2: Vec<WireId> = tx.project().wires_at(Endpoint::pin(hold, 1)).collect();
            for w in wires.into_iter().chain(wires2) {
                tx.remove(w)?;
            }
            tx.remove(hold)
        })
        .unwrap();
    session.record(&c);
    session.record(&doc.undo().unwrap().unwrap());
    session.record(&doc.redo().unwrap().unwrap());
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("3")).unwrap();
    session.record(&c);
    session.save(doc.project()).unwrap();
    session.close().unwrap();

    assert_same(&ProjectFile::read(&path).unwrap(), doc.project());
}

#[test]
fn a_second_open_is_refused_while_the_file_is_open() {
    let (_dir, path) = temp();
    let file = ProjectFile::create(&path, &Project::new("Locked")).unwrap();
    match ProjectFile::open(&path) {
        Err(StorageError::Locked(p)) => assert_eq!(p, path),
        other => panic!("expected Locked, got {other:?}"),
    }
    file.close().unwrap();
    ProjectFile::open(&path).unwrap();
}

#[test]
fn files_that_are_not_projects_are_refused() {
    let (dir, _) = temp();

    let text = dir.path().join("notes.novo");
    std::fs::write(&text, "this is not a database, just some text that is long enough to have a header ................................................................................").unwrap();
    assert!(matches!(
        ProjectFile::open(&text),
        Err(StorageError::NotAProject(_))
    ));

    let other_db = dir.path().join("other.sqlite");
    let conn = rusqlite::Connection::open(&other_db).unwrap();
    conn.execute_batch("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1);")
        .unwrap();
    drop(conn);
    assert!(matches!(
        ProjectFile::open(&other_db),
        Err(StorageError::NotAProject(_))
    ));

    let missing = dir.path().join("missing.novo");
    assert!(matches!(
        ProjectFile::open(&missing),
        Err(StorageError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn files_from_a_newer_version_are_refused_untouched() {
    let (_dir, path) = temp();
    ProjectFile::create(&path, &Project::new("Future"))
        .unwrap()
        .close()
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    drop(conn);
    let before = std::fs::read(&path).unwrap();

    match ProjectFile::open(&path) {
        Err(StorageError::NewerFormat {
            found, supported, ..
        }) => {
            assert_eq!(found, 99);
            assert_eq!(supported, schema::SCHEMA_VERSION);
        }
        other => panic!("expected NewerFormat, got {other:?}"),
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn damaged_records_are_reported_not_loaded() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    build_starter(&mut doc);
    ProjectFile::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();

    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE records SET data = '{\"kind\":\"wire\"' WHERE kind = 'wire'",
        [],
    )
    .unwrap();
    drop(conn);
    assert!(matches!(
        ProjectFile::read(&path),
        Err(StorageError::Corrupt { .. })
    ));
}

#[test]
fn dangling_references_are_refused_by_the_model() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    build_starter(&mut doc);
    ProjectFile::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();

    // Delete a page but keep the symbols drawn on it.
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("DELETE FROM records WHERE kind = 'page'", [])
        .unwrap();
    drop(conn);
    assert!(matches!(
        ProjectFile::read(&path),
        Err(StorageError::Model(ModelError::NotFound(_)))
    ));
}

/// Simulates a crash: the session goes away without `close`, so the
/// recovery file stays on disk.
fn crash(session: Session) {
    drop(session);
}

#[test]
fn autosave_recovers_unsaved_work_after_a_crash() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    build_starter(&mut doc);
    let mut session = Session::create(&path, doc.project()).unwrap();
    let saved = doc.project().clone();

    let (_, c) = doc.transact("Add page", |tx| tx.add_page("3")).unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();
    let (_, c) = doc
        .transact("Add device", |tx| tx.add_device(tag("-K9"), "Spare"))
        .unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();
    assert!(autosave_path(&path).exists());
    crash(session);

    let opened = Session::open(&path).unwrap();
    assert_same(&opened.project, &saved);
    let recovered = opened.recovered.expect("unsaved work is offered back");
    assert_same(&recovered.project, doc.project());
    assert!(recovered.autosaved_at.is_some());

    // Keep it and save: the project file now holds the recovered work.
    let mut session = opened.session;
    session.accept_recovery();
    assert!(session.has_unsaved_changes());
    session.save(&recovered.project).unwrap();
    assert!(!autosave_path(&path).exists());
    session.close().unwrap();

    let opened = Session::open(&path).unwrap();
    assert!(opened.recovered.is_none());
    assert_same(&opened.project, doc.project());
}

#[test]
fn recovery_can_be_discarded() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project()).unwrap();
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();
    crash(session);

    let mut opened = Session::open(&path).unwrap();
    assert!(opened.recovered.is_some());
    opened.session.discard_recovery().unwrap();
    assert!(!autosave_path(&path).exists());
    assert!(!opened.session.autosave_due());
    opened.session.close().unwrap();
    assert!(Session::open(&path).unwrap().recovered.is_none());
}

#[test]
fn a_clean_close_leaves_no_recovery_file() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project()).unwrap();
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();
    assert!(autosave_path(&path).exists());
    session.close().unwrap();
    assert!(!autosave_path(&path).exists());

    // Closing without saving drops the edit, as the user chose.
    let opened = Session::open(&path).unwrap();
    assert!(opened.recovered.is_none());
    assert_eq!(opened.project.pages().count(), 0);
}

#[test]
fn a_recovery_file_older_than_the_last_save_is_ignored() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project()).unwrap();
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();
    crash(session);

    // Keep the stale recovery file aside, save newer work, put it back.
    let stale = path.with_file_name("stale");
    std::fs::copy(autosave_path(&path), &stale).unwrap();
    let mut opened = Session::open(&path).unwrap();
    opened.session.discard_recovery().unwrap();
    let mut doc = Document::new(opened.project);
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("2")).unwrap();
    opened.session.record(&c);
    opened.session.save(doc.project()).unwrap();
    opened.session.close().unwrap();
    std::fs::rename(&stale, autosave_path(&path)).unwrap();

    let opened = Session::open(&path).unwrap();
    assert!(opened.recovered.is_none());
    assert!(!autosave_path(&path).exists());
    assert_same(&opened.project, doc.project());
}

#[test]
fn autosave_waits_for_changes_and_the_interval() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project()).unwrap();
    session.set_autosave_interval(Duration::ZERO);
    assert!(!session.autosave_if_due(doc.project()).unwrap());

    let (_, c) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    session.record(&c);
    assert!(session.autosave_if_due(doc.project()).unwrap());
    assert!(!session.autosave_if_due(doc.project()).unwrap());

    session.set_autosave_interval(Duration::from_secs(3600));
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("2")).unwrap();
    session.record(&c);
    assert!(!session.autosave_due());
    session.close().unwrap();
}

#[test]
fn save_as_moves_the_session_to_the_new_file() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project()).unwrap();
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();

    let copy = path.with_file_name("copy.novo");
    session.save_as(&copy, doc.project()).unwrap();
    assert_eq!(session.path(), copy);
    assert!(!session.has_unsaved_changes());
    assert!(!autosave_path(&path).exists());

    // The old file is free again and still holds its last save.
    assert_eq!(ProjectFile::read(&path).unwrap().pages().count(), 0);
    session.close().unwrap();
    assert_same(&ProjectFile::read(&copy).unwrap(), doc.project());
}

#[test]
fn create_replaces_an_existing_file() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    build_starter(&mut doc);
    ProjectFile::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();
    let empty = Project::new("Replaced");
    ProjectFile::create(&path, &empty).unwrap().close().unwrap();
    assert_same(&ProjectFile::read(&path).unwrap(), &empty);
}

#[test]
fn a_large_project_round_trips() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    doc.transact("Stress", |tx| {
        for p in 0..20 {
            let page = tx.add_page(format!("{p}"))?;
            let mut prev = None;
            for i in 0..250 {
                let s = tx.place_symbol(
                    page,
                    "iec/terminal",
                    at(i as f64 * 10.0, 0.0),
                    two_pins("1", "2"),
                    SymbolRole::Unassigned,
                )?;
                if let Some(prev) = prev {
                    tx.connect(Endpoint::pin(prev, 1), Endpoint::pin(s, 0))?;
                }
                prev = Some(s);
            }
        }
        Ok(())
    })
    .unwrap();
    assert!(doc.project().records().count() > 9_000);
    ProjectFile::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();
    assert_same(&ProjectFile::read(&path).unwrap(), doc.project());
}

#[test]
fn a_file_open_elsewhere_is_not_replaced() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let mut a = Session::create(&path, doc.project()).unwrap();
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    a.record(&c);
    a.autosave(doc.project()).unwrap();

    let other = Project::new("Other");
    assert!(matches!(
        Session::create(&path, &other),
        Err(StorageError::Locked(_))
    ));
    let mut b = Session::create(path.with_file_name("b.novo"), &other).unwrap();
    assert!(matches!(
        b.save_as(&path, &other),
        Err(StorageError::Locked(_))
    ));
    assert_eq!(b.path(), path.with_file_name("b.novo"));
    assert!(!path.with_file_name("motor starter.novo.saving").exists());

    // A's recovery file and file are untouched, and A can still save.
    assert!(autosave_path(&path).exists());
    a.save(doc.project()).unwrap();
    a.close().unwrap();
    b.close().unwrap();
    assert_same(&ProjectFile::read(&path).unwrap(), doc.project());
}

#[test]
fn create_replaces_a_file_that_is_not_a_database() {
    let (_dir, path) = temp();
    std::fs::write(&path, "plain text, not SQLite, long enough to be past any header ......................................................").unwrap();
    let project = Project::new("New");
    ProjectFile::create(&path, &project)
        .unwrap()
        .close()
        .unwrap();
    assert_same(&ProjectFile::read(&path).unwrap(), &project);
}

#[test]
fn save_as_to_the_same_file_is_a_save() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project()).unwrap();
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    session.record(&c);
    let same = path
        .parent()
        .unwrap()
        .join(".")
        .join(path.file_name().unwrap());
    session.save_as(&same, doc.project()).unwrap();
    assert!(!session.has_unsaved_changes());
    session.close().unwrap();
    assert_same(&ProjectFile::read(&path).unwrap(), doc.project());
}

#[test]
fn autosave_waits_for_the_recovery_decision() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project()).unwrap();
    session.set_autosave_interval(Duration::ZERO);
    let (_, c) = doc
        .transact("Two pages", |tx| {
            tx.add_page("1")?;
            tx.add_page("2")
        })
        .unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();
    crash(session);

    let opened = Session::open(&path).unwrap();
    let mut session = opened.session;
    session.set_autosave_interval(Duration::ZERO);
    // While the recovery dialog is showing, nothing overwrites the file.
    assert!(!session.autosave_due());
    assert!(!session.autosave_if_due(&opened.project).unwrap());
    session.autosave(&opened.project).unwrap();

    let mut doc = Document::new(opened.recovered.unwrap().project);
    session.accept_recovery();
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("3")).unwrap();
    session.record(&c);
    assert!(session.autosave_if_due(doc.project()).unwrap());
    crash(session);

    let opened = Session::open(&path).unwrap();
    let recovered = opened.recovered.expect("work is still recoverable");
    assert_eq!(recovered.project.pages().count(), 3);
    assert_same(&recovered.project, doc.project());
}

#[test]
fn no_recovery_is_offered_after_a_clean_save() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project()).unwrap();
    session.set_autosave_interval(Duration::ZERO);
    let (_, c) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();
    session.save(doc.project()).unwrap();
    assert!(!session.autosave_due());
    assert!(!session.autosave_if_due(doc.project()).unwrap());
    crash(session);

    let opened = Session::open(&path).unwrap();
    assert!(opened.recovered.is_none());
    assert_same(&opened.project, doc.project());
}

#[test]
fn an_unreadable_recovery_file_is_set_aside_not_deleted() {
    let (_dir, path) = temp();
    Session::create(&path, &Project::new("P"))
        .unwrap()
        .close()
        .unwrap();
    let garbage = b"not a recovery file, but maybe someone's work ....................................................";
    std::fs::write(autosave_path(&path), garbage).unwrap();

    let opened = Session::open(&path).unwrap();
    assert!(opened.recovered.is_none());
    let aside = opened.set_aside.expect("reported");
    assert!(matches!(aside.reason, StorageError::NotAProject(_)));
    assert_eq!(std::fs::read(&aside.path).unwrap(), garbage);
    assert!(!autosave_path(&path).exists());
}

#[test]
fn read_does_not_write_and_works_on_read_only_files() {
    let (_dir, path) = temp();
    let mut doc = Document::default();
    build_starter(&mut doc);
    ProjectFile::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms.clone()).unwrap();

    assert_same(&ProjectFile::read(&path).unwrap(), doc.project());
    assert_eq!(std::fs::read(&path).unwrap(), before);

    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&path, perms).unwrap();
}
