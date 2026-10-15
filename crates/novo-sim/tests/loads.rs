use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId, SymbolRole, Tx,
};
use novo_sim::{PoleState, is_load, loads_on, solve};

fn put(tx: &mut Tx<'_>, page: novo_core::PageId, key: &str, y: f64, role: SymbolRole) -> SymbolId {
    tx.place_symbol(
        page,
        key,
        Placement::at(Point::new(0.0, y)),
        vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
        role,
    )
    .unwrap()
}

/// L1 and N feed a lamp H1; a second lamp H2 only sees L1 on both pins.
fn lamps(wire_neutral: bool) -> (Document, SymbolId, SymbolId) {
    let mut doc = Document::default();
    let (ids, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let n = tx.add_potential("N", PotentialKind::Neutral)?;
            let h1 = tx.add_device(DeviceTag::product("H1"), "")?;
            let h2 = tx.add_device(DeviceTag::product("H2"), "")?;
            let fl = put(
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
            let lamp1 = put(
                tx,
                page,
                "iec/signalling/lamp",
                20.0,
                SymbolRole::Device {
                    device: h1,
                    main: true,
                },
            );
            let lamp2 = put(
                tx,
                page,
                "iec/signalling/lamp",
                40.0,
                SymbolRole::Device {
                    device: h2,
                    main: true,
                },
            );
            tx.connect(Endpoint::pin(fl, 0), Endpoint::pin(lamp1, 0))?;
            tx.connect(Endpoint::pin(fl, 0), Endpoint::pin(lamp2, 0))?;
            tx.connect(Endpoint::pin(fl, 0), Endpoint::pin(lamp2, 1))?;
            if wire_neutral {
                tx.connect(Endpoint::pin(fnn, 0), Endpoint::pin(lamp1, 1))?;
            }
            Ok((lamp1, lamp2))
        })
        .unwrap();
    (doc, ids.0, ids.1)
}

#[test]
fn a_lamp_between_two_potentials_is_on() {
    let (doc, on, same) = lamps(true);
    let netlist = doc.project().netlist();
    let solution = solve(doc.project(), &netlist, &PoleState::all_open());
    let lit = loads_on(doc.project(), &netlist, &solution);
    assert!(lit.contains(&on));
    assert!(!lit.contains(&same), "the same potential on both pins");
    assert!(is_load(doc.project().symbol(on).unwrap()));
}

#[test]
fn a_lamp_with_one_pin_unconnected_is_off() {
    let (doc, on, _) = lamps(false);
    let netlist = doc.project().netlist();
    let solution = solve(doc.project(), &netlist, &PoleState::all_open());
    assert!(loads_on(doc.project(), &netlist, &solution).is_empty());
    assert!(is_load(doc.project().symbol(on).unwrap()));
}
