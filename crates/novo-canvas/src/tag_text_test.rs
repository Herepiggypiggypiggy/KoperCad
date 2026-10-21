//! The tag a device shows on a page: short when it has the page's structure,
//! once for the pins of a plug.

use novo_core::{DeviceTag, Document, Pin, Placement, Point, SymbolId, SymbolRole};

use super::{repeats_pin_tag, symbol_text};

fn pins(
    tx: &mut novo_core::Tx<'_>,
    page: novo_core::PageId,
    device: novo_core::DeviceId,
    key: &str,
    n: u8,
) -> Vec<SymbolId> {
    (0..n)
        .map(|i| {
            tx.place_symbol(
                page,
                key,
                Placement::at(Point::new(10.0 * f64::from(i), 0.0)),
                vec![Pin::new((i + 1).to_string(), 0.0, 0.0)],
                SymbolRole::Device {
                    device,
                    main: false,
                },
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn a_tag_is_short_on_its_own_structure_and_full_elsewhere() {
    let mut doc = Document::default();
    let (page, own, other) = doc
        .transact("setup", |tx| {
            let page = tx.add_page("05")?;
            tx.update::<novo_core::Page>(page, |p| {
                p.structure = DeviceTag::parse("=A1+CAB1").unwrap();
            })?;
            let own = tx.add_device(DeviceTag::parse("=A1+CAB1-XM1")?, "Strip")?;
            let other = tx.add_device(DeviceTag::parse("=B2+CAB2-X1")?, "Plug")?;
            let a = pins(tx, page, own, "iec/connectivity/device_pin", 1)[0];
            let b = pins(tx, page, other, "iec/connectivity/device_pin", 1)[0];
            Ok((page, a, b))
        })
        .unwrap()
        .0;
    let _ = page;
    let p = doc.project();
    let text = |s| symbol_text(p, p.symbol(s).unwrap());
    assert_eq!(text(own).as_deref(), Some("-XM1"));
    assert_eq!(text(other).as_deref(), Some("=B2+CAB2-X1"));
}

#[test]
fn a_page_without_structure_keeps_the_full_tag() {
    let mut doc = Document::default();
    let sym = doc
        .transact("setup", |tx| {
            let page = tx.add_page("1")?;
            let d = tx.add_device(DeviceTag::parse("=A1+CAB1-K1")?, "Relay")?;
            Ok(pins(tx, page, d, "iec/coils/relay", 1)[0])
        })
        .unwrap()
        .0;
    let p = doc.project();
    assert_eq!(
        symbol_text(p, p.symbol(sym).unwrap()).as_deref(),
        Some("=A1+CAB1-K1")
    );
}

#[test]
fn the_pins_of_a_plug_show_its_tag_once() {
    for key in ["iec/connectivity/device_pin", "import/box"] {
        let mut doc = Document::default();
        let ids = doc
            .transact("setup", |tx| {
                let page = tx.add_page("1")?;
                let d = tx.add_device(DeviceTag::parse("-X1")?, "Plug")?;
                Ok(pins(tx, page, d, key, 3))
            })
            .unwrap()
            .0;
        let p = doc.project();
        let repeats: Vec<bool> = ids
            .iter()
            .map(|&s| repeats_pin_tag(p, p.symbol(s).unwrap()))
            .collect();
        assert_eq!(repeats, [false, true, true], "{key}");
    }
}

#[test]
fn other_symbols_of_a_device_keep_their_tag_each() {
    let mut doc = Document::default();
    let ids = doc
        .transact("setup", |tx| {
            let page = tx.add_page("1")?;
            let d = tx.add_device(DeviceTag::parse("-K1")?, "Relay")?;
            Ok(pins(tx, page, d, "iec/contacts/no", 2))
        })
        .unwrap()
        .0;
    let p = doc.project();
    assert!(
        ids.iter()
            .all(|&s| !repeats_pin_tag(p, p.symbol(s).unwrap()))
    );
}

#[test]
fn the_single_poles_of_one_device_in_a_row_show_its_tag_once() {
    let mut doc = Document::default();
    let (near, far) = doc
        .transact("setup", |tx| {
            let page = tx.add_page("1")?;
            let d = tx.add_device(DeviceTag::parse("-Q1")?, "Contactor")?;
            let near = pins(tx, page, d, "iec/contactors/main_1p", 3);
            // A second set of poles a page away from the first.
            let role = SymbolRole::Device {
                device: d,
                main: false,
            };
            let far = tx.place_symbol(
                page,
                "iec/contactors/main_1p",
                Placement::at(Point::new(200.0, 0.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                role,
            )?;
            Ok((near, far))
        })
        .unwrap()
        .0;
    let p = doc.project();
    let repeats = |s: SymbolId| repeats_pin_tag(p, p.symbol(s).unwrap());
    assert_eq!(
        near.iter().map(|&s| repeats(s)).collect::<Vec<_>>(),
        [false, true, true]
    );
    assert!(!repeats(far), "poles far apart keep a tag each");
}
