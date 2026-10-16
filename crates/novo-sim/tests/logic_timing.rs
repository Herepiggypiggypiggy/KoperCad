use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId, SymbolRole, Tx,
};
use novo_sim::timing::{Level, Stimulus, record_logic};

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

#[test]
fn the_logic_recorder_draws_the_button_and_the_coil() {
    let (doc, start, _) = circuit("t/contact/no_delay_on");
    let d = record_logic(
        doc.project(),
        &[
            Stimulus {
                t: 1.0,
                symbol: start,
                closed: true,
            },
            Stimulus {
                t: 4.0,
                symbol: start,
                closed: false,
            },
        ],
        6.0,
        0.5,
    );
    let coil = d
        .channels
        .iter()
        .find(|c| c.group == "Coils")
        .expect("a coil row");
    assert_eq!(coil.level_at(0.5), Level::Low);
    assert_eq!(coil.level_at(2.0), Level::High);
    assert_eq!(coil.level_at(5.0), Level::Low);
    assert_eq!(coil.times_to(Level::High), vec![1.0]);
    let input = d
        .channels
        .iter()
        .find(|c| c.group == "Inputs")
        .expect("an input row");
    assert_eq!(input.times_to(Level::High), vec![1.0]);
    assert_eq!(input.times_to(Level::Low), vec![4.0]);
}
