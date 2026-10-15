//! Drawn channels: which symbol draws which channel, labels, free
//! channels and the problems of plc.md §7.

use novo_core::{
    DeviceId, DeviceKind, DeviceTag, Document, Endpoint, PageId, Pin, Placement, Point, SymbolId,
    SymbolRole, Value,
};
use novo_plc::card::{ADDRESS_STYLE, CHANNEL, CHANNELS, SLOT, START_INPUT};
use novo_plc::{ChannelType, FromKey, Issue, PlcIndex, Space};

struct Fixture {
    doc: Document,
    page: PageId,
    card: DeviceId,
}

/// A page and a DI8 DQ8 card -K1 in slot 1 with inputs from byte 4 and
/// outputs from byte 0.
fn fixture() -> Fixture {
    let mut doc = Document::default();
    let (page, card) = doc
        .transact("Setup", |tx| {
            let page = tx.add_page("1")?;
            let card = tx.add_device_of_kind(DeviceTag::parse("-K1")?, "", DeviceKind::Plc)?;
            tx.set_property(card, CHANNELS, Some(Value::from("DI8 DQ8")))?;
            tx.set_property(card, SLOT, Some(Value::Int(1)))?;
            tx.set_property(card, START_INPUT, Some(Value::Int(4)))?;
            tx.set_property(card, "plc.start.q", Some(Value::Int(0)))?;
            Ok((page, card))
        })
        .unwrap()
        .0;
    Fixture { doc, page, card }
}

impl Fixture {
    fn place(&mut self, key: &str, channel: Option<&str>, x: f64) -> SymbolId {
        let (page, card) = (self.page, self.card);
        self.doc
            .transact("Place channel", |tx| {
                let symbol = tx.place_symbol(
                    page,
                    key,
                    Placement::at(Point::new(x, 50.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Device {
                        device: card,
                        main: false,
                    },
                )?;
                if let Some(c) = channel {
                    tx.set_property(symbol, CHANNEL, Some(Value::from(c)))?;
                }
                Ok(symbol)
            })
            .unwrap()
            .0
    }

    fn index(&self) -> PlcIndex {
        PlcIndex::build(self.doc.project(), &FromKey)
    }
}

const DI: &str = "iec/plc/digital_input";
const DQ: &str = "iec/plc/digital_output";

#[test]
fn channels_get_derived_address_labels() {
    let mut f = fixture();
    let di3 = f.place(DI, Some("DI 3"), 10.0);
    let dq7 = f.place(DQ, Some("DQ 7"), 20.0);
    let index = f.index();
    assert_eq!(index.address_label(di3).as_deref(), Some("%IX4.3"));
    assert_eq!(index.address_label(dq7).as_deref(), Some("%QX0.7"));

    // Moving the card's start relabels every channel; nothing is stored on
    // the symbols.
    f.doc
        .transact("Start", |tx| {
            tx.set_property(f.card, START_INPUT, Some(Value::Int(10)))?;
            tx.set_info(|i| {
                i.properties
                    .insert(ADDRESS_STYLE.into(), Value::from("plain"));
            })
        })
        .unwrap();
    assert_eq!(f.index().address_label(di3).as_deref(), Some("I10.3"));
}

#[test]
fn the_next_placement_takes_the_lowest_free_channel() {
    let mut f = fixture();
    let index = f.index();
    assert_eq!(
        index.next_channel(f.card, ChannelType::DigitalInput),
        Some(0)
    );
    f.place(DI, Some("DI 0"), 10.0);
    f.place(DI, Some("DI 2"), 20.0);
    let index = f.index();
    assert_eq!(
        index.next_channel(f.card, ChannelType::DigitalInput),
        Some(1)
    );
    assert_eq!(
        index.free_channels(f.card, ChannelType::DigitalInput),
        vec![1, 3, 4, 5, 6, 7]
    );
    assert_eq!(index.cards_with_free(ChannelType::AnalogInput), vec![]);
    assert_eq!(
        index.cards_with_free(ChannelType::DigitalOutput),
        vec![f.card]
    );
}

#[test]
fn the_overview_symbol_is_not_a_channel() {
    let mut f = fixture();
    let card = f.card;
    let overview = f
        .doc
        .transact("Overview", |tx| {
            tx.place_symbol(
                f.page,
                DI,
                Placement::at(Point::new(0.0, 0.0)),
                vec![],
                SymbolRole::Device {
                    device: card,
                    main: true,
                },
            )
        })
        .unwrap()
        .0;
    assert_eq!(f.index().channel_of(overview), None);
}

fn kinds(index: &PlcIndex) -> Vec<&'static str> {
    index
        .issues()
        .iter()
        .map(|i| match i {
            Issue::Overlap { .. } => "overlap",
            Issue::DrawnTwice { .. } => "drawn_twice",
            Issue::NoSuchChannel { .. } => "no_such_channel",
            Issue::Unnumbered { .. } => "unnumbered",
            Issue::TypeMismatch { .. } => "type_mismatch",
            Issue::NoLayout { .. } => "no_layout",
            Issue::BadLayout { .. } => "bad_layout",
            Issue::NoStart { .. } => "no_start",
            Issue::Unconnected { .. } => "unconnected",
            Issue::SharedSlot { .. } => "shared_slot",
        })
        .collect()
}

#[test]
fn channel_problems() {
    let mut f = fixture();
    let a = f.place(DI, Some("DI 1"), 10.0);
    let b = f.place(DI, Some("DI 1"), 20.0);
    let beyond = f.place(DI, Some("DI 8"), 30.0);
    let unnumbered = f.place(DQ, None, 40.0);
    let index = f.index();
    let issues = index.issues();
    assert!(issues.contains(&Issue::DrawnTwice {
        card: f.card,
        kind: ChannelType::DigitalInput,
        number: 1,
        symbols: vec![a, b],
    }));
    assert!(issues.contains(&Issue::NoSuchChannel {
        symbol: beyond,
        card: f.card,
        kind: ChannelType::DigitalInput,
        number: 8,
        count: 8,
    }));
    assert!(issues.contains(&Issue::Unnumbered {
        symbol: unnumbered,
        card: f.card,
    }));
    // None of the four is wired.
    assert_eq!(
        kinds(&index)
            .iter()
            .filter(|k| **k == "unconnected")
            .count(),
        4
    );
    assert!(
        issues
            .iter()
            .find(|i| matches!(i, Issue::DrawnTwice { .. }))
            .unwrap()
            .is_error()
    );
}

#[test]
fn a_wired_channel_is_connected() {
    let mut f = fixture();
    let di = f.place(DI, Some("DI 0"), 10.0);
    let page = f.page;
    f.doc
        .transact("Wire", |tx| {
            let k1 = tx.add_device(DeviceTag::parse("-K9")?, "")?;
            let contact = tx.place_symbol(
                page,
                "iec/contacts/no",
                Placement::at(Point::new(10.0, 20.0)),
                vec![Pin::new("13", 0.0, -5.0), Pin::new("14", 0.0, 5.0)],
                SymbolRole::Device {
                    device: k1,
                    main: false,
                },
            )?;
            tx.connect(Endpoint::pin(contact, 1), Endpoint::pin(di, 0))?;
            Ok(())
        })
        .unwrap();
    assert!(kinds(&f.index()).is_empty(), "{:?}", f.index().issues());
}

#[test]
fn card_problems() {
    let mut f = fixture();
    f.doc
        .transact("More cards", |tx| {
            // Overlaps -A1's inputs (4..5) and shares its slot.
            let a2 = tx.add_device_of_kind(DeviceTag::parse("-K2")?, "", DeviceKind::Plc)?;
            tx.set_property(a2, CHANNELS, Some(Value::from("DI16")))?;
            tx.set_property(a2, SLOT, Some(Value::Int(1)))?;
            tx.set_property(a2, START_INPUT, Some(Value::Int(3)))?;
            // No layout at all.
            tx.add_device_of_kind(DeviceTag::parse("-K3")?, "", DeviceKind::Plc)?;
            // A layout but no start.
            let a4 = tx.add_device_of_kind(DeviceTag::parse("-K4")?, "", DeviceKind::Plc)?;
            tx.set_property(a4, CHANNELS, Some(Value::from("AI4")))?;
            Ok(())
        })
        .unwrap();
    let index = f.index();
    let mut found = kinds(&index);
    found.sort();
    assert_eq!(found, ["no_layout", "no_start", "overlap", "shared_slot"]);
    assert!(index.issues().contains(&Issue::Overlap {
        cards: [f.card, index.cards()[1].device],
        space: Space::Input,
    }));
}

#[test]
fn the_symbol_library_and_from_key_agree() {
    use novo_core::SymbolKey;
    use novo_plc::Functions;
    let library = novo_symbols::Library::builtin().unwrap();
    for name in [
        "digital_input",
        "digital_output",
        "relay_output",
        "analog_input",
        "analog_output",
    ] {
        let key = SymbolKey(format!("iec/plc/{name}"));
        assert_eq!(library.function(&key), FromKey.function(&key), "{name}");
        assert!(ChannelType::from_function(library.function(&key).unwrap()).is_some());
    }
}

#[test]
fn a_stored_channel_of_another_type_is_a_mismatch() {
    let mut f = fixture();
    let di = f.place(DI, Some("DQ 3"), 10.0);
    let index = f.index();
    assert_eq!(index.address_label(di), None);
    assert!(index.issues().contains(&Issue::TypeMismatch {
        symbol: di,
        card: f.card,
        drawn: ChannelType::DigitalInput,
        stored: ChannelType::DigitalOutput,
    }));
    // The DQ channel it names is still free.
    assert_eq!(
        index.next_channel(f.card, ChannelType::DigitalOutput),
        Some(0)
    );
}

#[test]
fn stored_channels_parse() {
    use novo_plc::{channel_key, parse_channel};
    assert_eq!(channel_key(ChannelType::RelayOutput, 2), "RO 2");
    assert_eq!(parse_channel("RO 2"), Some((ChannelType::RelayOutput, 2)));
    assert_eq!(
        parse_channel(" do3 "),
        Some((ChannelType::DigitalOutput, 3))
    );
    assert_eq!(parse_channel("3"), None);
    assert_eq!(parse_channel("XX 3"), None);
}

#[test]
fn rockwell_projects_report_no_byte_overlaps() {
    let mut f = fixture();
    f.doc
        .transact("Overlapping card, Rockwell style", |tx| {
            let a2 = tx.add_device_of_kind(DeviceTag::parse("-K2")?, "", DeviceKind::Plc)?;
            tx.set_property(a2, CHANNELS, Some(Value::from("DI8")))?;
            tx.set_property(a2, SLOT, Some(Value::Int(2)))?;
            tx.set_property(a2, START_INPUT, Some(Value::Int(4)))?;
            tx.set_info(|i| {
                i.properties
                    .insert(ADDRESS_STYLE.into(), Value::from("rockwell"));
            })
        })
        .unwrap();
    assert!(kinds(&f.index()).is_empty(), "{:?}", f.index().issues());
}

#[test]
fn issues_have_messages_and_marks() {
    let mut f = fixture();
    let a = f.place(DI, Some("DI 9"), 10.0);
    let index = f.index();
    let issue = index
        .issues()
        .iter()
        .find(|i| matches!(i, Issue::NoSuchChannel { .. }))
        .unwrap();
    assert_eq!(
        issue.message(f.doc.project()),
        "-K1 has 8 DI channels, so DI 9 does not exist"
    );
    assert_eq!(issue.at(), vec![a.into()]);
    for issue in index.issues() {
        assert!(!issue.message(f.doc.project()).ends_with('.'));
    }
}

#[test]
fn shared_slots_clash_under_rockwell() {
    let mut f = fixture();
    let shared = |f: &Fixture| {
        f.index()
            .issues()
            .iter()
            .find(|i| matches!(i, Issue::SharedSlot { .. }))
            .cloned()
            .unwrap()
    };
    f.doc
        .transact("Same slot", |tx| {
            let k2 = tx.add_device_of_kind(DeviceTag::parse("-K2")?, "", DeviceKind::Plc)?;
            tx.set_property(k2, CHANNELS, Some(Value::from("DQ8")))?;
            tx.set_property(k2, SLOT, Some(Value::Int(1)))?;
            tx.set_property(k2, "plc.start.q", Some(Value::Int(1)))
        })
        .unwrap();
    assert!(!shared(&f).is_error());
    f.doc
        .transact("Rockwell", |tx| {
            tx.set_info(|i| {
                i.properties
                    .insert(ADDRESS_STYLE.into(), Value::from("rockwell"));
            })
        })
        .unwrap();
    assert!(shared(&f).is_error());
}

#[test]
fn a_bad_layout_is_reported_once() {
    let mut f = fixture();
    f.place(DI, Some("DI 0"), 10.0);
    f.doc
        .transact("Typo", |tx| {
            tx.set_property(f.card, CHANNELS, Some(Value::from("DX8")))
        })
        .unwrap();
    let mut found = kinds(&f.index());
    found.retain(|k| *k != "unconnected");
    assert_eq!(found, ["bad_layout"]);
}
