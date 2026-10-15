use std::collections::BTreeSet;

use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId, SymbolRole, Tx,
};
use novo_sim::{Live, PoleState, settle};

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
