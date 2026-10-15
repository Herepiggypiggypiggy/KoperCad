use std::collections::BTreeSet;

use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId, SymbolRole, Tx,
};
use novo_sim::{Clock, PoleState, TimerKind, settle_timed, timer_progress};

fn put(tx: &mut Tx<'_>, page: novo_core::PageId, key: &str, y: f64, role: SymbolRole) -> SymbolId {
    let id = tx
        .place_symbol(
            page,
            key,
            Placement::at(Point::new(0.0, y)),
            vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
            role,
        )
        .unwrap();
    if key.contains("contact") {
        tx.update::<novo_core::SymbolInstance>(id, |s| s.pass_through = vec![vec![0, 1]])
            .unwrap();
    }
    id
}

/// L1 -> start (hand-set) -> coil K1 (2 s) -> N, with one delayed contact.
fn circuit(contact: &str) -> (Document, SymbolId, SymbolId) {
    let mut doc = Document::default();
    let (ids, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let n = tx.add_potential("N", PotentialKind::Neutral)?;
            let k1 = tx.add_device(DeviceTag::product("K1"), "")?;
            tx.update::<novo_core::Device>(k1, |d| {
                d.properties.insert("delay".into(), "2 s".into());
            })?;
            let s2 = tx.add_device(DeviceTag::product("S2"), "")?;
            let dev = |d, main| SymbolRole::Device { device: d, main };
            let f1 = put(
                tx,
                page,
                "t/feed",
                0.0,
                SymbolRole::PotentialFeed { potential: l1 },
            );
            let fnn = put(
                tx,
                page,
                "t/feed",
                10.0,
                SymbolRole::PotentialFeed { potential: n },
            );
            let start = put(tx, page, "t/contact/no", 20.0, dev(s2, true));
            let coil = put(tx, page, "t/coil/timer", 30.0, dev(k1, true));
            put(tx, page, contact, 40.0, dev(k1, false));
            tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(start, 0))?;
            tx.connect(Endpoint::pin(start, 1), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(fnn, 1))?;
            Ok((start, coil))
        })
        .unwrap();
    (doc, ids.0, ids.1)
}

fn step(
    doc: &Document,
    start: SymbolId,
    pressed: bool,
    held: &BTreeSet<SymbolId>,
    clock: &mut Clock,
) -> BTreeSet<SymbolId> {
    let project = doc.project();
    let netlist = project.netlist();
    let mut hand = PoleState::all_open();
    if pressed {
        hand.set_all(project, start, true);
    }
    settle_timed(project, &netlist, &hand, &BTreeSet::new(), held, clock).energized
}

#[test]
fn an_on_delay_timer_reports_its_progress_while_the_coil_is_on() {
    let (doc, start, coil) = circuit("t/contact/no_delay_on");
    let mut clock = Clock::new();
    assert!(timer_progress(doc.project(), &clock).is_empty());
    let held = step(&doc, start, true, &BTreeSet::new(), &mut clock);
    clock.advance(0.5);
    let held = step(&doc, start, true, &held, &mut clock);
    let p = timer_progress(doc.project(), &clock)[&coil];
    assert_eq!(p.kind, TimerKind::OnDelay);
    assert!((p.elapsed - 0.25).abs() < 1e-9 && (p.remaining - 1.5).abs() < 1e-9);
    clock.advance(2.0);
    step(&doc, start, true, &held, &mut clock);
    assert!(timer_progress(doc.project(), &clock).is_empty(), "done");
}

#[test]
fn an_off_delay_timer_counts_after_the_coil_drops() {
    let (doc, start, coil) = circuit("t/contact/no_delay_off");
    let mut clock = Clock::new();
    let held = step(&doc, start, true, &BTreeSet::new(), &mut clock);
    assert!(timer_progress(doc.project(), &clock).is_empty(), "coil on");
    clock.advance(1.0);
    let held = step(&doc, start, false, &held, &mut clock);
    clock.advance(1.0);
    step(&doc, start, false, &held, &mut clock);
    let p = timer_progress(doc.project(), &clock)[&coil];
    assert_eq!(p.kind, TimerKind::OffDelay);
    assert!((p.elapsed - 0.5).abs() < 1e-9);
}

#[test]
fn a_plain_coil_is_no_timer() {
    let (doc, start, _) = circuit("t/contact/no");
    let mut clock = Clock::new();
    step(&doc, start, true, &BTreeSet::new(), &mut clock);
    assert!(timer_progress(doc.project(), &clock).is_empty());
}
