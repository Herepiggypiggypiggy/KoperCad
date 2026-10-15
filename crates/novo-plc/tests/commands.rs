//! PLC edits, each one transaction and one undo step.

use novo_core::{DeviceTag, Document, Endpoint, Pin, Placement, Point, SymbolRole, Value};
use novo_plc::card::CHANNELS;
use novo_plc::commands::{
    NewCardLayout, StartMove, add_card, apply_renumber, place_channel, renumber_plan,
    signal_channel_type, to_plc,
};
use novo_plc::{ChannelType, FromKey, PlcIndex, Space, cards};

fn typed(layout: &str) -> NewCardLayout {
    NewCardLayout::Typed(layout.parse().unwrap())
}

fn pin() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0)]
}

#[test]
fn added_cards_get_the_next_free_range() {
    let mut doc = Document::default();
    let (a, b) = doc
        .transact("Cards", |tx| {
            let a = add_card(
                tx,
                DeviceTag::parse("-K1")?,
                "DI card",
                typed("DI16"),
                "",
                Some(1),
            )?;
            let b = add_card(
                tx,
                DeviceTag::parse("-K2")?,
                "",
                typed("DI8 AQ2"),
                "",
                Some(2),
            )?;
            Ok((a, b))
        })
        .unwrap()
        .0;
    let all = cards(doc.project());
    assert_eq!(all[0].device, a);
    assert_eq!(all[0].range(Space::Input), Some(0..2));
    assert_eq!(all[1].device, b);
    assert_eq!(all[1].range(Space::Input), Some(2..3));
    assert_eq!(all[1].range(Space::Output), Some(0..4));
    // A part with a layout works the same way.
    let c = doc
        .transact("Card from part", |tx| {
            let part = tx.add_part("Acme", "AI4")?;
            tx.set_property(part, CHANNELS, Some(Value::from("AI4")))?;
            add_card(
                tx,
                DeviceTag::parse("-K3")?,
                "",
                NewCardLayout::Part(part),
                "",
                Some(3),
            )
        })
        .unwrap()
        .0;
    let card = cards(doc.project())
        .into_iter()
        .find(|x| x.device == c)
        .unwrap();
    // After -K2's byte 2, on a word.
    assert_eq!(card.range(Space::Input), Some(4..12));
    doc.undo().unwrap();
    assert_eq!(cards(doc.project()).len(), 2);
}

#[test]
fn channels_are_placed_in_order_until_the_card_is_full() {
    let mut doc = Document::default();
    let (page, card) = doc
        .transact("Setup", |tx| {
            let page = tx.add_page("1")?;
            let card = add_card(tx, DeviceTag::parse("-K1")?, "", typed("DI2"), "", None)?;
            Ok((page, card))
        })
        .unwrap()
        .0;
    let place = |doc: &mut Document, x: f64| {
        doc.transact("Place", |tx| {
            place_channel(
                tx,
                &FromKey,
                card,
                ChannelType::DigitalInput,
                page,
                "iec/plc/digital_input",
                Placement::at(Point::new(x, 0.0)),
                pin(),
            )
        })
        .map(|r| r.0)
    };
    let first = place(&mut doc, 10.0).unwrap();
    let second = place(&mut doc, 20.0).unwrap();
    let index = PlcIndex::build(doc.project(), &FromKey);
    assert_eq!(index.address_label(first).as_deref(), Some("%IX0.0"));
    assert_eq!(index.address_label(second).as_deref(), Some("%IX0.1"));
    assert!(place(&mut doc, 30.0).is_err());
    assert_eq!(
        doc.project().symbols().count(),
        2,
        "a refused placement leaves nothing"
    );
}

#[test]
fn to_plc_places_and_connects_in_one_step() {
    let mut doc = Document::default();
    let (card, contact) = doc
        .transact("Setup", |tx| {
            let page = tx.add_page("1")?;
            let card = add_card(tx, DeviceTag::parse("-K1")?, "", typed("DI8"), "", None)?;
            let k9 = tx.add_device(DeviceTag::parse("-K9")?, "Pump running")?;
            let contact = tx.place_symbol(
                page,
                "iec/contacts/no",
                Placement::at(Point::new(50.0, 40.0)),
                vec![Pin::new("13", 0.0, -5.0), Pin::new("14", 0.0, 5.0)],
                SymbolRole::Device {
                    device: k9,
                    main: false,
                },
            )?;
            Ok((card, contact))
        })
        .unwrap()
        .0;
    let kind = signal_channel_type("contact.nc").unwrap();
    let channel = doc
        .transact("To PLC", |tx| {
            to_plc(
                tx,
                &FromKey,
                card,
                Endpoint::pin(contact, 1),
                kind,
                "iec/plc/digital_input",
                Placement::at(Point::new(50.0, 60.0)),
                pin(),
            )
        })
        .unwrap()
        .0;
    let index = PlcIndex::build(doc.project(), &FromKey);
    assert_eq!(index.channel_of(channel).unwrap().number, Some(0));
    assert!(index.issues().is_empty(), "{:?}", index.issues());
    let rows = index.io_rows(doc.project());
    assert_eq!(rows[0].signal, "Pump running");
    doc.undo().unwrap();
    assert_eq!(doc.project().symbol(channel), None);
    assert_eq!(doc.project().wires().count(), 0);
}

#[test]
fn renumber_io_repacks_in_slot_order() {
    let mut doc = Document::default();
    let (a, b, c) = doc
        .transact("Cards", |tx| {
            let a = add_card(tx, DeviceTag::parse("-K1")?, "", typed("DI16"), "", Some(1))?;
            let b = add_card(tx, DeviceTag::parse("-K2")?, "", typed("DI8"), "", Some(2))?;
            let c = add_card(tx, DeviceTag::parse("-K3")?, "", typed("AI2"), "", Some(3))?;
            Ok((a, b, c))
        })
        .unwrap()
        .0;
    assert!(renumber_plan(doc.project(), "").is_empty());
    doc.transact("Delete", |tx| tx.remove(a)).unwrap();
    // -K2 sat at byte 2 and -K3 at 4; re-packed they start at 0 and 2.
    let plan = renumber_plan(doc.project(), "");
    assert_eq!(
        plan,
        [
            StartMove {
                card: b,
                space: Space::Input,
                from: Some(2),
                to: 0
            },
            StartMove {
                card: c,
                space: Space::Input,
                from: Some(4),
                to: 2
            },
        ]
    );
    doc.transact("Renumber io", |tx| apply_renumber(tx, &plan))
        .unwrap();
    assert!(renumber_plan(doc.project(), "").is_empty());
    doc.undo().unwrap();
    assert_eq!(renumber_plan(doc.project(), "").len(), 2);
}

#[test]
fn signal_types_from_library_functions() {
    let library = novo_symbols::Library::builtin().unwrap();
    let kind = |key: &str| signal_channel_type(&library.get(key).unwrap().function);
    assert_eq!(kind("iec/contacts/no"), Some(ChannelType::DigitalInput));
    assert_eq!(kind("iec/contacts/nc"), Some(ChannelType::DigitalInput));
    assert_eq!(
        signal_channel_type("coil.valve"),
        Some(ChannelType::DigitalOutput)
    );
    assert_eq!(
        signal_channel_type("signal.lamp"),
        Some(ChannelType::DigitalOutput)
    );
    assert_eq!(
        signal_channel_type("sensor.transmitter"),
        Some(ChannelType::AnalogInput)
    );
    assert_eq!(signal_channel_type("motor.three_phase"), None);
}
