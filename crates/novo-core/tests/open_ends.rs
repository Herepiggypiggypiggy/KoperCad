//! Open wire ends: an endpoint that is a free position, not a pin
//! (DESIGN §4 "Open ends").

use novo_core::*;

fn two_pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)]
}

fn symbol_at(tx: &mut Tx<'_>, page: PageId, x: f64) -> SymbolId {
    tx.place_symbol(
        page,
        "iec/general/box",
        Placement::at(Point::new(x, 0.0)),
        two_pins(),
        SymbolRole::Unassigned,
    )
    .unwrap()
}

fn setup() -> (Document, PageId, SymbolId, SymbolId) {
    let mut doc = Document::default();
    let ((page, a, b), _) = doc
        .transact("Setup", |tx| {
            let page = tx.add_page("1")?;
            let a = symbol_at(tx, page, 0.0);
            let b = symbol_at(tx, page, 50.0);
            Ok((page, a, b))
        })
        .unwrap();
    (doc, page, a, b)
}

fn wire_between(doc: &Document, from: Endpoint, to: Endpoint) -> WireId {
    doc.project()
        .wires()
        .find(|w| w.from == from && w.to == to)
        .expect("wire")
        .id
}

#[test]
fn a_wire_can_end_on_a_free_position() {
    let (mut doc, page, a, _) = setup();
    let open = Endpoint::Free {
        at: Point::new(20.0, 30.0),
    };
    doc.execute(Command::Connect {
        from: Endpoint::pin(a, 1),
        to: open,
        route: vec![Point::new(0.0, 30.0)],
        page: None,
    })
    .unwrap();
    let project = doc.project();
    let wire = project.wires().next().unwrap();
    assert_eq!(wire.page, page);
    assert_eq!(wire.to, open);
    assert!(open.is_free() && wire.to.as_pin().is_none());
    assert_eq!(
        project.endpoint_position(open),
        Some(Point::new(20.0, 30.0))
    );

    // The wire belongs to the net of its connected end; the open end is in
    // no net.
    let nets = project.netlist();
    let net = nets.net_of_wire(wire).expect("net of the pin");
    assert_eq!(Some(net), nets.net_of(Endpoint::pin(a, 1)));
    assert_eq!(nets.net_of(open), None);

    // Undo and redo like any wire.
    doc.undo().unwrap();
    assert_eq!(doc.project().wires().count(), 0);
    doc.redo().unwrap();
    assert_eq!(doc.project().wires().next().unwrap().to, open);
}

#[test]
fn open_ends_at_one_point_do_not_join_wires() {
    let (mut doc, _, a, b) = setup();
    let open = Endpoint::Free {
        at: Point::new(25.0, 40.0),
    };
    doc.transact("Two stubs", |tx| {
        tx.connect(Endpoint::pin(a, 1), open)?;
        tx.connect(Endpoint::pin(b, 1), open)?;
        Ok(())
    })
    .unwrap();
    let nets = doc.project().netlist();
    assert!(!nets.connected(Endpoint::pin(a, 1), Endpoint::pin(b, 1)));
}

#[test]
fn a_wire_open_at_both_ends_needs_a_page() {
    let (mut doc, page, _, _) = setup();
    let (from, to) = (
        Endpoint::Free {
            at: Point::new(0.0, 40.0),
        },
        Endpoint::Free {
            at: Point::new(30.0, 40.0),
        },
    );
    let refused = doc.execute(Command::Connect {
        from,
        to,
        route: vec![],
        page: None,
    });
    assert!(
        matches!(refused, Err(ModelError::Refused { .. })),
        "{refused:?}"
    );

    doc.execute(Command::Connect {
        from,
        to,
        route: vec![],
        page: Some(page),
    })
    .unwrap();
    let nets = doc.project().netlist();
    let wire = doc.project().wire(wire_between(&doc, from, to)).unwrap();
    assert_eq!(wire.page, page);
    assert_eq!(nets.net_of_wire(wire), None);
    assert!(nets.nets().iter().all(|n| n.wires.is_empty()));

    // Two coincident open ends are one point, not a wire.
    let same = doc.execute(Command::Connect {
        from,
        to: from,
        route: vec![],
        page: Some(page),
    });
    assert!(
        matches!(same, Err(ModelError::SelfConnection(_))),
        "{same:?}"
    );
}

#[test]
fn an_open_end_is_connected_by_replacing_it() {
    let (mut doc, _, a, b) = setup();
    let open = Endpoint::Free {
        at: Point::new(40.0, 5.0),
    };
    doc.execute(Command::Connect {
        from: Endpoint::pin(a, 1),
        to: open,
        route: vec![],
        page: None,
    })
    .unwrap();
    let wire = doc.project().wires().next().unwrap().id;
    doc.transact("Connect the end", |tx| {
        tx.update::<Wire>(wire, |w| w.to = Endpoint::pin(b, 0))
    })
    .unwrap();
    let nets = doc.project().netlist();
    assert!(nets.connected(Endpoint::pin(a, 1), Endpoint::pin(b, 0)));
    assert!(doc.project().wires().all(|w| !w.to.is_free()));
}

#[test]
fn open_ends_are_stored_in_files_and_old_files_still_read() {
    let open = Endpoint::Free {
        at: Point::new(1.5, -2.0),
    };
    let json = serde_json::to_string(&open).unwrap();
    assert_eq!(json, r#"{"type":"free","at":{"x":1.5,"y":-2.0}}"#);
    assert_eq!(serde_json::from_str::<Endpoint>(&json).unwrap(), open);
    let pin: Endpoint = serde_json::from_str(r#"{"type":"pin","symbol":3,"pin":1}"#).unwrap();
    assert!(!pin.is_free());
}

#[test]
fn moving_between_pages_carries_open_ends() {
    let (mut doc, page, a, _) = setup();
    let open = Endpoint::Free {
        at: Point::new(20.0, 30.0),
    };
    doc.execute(Command::Connect {
        from: Endpoint::pin(a, 1),
        to: open,
        route: vec![Point::new(0.0, 30.0)],
        page: None,
    })
    .unwrap();
    let (other, _) = doc.transact("Page", |tx| tx.add_page("2")).unwrap();
    // The wire's other end moves, so the wire goes with it, open end too.
    doc.transact("Move", |tx| {
        tx.move_to_page(&[a.into()], other, Point::new(5.0, 0.0))
    })
    .unwrap();
    let project = doc.project();
    let wire = project.wires().next().unwrap();
    assert_ne!(page, other);
    assert_eq!(wire.page, other);
    assert_eq!(
        wire.to,
        Endpoint::Free {
            at: Point::new(25.0, 30.0)
        }
    );
    assert_eq!(wire.route, [Point::new(5.0, 30.0)]);
}
