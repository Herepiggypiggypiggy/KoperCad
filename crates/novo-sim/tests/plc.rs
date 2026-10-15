use std::collections::BTreeSet;

use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId, SymbolRole, Tx,
};
use novo_sim::{Clock, PoleState, inputs_seen, settle_timed};

fn put(tx: &mut Tx<'_>, page: novo_core::PageId, key: &str, y: f64, role: SymbolRole) -> SymbolId {
    tx.place_symbol(
        page,
        key,
        Placement::at(Point::new(0.0, y)),
        vec![Pin::new("1", 0.0, 0.0)],
        role,
    )
    .unwrap()
}

/// DQ0 of one card is wired to DI0 of another, with 24 V defined.
fn loopback(with_supply: bool) -> (Document, SymbolId, SymbolId) {
    let mut doc = Document::default();
    let (ids, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            if with_supply {
                tx.add_potential("24V", PotentialKind::DcPositive)?;
            }
            let a = tx.add_device(DeviceTag::product("A1"), "")?;
            let b = tx.add_device(DeviceTag::product("A2"), "")?;
            let dq = put(
                tx,
                page,
                "iec/plc/digital_output",
                0.0,
                SymbolRole::Device {
                    device: a,
                    main: false,
                },
            );
            let di = put(
                tx,
                page,
                "iec/plc/digital_input",
                20.0,
                SymbolRole::Device {
                    device: b,
                    main: false,
                },
            );
            tx.connect(Endpoint::pin(dq, 0), Endpoint::pin(di, 0))?;
            Ok((dq, di))
        })
        .unwrap();
    (doc, ids.0, ids.1)
}

fn seen(doc: &Document, forced: &BTreeSet<SymbolId>) -> BTreeSet<SymbolId> {
    let project = doc.project();
    let netlist = project.netlist();
    let settled = settle_timed(
        project,
        &netlist,
        &PoleState::all_open(),
        forced,
        &BTreeSet::new(),
        &mut Clock::new(),
    );
    inputs_seen(project, &netlist, &settled.solution)
}

#[test]
fn an_output_switched_on_lights_the_input_it_is_wired_to() {
    let (doc, dq, di) = loopback(true);
    assert!(seen(&doc, &BTreeSet::new()).is_empty(), "outputs start off");
    assert_eq!(seen(&doc, &BTreeSet::from([dq])), BTreeSet::from([di]));
}

#[test]
fn an_output_with_no_supply_potential_drives_nothing() {
    let (doc, dq, _) = loopback(false);
    assert!(seen(&doc, &BTreeSet::from([dq])).is_empty());
}
