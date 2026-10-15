//! The I/O list rows.

use novo_core::{
    DeviceKind, DeviceTag, Document, Endpoint, Pin, Placement, Point, SymbolRole, Value,
};
use novo_plc::card::{CHANNEL, CHANNELS, SIGNAL, SLOT, START_INPUT, START_OUTPUT, STATION};
use novo_plc::{ChannelType, FromKey, PlcIndex};

#[test]
fn every_channel_is_a_row_in_address_order() {
    let mut doc = Document::default();
    let (di3, dq0, k9_contact, x2) = doc
        .transact("Setup", |tx| {
            let page = tx.add_page("3")?;
            let card = tx.add_device_of_kind(DeviceTag::parse("-K1")?, "", DeviceKind::Plc)?;
            tx.set_property(card, CHANNELS, Some(Value::from("DQ2 DI4")))?;
            tx.set_property(card, SLOT, Some(Value::Int(2)))?;
            tx.set_property(card, START_INPUT, Some(Value::Int(4)))?;
            tx.set_property(card, START_OUTPUT, Some(Value::Int(0)))?;
            let place = |tx: &mut novo_core::Tx, key: &str, x: f64, channel: &str| {
                let s = tx.place_symbol(
                    page,
                    key,
                    Placement::at(Point::new(x, 100.0)),
                    vec![Pin::new("I3", 0.0, 0.0)],
                    SymbolRole::Device {
                        device: card,
                        main: false,
                    },
                )?;
                tx.set_property(s, CHANNEL, Some(Value::from(channel)))?;
                Ok::<_, novo_core::ModelError>(s)
            };
            let di3 = place(tx, "iec/plc/digital_input", 50.0, "DI 3")?;
            let dq0 = place(tx, "iec/plc/digital_output", 80.0, "DQ 0")?;
            tx.set_property(dq0, SIGNAL, Some(Value::from("Horn")))?;
            let k9 = tx.add_device(DeviceTag::parse("-K9")?, "Pump 1 running")?;
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
            let strip = tx.add_terminal_strip(DeviceTag::parse("-X2")?)?;
            let t7 = tx.add_terminal(strip, "7")?;
            let terminal = tx.place_symbol(
                page,
                "iec/terminals/terminal",
                Placement::at(Point::new(50.0, 70.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Terminal {
                    terminals: vec![t7],
                },
            )?;
            tx.connect(Endpoint::pin(contact, 1), Endpoint::pin(terminal, 0))?;
            tx.connect(Endpoint::pin(terminal, 0), Endpoint::pin(di3, 0))?;
            Ok((di3, dq0, contact, t7))
        })
        .unwrap()
        .0;
    let index = PlcIndex::build(doc.project(), &FromKey);
    let rows = index.io_rows(doc.project());
    let summary: Vec<_> = rows
        .iter()
        .map(|r| (r.address.clone().unwrap(), r.kind, r.channel, r.spare))
        .collect();
    use ChannelType::*;
    assert_eq!(
        summary,
        [
            ("%IX4.0".to_owned(), DigitalInput, 0, true),
            ("%IX4.1".to_owned(), DigitalInput, 1, true),
            ("%IX4.2".to_owned(), DigitalInput, 2, true),
            ("%IX4.3".to_owned(), DigitalInput, 3, false),
            ("%QX0.0".to_owned(), DigitalOutput, 0, false),
            ("%QX0.1".to_owned(), DigitalOutput, 1, true),
        ]
    );
    let di = &rows[3];
    assert_eq!(di.station_title, "PLC");
    assert_eq!((di.card_tag.as_str(), di.slot), ("-K1", Some(2)));
    assert_eq!(di.pin, "I3");
    assert_eq!(di.signal, "Pump 1 running");
    assert_eq!(di.connected, vec![("-K9:14".to_owned(), k9_contact.into())]);
    assert_eq!(di.terminals, vec![("-X2:7".to_owned(), x2.into())]);
    assert_eq!(di.drawn_at.as_ref().map(|d| d.1), Some(di3.into()));
    assert!(di.drawn_at.as_ref().unwrap().0.starts_with("3."));
    let dq = &rows[4];
    assert_eq!(
        (dq.signal.as_str(), dq.drawn_at.as_ref().map(|d| d.1)),
        ("Horn", Some(dq0.into()))
    );
    assert!(rows[0].connected.is_empty() && rows[0].drawn_at.is_none());
}

#[test]
fn stations_group_rows_and_missing_starts_sort_last() {
    let mut doc = Document::default();
    doc.transact("Cards", |tx| {
        let a = tx.add_device_of_kind(DeviceTag::parse("-K1")?, "", DeviceKind::Plc)?;
        tx.set_property(a, CHANNELS, Some(Value::from("DI1")))?;
        tx.set_property(a, STATION, Some(Value::from("B")))?;
        tx.set_property(a, START_INPUT, Some(Value::Int(0)))?;
        let b = tx.add_device_of_kind(DeviceTag::parse("-K2")?, "", DeviceKind::Plc)?;
        tx.set_property(b, CHANNELS, Some(Value::from("DI1")))?;
        let c = tx.add_device_of_kind(DeviceTag::parse("-K3")?, "", DeviceKind::Plc)?;
        tx.set_property(c, CHANNELS, Some(Value::from("DI1")))?;
        tx.set_property(c, START_INPUT, Some(Value::Int(9)))?;
        Ok(())
    })
    .unwrap();
    let rows = PlcIndex::build(doc.project(), &FromKey).io_rows(doc.project());
    let order: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r.station_title.as_str(),
                r.card_tag.as_str(),
                r.address.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            ("PLC", "-K3", Some("%IX9.0")),
            ("PLC", "-K2", None),
            ("PLC station B", "-K1", Some("%IX0.0")),
        ]
    );
}

#[test]
fn rockwell_rows_need_no_start_and_follow_the_slots() {
    use novo_plc::card::ADDRESS_STYLE;
    let mut doc = Document::default();
    doc.transact("Cards", |tx| {
        for (tag, slot) in [("-K1", 1), ("-K2", 2)] {
            let c = tx.add_device_of_kind(DeviceTag::parse(tag)?, "", DeviceKind::Plc)?;
            tx.set_property(c, CHANNELS, Some(Value::from("DI2")))?;
            tx.set_property(c, SLOT, Some(Value::Int(slot)))?;
        }
        tx.set_info(|i| {
            i.properties
                .insert(ADDRESS_STYLE.into(), Value::from("rockwell"));
        })
    })
    .unwrap();
    let project = doc.project();
    let rows = PlcIndex::build(project, &FromKey)
        .io_rows_with(project, &novo_core::Netlist::build(project));
    let addresses: Vec<_> = rows.iter().map(|r| r.address.clone().unwrap()).collect();
    assert_eq!(
        addresses,
        [
            "Local:1:I.Data.0",
            "Local:1:I.Data.1",
            "Local:2:I.Data.0",
            "Local:2:I.Data.1"
        ]
    );
}
