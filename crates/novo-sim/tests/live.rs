use novo_core::{
    Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId, SymbolRole, Tx, WireId,
};
use novo_sim::{Live, PoleState, solve};

fn symbol(tx: &mut Tx<'_>, page: novo_core::PageId, y: f64, role: SymbolRole) -> SymbolId {
    tx.place_symbol(
        page,
        "test/block",
        Placement::at(Point::new(0.0, y)),
        vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
        role,
    )
    .unwrap()
}

/// Feed L1 -> contact -> load net. Returns (contact, wire to the load).
fn circuit(doc: &mut Document) -> (SymbolId, WireId, novo_core::PotentialId) {
    let (out, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let feed = symbol(tx, page, 0.0, SymbolRole::PotentialFeed { potential: l1 });
            let contact = symbol(tx, page, 40.0, SymbolRole::Unassigned);
            tx.update::<novo_core::SymbolInstance>(contact, |s| s.pass_through = vec![vec![0, 1]])?;
            let load = symbol(tx, page, 80.0, SymbolRole::Unassigned);
            tx.connect(Endpoint::pin(feed, 1), Endpoint::pin(contact, 0))?;
            let to_load = tx.connect(Endpoint::pin(contact, 1), Endpoint::pin(load, 0))?;
            Ok((contact, to_load, l1))
        })
        .unwrap();
    out
}

#[test]
fn a_potential_follows_the_contact_state() {
    let mut doc = Document::default();
    let (contact, to_load, l1) = circuit(&mut doc);
    let netlist = doc.project().netlist();
    let wire = doc.project().wire(to_load).unwrap();
    let net = netlist.net_of_wire(wire).unwrap();

    let open = solve(doc.project(), &netlist, &PoleState::all_open());
    assert_eq!(open.net(net), &Live::Dead);

    let mut state = PoleState::all_open();
    state.set_all(doc.project(), contact, true);
    let closed = solve(doc.project(), &netlist, &state);
    assert_eq!(closed.net(net).potential(), Some(l1));
    // The declared view is untouched: the netlist already says L1 can reach it.
    assert_eq!(netlist.net(net).carried, vec![l1]);

    state.set(contact, 0, false);
    assert_eq!(solve(doc.project(), &netlist, &state).net(net), &Live::Dead);
}

#[test]
fn a_net_with_its_own_potential_keeps_it() {
    let mut doc = Document::default();
    let (contact, to_load, l1) = circuit(&mut doc);
    let (l2, _) = doc
        .transact("Second feed", |tx| {
            let l2 = tx.add_potential("L2", PotentialKind::Line)?;
            let page = tx.project().pages().next().unwrap().id;
            let feed = symbol(tx, page, 120.0, SymbolRole::PotentialFeed { potential: l2 });
            let load = tx
                .project()
                .wire(to_load)
                .map(|w| w.to)
                .expect("wire has an end");
            tx.connect(Endpoint::pin(feed, 1), load)?;
            Ok(l2)
        })
        .unwrap();
    let netlist = doc.project().netlist();
    let net = netlist
        .net_of_wire(doc.project().wire(to_load).unwrap())
        .unwrap();
    // The load net now defines L2 itself, so L1 cannot enter it.
    let mut state = PoleState::all_open();
    state.set_all(doc.project(), contact, true);
    let live = solve(doc.project(), &netlist, &state);
    assert_eq!(live.net(net).potential(), Some(l2));
    assert_ne!(Some(l1), live.net(net).potential());
}

#[test]
fn two_potentials_meeting_through_closed_poles_are_shorted() {
    let mut doc = Document::default();
    let ((c1, c2, load_net, l1, l2), _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let l2 = tx.add_potential("L2", PotentialKind::Line)?;
            let f1 = symbol(tx, page, 0.0, SymbolRole::PotentialFeed { potential: l1 });
            let f2 = symbol(tx, page, 20.0, SymbolRole::PotentialFeed { potential: l2 });
            let c1 = symbol(tx, page, 40.0, SymbolRole::Unassigned);
            let c2 = symbol(tx, page, 60.0, SymbolRole::Unassigned);
            for c in [c1, c2] {
                tx.update::<novo_core::SymbolInstance>(c, |s| s.pass_through = vec![vec![0, 1]])?;
            }
            let load = symbol(tx, page, 80.0, SymbolRole::Unassigned);
            tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(c1, 0))?;
            tx.connect(Endpoint::pin(f2, 1), Endpoint::pin(c2, 0))?;
            tx.connect(Endpoint::pin(c1, 1), Endpoint::pin(load, 0))?;
            tx.connect(Endpoint::pin(c2, 1), Endpoint::pin(load, 0))?;
            Ok((c1, c2, Endpoint::pin(load, 0), l1, l2))
        })
        .unwrap();
    let netlist = doc.project().netlist();
    let mut state = PoleState::all_open();
    state.set_all(doc.project(), c1, true);
    let one = solve(doc.project(), &netlist, &state);
    assert_eq!(one.of(&netlist, load_net).potential(), Some(l1));
    state.set_all(doc.project(), c2, true);
    let both = solve(doc.project(), &netlist, &state);
    let Live::Shorted(mut meet) = both.of(&netlist, load_net).clone() else {
        panic!("expected a short")
    };
    meet.sort();
    let mut want = vec![l1, l2];
    want.sort();
    assert_eq!(meet, want);
}
