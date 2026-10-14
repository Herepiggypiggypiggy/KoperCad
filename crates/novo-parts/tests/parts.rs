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
    let devices: Vec<DeviceId> = doc.project().devices().map(|d| d.id).collect();
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
        assert!(part.fits(&code) || code == "X", "{part} for -{code}");
        let command = assign_part(doc.project(), &[device], part, AssignMode::Replace);
        run(&mut doc, command);
    }
    assert!(missing_parts(doc.project()).is_empty());
    // The terminal strip gets terminal blocks, the motor a motor.
    let x1 = doc.project().device(gate.x1).unwrap();
    assert_eq!(
        doc.project().part(x1.parts[0]).unwrap().properties["letter_codes"],
        Value::Text("X".into())
    );
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
        Some("8 devices without a part")
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
