//! The I/O list: every PLC channel, spares included, in address order.

use novo_core::{
    DeviceKind, DeviceTag, Document, Endpoint, Pin, Placement, Point, SymbolRole, Value,
};
use novo_plc::card::{CHANNEL, CHANNELS, SLOT, START_INPUT};
use novo_reports::{IoList, Report, default_generated_pages};

/// A DI8 DQ8 card -K1 with DI 1 drawn and wired to -S1:13.
fn project() -> Document {
    let mut doc = Document::default();
    doc.transact("PLC", |tx| {
        let page = tx.add_page("1")?;
        let card = tx.add_device_of_kind(DeviceTag::parse("-K1")?, "", DeviceKind::Plc)?;
        tx.set_property(card, CHANNELS, Some(Value::from("DI8 DQ8")))?;
        tx.set_property(card, SLOT, Some(Value::Int(1)))?;
        tx.set_property(card, START_INPUT, Some(Value::Int(4)))?;
        tx.set_property(card, "plc.start.q", Some(Value::Int(0)))?;
        let di = tx.place_symbol(
            page,
            "iec/plc/digital_input",
            Placement::at(Point::new(40.0, 50.0)),
            vec![Pin::new("I1", 0.0, 0.0)],
            SymbolRole::Device {
                device: card,
                main: false,
            },
        )?;
        tx.set_property(di, CHANNEL, Some(Value::from("DI 1")))?;
        let s1 = tx.add_device(DeviceTag::parse("-S1")?, "Start")?;
        let button = tx.place_symbol(
            page,
            "iec/contacts/no",
            Placement::at(Point::new(40.0, 20.0)),
            vec![Pin::new("13", 0.0, 0.0), Pin::new("14", 0.0, 10.0)],
            SymbolRole::Device {
                device: s1,
                main: true,
            },
        )?;
        tx.connect(Endpoint::pin(button, 1), Endpoint::pin(di, 0))?;
        Ok(())
    })
    .expect("plc");
    doc
}

#[test]
fn every_channel_is_listed_with_spares_in_address_order() {
    let doc = project();
    let output = IoList.generate(doc.project());
    assert_eq!(output.sections.len(), 1);
    let section = &output.sections[0];
    assert_eq!(section.title, "PLC");
    let rows = section.text_rows();
    assert_eq!(rows.len(), 16, "{rows:#?}");
    // Inputs first, then outputs.
    assert!(rows[..8].iter().all(|r| r[1] == "DI"), "{rows:#?}");
    assert!(rows[8..].iter().all(|r| r[1] == "DQ"), "{rows:#?}");
    let col = |key: &str| section.column(key).expect(key);
    // DI 1 is drawn and wired; the rest are spares.
    let drawn: Vec<&Vec<String>> = rows
        .iter()
        .filter(|r| !r[col("drawn")].is_empty())
        .collect();
    assert_eq!(drawn.len(), 1, "{rows:#?}");
    let di1 = drawn[0];
    assert_eq!(di1[col("channel")], "1");
    assert_eq!(di1[col("pin")], "I1");
    assert!(di1[col("connected")].contains("-S1:14"), "{di1:?}");
    assert!(!di1[col("address")].is_empty());
    let spares = rows.iter().filter(|r| r[col("signal")] == "Spare").count();
    assert_eq!(spares, 15);
    // The row of the drawn channel links to its symbol first.
    let row = section
        .rows
        .iter()
        .find(|r| !r.sources.is_empty() && r.cells[col("drawn")].to_string() == di1[col("drawn")])
        .expect("row");
    assert!(matches!(row.sources[0], novo_core::AnyId::Symbol(_)));
}

#[test]
fn a_project_with_plc_cards_gets_an_io_page() {
    let doc = project();
    let specs = default_generated_pages(doc.project());
    assert!(
        specs
            .iter()
            .any(|s| s.generator == "io" && s.title == "I/O list"),
        "{specs:?}"
    );
    // Without cards there is nothing to list, so no page.
    let empty = Document::default();
    assert!(
        default_generated_pages(empty.project())
            .iter()
            .all(|s| s.generator != "io")
    );
}
