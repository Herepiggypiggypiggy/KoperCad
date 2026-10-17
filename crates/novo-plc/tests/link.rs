//! The PLC engineering link: tag lists, the export formats and the way a
//! programmer's edits come back.

use novo_core::{DeviceKind, DeviceTag, Document, Pin, Placement, Point, SymbolRole, Value};
use novo_plc::card::{CHANNEL, CHANNELS, SIGNAL, SLOT, START_INPUT, START_OUTPUT, STATION};
use novo_plc::link::{
    ExportOptions, FileKind, LinkFormat, TAG, TagOptions, apply_import, export, hardware,
    plan_import, read, tag_set,
};
use novo_plc::{AddressStyle, FromKey, PlcIndex};

/// Two stations. Station A: `-K1` (slot 2, `DQ2 DI4`, inputs from byte 4,
/// outputs from 0) with DI 3 "Pump 1 running", DI 1 "Pump 1 running" (the
/// same words twice), DQ 0 "Horn & beacon <red>", and `-K2` (slot 3, `AI2`,
/// inputs from byte 8) with AI 0 "Level tank 1". Station B: `-K5` (slot 1,
/// `DI2`) with DI 0 "Door closed (Nood-stop)".
fn project() -> Document {
    let mut doc = Document::default();
    doc.transact("Setup", |tx| {
        let page = tx.add_page("3")?;
        let mut x = 20.0;
        let mut channel =
            |tx: &mut novo_core::Tx, card, key: &str, ch: &str, signal: Option<&str>| {
                x += 20.0;
                let s = tx.place_symbol(
                    page,
                    key,
                    Placement::at(Point::new(x, 100.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Device {
                        device: card,
                        main: false,
                    },
                )?;
                tx.set_property(s, CHANNEL, Some(Value::from(ch)))?;
                if let Some(signal) = signal {
                    tx.set_property(s, SIGNAL, Some(Value::from(signal)))?;
                }
                Ok::<_, novo_core::ModelError>(s)
            };
        let k1 = tx.add_device_of_kind(DeviceTag::parse("-K1")?, "", DeviceKind::Plc)?;
        tx.set_property(k1, CHANNELS, Some(Value::from("DQ2 DI4")))?;
        tx.set_property(k1, SLOT, Some(Value::Int(2)))?;
        tx.set_property(k1, START_INPUT, Some(Value::Int(4)))?;
        tx.set_property(k1, START_OUTPUT, Some(Value::Int(0)))?;
        channel(
            tx,
            k1,
            "iec/plc/digital_input",
            "DI 3",
            Some("Pump 1 running"),
        )?;
        channel(
            tx,
            k1,
            "iec/plc/digital_input",
            "DI 1",
            Some("Pump 1 running"),
        )?;
        channel(
            tx,
            k1,
            "iec/plc/digital_output",
            "DQ 0",
            Some("Horn & beacon <red>"),
        )?;
        let k2 = tx.add_device_of_kind(DeviceTag::parse("-K2")?, "", DeviceKind::Plc)?;
        tx.set_property(k2, CHANNELS, Some(Value::from("AI2")))?;
        tx.set_property(k2, SLOT, Some(Value::Int(3)))?;
        tx.set_property(k2, START_INPUT, Some(Value::Int(8)))?;
        channel(tx, k2, "iec/plc/analog_input", "AI 0", Some("Level tank 1"))?;
        let k5 = tx.add_device_of_kind(DeviceTag::parse("-K5")?, "", DeviceKind::Plc)?;
        tx.set_property(k5, CHANNELS, Some(Value::from("DI2")))?;
        tx.set_property(k5, STATION, Some(Value::from("B")))?;
        tx.set_property(k5, SLOT, Some(Value::Int(1)))?;
        tx.set_property(k5, START_INPUT, Some(Value::Int(0)))?;
        channel(
            tx,
            k5,
            "iec/plc/digital_input",
            "DI 0",
            Some("Door closed (Nood-stop)"),
        )?;
        Ok(())
    })
    .unwrap();
    doc
}

fn index(doc: &Document) -> PlcIndex {
    PlcIndex::build(doc.project(), &FromKey)
}

fn text(file: &novo_plc::link::ExportFile) -> String {
    String::from_utf8(file.bytes.clone()).unwrap()
}

#[test]
fn names_come_from_the_signal_and_never_repeat() {
    let doc = project();
    let set = tag_set(
        doc.project(),
        &index(&doc),
        AddressStyle::Iec,
        &TagOptions::default(),
    );
    let names: Vec<_> = set
        .tags
        .iter()
        .map(|t| (t.address_text.as_str(), t.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("%IX4.1", "Pump_1_running"),
            ("%IX4.3", "Pump_1_running_2"),
            ("%IW8", "Level_tank_1"),
            ("%QX0.0", "Horn_beacon_red"),
            ("%IX0.0", "Door_closed_Nood_stop"),
        ]
    );
    assert_eq!(set.stations(), ["", "B"]);
    assert!(set.warnings.is_empty(), "{:?}", set.warnings);
}

#[test]
fn spares_are_named_by_their_card_and_channel() {
    let doc = project();
    let options = TagOptions {
        include_spares: true,
        ..TagOptions::default()
    };
    let set = tag_set(doc.project(), &index(&doc), AddressStyle::Iec, &options);
    let spare: Vec<_> = set
        .tags
        .iter()
        .filter(|t| t.spare)
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(
        spare,
        [
            "Spare_K1_DI0",
            "Spare_K1_DI2",
            "Spare_K2_AI1",
            "Spare_K1_DQ1",
            "Spare_K5_DI1"
        ]
    );
}

#[test]
fn a_card_without_a_start_is_left_out_and_said_so() {
    let mut doc = project();
    let k1 = doc
        .project()
        .devices()
        .find(|d| d.tag.to_string() == "-K1")
        .unwrap()
        .id;
    doc.transact("Clear start", |tx| {
        tx.set_property(k1, START_INPUT, None)?;
        Ok(())
    })
    .unwrap();
    let set = tag_set(
        doc.project(),
        &index(&doc),
        AddressStyle::Iec,
        &TagOptions::default(),
    );
    assert!(
        set.tags
            .iter()
            .all(|t| t.card_tag != "-K1" || t.kind.code() == "DQ")
    );
    assert_eq!(set.warnings.len(), 2, "{:?}", set.warnings);
    assert!(set.warnings[0].contains("-K1 DI 1 has no address"));
}

#[test]
fn csv_reads_back_to_no_changes() {
    let doc = project();
    let out = export(
        doc.project(),
        &index(&doc),
        LinkFormat::Csv,
        &ExportOptions::default(),
    );
    assert_eq!(out.files.len(), 1);
    let csv = text(&out.files[0]);
    assert!(csv.starts_with("\u{feff}Station;Card;Slot;Address;Type;Channel;Tag;"));
    assert!(csv.contains("-K1;2;%IX4.3;DI;3;Pump_1_running_2;BOOL;Pump 1 running;"));
    let file = read(csv.as_bytes()).unwrap();
    assert_eq!(file.kind, FileKind::Csv);
    let plan = plan_import(doc.project(), &index(&doc), &file);
    assert!(plan.updates.is_empty(), "{:?}", plan.updates);
    assert_eq!(plan.unchanged, 5);
    assert_eq!((plan.not_in_file, plan.unmatched.len()), (0, 0));
}

#[test]
fn an_edited_csv_brings_comments_and_tag_names_home() {
    let mut doc = project();
    let csv = text(
        &export(
            doc.project(),
            &index(&doc),
            LinkFormat::Csv,
            &ExportOptions::default(),
        )
        .files[0],
    );
    let edited = csv
        .replace(
            "Horn_beacon_red;BOOL;Horn & beacon <red>",
            "HORN_Q1;BOOL;Alarm horn",
        )
        .replace(
            "Level_tank_1;INT;Level tank 1",
            "Level_tank_1;INT;Level tank 1 (0-10 V)",
        );
    let file = read(edited.as_bytes()).unwrap();
    let plan = plan_import(doc.project(), &index(&doc), &file);
    assert_eq!(plan.updates.len(), 2, "{plan:?}");
    let by = |c: &str| plan.updates.iter().find(|u| u.channel == c).unwrap();
    let horn = by("-K1 DQ 0");
    assert_eq!(horn.channel, "-K1 DQ 0");
    assert_eq!(horn.new_signal.as_deref(), Some("Alarm horn"));
    assert_eq!(horn.new_tag.as_deref(), Some("HORN_Q1"));
    let level = by("-K2 AI 0");
    assert_eq!(level.new_signal.as_deref(), Some("Level tank 1 (0-10 V)"));
    assert_eq!(level.new_tag, None, "the name still follows the signal");
    doc.transact("Import PLC tags", |tx| apply_import(tx, &plan))
        .unwrap();
    let again = export(
        doc.project(),
        &index(&doc),
        LinkFormat::Csv,
        &ExportOptions::default(),
    );
    let csv = text(&again.files[0]);
    assert!(csv.contains("HORN_Q1;BOOL;Alarm horn;"));
    assert!(csv.contains("Level tank 1 (0-10 V);"));
    // Applying changed nothing a second plan would change.
    let plan = plan_import(
        doc.project(),
        &index(&doc),
        &read(edited.as_bytes()).unwrap(),
    );
    // The file still has the old tag on the reworded channel, which now
    // differs from the name the project would export: that is the only
    // thing left to say, and signals are not touched again.
    assert!(
        plan.updates
            .iter()
            .all(|u| u.new_signal.is_none() && u.channel == "-K2 AI 0"),
        "{:?}",
        plan.updates
    );
    let sym = doc
        .project()
        .symbols()
        .find(|s| s.properties.contains_key(TAG))
        .unwrap();
    assert_eq!(sym.properties.get(TAG), Some(&Value::from("HORN_Q1")));
}

#[test]
fn addresses_never_come_back() {
    let doc = project();
    let csv = text(
        &export(
            doc.project(),
            &index(&doc),
            LinkFormat::Csv,
            &ExportOptions::default(),
        )
        .files[0],
    );
    let moved = csv.replace("%IX4.3", "%IX9.3");
    let plan = plan_import(
        doc.project(),
        &index(&doc),
        &read(moved.as_bytes()).unwrap(),
    );
    assert!(plan.updates.is_empty());
    assert_eq!(plan.address_notes.len(), 1);
    assert!(plan.address_notes[0].contains("-K1 DI 3"));
    assert!(plan.address_notes[0].contains("%IX9.3") && plan.address_notes[0].contains("%IX4.3"));
}

#[test]
fn plcopen_has_one_file_per_station_with_escaped_comments() {
    let doc = project();
    let out = export(
        doc.project(),
        &index(&doc),
        LinkFormat::PlcOpen,
        &ExportOptions::default(),
    );
    assert_eq!(out.files.len(), 2);
    assert_eq!(out.files[1].suffix, "B");
    let xml = text(&out.files[0]);
    assert!(xml.contains("xmlns=\"http://www.plcopen.org/xml/tc6_0201\""));
    assert!(xml.contains("<variable name=\"Pump_1_running_2\" address=\"%IX4.3\">"));
    assert!(xml.contains("<type><BOOL/></type>"));
    assert!(xml.contains("<variable name=\"Level_tank_1\" address=\"%IW8\">"));
    assert!(xml.contains("<type><INT/></type>"));
    assert!(xml.contains("Horn &amp; beacon &lt;red&gt;"));
    let file = read(xml.as_bytes()).unwrap();
    assert_eq!(file.kind, FileKind::PlcOpen);
    assert_eq!(file.tags.len(), 4);
    let horn = file
        .tags
        .iter()
        .find(|t| t.name == "Horn_beacon_red")
        .unwrap();
    assert_eq!(horn.comment, "Horn & beacon <red>");
    assert_eq!(horn.address, "%QX0.0");
    assert_eq!(horn.data_type, "BOOL");
    // Station A's file holds station A's channels only, so its addresses
    // match even though station B starts at the same byte.
    let plan = plan_import(doc.project(), &index(&doc), &file);
    assert!(
        plan.updates.is_empty() && plan.unmatched.is_empty(),
        "{plan:?}"
    );
}

#[test]
fn an_address_in_two_stations_is_not_guessed() {
    let doc = project();
    let out = export(
        doc.project(),
        &index(&doc),
        LinkFormat::PlcOpen,
        &ExportOptions::default(),
    );
    // Station B's DI 0 is %IX0.0; station A has no %IX0.0 so it is clear. Make
    // the file claim %IX4.3 under a new name: one channel, no clash.
    let xml = text(&out.files[1]).replace("%IX0.0", "%IX4.3");
    let plan = plan_import(doc.project(), &index(&doc), &read(xml.as_bytes()).unwrap());
    assert_eq!(plan.updates.len(), 1);
    assert_eq!(plan.updates[0].channel, "-K1 DI 3");
}

#[test]
fn tia_table_is_excel_with_siemens_addresses() {
    let doc = project();
    let out = export(
        doc.project(),
        &index(&doc),
        LinkFormat::Tia,
        &ExportOptions::default(),
    );
    assert_eq!(out.files.len(), 2);
    assert_eq!(out.files[0].extension, "xlsx");
    let file = read(&out.files[0].bytes).unwrap();
    assert_eq!(file.kind, FileKind::Tia);
    let di3 = file
        .tags
        .iter()
        .find(|t| t.name == "Pump_1_running_2")
        .unwrap();
    assert_eq!(di3.address, "%I4.3");
    assert_eq!(di3.data_type, "Bool");
    let level = file.tags.iter().find(|t| t.name == "Level_tank_1").unwrap();
    assert_eq!(
        (level.address.as_str(), level.data_type.as_str()),
        ("%IW8", "Int")
    );
    let plan = plan_import(doc.project(), &index(&doc), &file);
    assert!(
        plan.updates.is_empty() && plan.unmatched.is_empty(),
        "{plan:?}"
    );
}

#[test]
fn studio_5000_aliases_point_at_the_module_data() {
    let doc = project();
    let out = export(
        doc.project(),
        &index(&doc),
        LinkFormat::Studio5000,
        &ExportOptions::default(),
    );
    let csv = text(&out.files[0]);
    assert!(csv.starts_with("remark,CSV-Import-Export\r\n"));
    assert!(csv.contains("TYPE,SCOPE,NAME,DESCRIPTION,DATATYPE,SPECIFIER,ATTRIBUTES\r\n"));
    assert!(csv.contains("ALIAS,,Pump_1_running_2,\"Pump 1 running\",,Local:2:I.Data.3,"));
    assert!(csv.contains("ALIAS,,Level_tank_1,\"Level tank 1\",,Local:3:I.Ch0Data,"));
    let file = read(csv.as_bytes()).unwrap();
    assert_eq!(file.kind, FileKind::Studio5000);
    assert_eq!(file.tags.len(), 4);
    let plan = plan_import(doc.project(), &index(&doc), &file);
    assert!(
        plan.updates.is_empty() && plan.unmatched.is_empty(),
        "{plan:?}"
    );
}

#[test]
fn studio_5000_names_fit_forty_characters() {
    let mut doc = project();
    let sym = doc.project().symbols().next().unwrap().id;
    doc.transact("Long", |tx| {
        tx.set_property(
            sym,
            SIGNAL,
            Some(Value::from(
                "Extremely long description of what this input actually does in the plant",
            )),
        )?;
        Ok(())
    })
    .unwrap();
    let out = export(
        doc.project(),
        &index(&doc),
        LinkFormat::Studio5000,
        &ExportOptions::default(),
    );
    for line in text(&out.files[0])
        .lines()
        .filter(|l| l.starts_with("ALIAS"))
    {
        let name = line.split(',').nth(2).unwrap();
        assert!(name.len() <= 40, "{name}");
    }
}

#[test]
fn unknown_addresses_and_spares_are_reported_not_applied() {
    let doc = project();
    let file = read(
        b"Name,Logical Address,Comment\nGhost,%IX9.9,Nothing here\nSpare_one,%IX4.0,Was spare\nNotAnAddress,hello,x\n",
    )
    .unwrap();
    assert_eq!(file.tags.len(), 3);
    let plan = plan_import(doc.project(), &index(&doc), &file);
    assert!(plan.updates.is_empty());
    assert_eq!(plan.unmatched.len(), 2);
    assert_eq!(
        plan.spare,
        [("-K1 DI 0".to_owned(), "Spare_one".to_owned())]
    );
}

#[test]
fn a_word_tag_on_a_bit_channel_is_a_warning() {
    let doc = project();
    let file = read(b"Name;Address;Data type\nX;%IW8;INT\nY;%IX4.3;INT\n").unwrap();
    let plan = plan_import(doc.project(), &index(&doc), &file);
    assert_eq!(plan.warnings.len(), 1, "{:?}", plan.warnings);
    assert!(plan.warnings[0].contains("type INT does not fit -K1 DI 3"));
}

#[test]
fn the_address_plan_lists_every_card() {
    let doc = project();
    let csv = hardware(doc.project(), &index(&doc));
    let lines: Vec<_> = csv.lines().collect();
    assert_eq!(lines.len(), 4, "{csv}");
    assert!(
        lines[1].contains(";2;-K1;DQ2 DI4;4;4;1;0;0;1;3;3"),
        "{}",
        lines[1]
    );
}

#[test]
fn not_a_tag_file() {
    assert!(read(b"hello world").is_err());
    assert!(read(b"<html></html>").is_err());
}

#[test]
fn a_real_rename_that_starts_the_same_is_not_swallowed() {
    let doc = project();
    let csv = text(
        &export(
            doc.project(),
            &index(&doc),
            LinkFormat::Csv,
            &ExportOptions::default(),
        )
        .files[0],
    );
    let renamed = csv.replace("Level_tank_1;INT", "Level;INT");
    let plan = plan_import(
        doc.project(),
        &index(&doc),
        &read(renamed.as_bytes()).unwrap(),
    );
    assert_eq!(plan.updates.len(), 1, "{plan:?}");
    assert_eq!(plan.updates[0].new_tag.as_deref(), Some("Level"));
}

#[test]
fn line_numbers_in_plcopen_follow_the_file() {
    let doc = project();
    let out = export(
        doc.project(),
        &index(&doc),
        LinkFormat::PlcOpen,
        &ExportOptions::default(),
    );
    let xml = text(&out.files[0]);
    let file = read(xml.as_bytes()).unwrap();
    for tag in &file.tags {
        let line = xml.lines().nth(tag.line - 1).unwrap();
        assert!(
            line.contains(&format!("name=\"{}\"", tag.name)),
            "{} -> {line}",
            tag.name
        );
    }
}
