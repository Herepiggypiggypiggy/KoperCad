//! WS-07 acceptance: parts on the Gate 1 project, the searchable
//! properties panel (WORKFLOW.md J12), bulk edit and CSV import.

use novo_core::{AnyId, CableCore, DeviceId, DeviceTag, Document, Record, Value};
use novo_parts::{
    AssignMode, BulkTable, Catalog, CatalogPart, EditError, Group, PropertySheet, RowKey, RowValue,
    apply_table, assign_cable_part, assign_part, clear_parts, import_catalog, missing_parts,
    remove_unused_parts, suggest_for_device,
};
use novo_tests::{motor_starter, stress_page};

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn run(doc: &mut Document, command: Option<novo_core::Command>) {
    doc.execute(command.expect("a change"))
        .expect("command applies");
}

#[test]
fn starter_catalog_finds_parts_by_code_mnemonic_and_words() {
    let catalog = Catalog::starter();
    assert!(catalog.len() >= 25);
    // Order numbers match without dashes and case.
    let hit = catalog.search("3rt20161bb41", 5);
    assert_eq!(hit[0].order_number, "3RT2016-1BB41");
    // Mnemonics from WORKFLOW.md J2.
    assert!(
        catalog
            .search("mpcb", 10)
            .iter()
            .all(|p| p.keywords.contains(&"mpcb".into()))
    );
    assert_eq!(catalog.search("ol 7", 10)[0].letter_codes[0], "F");
    // Several words narrow the result.
    let hits = catalog.search("contactor 24vdc", 10);
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|p| p.description.contains("24 V DC")));
    assert!(catalog.search("", 10).is_empty());
    assert!(catalog.search("nothing-like-this", 10).is_empty());
}

#[test]
fn every_gate1_part_is_in_the_starter_catalog() {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    for part in doc.project().parts() {
        assert!(
            catalog
                .find(&part.manufacturer, &part.order_number)
                .is_some(),
            "{} {} missing from the starter catalog",
            part.manufacturer,
            part.order_number
        );
    }
    assert!(missing_parts(doc.project()).is_empty());
}

#[test]
fn gate1_devices_get_real_parts_from_suggestions() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    // -X1 has no part of its own: its terminal blocks are the strip style.
    let devices: Vec<DeviceId> = doc
        .project()
        .devices()
        .filter(|d| d.id != gate.x1)
        .map(|d| d.id)
        .collect();
    let command = clear_parts(doc.project(), &devices);
    run(&mut doc, command);
    let command = remove_unused_parts(doc.project());
    run(&mut doc, command);
    assert_eq!(missing_parts(doc.project()).len(), devices.len());

    for &device in &devices {
        let top = suggest_for_device(&catalog, doc.project(), device, "", 1);
        let part = top.first().expect("a suggestion for every Gate 1 device");
        let code = doc
            .project()
            .device(device)
            .unwrap()
            .tag
            .letter_code()
            .to_owned();
        assert!(part.fits(&code), "{part} for -{code}");
        let command = assign_part(doc.project(), &[device], part, AssignMode::Replace);
        run(&mut doc, command);
    }
    assert!(missing_parts(doc.project()).is_empty());
    // The terminal strip is offered terminal blocks (for its style).
    let blocks = suggest_for_device(&catalog, doc.project(), gate.x1, "", 3);
    assert!(!blocks.is_empty() && blocks.iter().all(|b| b.fits("X")));
    // Dimensions travel with the project copy for the cabinet layout.
    let k1 = doc.project().device(gate.k1).unwrap();
    let k1_part = doc.project().part(k1.parts[0]).unwrap();
    assert_eq!(k1_part.properties["width_mm"], Value::Float(45.0));
}

#[test]
fn assigning_one_part_to_many_devices_is_one_step_and_reuses_the_part() {
    let mut doc = Document::default();
    stress_page(&mut doc, 150).unwrap();
    let devices: Vec<DeviceId> = doc.project().devices().map(|d| d.id).collect();
    let catalog = Catalog::starter();
    let part = catalog.find("Siemens", "3RT2016-1BB41").unwrap();
    let depth = doc.undo_depth();
    let command = assign_part(doc.project(), &devices, part, AssignMode::Replace);
    run(&mut doc, command);
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(doc.project().parts().count(), 1);
    // Again: nothing to do.
    assert!(assign_part(doc.project(), &devices, part, AssignMode::Replace).is_none());
    // Adding an accessory keeps the main part.
    let aux = CatalogPart::new("Siemens", "3RH2911-1HA11");
    let command = assign_part(doc.project(), &devices[..1], &aux, AssignMode::Add);
    run(&mut doc, command);
    assert_eq!(doc.project().device(devices[0]).unwrap().parts.len(), 2);
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.project().parts().count(), 0);
}

#[test]
fn cable_part_names_cores_and_keeps_the_protective_conductor() {
    let mut doc = Document::default();
    let (cable, _) = doc
        .transact("cable", |tx| {
            tx.add_cable(
                tag("-W1"),
                ["1", "2", "3", "PE"].map(CableCore::new).to_vec(),
            )
        })
        .unwrap();
    let catalog = Catalog::starter();
    let five = catalog.find("Lapp", "1119305").unwrap();
    let command = assign_cable_part(doc.project(), cable, five);
    run(&mut doc, command);
    let names: Vec<String> = doc
        .project()
        .cable(cable)
        .unwrap()
        .cores
        .iter()
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(names, ["BU", "BN", "BK", "GNYE", "GY"]);
    let cable_rec = doc.project().cable(cable).unwrap();
    assert!(cable_rec.part.is_some());
    assert!(
        cable_rec
            .cores
            .iter()
            .all(|c| c.cross_section_mm2 == Some(1.5))
    );
}

#[test]
fn j12_any_property_is_found_by_part_of_its_name() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    let part = catalog.find("Siemens", "3RT2016-1BB41").unwrap();
    let command = assign_part(doc.project(), &[gate.k1], part, AssignMode::Replace);
    run(&mut doc, command);

    let sheet = PropertySheet::build(doc.project(), &[gate.k1.into()]);
    assert_eq!(sheet.title(), "=A1+CAB-K1");
    // Common fields first: tag, description, part.
    let first: Vec<&str> = sheet
        .rows()
        .iter()
        .take(3)
        .map(|r| r.label.as_str())
        .collect();
    assert_eq!(first, ["Tag", "Description", "Part"]);
    assert_eq!(sheet.rows()[0].group, Group::Common);
    // Names, synonyms and values all find rows.
    let labels =
        |q: &str| -> Vec<String> { sheet.filter(q).iter().map(|r| r.label.clone()).collect() };
    assert!(labels("order").contains(&"Part".to_owned()));
    assert!(labels("order").contains(&"Order number".to_owned()));
    assert!(labels("rated cur").contains(&"Rated current".to_owned()));
    assert!(labels("coil").contains(&"Coil voltage".to_owned()));
    assert!(labels("24 V").contains(&"Coil voltage".to_owned()));
    assert!(labels("manufacturer").contains(&"Manufacturer".to_owned()));
    assert_eq!(
        sheet.best_match("desc").unwrap().key,
        RowKey::Field("description".into())
    );
    // The part's identity is read-only; the part is changed by picking
    // another.
    let order = sheet
        .row(&RowKey::PartField("order_number".into()))
        .unwrap();
    assert!(matches!(
        sheet.edit(doc.project(), &order.key, "x"),
        Err(EditError::ReadOnly(_))
    ));
    assert_eq!(
        sheet.edit(doc.project(), &RowKey::Part, "x"),
        Err(EditError::UsePartSearch)
    );

    // A symbol selection shows its device.
    let sheet = PropertySheet::build(doc.project(), &[gate.k1_coil.into(), gate.k1_main.into()]);
    assert_eq!(sheet.targets(), &[AnyId::from(gate.k1)]);

    // A wire finds its cross-section under "cross" and "section".
    let wire = gate.motor_wires[0];
    let sheet = PropertySheet::build(doc.project(), &[wire.into()]);
    assert!(
        sheet
            .filter("cross")
            .iter()
            .any(|r| r.label == "Cross-section")
    );
    assert!(
        sheet
            .filter("doorsnede")
            .iter()
            .any(|r| r.label == "Cross-section")
    );
    let command = sheet
        .edit(
            doc.project(),
            &RowKey::Field("cross_section_mm2".into()),
            "2,5",
        )
        .unwrap();
    run(&mut doc, command);
    assert_eq!(
        doc.project().wire(wire).unwrap().cross_section_mm2,
        Some(2.5)
    );
    let sheet = PropertySheet::build(doc.project(), &[wire.into()]);
    assert!(matches!(
        sheet.edit(
            doc.project(),
            &RowKey::Field("cross_section_mm2".into()),
            "thick"
        ),
        Err(EditError::NotANumber { .. })
    ));
}

#[test]
fn j12_editing_50_devices_is_one_change_and_one_undo_step() {
    let mut doc = Document::default();
    stress_page(&mut doc, 150).unwrap();
    let selection: Vec<AnyId> = doc.project().devices().map(|d| d.id.into()).collect();
    assert_eq!(selection.len(), 50);
    let sheet = PropertySheet::build(doc.project(), &selection);
    assert_eq!(sheet.title(), "50 devices");

    let depth = doc.undo_depth();
    let command = sheet
        .edit(
            doc.project(),
            &RowKey::Field("description".into()),
            "Pump contactor",
        )
        .unwrap();
    run(&mut doc, command);
    assert_eq!(doc.undo_depth(), depth + 1);
    assert!(
        doc.project()
            .devices()
            .all(|d| d.description == "Pump contactor")
    );

    let sheet = PropertySheet::build(doc.project(), &selection);
    let command = sheet
        .add_property(doc.project(), "Rated power kW", "4,5")
        .unwrap();
    run(&mut doc, command);
    assert!(
        doc.project()
            .devices()
            .all(|d| d.properties["rated_power_kw"] == Value::Float(4.5))
    );
    assert_eq!(doc.undo_depth(), depth + 2);

    doc.undo().unwrap();
    doc.undo().unwrap();
    assert!(doc.project().devices().all(|d| d.description.is_empty()));
}

#[test]
fn mixed_values_and_typed_properties() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let sheet = PropertySheet::build(doc.project(), &[gate.s0.into(), gate.s1.into()]);
    assert_eq!(sheet.title(), "2 devices");
    let description = sheet.row(&RowKey::Field("description".into())).unwrap();
    assert_eq!(description.value, RowValue::Mixed(2));

    // The motor's power keeps its type: typing 3 stays a decimal number.
    let sheet = PropertySheet::build(doc.project(), &[gate.m1.into()]);
    let power = sheet.row(&RowKey::Property("power_kw".into())).unwrap();
    assert_eq!(power.unit, Some("kW"));
    assert_eq!(power.label, "Power");
    let command = sheet.edit(doc.project(), &power.key, "3").unwrap();
    run(&mut doc, command);
    assert_eq!(
        doc.project().device(gate.m1).unwrap().properties["power_kw"],
        Value::Float(3.0)
    );
    // Clearing a property removes it.
    let sheet = PropertySheet::build(doc.project(), &[gate.m1.into()]);
    let command = sheet.edit(doc.project(), &power.key, "").unwrap();
    run(&mut doc, command);
    assert!(
        !doc.project()
            .device(gate.m1)
            .unwrap()
            .properties
            .contains_key("power_kw")
    );
}

#[test]
fn tags_are_checked_before_they_change() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let sheet = PropertySheet::build(doc.project(), &[gate.k1.into()]);
    let tag_key = RowKey::Field("tag".into());
    assert_eq!(
        sheet.validate(doc.project(), &tag_key, "-F2"),
        Err(EditError::TagInUse {
            tag: "=A1+CAB-F2".into()
        })
    );
    assert!(matches!(
        sheet.validate(doc.project(), &tag_key, "K 1"),
        Err(EditError::InvalidTag(_))
    ));
    // Typing only the product aspect keeps function and location (J4).
    let command = sheet.edit(doc.project(), &tag_key, "-K10").unwrap();
    run(&mut doc, command);
    assert_eq!(
        doc.project().device(gate.k1).unwrap().tag,
        tag("=A1+CAB-K10")
    );
    // Several devices cannot share a new tag.
    let sheet = PropertySheet::build(doc.project(), &[gate.s0.into(), gate.s1.into()]);
    assert_eq!(
        sheet.edit(doc.project(), &tag_key, "-S5"),
        Err(EditError::TagNeedsOne)
    );
}

#[test]
fn fixed_lists_accept_their_words() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let sheet = PropertySheet::build(doc.project(), &[gate.pe.into()]);
    let kind = sheet.row(&RowKey::Field("kind".into())).unwrap();
    assert!(kind.choices.contains(&"protective_earth"));
    let command = sheet.edit(doc.project(), &kind.key, "Neutral").unwrap();
    run(&mut doc, command);
    assert!(matches!(
        doc.project().record(gate.pe.into()),
        Some(Record::Potential(p)) if p.kind == novo_core::PotentialKind::Neutral
    ));
    let sheet = PropertySheet::build(doc.project(), &[gate.pe.into()]);
    let err = sheet.edit(doc.project(), &kind.key, "purple").unwrap_err();
    assert!(err.to_string().contains("choose line, neutral"), "{err}");
}

#[test]
fn bulk_table_round_trips_through_excel_text() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    let table = BulkTable::devices(doc.project(), None);
    let tsv = table.to_tsv();
    let header = tsv.lines().next().unwrap();
    assert!(header.starts_with("Tag\tDescription\tManufacturer\tOrder number\tType number"));
    assert!(header.contains("power_kw"));
    // Unchanged text changes nothing.
    assert_eq!(
        apply_table(doc.project(), &catalog, &tsv).unwrap().changed,
        0
    );

    // Edit as a user would in Excel: a new description, a bigger contactor
    // by order number alone, a property, and a line for a missing device.
    let edited: String = tsv
        .lines()
        .map(|line| {
            if line.starts_with("=A1+CAB-K1\t") {
                let mut cells: Vec<&str> = line.split('\t').collect();
                cells[1] = "Main contactor";
                cells[2] = "";
                cells[3] = "3RT2017-1BB41";
                cells.join("\t")
            } else {
                line.to_owned()
            }
        })
        .chain(["-K99\tGhost".to_owned()])
        .collect::<Vec<_>>()
        .join("\n");
    let outcome = apply_table(doc.project(), &catalog, &edited).unwrap();
    assert_eq!(outcome.changed, 1);
    assert_eq!(outcome.skipped.len(), 1);
    let parts_before = doc.project().parts().count();
    let depth = doc.undo_depth();
    let command = outcome.command;
    run(&mut doc, command);
    assert_eq!(doc.undo_depth(), depth + 1);
    let k1 = doc.project().device(gate.k1).unwrap();
    assert_eq!(k1.description, "Main contactor");
    let part = doc.project().part(k1.parts[0]).unwrap();
    assert_eq!(part.order_number, "3RT2017-1BB41");
    assert_eq!(part.manufacturer, "Siemens");
    assert_eq!(doc.project().parts().count(), parts_before + 1);

    // A paste with only Tag and one property column, `;`-separated with a
    // decimal comma, as Dutch or German Excel writes it.
    let paste = "Tag;Power [kW]\n-M1;4,0\n";
    let command = apply_table(doc.project(), &catalog, paste).unwrap().command;
    run(&mut doc, command);
    assert_eq!(
        doc.project().device(gate.m1).unwrap().properties["power_kw"],
        Value::Float(4.0)
    );
    assert!(apply_table(doc.project(), &catalog, "Description\nx").is_err());
}

#[test]
fn csv_import_reads_a_german_price_list() {
    let csv = "\u{feff}Hersteller;Bestellnummer;Typ;Beschreibung;Kennbuchstabe;Breite;Höhe;Tiefe;Bemessungsstrom A\n\
               Eaton;199263;DILM9-10(24VDC);Schütz 9 A, 24 V DC;K;45;68;87,5;9\n\
               ;276852;;Ohne Hersteller;;;;;\n\
               Eaton;;;Keine Nummer;;;;;\n";
    let report = import_catalog(csv, "Unknown").unwrap();
    assert_eq!(report.parts.len(), 2);
    assert_eq!(report.skipped, vec![(4, "no order number".to_owned())]);
    assert_eq!(report.data_columns, vec!["Bemessungsstrom A".to_owned()]);
    let dilm = &report.parts[0];
    assert_eq!(dilm.manufacturer, "Eaton");
    assert_eq!(dilm.type_number, "DILM9-10(24VDC)");
    assert_eq!(dilm.letter_codes, ["K"]);
    assert_eq!(dilm.dimensions.unwrap().depth_mm, 87.5);
    assert_eq!(dilm.data["bemessungsstrom_a"], Value::Int(9));
    assert_eq!(report.parts[1].manufacturer, "Unknown");

    let mut catalog = Catalog::starter();
    let before = catalog.len();
    assert_eq!(catalog.merge(report.parts.into_iter().collect()), 2);
    assert_eq!(catalog.len(), before + 2);
    assert_eq!(catalog.search("dilm9", 3)[0].order_number, "199263");
    assert!(import_catalog("Name;Price\nx;1", "").is_err());
}

#[test]
fn catalog_file_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("NovoElectric").join("parts.json");
    let catalog = Catalog::starter();
    catalog.save(&path).unwrap();
    let loaded = Catalog::load(&path).unwrap();
    assert_eq!(loaded.len(), catalog.len());
    for part in catalog.parts() {
        assert_eq!(loaded.get(&part.key()), Some(part));
    }
    // A project copy converts back to the same catalog entry.
    let part = catalog.find("Siemens", "3RV2011-1JA10").unwrap();
    let model = part.to_model(novo_core::PartId(7));
    let mut back = CatalogPart::from_model(&model);
    back.keywords = part.keywords.clone();
    back.sources = part.sources.clone();
    assert_eq!(&back, part);
    // A newer format is refused, not misread.
    std::fs::write(&path, r#"{"format": 99, "parts": []}"#).unwrap();
    assert!(Catalog::load(&path).is_err());
}

#[test]
fn empty_optional_numbers_take_whole_and_decimal_values() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let terminal = doc.project().terminals().next().unwrap().id;
    let sheet = PropertySheet::build(doc.project(), &[terminal.into()]);
    let command = sheet
        .edit(doc.project(), &RowKey::Field("jumper".into()), "2")
        .unwrap();
    run(&mut doc, command);
    assert_eq!(doc.project().terminal(terminal).unwrap().jumper, Some(2));
    let wire = gate.motor_wires[1];
    let sheet = PropertySheet::build(doc.project(), &[wire.into()]);
    let key = RowKey::Field("number".into());
    let command = sheet.edit(doc.project(), &key, "101").unwrap();
    run(&mut doc, command);
    assert_eq!(
        doc.project().wire(wire).unwrap().number.as_deref(),
        Some("101")
    );
    let sheet = PropertySheet::build(doc.project(), &[wire.into()]);
    let command = sheet.edit(doc.project(), &key, "").unwrap();
    run(&mut doc, command);
    assert_eq!(doc.project().wire(wire).unwrap().number, None);
}

#[test]
fn part_data_can_be_overridden_and_reset() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    let part = catalog.find("Siemens", "3RT2016-1BB41").unwrap();
    let command = assign_part(doc.project(), &[gate.k1], part, AssignMode::Replace);
    run(&mut doc, command);

    let sheet = PropertySheet::build(doc.project(), &[gate.k1.into()]);
    let rated = sheet.filter("rated current")[0].clone();
    assert_eq!(rated.group, Group::PartData);
    assert_eq!(rated.unit, Some("A"));
    // Editing the part's value overrides it on this device only.
    let command = sheet.edit(doc.project(), &rated.key, "12").unwrap();
    run(&mut doc, command);
    let k1 = doc.project().device(gate.k1).unwrap();
    assert_eq!(k1.properties["rated_current_a"], Value::Float(12.0));
    assert_eq!(
        doc.project().part(k1.parts[0]).unwrap().properties["rated_current_a"],
        Value::Float(9.0)
    );
    let sheet = PropertySheet::build(doc.project(), &[gate.k1.into()]);
    let own = sheet
        .row(&RowKey::Property("rated_current_a".into()))
        .unwrap();
    assert!(own.overrides_part);
    // An ordinary property is not an override.
    let sheet_m1 = PropertySheet::build(doc.project(), &[gate.m1.into()]);
    assert!(
        !sheet_m1
            .row(&RowKey::Property("power_kw".into()))
            .unwrap()
            .overrides_part
    );

    let command = sheet.reset_to_part(doc.project(), "rated_current_a");
    run(&mut doc, command);
    assert!(
        !doc.project()
            .device(gate.k1)
            .unwrap()
            .properties
            .contains_key("rated_current_a")
    );
    let sheet = PropertySheet::build(doc.project(), &[gate.k1.into()]);
    assert!(
        sheet
            .reset_to_part(doc.project(), "rated_current_a")
            .is_none()
    );
}

#[test]
fn fill_down_sets_one_column_on_many_devices_in_one_step() {
    let mut doc = Document::default();
    stress_page(&mut doc, 30).unwrap();
    let devices: Vec<DeviceId> = doc.project().devices().map(|d| d.id).collect();
    let catalog = Catalog::starter();
    let depth = doc.undo_depth();
    let command = novo_parts::fill(
        doc.project(),
        &catalog,
        &devices,
        &novo_parts::TableColumn::OrderNumber,
        "3RT2016-1BB41",
    )
    .unwrap();
    run(&mut doc, command);
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(doc.project().parts().count(), 1);
    assert!(missing_parts(doc.project()).is_empty());
    assert!(
        novo_parts::fill(
            doc.project(),
            &catalog,
            &devices,
            &novo_parts::TableColumn::Tag,
            "x"
        )
        .is_err()
    );
}

#[test]
fn import_preview_shows_what_would_change() {
    let catalog = Catalog::starter();
    let mut changed = catalog.find("Siemens", "3RV2011-1JA10").unwrap().clone();
    changed.description = "Updated text".into();
    let same = catalog.find("Lapp", "1119304").unwrap().clone();
    let new = CatalogPart::new("Eaton", "199263");
    let preview = catalog.preview_merge(&[changed.clone(), same, new.clone()]);
    assert_eq!(preview.added, vec![new.key()]);
    assert_eq!(preview.changed, vec![changed.key()]);
    assert_eq!(preview.unchanged, 1);
}

#[test]
fn certain_parts_are_assigned_and_the_rest_proposed() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    let devices: Vec<DeviceId> = doc.project().devices().map(|d| d.id).collect();
    let command = clear_parts(doc.project(), &devices);
    run(&mut doc, command);
    assert_eq!(
        novo_parts::missing_parts_summary(doc.project()).as_deref(),
        Some("7 devices without a part")
    );

    // The 2.2 kW motor has exactly one fitting part; a contactor has several.
    let proposals = novo_parts::propose_parts(&catalog, doc.project(), None);
    let of = |id| proposals.iter().find(|p| p.device == id).unwrap();
    assert_eq!(of(gate.m1).candidates.len(), 1);
    assert!(of(gate.k1).candidates.len() > 1);

    // A device's own sizing narrows the choice: 8 A fits one 3RU2116 range.
    let command = PropertySheet::build(doc.project(), &[gate.f2.into()])
        .add_property(doc.project(), "current_a", "8")
        .unwrap();
    run(&mut doc, command);
    let proposals = novo_parts::propose_parts(&catalog, doc.project(), Some(&[gate.f2]));
    let orders: Vec<&str> = proposals[0]
        .candidates
        .iter()
        .map(|p| p.order_number.as_str())
        .collect();
    assert!(orders.contains(&"3RU2116-1JB0"));
    assert!(!orders.contains(&"3RU2116-1FB0"));

    let depth = doc.undo_depth();
    let command = novo_parts::auto_assign_parts(&catalog, doc.project());
    run(&mut doc, command);
    assert_eq!(doc.undo_depth(), depth + 1);
    let m1 = doc.project().device(gate.m1).unwrap();
    assert_eq!(
        doc.project().part(m1.parts[0]).unwrap().order_number,
        "1LE1003-0EB42-2AA4"
    );
    assert!(doc.project().device(gate.k1).unwrap().parts.is_empty());
}

fn set_property(doc: &mut Document, target: DeviceId, key: &str, value: Value) {
    doc.execute(novo_core::Command::SetProperty {
        target: target.into(),
        key: key.into(),
        value: Some(value),
    })
    .unwrap();
}

#[test]
fn bulk_table_property_named_like_a_column_round_trips() {
    // Review of ea67da2, item 1: `text` read back as Description and
    // `article` as Order number, so an unchanged paste rewrote 8 devices.
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    set_property(&mut doc, gate.k1, "text", Value::Text("hello".into()));
    set_property(&mut doc, gate.m1, "article", Value::Text("12345".into()));
    for key in ["name", "order", "model", "type", "manufacturer", "tag"] {
        set_property(&mut doc, gate.k1, key, Value::Text(format!("{key} value")));
    }
    let before = doc.project().clone();
    let catalog = Catalog::starter();
    let tsv = BulkTable::devices(doc.project(), None).to_tsv();
    let header = tsv.lines().next().unwrap();
    assert!(header.contains("\tproperties.text"), "{header}");
    assert!(header.contains("\tproperties.article"), "{header}");
    let outcome = apply_table(doc.project(), &catalog, &tsv).unwrap();
    assert_eq!(outcome.changed, 0);
    assert!(outcome.command.is_none());
    assert!(outcome.skipped.is_empty());

    // Editing a prefixed property column edits that property only.
    let edited = tsv.replace("\thello\t", "\tbye\t");
    assert_ne!(edited, tsv);
    let command = apply_table(doc.project(), &catalog, &edited)
        .unwrap()
        .command;
    run(&mut doc, command);
    let k1 = doc.project().device(gate.k1).unwrap();
    assert_eq!(k1.properties["text"], Value::Text("bye".into()));
    let k1_before = before.device(gate.k1).unwrap();
    assert_eq!(k1.description, k1_before.description);
    assert_eq!(k1.parts, k1_before.parts);
    let m1 = doc.project().device(gate.m1).unwrap();
    assert_eq!(m1.parts, before.device(gate.m1).unwrap().parts);
    assert_eq!(doc.project().parts().count(), before.parts().count());
}

#[test]
fn bulk_paste_with_two_columns_for_one_field_is_refused() {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    // `Text` is a Description header; `Article` an Order number header.
    for paste in [
        "Tag\tDescription\tText\n-K1\ta\tb\n",
        "Tag\tOrder number\tArticle\n-K1\t3RT2016-1BB41\t12345\n",
        "Tag\tproperties.power_kw\tproperties.power_kw\n-M1\t1\t2\n",
        "Tag\tBMK\n-K1\t-K1\n",
    ] {
        let err = apply_table(doc.project(), &catalog, paste).unwrap_err();
        assert!(
            matches!(err, novo_parts::BulkError::DuplicateColumn { .. }),
            "{paste:?}: {err}"
        );
    }
}

#[test]
fn bulk_paste_with_another_function_or_location_does_not_edit_a_namesake() {
    // Review of ea67da2, item 2: `=ZZ9+ELSEWHERE-K1` edited `=A1+CAB-K1`.
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    let project = doc.project();
    assert_eq!(novo_parts::find_device(project, "=ZZ9+ELSEWHERE-K1"), None);
    assert_eq!(novo_parts::find_device(project, "=ZZ9-K1"), None);
    assert_eq!(novo_parts::find_device(project, "+ELSEWHERE-K1"), None);
    assert_eq!(
        novo_parts::find_device(project, "=A1+CAB-K1"),
        Some(gate.k1)
    );
    // Only the product aspect still finds the one device with it.
    assert_eq!(novo_parts::find_device(project, "-K1"), Some(gate.k1));

    let outcome = apply_table(
        project,
        &catalog,
        "Tag\tDescription\n=ZZ9+ELSEWHERE-K1\tWrong device\n",
    )
    .unwrap();
    assert_eq!(outcome.changed, 0);
    assert!(outcome.command.is_none());
    assert_eq!(outcome.skipped.len(), 1);
    assert!(outcome.skipped[0].1.contains("=ZZ9+ELSEWHERE-K1"));
}

#[test]
fn bulk_table_keeps_tabs_line_breaks_and_quotes_in_cells() {
    // Review of ea67da2, item 3: the TSV turned tabs and line breaks into
    // spaces, so an unchanged paste rewrote multi-line descriptions.
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let mut k1 = doc.project().device(gate.k1).unwrap().clone();
    let description = "Main contactor\nline 2\twith a tab, \"quoted\"\r\n24\" rack ";
    k1.description = description.into();
    doc.execute(novo_core::Command::Replace {
        record: Record::Device(k1),
    })
    .unwrap();
    set_property(
        &mut doc,
        gate.m1,
        "note",
        Value::Text("\"starts quoted\" then".into()),
    );
    let catalog = Catalog::starter();
    let tsv = BulkTable::devices(doc.project(), None).to_tsv();
    let outcome = apply_table(doc.project(), &catalog, &tsv).unwrap();
    assert_eq!(outcome.changed, 0, "unchanged paste of\n{tsv}");

    // Every row still has one field per column.
    let rows: Vec<Vec<String>> = novo_parts::parse_delimited(&tsv)
        .unwrap()
        .into_iter()
        .map(|r| r.fields)
        .collect();
    assert_eq!(rows.len(), doc.project().devices().count() + 1);
    assert!(rows.iter().all(|r| r.len() == rows[0].len()));
    let k1_row = rows.iter().find(|r| r[0] == "=A1+CAB-K1").unwrap();
    assert_eq!(k1_row[1], description);

    // Editing a quoted cell on another row keeps the description intact.
    let edited = tsv.replace("\"\"\"starts quoted\"\" then\"", "\"\"\"x\"\" then\"");
    assert_ne!(edited, tsv);
    let command = apply_table(doc.project(), &catalog, &edited)
        .unwrap()
        .command;
    run(&mut doc, command);
    assert_eq!(
        doc.project().device(gate.m1).unwrap().properties["note"],
        Value::Text("\"x\" then".into())
    );
    assert_eq!(
        doc.project().device(gate.k1).unwrap().description,
        description
    );
}

#[test]
fn texts_have_no_part_and_terminals_show_theirs() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let created = doc
        .execute(novo_core::Command::AddText {
            page: gate.main_page,
            content: novo_core::TextContent::Literal {
                text: "Main supply".into(),
            },
            placement: Default::default(),
            attached_to: None,
        })
        .unwrap()
        .created;
    let [AnyId::Text(note)] = created[..] else {
        panic!("{created:?}")
    };
    let sheet = PropertySheet::build(doc.project(), &[note.into()]);
    assert_eq!(sheet.title(), "Text Main supply");
    assert!(sheet.row(&RowKey::Part).is_none());
    let sheet = PropertySheet::build(doc.project(), &[note.into(), gate.k1.into()]);
    assert_eq!(sheet.title(), "2 objects");
    assert!(sheet.row(&RowKey::Part).is_none());

    // A terminal carries its own article (novo-core #35).
    let k1_part = doc.project().device(gate.k1).unwrap().parts[0];
    let mut terminal = doc.project().terminals().next().unwrap().clone();
    let sheet = PropertySheet::build(doc.project(), &[terminal.id.into()]);
    assert_eq!(
        sheet.row(&RowKey::Part).map(|r| &r.value),
        Some(&RowValue::Same(String::new()))
    );
    terminal.part = Some(k1_part);
    let id = terminal.id;
    doc.execute(novo_core::Command::Replace {
        record: Record::Terminal(terminal),
    })
    .unwrap();
    let sheet = PropertySheet::build(doc.project(), &[id.into()]);
    let RowValue::Same(label) = &sheet.row(&RowKey::Part).unwrap().value else {
        panic!("one part")
    };
    assert!(label.starts_with("Siemens 3RT2016"), "{label}");
    // A part used only by a terminal is not unused.
    let devices: Vec<DeviceId> = doc.project().devices().map(|d| d.id).collect();
    let command = clear_parts(doc.project(), &devices);
    run(&mut doc, command);
    assert!(!novo_parts::unused_parts(doc.project()).contains(&k1_part));
}

#[test]
fn property_labels_handle_non_ascii_keys() {
    // U+212A KELVIN SIGN lowercases to a 1-byte `k`; slicing the original
    // key at an offset from the lowercased one used to panic.
    assert_eq!(
        novo_parts::property_label("temp_\u{212A}_a"),
        ("Temp \u{212A}".to_string(), Some("A"))
    );
    assert_eq!(
        novo_parts::property_label("\u{212A}"),
        ("\u{212A}".to_string(), None)
    );
    assert_eq!(
        novo_parts::property_label("rated_current_A"),
        ("Rated current".to_string(), Some("A"))
    );
}

// ----- Follow-ups from the #36 reviews and terminal parts (#35) ------------

fn k1_part(doc: &Document, gate: &novo_tests::MotorStarter) -> novo_core::Part {
    let k1 = doc.project().device(gate.k1).unwrap();
    doc.project().part(k1.parts[0]).unwrap().clone()
}

#[test]
fn part_cells_change_only_through_the_order_number() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    let part = k1_part(&doc, &gate);

    // A changed Type number with the same part is reported, not dropped.
    let paste = format!(
        "Tag\tManufacturer\tOrder number\tType number\n-K1\t{}\t{}\tSomething else\n",
        part.manufacturer, part.order_number
    );
    let outcome = apply_table(doc.project(), &catalog, &paste).unwrap();
    assert!(outcome.command.is_none());
    assert_eq!(outcome.ignored.len(), 1);
    assert_eq!(outcome.ignored[0].0, 2);
    assert!(outcome.ignored[0].1.contains("Type number"));

    // So is a Manufacturer change without an Order number column.
    let outcome = apply_table(doc.project(), &catalog, "Tag\tManufacturer\n-K1\tABB\n").unwrap();
    assert!(outcome.command.is_none());
    assert_eq!(outcome.ignored.len(), 1);

    // An unchanged Order number written differently changes nothing.
    let paste = format!(
        "Tag\tOrder number\n-K1\t{}\n",
        part.order_number.to_lowercase()
    );
    let outcome = apply_table(doc.project(), &catalog, &paste).unwrap();
    assert!(outcome.command.is_none());
    assert!(outcome.ignored.is_empty());

    // One cell or a fill of Manufacturer or Type number is refused.
    for column in [
        novo_parts::TableColumn::Manufacturer,
        novo_parts::TableColumn::TypeNumber,
    ] {
        let err = novo_parts::set_cell(doc.project(), &catalog, gate.k1, &column, "x").unwrap_err();
        assert!(matches!(err, novo_parts::BulkError::PartColumnAlone(_)));
        let err = novo_parts::fill(doc.project(), &catalog, &[gate.k1], &column, "x").unwrap_err();
        assert!(matches!(err, novo_parts::BulkError::PartColumnAlone(_)));
    }

    // A new Order number still picks another part.
    let other = catalog
        .suggest("K", "", 50)
        .into_iter()
        .find(|p| p.order_number != part.order_number)
        .unwrap()
        .clone();
    let paste = format!(
        "Tag\tManufacturer\tOrder number\n-K1\t{}\t{}\n",
        other.manufacturer, other.order_number
    );
    let command = apply_table(doc.project(), &catalog, &paste)
        .unwrap()
        .command;
    run(&mut doc, command);
    assert_eq!(k1_part(&doc, &gate).order_number, other.order_number);
}

#[test]
fn two_rows_for_one_device_keep_the_first() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    let paste = "Tag\tDescription\n-K1\tfirst\n=A1+CAB-K1\tsecond\n";
    let outcome = apply_table(doc.project(), &catalog, paste).unwrap();
    assert_eq!(outcome.changed, 1);
    assert_eq!(outcome.skipped.len(), 1);
    assert_eq!(outcome.skipped[0].0, 3);
    assert!(
        outcome.skipped[0].1.contains("line 2"),
        "{:?}",
        outcome.skipped
    );
    run(&mut doc, outcome.command);
    assert_eq!(doc.project().device(gate.k1).unwrap().description, "first");
}

#[test]
fn property_headers_use_the_project_key_or_the_normal_form() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    set_property(&mut doc, gate.k1, "rated_current_a", Value::Int(9));
    let paste = "Tag\tproperties.Rated Current A\tproperties.Coil Voltage\n-K1\t12\t24 VDC\n";
    let command = apply_table(doc.project(), &catalog, paste).unwrap().command;
    run(&mut doc, command);
    let k1 = doc.project().device(gate.k1).unwrap();
    assert_eq!(k1.properties["rated_current_a"], Value::Int(12));
    assert_eq!(k1.properties["coil_voltage"], Value::Text("24 VDC".into()));
    assert!(!k1.properties.contains_key("Rated Current A"));
    assert!(!k1.properties.contains_key("Coil Voltage"));
    // Two headers that mean one property are refused.
    let err = apply_table(
        doc.project(),
        &catalog,
        "Tag\tproperties.coil_voltage\tproperties.Coil Voltage\n-K1\ta\tb\n",
    )
    .unwrap_err();
    assert!(matches!(err, novo_parts::BulkError::DuplicateColumn { .. }));
}

#[test]
fn terminals_get_their_own_part_and_are_listed_when_missing() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    // The -X1 strip style covers its terminals.
    assert!(missing_parts(doc.project()).is_empty());
    let terminals: Vec<novo_core::TerminalId> = doc
        .project()
        .terminals_of_strip(gate.x1)
        .iter()
        .map(|t| t.id)
        .collect();
    clear_strip_style(&mut doc, gate.x1);
    let expected: Vec<AnyId> = terminals.iter().map(|&t| AnyId::from(t)).collect();
    assert_eq!(missing_parts(doc.project()), expected);

    // Each terminal gets its own article in one step.
    let block = suggest_for_device(&catalog, doc.project(), gate.x1, "", 1)[0].clone();
    let depth = doc.undo_depth();
    let command = novo_parts::assign_terminal_part(doc.project(), &terminals, &block);
    run(&mut doc, command);
    assert_eq!(doc.undo_depth(), depth + 1);
    let p = doc.project();
    let id = p.terminal(terminals[0]).unwrap().part.unwrap();
    assert_eq!(p.part(id).unwrap().order_number, block.order_number);
    for &t in &terminals {
        assert_eq!(novo_parts::terminal_part(p, t), Some(id));
    }
    assert!(missing_parts(p).is_empty());
    assert!(!novo_parts::unused_parts(p).contains(&id));
    // Assigning it again changes nothing.
    assert!(novo_parts::assign_terminal_part(p, &terminals, &block).is_none());
}

#[test]
fn a_strip_style_part_covers_its_kind_and_is_not_unused() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    let block = suggest_for_device(&catalog, doc.project(), gate.x1, "", 1)[0].clone();
    // Without the strip style the block is in the project but unused.
    clear_strip_style(&mut doc, gate.x1);
    let id = novo_parts::find_project_part(doc.project(), &block.key()).unwrap();
    assert!(novo_parts::unused_parts(doc.project()).contains(&id));

    // The strip editor (novo-terminals) makes it the feed-through style.
    doc.transact("Style", |tx| {
        novo_terminals::set_strip_style(
            tx,
            gate.x1,
            novo_core::TerminalKind::Feedthrough,
            &novo_terminals::Style {
                symbol: novo_core::SymbolKey(
                    novo_terminals::default_symbol(novo_core::TerminalKind::Feedthrough).into(),
                ),
                part: Some(id),
            },
        )
    })
    .unwrap();
    let p = doc.project();
    assert!(!novo_parts::unused_parts(p).contains(&id));
    assert!(novo_parts::remove_unused_parts(p).is_none_or(|c| {
        match c {
            novo_core::Command::Batch { commands, .. } => !commands.iter().any(
                |c| matches!(c, novo_core::Command::Remove { id: r } if *r == AnyId::from(id)),
            ),
            _ => true,
        }
    }));
    // Terminals 1 to 3 are feed-through and covered; PE is not.
    let terminals = p.terminals_of_strip(gate.x1);
    for t in &terminals[..3] {
        assert_eq!(novo_parts::terminal_part(p, t.id), Some(id));
    }
    let pe = terminals[3].id;
    assert_eq!(missing_parts(p), vec![AnyId::from(pe)]);
    // The strip device's own part list (end plates, accessories) is never
    // a terminal's article.
    let mut x1 = p.device(gate.x1).unwrap().clone();
    x1.parts = vec![id];
    run(
        &mut doc,
        Some(novo_core::Command::Replace {
            record: Record::Device(x1),
        }),
    );
    let p = doc.project();
    assert_eq!(novo_parts::terminal_part(p, pe), None);
    assert_eq!(missing_parts(p), vec![AnyId::from(pe)]);
}

/// Removes the strip's terminal articles per kind (its style), as the
/// strip editor does.
fn clear_strip_style(doc: &mut Document, strip: DeviceId) {
    use novo_core::TerminalKind;
    doc.transact("Clear style", |tx| {
        for kind in [TerminalKind::Feedthrough, TerminalKind::ProtectiveEarth] {
            let style = novo_terminals::Style {
                symbol: novo_core::SymbolKey(novo_terminals::default_symbol(kind).into()),
                part: None,
            };
            novo_terminals::set_strip_style(tx, strip, kind, &style)?;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn bulk_paste_reports_what_it_cannot_read() {
    // Follow-ups from the #36 reviews: typed cells, Excel formulas, file
    // line numbers and unclosed quotes.
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    set_property(&mut doc, gate.k1, "spare", Value::Bool(false));
    set_property(&mut doc, gate.m1, "power_kw", Value::Float(4.0));
    let catalog = Catalog::starter();
    let project = doc.project();

    // `maybe` in a yes/no cell and `fast` in a number cell are reported,
    // and the values keep their type.
    let outcome = apply_table(
        project,
        &catalog,
        "Tag\tproperties.spare\tproperties.power_kw\n-K1\tmaybe\t\n-M1\t\tfast\n",
    )
    .unwrap();
    let notes: Vec<usize> = outcome.ignored.iter().map(|(line, _)| *line).collect();
    assert_eq!(notes, vec![2, 3], "{:?}", outcome.ignored);
    assert!(outcome.ignored[0].1.contains("yes or no"));
    assert!(outcome.ignored[1].1.contains("a number"));

    // A tag Excel turned into a formula is skipped with advice; a tag
    // with Excel's text apostrophe still finds its device. Lines count in
    // the pasted text, past blank lines and multi-line cells.
    let outcome = apply_table(
        project,
        &catalog,
        "Tag\tDescription\n#NAME?\tx\n\n'-K1\t\"two\nlines\"\nnone\ty\n",
    )
    .unwrap();
    assert_eq!(outcome.changed, 1);
    let skipped: Vec<usize> = outcome.skipped.iter().map(|(line, _)| *line).collect();
    assert_eq!(skipped, vec![2, 6], "{:?}", outcome.skipped);
    assert!(
        outcome.skipped[0]
            .1
            .contains("format the Tag column as Text")
    );

    let err =
        apply_table(project, &catalog, "Tag\tDescription\n-K1\t\"open\n-M1\tx\n").unwrap_err();
    assert_eq!(
        err,
        novo_parts::BulkError::Text(novo_parts::ParseError::UnterminatedQuote { line: 2 })
    );
}

#[test]
fn a_terminal_block_given_to_a_strip_is_ordered_per_terminal() {
    // Tester finding F4: the strip's top suggestion (UT 2,5) landed on the
    // strip device, so the BOM ordered one block for four terminals while
    // missing parts called every terminal covered.
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    clear_strip_style(&mut doc, gate.x1);
    let block = suggest_for_device(&catalog, doc.project(), gate.x1, "", 1)[0].clone();
    assert_eq!(block.order_number, "3044076");
    assert_eq!(
        novo_parts::terminal_kind_of(&block),
        novo_core::TerminalKind::Feedthrough
    );
    let depth = doc.undo_depth();
    let command = assign_part(doc.project(), &[gate.x1], &block, AssignMode::Replace);
    run(&mut doc, command);
    assert_eq!(doc.undo_depth(), depth + 1);
    let p = doc.project();
    assert!(p.device(gate.x1).unwrap().parts.is_empty());
    let terminals = p.terminals_of_strip(gate.x1);
    let id = novo_parts::terminal_part(p, terminals[0].id).unwrap();
    assert_eq!(p.part(id).unwrap().order_number, "3044076");
    for t in &terminals[..3] {
        assert_eq!(novo_parts::terminal_part(p, t.id), Some(id));
    }
    // PE is not a feed-through terminal: it is still missing its block.
    let pe = terminals[3].id;
    assert_eq!(missing_parts(p), vec![AnyId::from(pe)]);
    // Assigning the same block again changes nothing.
    assert!(assign_part(p, &[gate.x1], &block, AssignMode::Replace).is_none());

    let pe_block = suggest_for_device(&catalog, p, gate.x1, "pe", 1)[0].clone();
    assert_eq!(pe_block.order_number, "3044089");
    let command = assign_part(p, &[gate.x1], &pe_block, AssignMode::Replace);
    run(&mut doc, command);
    let p = doc.project();
    assert_eq!(
        p.part(novo_parts::terminal_part(p, pe).unwrap())
            .unwrap()
            .order_number,
        "3044089"
    );
    assert!(missing_parts(p).is_empty());
}

#[test]
fn a_pe_terminal_is_offered_a_pe_block_first_and_accessories_stay_on_the_strip() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    let p = doc.project();
    let terminals = p.terminals_of_strip(gate.x1);
    let first = |t: &novo_core::Terminal| {
        novo_parts::suggest_for_terminal(&catalog, p, t.id, "", 1)[0]
            .order_number
            .clone()
    };
    assert_eq!(first(terminals[0]), "3044076");
    assert_eq!(first(terminals[3]), "3044089");

    // An end cover is coded X but connects nothing: it is an accessory of
    // the strip device, not a terminal article.
    let mut cover = CatalogPart::new("Phoenix Contact", "3047028");
    cover.description = "End cover".into();
    cover.letter_codes = vec!["X".into()];
    assert!(!novo_parts::is_terminal_block(&cover));
    // A block pasted without a connections column is still a block, and
    // an explicit 0 connections marks an accessory whatever its name.
    let mut pasted = CatalogPart::new("Phoenix Contact", "3044102");
    pasted.description = "Feed-through terminal block UT 4".into();
    pasted.letter_codes = vec!["X".into()];
    assert!(novo_parts::is_terminal_block(&pasted));
    for zero in [Value::Int(0), Value::Float(0.0), Value::Text("0".into())] {
        pasted.data.insert("connections".into(), zero);
        assert!(!novo_parts::is_terminal_block(&pasted));
    }
    pasted
        .data
        .insert("connections".into(), Value::Text("2".into()));
    assert!(novo_parts::is_terminal_block(&pasted));
    // Accessory words after "with" or "for" describe a block's features.
    pasted.data.clear();
    for description in [
        "Feed-through terminal block with marker groove",
        "Terminal block for plug-in bridge, with end plate",
        "Durchgangsreihenklemme mit Brückenschacht",
    ] {
        pasted.description = description.into();
        assert!(novo_parts::is_terminal_block(&pasted), "{description}");
    }
    for description in [
        "End cover for UT 2,5",
        "Plug-in bridge, 2-pole",
        "Zack marker strip",
    ] {
        pasted.description = description.into();
        assert!(!novo_parts::is_terminal_block(&pasted), "{description}");
    }
    let command = assign_part(p, &[gate.x1], &cover, AssignMode::Add);
    run(&mut doc, command);
    let p = doc.project();
    let x1 = p.device(gate.x1).unwrap();
    assert_eq!(x1.parts.len(), 1);
    assert_eq!(p.part(x1.parts[0]).unwrap().order_number, "3047028");
    // The terminals keep their blocks.
    let t = p.terminals_of_strip(gate.x1)[0].id;
    assert_eq!(
        p.part(novo_parts::terminal_part(p, t).unwrap())
            .unwrap()
            .order_number,
        "3044076"
    );
}

#[test]
fn internal_properties_are_not_shown_in_the_sheet_or_the_bulk_table() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    set_property(
        &mut doc,
        gate.x1,
        "terminal_part.feed_through",
        Value::Int(1),
    );
    set_property(&mut doc, gate.x1, "terminal.suggest", Value::Bool(true));
    set_property(&mut doc, gate.x1, "colour", Value::Text("grey".into()));
    let sheet = PropertySheet::build(doc.project(), &[gate.x1.into()]);
    let properties: Vec<&RowKey> = sheet
        .rows()
        .iter()
        .filter(|r| r.group == Group::Properties)
        .map(|r| &r.key)
        .collect();
    assert_eq!(properties, [&RowKey::Property("colour".into())]);

    let tsv = BulkTable::devices(doc.project(), Some(&[gate.x1])).to_tsv();
    let header = tsv.lines().next().unwrap();
    assert!(header.contains("colour"), "{header}");
    assert!(!header.contains("terminal"), "{header}");
    // Pasting the table back leaves the hidden properties alone.
    let catalog = Catalog::starter();
    let outcome = apply_table(doc.project(), &catalog, &tsv).unwrap();
    assert!(outcome.command.is_none());
    let x1 = doc.project().device(gate.x1).unwrap();
    assert_eq!(x1.properties["terminal_part.feed_through"], Value::Int(1));
}

#[test]
fn pe_blocks_are_recognised_by_prefix_description_and_colour() {
    use novo_core::TerminalKind;
    let block = |type_number: &str, description: &str| {
        let mut p = CatalogPart::new("Maker", type_number);
        p.type_number = type_number.into();
        p.description = description.into();
        p.letter_codes = vec!["X".into()];
        novo_parts::terminal_kind_of(&p)
    };
    let pe = TerminalKind::ProtectiveEarth;
    let ft = TerminalKind::Feedthrough;
    assert_eq!(
        block("UT 2,5-PE", "Protective conductor terminal block"),
        pe
    );
    assert_eq!(block("WPE 2.5", "PE terminal"), pe);
    assert_eq!(block("WPE 2.5", ""), pe);
    assert_eq!(
        block(
            "2002-1207",
            "2-conductor ground terminal block, green-yellow"
        ),
        pe
    );
    assert_eq!(block("2002-1207", "Schutzleiterklemme gn/ye"), pe);
    assert_eq!(block("UT 2,5", "Feed-through terminal block, grey"), ft);
    assert_eq!(
        block("WDU 2.5", "Feed-through terminal with marker groove"),
        ft
    );
    assert_eq!(block("TYPE 4", "Open terminal"), ft);
    assert_eq!(
        block("UT 4", "Feed-through terminal block with protective cover"),
        ft
    );
    assert_eq!(block("UT 4", "Terminal block for PE/N distribution"), ft);
    assert_eq!(block("UT 4", "Terminal block, for PE"), ft);
}

#[test]
fn panel_data_is_editable_in_the_part_rows_and_checked() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let catalog = Catalog::starter();
    let part = catalog.find("Siemens", "3RT2016-1BB41").unwrap();
    let command = assign_part(doc.project(), &[gate.k1], part, AssignMode::Replace);
    run(&mut doc, command);
    let target = AnyId::from(gate.k1);

    // The panel keys are listed under the part's data: the catalogue's
    // planning value for the loss, empty for a key the part does not give.
    let sheet = PropertySheet::build(doc.project(), &[target]);
    let loss = sheet.row(&RowKey::PartData("power_loss_w".into())).unwrap();
    assert_eq!(loss.value, RowValue::Same("2.5".into()));
    assert_eq!(loss.unit, Some("W"));
    assert!(loss.editable);
    let mounting = sheet.row(&RowKey::PartData("mounting".into())).unwrap();
    assert!(mounting.choices.contains(&"din_rail"));

    // Typed values are stored on the device (an override of the part's)
    // and read back through PanelData.
    for (key, text) in [
        ("power_loss_w", "4,5"),
        ("mounting", "DIN rail"),
        ("clearance_above_mm", "30"),
    ] {
        let sheet = PropertySheet::build(doc.project(), &[target]);
        let command = sheet
            .edit(doc.project(), &RowKey::PartData(key.into()), text)
            .unwrap();
        run(&mut doc, command);
    }
    let k1 = doc.project().device(gate.k1).unwrap();
    assert_eq!(k1.properties["mounting"], Value::Text("din_rail".into()));
    let data = novo_parts::PanelData::of_device(
        &k1.properties,
        doc.project().part(k1.parts[0]).map(|p| &p.properties),
    );
    assert_eq!(data.power_loss_w, Some(4.5));
    assert_eq!(data.mounting, Some(novo_parts::Mounting::DinRail));
    assert_eq!(data.clearances.above_mm, Some(30.0));
    assert_eq!(data.clearances.below_mm, None);

    // Nonsense is refused with a reason.
    let sheet = PropertySheet::build(doc.project(), &[target]);
    let err = |key: &str, text: &str| {
        sheet
            .edit(doc.project(), &RowKey::PartData(key.into()), text)
            .unwrap_err()
    };
    assert!(matches!(
        err("power_loss_w", "-3"),
        EditError::Invalid { .. }
    ));
    assert!(matches!(
        err("clearance_left_mm", "wide"),
        EditError::NotANumber { .. }
    ));
    assert!(matches!(err("mounting", "glue"), EditError::Invalid { .. }));
}

#[test]
fn a_format_1_catalog_is_migrated_on_load_and_saved_as_format_2() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("parts.json");
    std::fs::write(
        &path,
        r#"{"format": 1, "parts": [{"manufacturer": "ABB", "order_number": "X1",
            "data": {"power_dissipation_w": 12.5, "mounting_type": "Hutschiene"}}]}"#,
    )
    .unwrap();
    let catalog = Catalog::load(&path).unwrap();
    let part = catalog.find("ABB", "X1").unwrap();
    assert_eq!(part.data["power_loss_w"], Value::Float(12.5));
    assert_eq!(part.data["mounting"], Value::Text("din_rail".into()));
    assert!(!part.data.contains_key("power_dissipation_w"));
    catalog.save(&path).unwrap();
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("\"format\": 2")
    );
}

#[test]
fn csv_import_maps_maker_headers_to_panel_keys() {
    let report = import_catalog(
        "Bestellnummer;Verlustleistung [W];Montage\nA1;3,2;Schraube\n",
        "ABB",
    )
    .unwrap();
    let data = novo_parts::PanelData::of_properties(&report.parts[0].data);
    assert_eq!(data.power_loss_w, Some(3.2));
    assert_eq!(data.mounting, Some(novo_parts::Mounting::Screw));
}

#[test]
fn a_mixed_field_becomes_shared_after_one_edit_and_mixed_again_on_undo() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let selection: Vec<AnyId> = vec![gate.s0.into(), gate.s1.into()];
    let key = RowKey::Field("description".into());
    let sheet = PropertySheet::build(doc.project(), &selection);
    assert_eq!(sheet.row(&key).unwrap().value, RowValue::Mixed(2));
    // A mixed field has no single value to show, only the count.
    assert_eq!(sheet.row(&key).unwrap().value.same(), None);

    let depth = doc.undo_depth();
    let command = sheet.edit(doc.project(), &key, "Switch").unwrap();
    run(&mut doc, command);
    assert_eq!(doc.undo_depth(), depth + 1);
    let sheet = PropertySheet::build(doc.project(), &selection);
    assert_eq!(sheet.row(&key).unwrap().value.same(), Some("Switch"));

    doc.undo().unwrap();
    let sheet = PropertySheet::build(doc.project(), &selection);
    assert_eq!(sheet.row(&key).unwrap().value, RowValue::Mixed(2));
}

/// A part whose text states an AC-3 current has the same number as its AC-3
/// current data, so the label and the input the calculations read agree
/// (audit C-49).
#[test]
fn starter_catalog_ac3_text_and_data_agree() {
    let catalog = Catalog::starter();
    let mut checked = 0;
    for part in catalog.parts() {
        let Some(rest) = part.description.split("AC-3 ").nth(1) else {
            continue;
        };
        let stated: f64 = rest
            .split_whitespace()
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("{}: no AC-3 number in the text", part.order_number));
        let data = match part.data.get("ac3_current_a") {
            Some(Value::Float(v)) => *v,
            other => panic!("{}: ac3_current_a is {other:?}", part.order_number),
        };
        assert_eq!(data, stated, "{}", part.order_number);
        checked += 1;
    }
    assert!(checked >= 5, "the starter catalog has AC-3 contactors");
}
