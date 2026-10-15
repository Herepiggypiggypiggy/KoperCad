//! Switching the project standard: counterparts are re-keyed in one undo
//! step, symbols the target pack cannot draw are reported.

use novo_core::{DeviceTag, Document, Pin, Placement, Point, SymbolId, SymbolRole};
use novo_symbols::Pack;
use novo_workflow::switch::{current, plan};

fn put(doc: &mut Document, key: &str, device: Option<&str>, x: f64) -> SymbolId {
    let (id, _) = doc
        .transact("Draw", |tx| {
            let existing = tx.project().pages().next().map(|p| p.id);
            let page = match existing {
                Some(p) => p,
                None => tx.add_page("1")?,
            };
            let role = match device {
                Some(tag) => {
                    let d = tx.add_device(DeviceTag::product(tag), "")?;
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    }
                }
                None => SymbolRole::Unassigned,
            };
            let id = tx.place_symbol(
                page,
                key,
                Placement::at(Point::new(x, 0.0)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                role,
            )?;
            Ok(id)
        })
        .unwrap();
    id
}

/// An IEC id the NFPA pack does not have (every shipped IEC symbol has a twin
/// today, so this one is made up, like a symbol added to IEC first).
fn iec_only() -> String {
    "iec/contacts/future_device".to_owned()
}

#[test]
fn counterparts_are_rekeyed_in_one_undo_step() {
    let mut doc = Document::default();
    let coil = put(&mut doc, "iec/coils/relay", Some("K1"), 0.0);
    let lamp = put(&mut doc, "iec/signalling/lamp", Some("H1"), 20.0);
    assert_eq!(current(doc.project()), Pack::Iec);

    let p = plan(doc.project(), Pack::Nfpa);
    assert!(p.is_complete(), "{:?}", p.without_counterpart);
    assert_eq!(p.rekey.len(), 2);
    doc.execute(p.command()).unwrap();
    assert_eq!(current(doc.project()), Pack::Nfpa);
    for id in [coil, lamp] {
        let key = &doc.project().symbol(id).unwrap().symbol.0;
        assert!(key.starts_with("nfpa/"), "{key}");
    }

    doc.undo().unwrap();
    assert_eq!(current(doc.project()), Pack::Iec);
    assert_eq!(
        doc.project().symbol(coil).unwrap().symbol.0,
        "iec/coils/relay"
    );

    // And back again from NFPA to IEC.
    doc.execute(plan(doc.project(), Pack::Nfpa).command())
        .unwrap();
    let back = plan(doc.project(), Pack::Iec);
    assert_eq!(back.rekey.len(), 2);
    doc.execute(back.command()).unwrap();
    assert_eq!(
        doc.project().symbol(lamp).unwrap().symbol.0,
        "iec/signalling/lamp"
    );
}

#[test]
fn symbols_without_a_counterpart_are_reported_and_keep_their_key() {
    let mut doc = Document::default();
    let only = iec_only();
    let odd = put(&mut doc, &only, Some("B7"), 0.0);
    let lamp = put(&mut doc, "iec/signalling/lamp", Some("H1"), 20.0);
    let free = put(&mut doc, &only, None, 40.0);

    let p = plan(doc.project(), Pack::Nfpa);
    assert!(!p.is_complete());
    assert_eq!(p.rekey.len(), 1);
    assert_eq!(p.without_counterpart.len(), 2);
    assert_eq!(
        p.devices_without_counterpart(),
        vec![DeviceTag::product("B7")]
    );
    assert!(
        p.without_counterpart
            .iter()
            .any(|u| u.symbol == free && u.device.is_none())
    );

    doc.execute(p.command()).unwrap();
    assert_eq!(doc.project().symbol(odd).unwrap().symbol.0, only);
    assert!(
        doc.project()
            .symbol(lamp)
            .unwrap()
            .symbol
            .0
            .starts_with("nfpa/")
    );
    // The leftovers are still planned for the next try.
    assert_eq!(plan(doc.project(), Pack::Nfpa).without_counterpart.len(), 2);
}

#[test]
fn symbols_of_a_company_library_are_left_alone() {
    let mut doc = Document::default();
    let id = put(&mut doc, "acme/relays/special", Some("K9"), 0.0);
    let p = plan(doc.project(), Pack::Nfpa);
    assert_eq!(
        (p.rekey.len(), p.without_counterpart.len(), p.foreign),
        (0, 0, 1)
    );
    doc.execute(p.command()).unwrap();
    assert_eq!(
        doc.project().symbol(id).unwrap().symbol.0,
        "acme/relays/special"
    );
    assert_eq!(current(doc.project()), Pack::Nfpa);
}
