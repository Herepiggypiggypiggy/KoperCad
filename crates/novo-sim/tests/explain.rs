//! The live view's "why": routes, breaks and causes on known circuits.

use std::collections::BTreeSet;

use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId, SymbolRole, Tx,
};
use novo_sim::explain::{HandKind, OpenWhy, Verdict, explain_coil, explain_symbol};
use novo_sim::{PoleState, Settled, rest_state, settle};

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
    if !key.contains("coil") && !key.contains("feed") {
        tx.update::<novo_core::SymbolInstance>(id, |s| s.pass_through = vec![vec![0, 1]])
            .unwrap();
    }
    id
}

const NO: &str = "iec/operators/pushbutton_no";
const NC: &str = "iec/operators/pushbutton_nc";
const KNO: &str = "iec/contacts/contact_no";
const KNC: &str = "iec/contacts/contact_nc";
const COIL: &str = "iec/relays/coil";

fn dev(device: novo_core::DeviceId, main: bool) -> SymbolRole {
    SymbolRole::Device { device, main }
}

/// Start/stop with a seal-in: L1 - S0 (stop, NC) - node A; A - S1 (start, NO)
/// - B; A - K1 seal (NO) - B; B - coil K1 - N.
struct Latch {
    doc: Document,
    s0: SymbolId,
    s1: SymbolId,
    seal: SymbolId,
    coil: SymbolId,
}

fn latch() -> Latch {
    let mut doc = Document::default();
    let (c, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let n = tx.add_potential("N", PotentialKind::Neutral)?;
            let k1 = tx.add_device(DeviceTag::product("K1"), "")?;
            let s0d = tx.add_device(DeviceTag::product("S0"), "")?;
            let s1d = tx.add_device(DeviceTag::product("S1"), "")?;
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
            let s0 = put(tx, page, NC, 20.0, dev(s0d, true));
            let s1 = put(tx, page, NO, 30.0, dev(s1d, true));
            let seal = put(tx, page, KNO, 40.0, dev(k1, false));
            let coil = put(tx, page, COIL, 50.0, dev(k1, true));
            tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(s0, 0))?;
            tx.connect(Endpoint::pin(s0, 1), Endpoint::pin(s1, 0))?;
            tx.connect(Endpoint::pin(s0, 1), Endpoint::pin(seal, 0))?;
            tx.connect(Endpoint::pin(s1, 1), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(seal, 1), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(fnn, 1))?;
            Ok(Latch {
                doc: Document::default(),
                s0,
                s1,
                seal,
                coil,
            })
        })
        .unwrap();
    Latch { doc, ..c }
}

fn names(_: SymbolId) -> String {
    "X".to_owned()
}

fn settled(l: &Latch, hand: &PoleState, held: &BTreeSet<SymbolId>) -> Settled {
    let project = l.doc.project();
    settle(project, &project.netlist(), hand, &BTreeSet::new(), held)
}

#[test]
fn a_dead_coil_names_the_open_start_path() {
    let l = latch();
    let project = l.doc.project();
    let netlist = project.netlist();
    let rest = settled(&l, &rest_state(project), &BTreeSet::new());
    let why = explain_coil(project, &netlist, &rest, l.coil);
    assert!(!why.on);
    let live_pin = &why.pins[0].explanation;
    let Verdict::Cut(route) = &live_pin.verdict else {
        panic!("expected a cut, got {:?}", live_pin.verdict);
    };
    // One open contact stops it: the start button or the seal-in contact.
    let breaks: Vec<_> = route.breaks().collect();
    assert_eq!(breaks.len(), 1);
    let b = breaks[0];
    assert!(b.pole.symbol == l.s1 || b.pole.symbol == l.seal);
    match b.why {
        Some(OpenWhy::Hand(HandKind::Pushbutton)) => assert_eq!(b.pole.symbol, l.s1),
        Some(OpenWhy::Coil {
            coil,
            on: false,
            delayed: false,
        }) => {
            assert_eq!(b.pole.symbol, l.seal);
            assert_eq!(coil, l.coil);
        }
        other => panic!("unexpected cause {other:?}"),
    }
    // The stop button is closed, so it is on the route but not a break.
    assert!(
        route
            .steps
            .iter()
            .any(|s| s.pole.symbol == l.s0 && s.closed)
    );
    let text = live_pin
        .lines(project, &|_| "X".to_owned())
        .iter()
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("dead"), "{text}");
}

#[test]
fn start_pulls_the_coil_in_and_seal_in_holds_it_with_the_route_through_the_seal() {
    let l = latch();
    let project = l.doc.project();
    let netlist = project.netlist();
    let mut hand = rest_state(project);
    hand.set_all(project, l.s1, true);
    let pressed = settled(&l, &hand, &BTreeSet::new());
    assert!(pressed.energized.contains(&l.coil));
    let why = explain_coil(project, &netlist, &pressed, l.coil);
    assert!(why.on);
    assert!(
        why.pins
            .iter()
            .all(|p| matches!(p.explanation.verdict, Verdict::Carries(_)))
    );

    // Let go of start: only the seal-in contact is left to feed the coil.
    let released = settled(&l, &rest_state(project), &pressed.energized);
    assert!(released.energized.contains(&l.coil));
    let why = explain_coil(project, &netlist, &released, l.coil);
    let Verdict::Carries(route) = &why.pins[0].explanation.verdict else {
        panic!("coil pin should carry");
    };
    let hops: Vec<SymbolId> = route.steps.iter().map(|s| s.pole.symbol).collect();
    assert_eq!(hops, vec![l.s0, l.seal]);
    assert!(route.steps.iter().all(|s| s.closed));
}

#[test]
fn stop_breaks_the_latch_and_the_break_is_the_stop_button() {
    let l = latch();
    let project = l.doc.project();
    let netlist = project.netlist();
    let mut hand = rest_state(project);
    hand.set_all(project, l.s1, true);
    let on = settled(&l, &hand, &BTreeSet::new());
    // Press stop (opens its NC contact) while the coil is held in.
    let mut stop = rest_state(project);
    stop.set_all(project, l.s0, false);
    let off = settled(&l, &stop, &on.energized);
    assert!(off.energized.is_empty());
    let why = explain_coil(project, &netlist, &off, l.coil);
    let Verdict::Cut(route) = &why.pins[0].explanation.verdict else {
        panic!("coil pin should be cut");
    };
    let b: Vec<_> = route.breaks().collect();
    assert_eq!(b[0].pole.symbol, l.s0);
    assert_eq!(b[0].why, Some(OpenWhy::Hand(HandKind::Pushbutton)));
}

/// Reversing starter control: forward K1 and reverse K2 coils, each behind the
/// other's NC contact.
struct Reversing {
    doc: Document,
    sf: SymbolId,
    sr: SymbolId,
    k1_coil: SymbolId,
    k2_coil: SymbolId,
    k1_nc: SymbolId,
}

fn reversing() -> Reversing {
    let mut doc = Document::default();
    let (c, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let n = tx.add_potential("N", PotentialKind::Neutral)?;
            let k1 = tx.add_device(DeviceTag::product("K1"), "")?;
            let k2 = tx.add_device(DeviceTag::product("K2"), "")?;
            let sfd = tx.add_device(DeviceTag::product("SF"), "")?;
            let srd = tx.add_device(DeviceTag::product("SR"), "")?;
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
            let sf = put(tx, page, NO, 20.0, dev(sfd, true));
            let sr = put(tx, page, NO, 30.0, dev(srd, true));
            let k2_nc = put(tx, page, KNC, 40.0, dev(k2, false));
            let k1_nc = put(tx, page, KNC, 50.0, dev(k1, false));
            let k1_coil = put(tx, page, COIL, 60.0, dev(k1, true));
            let k2_coil = put(tx, page, COIL, 70.0, dev(k2, true));
            // L1 - SF - K2/NC - K1 coil; L1 - SR - K1/NC - K2 coil.
            tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(sf, 0))?;
            tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(sr, 0))?;
            tx.connect(Endpoint::pin(sf, 1), Endpoint::pin(k2_nc, 0))?;
            tx.connect(Endpoint::pin(k2_nc, 1), Endpoint::pin(k1_coil, 0))?;
            tx.connect(Endpoint::pin(sr, 1), Endpoint::pin(k1_nc, 0))?;
            tx.connect(Endpoint::pin(k1_nc, 1), Endpoint::pin(k2_coil, 0))?;
            tx.connect(Endpoint::pin(k1_coil, 1), Endpoint::pin(fnn, 1))?;
            tx.connect(Endpoint::pin(k2_coil, 1), Endpoint::pin(fnn, 1))?;
            Ok(Reversing {
                doc: Document::default(),
                sf,
                sr,
                k1_coil,
                k2_coil,
                k1_nc,
            })
        })
        .unwrap();
    Reversing { doc, ..c }
}

#[test]
fn the_interlock_is_named_as_the_reason_the_second_coil_stays_off() {
    let r = reversing();
    let project = r.doc.project();
    let netlist = project.netlist();
    let none = BTreeSet::new();
    let mut hand = rest_state(project);
    hand.set_all(project, r.sf, true);
    let forward = settle(project, &netlist, &hand, &none, &none);
    assert!(forward.energized.contains(&r.k1_coil));

    // Now reverse is pressed too while forward runs: K1's NC contact is open.
    hand.set_all(project, r.sr, true);
    let both = settle(project, &netlist, &hand, &none, &forward.energized);
    assert!(both.energized.contains(&r.k1_coil));
    assert!(!both.energized.contains(&r.k2_coil));

    let why = explain_coil(project, &netlist, &both, r.k2_coil);
    assert!(!why.on);
    let Verdict::Cut(route) = &why.pins[0].explanation.verdict else {
        panic!("reverse coil should be cut");
    };
    let breaks: Vec<_> = route.breaks().collect();
    assert_eq!(breaks.len(), 1);
    assert_eq!(breaks[0].pole.symbol, r.k1_nc);
    assert_eq!(
        breaks[0].why,
        Some(OpenWhy::Coil {
            coil: r.k1_coil,
            on: true,
            delayed: false
        })
    );
    let lines = why.pins[0].explanation.lines(project, &names);
    assert!(
        lines
            .iter()
            .any(|l| l.text.contains("opens when pulled in")),
        "{lines:?}"
    );
    // Each line about a device can be followed: the break points at K1's contact.
    assert!(lines.iter().any(|l| l.symbol == Some(r.k1_nc)));
}

#[test]
fn a_feed_pin_carries_its_own_potential_with_no_steps() {
    let l = latch();
    let project = l.doc.project();
    let netlist = project.netlist();
    let rest = settled(&l, &rest_state(project), &BTreeSet::new());
    // The N feed side of the coil carries N straight from the feed.
    let pins = explain_symbol(project, &netlist, &rest, l.coil);
    assert_eq!(pins.len(), 2);
    let Verdict::Carries(route) = &pins[1].explanation.verdict else {
        panic!("pin 2 carries N");
    };
    assert!(route.steps.is_empty());
}

#[test]
fn a_contact_answer_leads_to_its_coil_one_hop_at_a_time() {
    use novo_sim::explain::answer_symbol;
    let r = reversing();
    let project = r.doc.project();
    let netlist = project.netlist();
    let none = BTreeSet::new();
    let mut hand = rest_state(project);
    hand.set_all(project, r.sf, true);
    let forward = settle(project, &netlist, &hand, &none, &none);
    let name = |s: SymbolId| {
        if s == r.k1_coil {
            "K1".to_owned()
        } else {
            "X".to_owned()
        }
    };
    let a = answer_symbol(project, &netlist, &forward, &none, r.k1_nc, &name);
    assert!(a.title.contains("open"));
    assert_eq!(a.lines[0].symbol, Some(r.k1_coil));
    assert!(a.lines[0].text.contains("coil K1 is on"), "{:?}", a.lines);

    // Following the hop: the coil is on and says what feeds it.
    let c = answer_symbol(project, &netlist, &forward, &none, r.k1_coil, &name);
    assert_eq!(c.title, "K1 is on");
    assert!(!c.dead);
}

#[test]
fn a_dead_coil_answer_marks_the_break_and_the_wires_to_light() {
    use novo_sim::explain::answer_symbol;
    let l = latch();
    let project = l.doc.project();
    let netlist = project.netlist();
    let rest = settled(&l, &rest_state(project), &BTreeSet::new());
    let a = answer_symbol(project, &netlist, &rest, &BTreeSet::new(), l.coil, &names);
    assert_eq!(a.title, "X is off");
    assert!(a.dead);
    assert!(a.breaks.contains(&l.s1) || a.breaks.contains(&l.seal));
    assert!(!a.nets.is_empty());
}
