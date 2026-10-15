//! Cards read from a project, and where a new card's addresses start.

use novo_core::{DeviceId, DeviceKind, DeviceTag, Document, Value};
use novo_plc::card::{ADDRESS_STYLE, CHANNELS, SLOT, STATION, start_key};
use novo_plc::{
    AddressStyle, CardLayout, ChannelType, LayoutError, LayoutSource, Space, address_style,
    allocate, cards,
};

/// Adds a PLC card with a layout on its part, a slot, and the starts
/// `allocate` gives it, the way the add-card command will.
fn add_card(doc: &mut Document, tag: &str, layout: &str, slot: i64) -> DeviceId {
    doc.transact("Add card", |tx| {
        let card = tx.add_device_of_kind(DeviceTag::parse(tag)?, "", DeviceKind::Plc)?;
        let part = tx.add_part("Acme", format!("IO-{layout}"))?;
        tx.set_property(part, CHANNELS, Some(Value::from(layout)))?;
        tx.assign_part(card, part)?;
        tx.set_property(card, SLOT, Some(Value::Int(slot)))?;
        let layout: CardLayout = layout.parse().expect("test layout");
        for (key, value) in allocate(tx.project(), "", &layout, Some(card)) {
            tx.set_property(card, key, Some(value))?;
        }
        Ok(card)
    })
    .expect("add card")
    .0
}

#[test]
fn cards_are_allocated_one_after_another() {
    let mut doc = Document::default();
    let di = add_card(&mut doc, "-K1", "DI16", 1);
    let dq = add_card(&mut doc, "-K2", "DQ8", 2);
    let ai = add_card(&mut doc, "-K3", "DI4 AI2", 3);
    let all = cards(doc.project());
    let card = |id| all.iter().find(|c| c.device == id).unwrap();

    assert_eq!(card(di).range(Space::Input), Some(0..2));
    assert_eq!(card(di).range(Space::Output), None);
    assert_eq!(card(dq).range(Space::Output), Some(0..1));
    // Inputs continue after -K1; the analog card starts on a word.
    assert_eq!(card(ai).range(Space::Input), Some(2..8));
    let address = card(ai).address(ChannelType::AnalogInput, 1).unwrap();
    assert_eq!(address.format(AddressStyle::Siemens), "%IW6");
    assert_eq!(
        card(ai).layout_source,
        LayoutSource::Part(doc.project().device(ai).unwrap().parts[0])
    );
}

#[test]
fn deleting_a_card_moves_no_other_address() {
    let mut doc = Document::default();
    let a1 = add_card(&mut doc, "-K1", "DI16", 1);
    let a2 = add_card(&mut doc, "-K2", "DI16", 2);
    doc.transact("Delete", |tx| tx.remove(a1)).unwrap();
    let a2_card = &cards(doc.project())[0];
    assert_eq!(a2_card.device, a2);
    assert_eq!(a2_card.range(Space::Input), Some(2..4));
    // A new card goes after -K2, not into the freed gap.
    let a3 = add_card(&mut doc, "-K3", "DI8", 3);
    let a3_card = cards(doc.project())
        .into_iter()
        .find(|c| c.device == a3)
        .unwrap();
    assert_eq!(a3_card.range(Space::Input), Some(4..5));
}

#[test]
fn stations_have_their_own_spaces() {
    let mut doc = Document::default();
    add_card(&mut doc, "-K1", "DI16", 1);
    let other = doc
        .transact("Card in station 2", |tx| {
            let card = tx.add_device_of_kind(DeviceTag::parse("-K2")?, "", DeviceKind::Plc)?;
            tx.set_property(card, STATION, Some(Value::from("PLC2")))?;
            tx.set_property(card, CHANNELS, Some(Value::from("DI8")))?;
            Ok(card)
        })
        .unwrap()
        .0;
    let layout: CardLayout = "DI8".parse().unwrap();
    assert_eq!(
        allocate(doc.project(), "PLC2", &layout, Some(other)),
        vec![(start_key(Space::Input), Value::Int(0))]
    );
    assert_eq!(
        allocate(doc.project(), "", &layout, None),
        vec![(start_key(Space::Input), Value::Int(2))]
    );
}

#[test]
fn a_device_layout_overrides_the_part() {
    let mut doc = Document::default();
    let card = add_card(&mut doc, "-K1", "DI16", 1);
    doc.transact("Override", |tx| {
        tx.set_property(card, CHANNELS, Some(Value::from("DI8 DQ8")))
    })
    .unwrap();
    let c = &cards(doc.project())[0];
    assert_eq!(c.layout_source, LayoutSource::Device);
    assert_eq!(c.channels().to_string(), "DI8 DQ8");
    // The output start was never allocated, so outputs have no address yet.
    assert_eq!(c.address(ChannelType::DigitalOutput, 0), None);
}

#[test]
fn bad_and_missing_layouts() {
    let mut doc = Document::default();
    doc.transact("Cards", |tx| {
        let bad = tx.add_device_of_kind(DeviceTag::parse("-K1")?, "", DeviceKind::Plc)?;
        tx.set_property(bad, CHANNELS, Some(Value::from("DX16")))?;
        tx.add_device_of_kind(DeviceTag::parse("-K2")?, "", DeviceKind::Plc)?;
        tx.add_device(DeviceTag::parse("-K1")?, "not a card")?;
        Ok(())
    })
    .unwrap();
    let all = cards(doc.project());
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].layout, Err(LayoutError::BadGroup("DX16".into())));
    assert_eq!(all[1].layout_source, LayoutSource::None);
    assert!(all[1].channels().groups.is_empty());
}

#[test]
fn address_style_comes_from_the_project() {
    let mut doc = Document::default();
    assert_eq!(address_style(doc.project()), AddressStyle::Iec);
    doc.transact("Style", |tx| {
        tx.set_info(|info| {
            info.properties
                .insert(ADDRESS_STYLE.into(), Value::from("siemens"));
        })
    })
    .unwrap();
    assert_eq!(address_style(doc.project()), AddressStyle::Siemens);
}
