use novo_core::{DeviceTag, Document, Pin, Placement, Point, SymbolRole, Value};
use novo_sim::rest_state;

fn put(
    tx: &mut novo_core::Tx<'_>,
    page: novo_core::PageId,
    key: &str,
    poles: usize,
    role: SymbolRole,
) -> novo_core::SymbolId {
    let id = tx
        .place_symbol(
            page,
            key,
            Placement::at(Point::new(0.0, 0.0)),
            vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
            role,
        )
        .unwrap();
    tx.update::<novo_core::SymbolInstance>(id, |s| s.pass_through = vec![vec![0, 1]; poles])
        .unwrap();
    id
}

#[test]
fn the_plant_at_rest_has_break_contacts_and_breakers_closed() {
    let mut doc = Document::default();
    let (ids, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let f1 = tx.add_device(DeviceTag::product("F1"), "")?;
            let f2 = tx.add_device(DeviceTag::product("F2"), "")?;
            let dev = |d| SymbolRole::Device {
                device: d,
                main: true,
            };
            let breaker = put(tx, page, "iec/protection/mcb_3p", 3, dev(f1));
            let off = put(tx, page, "iec/protection/mcb_3p", 3, dev(f2));
            tx.update::<novo_core::Device>(f2, |d| {
                d.properties
                    .insert("state".into(), Value::Text("off".into()));
            })?;
            let nc = put(
                tx,
                page,
                "iec/operators/pushbutton_nc",
                1,
                SymbolRole::Unassigned,
            );
            let no = put(
                tx,
                page,
                "iec/operators/pushbutton_no",
                1,
                SymbolRole::Unassigned,
            );
            Ok((breaker, off, nc, no))
        })
        .unwrap();
    let (breaker, off, nc, no) = ids;
    let rest = rest_state(doc.project());
    assert!((0..3).all(|g| rest.is_closed(breaker, g)));
    assert!((0..3).all(|g| !rest.is_closed(off, g)));
    assert!(rest.is_closed(nc, 0));
    assert!(!rest.is_closed(no, 0));
}
