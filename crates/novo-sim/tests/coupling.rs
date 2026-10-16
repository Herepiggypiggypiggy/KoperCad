use std::collections::BTreeSet;

use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId, SymbolRole, Tx,
};
use novo_sim::{Clock, Live, PoleState, settle, settle_timed};

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

struct Circuit {
    doc: Document,
    start: SymbolId,
    seal: SymbolId,
    nc: SymbolId,
    coil: SymbolId,
    lamp_net: Endpoint,
}

/// L1 -> start (hand-set NO) ->┬-> coil K1 -> N feed
///    L1 -> K1 NO (seal-in)  ->┘      K1 NC -> lamp (from L1)
fn circuit() -> Circuit {
    let mut doc = Document::default();
    let (c, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let n = tx.add_potential("N", PotentialKind::Neutral)?;
            let k1 = tx.add_device(DeviceTag::product("K1"), "")?;
            let s2 = tx.add_device(DeviceTag::product("S2"), "")?;
            let dev = |d, main| SymbolRole::Device { device: d, main };
            let f1 = put(
                tx,
                page,
                "t/feed",
                0.0,
                SymbolRole::PotentialFeed { potential: l1 },
            );
            let fn_ = put(
                tx,
                page,
                "t/feed",
                10.0,
                SymbolRole::PotentialFeed { potential: n },
            );
            let start = put(tx, page, "t/contact/no", 20.0, dev(s2, true));
            let seal = put(tx, page, "t/contact/no", 30.0, dev(k1, false));
            let nc = put(tx, page, "t/contact/nc", 40.0, dev(k1, false));
            let coil = put(tx, page, "t/coil", 50.0, dev(k1, true));
            let lamp = put(tx, page, "t/lamp", 60.0, SymbolRole::Unassigned);
            for c in [start, seal, nc] {
                tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(c, 0))?;
            }
            tx.connect(Endpoint::pin(start, 1), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(seal, 1), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(fn_, 1))?;
            tx.connect(Endpoint::pin(nc, 1), Endpoint::pin(lamp, 0))?;
            Ok(Circuit {
                doc: Document::default(),
                start,
                seal,
                nc,
                coil,
                lamp_net: Endpoint::pin(lamp, 0),
            })
        })
        .unwrap();
    Circuit { doc, ..c }
}

#[test]
fn a_coil_closes_its_contacts_and_seals_itself_in() {
    let c = circuit();
    let project = c.doc.project();
    let netlist = project.netlist();
    let none = BTreeSet::new();

    // At rest: nothing feeds the coil, the NC contact lights the lamp.
    let rest = settle(project, &netlist, &PoleState::all_open(), &none, &none);
    assert!(rest.energized.is_empty());
    assert!(rest.solution.of(&netlist, c.lamp_net).potential().is_some());

    // Press start: the coil is fed, the NO closes, the NC opens.
    let mut hand = PoleState::all_open();
    hand.set_all(project, c.start, true);
    let pressed = settle(project, &netlist, &hand, &none, &none);
    assert!(pressed.energized.contains(&c.coil));
    assert!(pressed.poles.is_closed(c.seal, 0));
    assert!(!pressed.poles.is_closed(c.nc, 0));
    assert_eq!(pressed.solution.of(&netlist, c.lamp_net), &Live::Dead);

    // Release start: the seal-in contact keeps the coil on only when the
    // previous state is held.
    let released = settle(
        project,
        &netlist,
        &PoleState::all_open(),
        &none,
        &pressed.energized,
    );
    assert!(released.energized.contains(&c.coil));
    let cold = settle(project, &netlist, &PoleState::all_open(), &none, &none);
    assert!(cold.energized.is_empty());
}

#[test]
fn a_forced_coil_drives_its_contacts_without_supply() {
    let c = circuit();
    let project = c.doc.project();
    let netlist = project.netlist();
    let none = BTreeSet::new();
    let forced = BTreeSet::from([c.coil]);
    let s = settle(project, &netlist, &PoleState::all_open(), &forced, &none);
    assert!(s.energized.contains(&c.coil));
    assert!(s.poles.is_closed(c.seal, 0));
    assert!(s.driven.contains(&c.seal) && s.driven.contains(&c.nc));
    assert!(!s.driven.contains(&c.start), "a push button has no coil");
}

#[test]
fn a_changeover_contact_switches_between_its_two_sides() {
    let mut doc = Document::default();
    let ((coil, a, b), _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let k1 = tx.add_device(DeviceTag::product("K1"), "")?;
            let dev = |main| SymbolRole::Device { device: k1, main };
            let feed = put(
                tx,
                page,
                "t/feed",
                0.0,
                SymbolRole::PotentialFeed { potential: l1 },
            );
            let coil = put(tx, page, "t/coil", 10.0, dev(true));
            let co = tx
                .place_symbol(
                    page,
                    "t/contact/changeover",
                    Placement::at(Point::new(0.0, 20.0)),
                    vec![
                        Pin::new("11", 0.0, 0.0),
                        Pin::new("12", 4.0, 8.0),
                        Pin::new("14", -4.0, 8.0),
                    ],
                    dev(false),
                )
                .unwrap();
            // Break side first, make side second, as the library lists them.
            tx.update::<novo_core::SymbolInstance>(co, |s| {
                s.pass_through = vec![vec![0, 1], vec![0, 2]]
            })?;
            let lamp_a = put(tx, page, "t/lamp", 30.0, SymbolRole::Unassigned);
            let lamp_b = put(tx, page, "t/lamp", 40.0, SymbolRole::Unassigned);
            tx.connect(Endpoint::pin(feed, 1), Endpoint::pin(co, 0))?;
            tx.connect(Endpoint::pin(co, 1), Endpoint::pin(lamp_a, 0))?;
            tx.connect(Endpoint::pin(co, 2), Endpoint::pin(lamp_b, 0))?;
            Ok((coil, Endpoint::pin(lamp_a, 0), Endpoint::pin(lamp_b, 0)))
        })
        .unwrap();
    let project = doc.project();
    let netlist = project.netlist();
    let none = BTreeSet::new();
    let rest = settle(project, &netlist, &PoleState::all_open(), &none, &none);
    assert!(rest.solution.of(&netlist, a).potential().is_some());
    assert_eq!(rest.solution.of(&netlist, b), &Live::Dead);
    let on = settle(
        project,
        &netlist,
        &PoleState::all_open(),
        &BTreeSet::from([coil]),
        &none,
    );
    assert_eq!(on.solution.of(&netlist, a), &Live::Dead);
    assert!(on.solution.of(&netlist, b).potential().is_some());
}

struct Timer {
    doc: Document,
    start: SymbolId,
    lamp: Endpoint,
}

/// L1 -> start (hand-set) -> coil K1 -> N; L1 -> `contact` of K1 -> lamp.
/// K1 has a 2 second delay.
fn timer(contact: &str) -> Timer {
    let mut doc = Document::default();
    let (t, _) = doc
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
            let delayed = put(tx, page, contact, 40.0, dev(k1, false));
            let lamp = put(tx, page, "t/lamp", 50.0, SymbolRole::Unassigned);
            tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(start, 0))?;
            tx.connect(Endpoint::pin(start, 1), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(fnn, 1))?;
            tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(delayed, 0))?;
            tx.connect(Endpoint::pin(delayed, 1), Endpoint::pin(lamp, 0))?;
            Ok(Timer {
                doc: Document::default(),
                start,
                lamp: Endpoint::pin(lamp, 0),
            })
        })
        .unwrap();
    Timer { doc, ..t }
}

fn lit(
    t: &Timer,
    pressed: bool,
    held: &BTreeSet<SymbolId>,
    clock: &mut Clock,
) -> (bool, bool, BTreeSet<SymbolId>) {
    let project = t.doc.project();
    let netlist = project.netlist();
    let mut hand = PoleState::all_open();
    if pressed {
        hand.set_all(project, t.start, true);
    }
    let s = settle_timed(project, &netlist, &hand, &BTreeSet::new(), held, clock);
    (
        s.solution.of(&netlist, t.lamp).potential().is_some(),
        s.timing,
        s.energized,
    )
}

#[test]
fn an_on_delay_contact_operates_after_its_time_and_releases_at_once() {
    let t = timer("t/contact/no_delay_on");
    let mut clock = Clock::new();
    let (lamp, timing, held) = lit(&t, true, &BTreeSet::new(), &mut clock);
    assert!(!lamp && timing, "the coil is on, the timer is counting");
    clock.advance(1.0);
    assert!(!lit(&t, true, &held, &mut clock).0);
    clock.advance(1.5);
    let (lamp, timing, held) = lit(&t, true, &held, &mut clock);
    assert!(lamp && !timing, "2 s have passed");
    let (lamp, _, _) = lit(&t, false, &held, &mut clock);
    assert!(!lamp, "dropping the coil releases at once");
}

#[test]
fn an_off_delay_contact_operates_at_once_and_releases_after_its_time() {
    let t = timer("t/contact/no_delay_off");
    let mut clock = Clock::new();
    let (lamp, _, held) = lit(&t, true, &BTreeSet::new(), &mut clock);
    assert!(lamp, "operates with the coil");
    let (lamp, timing, held) = lit(&t, false, &held, &mut clock);
    assert!(lamp && timing, "held while the time runs");
    clock.advance(1.0);
    let (lamp, _, held) = lit(&t, false, &held, &mut clock);
    assert!(lamp);
    clock.advance(1.5);
    let (lamp, timing, _) = lit(&t, false, &held, &mut clock);
    assert!(!lamp && !timing, "released after 2 s");
}

#[test]
fn stop_releases_the_seal_in_and_a_stale_hand_state_does_not_latch_it() {
    let mut doc = Document::default();
    let (c, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let n = tx.add_potential("N", PotentialKind::Neutral)?;
            let k1 = tx.add_device(DeviceTag::product("K1"), "")?;
            let s1d = tx.add_device(DeviceTag::product("S1"), "")?;
            let s2d = tx.add_device(DeviceTag::product("S2"), "")?;
            let dev = |d, main| SymbolRole::Device { device: d, main };
            let f1 = put(
                tx,
                page,
                "t/feed",
                0.0,
                SymbolRole::PotentialFeed { potential: l1 },
            );
            let fn_ = put(
                tx,
                page,
                "t/feed",
                10.0,
                SymbolRole::PotentialFeed { potential: n },
            );
            let stop = put(tx, page, "t/contact/nc", 20.0, dev(s1d, true));
            let start = put(tx, page, "t/contact/no", 30.0, dev(s2d, true));
            let seal = put(tx, page, "t/contact/no", 40.0, dev(k1, false));
            let lampc = put(tx, page, "t/contact/no", 45.0, dev(k1, false));
            let coil = put(tx, page, "t/coil", 50.0, dev(k1, true));
            let lamp = put(tx, page, "t/lamp", 60.0, SymbolRole::Unassigned);
            tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(stop, 0))?;
            tx.connect(Endpoint::pin(stop, 1), Endpoint::pin(start, 0))?;
            tx.connect(Endpoint::pin(stop, 1), Endpoint::pin(seal, 0))?;
            tx.connect(Endpoint::pin(start, 1), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(seal, 1), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(fn_, 1))?;
            tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(lampc, 0))?;
            tx.connect(Endpoint::pin(lampc, 1), Endpoint::pin(lamp, 0))?;
            Ok((stop, start, seal, coil, lampc))
        })
        .unwrap();
    let (stop, start, seal, coil, lampc) = c;
    let project = doc.project();
    let netlist = project.netlist();
    let none = BTreeSet::new();
    let mut hand = PoleState::all_open();
    hand.set_all(project, stop, true);
    hand.set_all(project, start, true);
    let on = settle(project, &netlist, &hand, &none, &none);
    assert!(on.energized.contains(&coil));
    hand.set_all(project, start, false);
    let held = settle(project, &netlist, &hand, &none, &on.energized);
    assert!(held.energized.contains(&coil));
    assert!(held.poles.is_closed(seal, 0) && held.poles.is_closed(lampc, 0));
    hand.set_all(project, stop, false);
    let off = settle(project, &netlist, &hand, &none, &held.energized);
    assert!(off.energized.is_empty(), "coil dropped");
    assert!(!off.poles.is_closed(seal, 0), "seal opens");
    assert!(!off.poles.is_closed(lampc, 0), "lamp contact opens");

    // A hand-set closed state on a contact the coil drives (set before it
    // was assigned, say) must not latch it: it follows the coil alone.
    let mut stale = PoleState::all_open();
    stale.set_all(project, seal, true);
    stale.set_all(project, lampc, true);
    let rest = settle(project, &netlist, &stale, &none, &none);
    assert!(rest.energized.is_empty());
    assert!(!rest.poles.is_closed(seal, 0), "stale hold contact opens");
    assert!(!rest.poles.is_closed(lampc, 0), "stale lamp contact opens");
}
