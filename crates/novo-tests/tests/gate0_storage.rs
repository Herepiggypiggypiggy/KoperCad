//! Gate 0 / WS-04: the Gate 1 project saved to a `.novo` file, closed and
//! reopened is identical, down to its netlist and potentials. Edits, undo and
//! crash recovery survive too.

use std::collections::BTreeSet;

use novo_core::*;
use novo_storage::Session;
use novo_tests::{motor_starter, motor_starter_with_changes, stress_page};

/// A net's potentials: shown, defined, carried and reachable.
type Potentials = [Vec<PotentialId>; 4];

type Electrical = (
    BTreeSet<(Vec<PinRef>, Potentials)>,
    Vec<(PinRef, Option<PotentialId>)>,
);

/// The connectivity of a project in a form that does not depend on net
/// numbering: every net as its sorted set of pins, with all its potentials
/// (pass-through data included), plus each pin's potential.
fn electrical(project: &Project) -> Electrical {
    let net = project.netlist();
    let pins: Vec<PinRef> = project
        .symbols()
        .flat_map(|s| (0..s.pins.len()).map(move |i| PinRef::new(s.id, u16::try_from(i).unwrap())))
        .collect();
    let mut nets = BTreeSet::new();
    for id in pins.iter().filter_map(|&p| net.net_of(Endpoint::from(p))) {
        let mut members: Vec<PinRef> = pins
            .iter()
            .copied()
            .filter(|&p| net.net_of(Endpoint::from(p)) == Some(id))
            .collect();
        members.sort();
        let n = net.net(id);
        let mut potentials = [
            n.potentials.clone(),
            n.defined.clone(),
            n.carried.clone(),
            n.reachable.clone(),
        ];
        potentials.iter_mut().for_each(|p| p.sort());
        nets.insert((members, potentials));
    }
    let per_pin = pins
        .iter()
        .map(|&p| (p, net.potential_of(Endpoint::from(p))))
        .collect();
    (nets, per_pin)
}

#[test]
fn gate1_project_saves_and_reopens_identically() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("starter.novo");
    let mut doc = Document::default();
    let s = motor_starter(&mut doc).unwrap();
    Session::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();

    let opened = Session::open(&path).unwrap();
    assert!(opened.recovered.is_none());
    assert_eq!(opened.project.to_json(), doc.project().to_json());
    // Connectivity and potentials are derived, not stored: they must come
    // back the same from the reopened records.
    let (nets, pins) = electrical(doc.project());
    assert_eq!(
        (nets.len(), pins.len()),
        (16, 48),
        "10 main-circuit and 6 control nets, every pin"
    );
    assert_eq!(electrical(&opened.project), (nets, pins));
    let net = opened.project.netlist();
    assert_eq!(net.potential_of(Endpoint::pin(s.m1_sym, 0)), Some(s.l1));
    assert_eq!(net.potential_of(Endpoint::pin(s.k1_coil, 0)), Some(s.l));
    assert_eq!(net.potential_conflicts().count(), 0);
}

#[test]
fn incremental_saves_after_edits_and_undo_reopen_identically() {
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
    let (_, c) = doc.transact("Delete -K1", |tx| tx.remove(s.k1)).unwrap();
    session.record(&c);
    session.save(doc.project()).unwrap();
    let c = doc.undo().unwrap().unwrap();
    session.record(&c);
    let (_, c) = doc
        .transact("Rename", |tx| {
            tx.set_tag(s.m1, DeviceTag::parse("=A1+FLD-M2").unwrap())
        })
        .unwrap();
    session.record(&c);
    session.save(doc.project()).unwrap();
    session.close().unwrap();

    let opened = Session::open(&path).unwrap();
    assert_eq!(opened.project.to_json(), doc.project().to_json());
    assert_eq!(electrical(&opened.project), electrical(doc.project()));
}

#[test]
fn crash_recovery_offers_the_unsaved_starter() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("starter.novo");
    let mut doc = Document::default();
    Session::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();
    let mut session = Session::open(&path).unwrap().session;
    motor_starter(&mut doc).unwrap();
    let mut all = ChangeSet {
        info: true,
        ..ChangeSet::default()
    };
    all.added.extend(doc.project().records().map(|r| r.id()));
    session.record(&all);
    session.autosave(doc.project()).unwrap();
    drop(session); // the app dies without closing

    let opened = Session::open(&path).unwrap();
    assert_eq!(
        opened.project.records().count(),
        0,
        "the last save was empty"
    );
    let recovered = opened.recovered.expect("unsaved work is offered back");
    assert_eq!(recovered.project.to_json(), doc.project().to_json());
    assert_eq!(electrical(&recovered.project), electrical(doc.project()));
}

/// The same, fed the way the app does it: each transaction's own
/// `ChangeSet`, with autosaves in between.
#[test]
fn crash_recovery_from_the_changesets_the_app_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("starter.novo");
    let mut doc = Document::default();
    Session::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();
    let mut session = Session::open(&path).unwrap().session;
    let (s, c) = motor_starter_with_changes(&mut doc).unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();
    let (_, c) = doc
        .transact("Move coil", |tx| {
            tx.move_symbol(s.k1_coil, Placement::at(Point::new(80.0, 168.0)))
        })
        .unwrap();
    session.record(&c);
    let (_, c) = doc.transact("Delete -S1", |tx| tx.remove(s.s1)).unwrap();
    session.record(&c);
    session.autosave(doc.project()).unwrap();
    drop(session);

    let opened = Session::open(&path).unwrap();
    assert_eq!(opened.project.records().count(), 0);
    let recovered = opened.recovered.expect("unsaved work is offered back");
    assert_eq!(recovered.project.to_json(), doc.project().to_json());
    assert_eq!(electrical(&recovered.project), electrical(doc.project()));
}

#[test]
fn stress_page_saves_and_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stress.novo");
    let mut doc = Document::default();
    stress_page(&mut doc, 10_000).unwrap();
    let t = std::time::Instant::now();
    Session::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();
    eprintln!("save 10k: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let opened = Session::open(&path).unwrap();
    eprintln!("open 10k: {:?}", t.elapsed());
    assert_eq!(opened.project.to_json(), doc.project().to_json());
}
