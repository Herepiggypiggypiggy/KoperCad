//! "Assign parts to all devices" must never match a part of another kind
//! (hardware test 2026-10-28: a frequency converter was given a 24 V power
//! supply), and the generic sample parts must let a whole machine be
//! assigned.

use novo_core::{DeviceId, DeviceTag, Document, Pin, Placement, Point, SymbolRole, Value};
use novo_parts::{
    Catalog, CatalogPart, Confidence, DeviceNeed, Role, assign_choices, auto_assign_parts,
    conflict, part_role, propose_parts,
};
use novo_tests::{MotorStarter, motor_starter};

/// A device `tag` drawn with the library symbol `symbol`.
fn add(doc: &mut Document, page: novo_core::PageId, tag: &str, symbol: &str) -> DeviceId {
    let (id, _) = doc
        .transact("Add device", |tx| {
            let id = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
            tx.place_symbol(
                page,
                symbol,
                Placement::at(Point::new(200.0, 40.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Device {
                    device: id,
                    main: true,
                },
            )?;
            Ok(id)
        })
        .unwrap();
    id
}

fn set(doc: &mut Document, device: DeviceId, key: &str, value: Value) {
    doc.execute(novo_core::Command::SetProperty {
        target: device.into(),
        key: key.into(),
        value: Some(value),
    })
    .unwrap();
}

fn project() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    (doc, gate)
}

fn orders(catalog: &Catalog, doc: &Document, device: DeviceId) -> Vec<String> {
    propose_parts(catalog, doc.project(), Some(&[device]))
        .first()
        .map(|p| {
            p.candidates
                .iter()
                .map(|c| c.part.order_number.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_frequency_converter_is_never_given_a_power_supply() {
    let (mut doc, gate) = project();
    let t1 = add(
        &mut doc,
        gate.main_page,
        "-T1",
        "iec/drives/frequency_converter",
    );
    let catalog = Catalog::starter();
    // The 24 V supply carries the letter T too, as it did on the laptop.
    let psu = catalog.find("Phoenix Contact", "2904601").unwrap();
    assert!(psu.letter_codes.iter().any(|c| c == "T"));
    for order in orders(&catalog, &doc, t1) {
        let part = catalog.find_order_number(&order).unwrap();
        assert_eq!(part_role(part), Some(Role::Drive), "{order}");
    }
    // With only the supply in the catalog there is nothing to propose.
    let mut only_psu = Catalog::new();
    only_psu.upsert(psu.clone());
    assert!(propose_parts(&only_psu, doc.project(), Some(&[t1])).is_empty());
    assert!(auto_assign_parts(&only_psu, doc.project()).is_none());
}

#[test]
fn a_drive_is_matched_by_its_power() {
    let (mut doc, gate) = project();
    let t1 = add(
        &mut doc,
        gate.main_page,
        "-T1",
        "iec/drives/frequency_converter",
    );
    set(&mut doc, t1, "power_kw", Value::Float(4.0));
    let catalog = Catalog::starter();
    let proposals = propose_parts(&catalog, doc.project(), Some(&[t1]));
    let [only] = &proposals[..] else {
        panic!("one proposal");
    };
    assert_eq!(only.candidates.len(), 1, "{:?}", only.candidates);
    let best = only.default_choice().expect("a clear choice");
    assert_eq!(best.part.order_number, "GEN-VFD-4K0");
    assert_eq!(best.confidence, Confidence::Certain);
    assert!(
        best.reason.contains("frequency converter"),
        "{}",
        best.reason
    );
}

#[test]
fn a_network_port_has_no_part_of_its_own() {
    let (mut doc, gate) = project();
    let t2 = add(&mut doc, gate.main_page, "-T2", "iec/network/port_ethernet");
    let catalog = Catalog::starter();
    assert!(propose_parts(&catalog, doc.project(), Some(&[t2])).is_empty());
}

#[test]
fn several_certain_parts_are_not_taken_without_asking() {
    let (mut doc, gate) = project();
    let g1 = add(
        &mut doc,
        gate.main_page,
        "-G1",
        "iec/supplies/power_supply_dc",
    );
    let catalog = Catalog::starter();
    let proposals = propose_parts(&catalog, doc.project(), Some(&[g1]));
    let p = &proposals[0];
    assert!(
        p.candidates.len() >= 4,
        "Phoenix plus three generic supplies"
    );
    assert!(
        p.candidates
            .iter()
            .all(|c| c.confidence == Confidence::Certain)
    );
    assert!(p.default_choice().is_none());
}

#[test]
fn a_part_that_does_not_say_what_it_is_is_only_a_guess() {
    let (mut doc, gate) = project();
    let t1 = add(
        &mut doc,
        gate.main_page,
        "-T1",
        "iec/drives/frequency_converter",
    );
    let mut bare = Catalog::new();
    let mut part = CatalogPart::new("Acme", "X-1");
    part.letter_codes = vec!["T".into()];
    bare.upsert(part);
    let proposals = propose_parts(&bare, doc.project(), Some(&[t1]));
    let c = &proposals[0].candidates[0];
    assert_eq!(c.confidence, Confidence::Unsure);
    assert!(c.reason.contains("does not say"), "{}", c.reason);
    assert!(proposals[0].default_choice().is_none());
    assert!(auto_assign_parts(&bare, doc.project()).is_none());
}

#[test]
fn a_letter_only_catalog_still_assigns_the_one_clear_part() {
    let (mut doc, gate) = project();
    // A symbol that says nothing about the kind of device.
    let b1 = add(&mut doc, gate.main_page, "-B1", "iec/sensors/inductive");
    let mut bare = Catalog::new();
    let mut part = CatalogPart::new("Acme", "S-1");
    part.letter_codes = vec!["B".into()];
    bare.upsert(part);
    let proposals = propose_parts(&bare, doc.project(), Some(&[b1]));
    let c = proposals[0].default_choice().expect("the only part");
    assert_eq!(c.confidence, Confidence::Likely);
}

#[test]
fn a_coil_for_another_voltage_is_ruled_out() {
    let mut part = CatalogPart::new("Acme", "C-24");
    part.letter_codes = vec!["K".into()];
    part.keywords = vec!["contactor".into()];
    part.data
        .insert("coil_voltage".into(), Value::Text("24 V DC".into()));
    let need = DeviceNeed {
        letter_code: "K".into(),
        role: Some(Role::Contactor),
        coil: Some(novo_parts::Supply {
            volts: 230.0,
            ac: true,
            dc: false,
        }),
        poles: None,
    };
    let why = conflict(&part, &need).expect("ruled out");
    assert!(why.contains("24") && why.contains("230"), "{why}");
    let ok = DeviceNeed {
        coil: Some(novo_parts::Supply {
            volts: 24.0,
            ac: false,
            dc: true,
        }),
        ..need
    };
    assert!(conflict(&part, &ok).is_none());
}

#[test]
fn chosen_parts_are_assigned_in_one_undo_step() {
    let (mut doc, gate) = project();
    let g1 = add(
        &mut doc,
        gate.main_page,
        "-G1",
        "iec/supplies/power_supply_dc",
    );
    let t1 = add(
        &mut doc,
        gate.main_page,
        "-T1",
        "iec/drives/frequency_converter",
    );
    let catalog = Catalog::starter();
    let psu = catalog.find("Generic", "GEN-PSU-24V10A").unwrap();
    let vfd = catalog.find("Generic", "GEN-VFD-4K0").unwrap();
    let before = doc.project().parts().count();
    let depth = doc.undo_depth();
    let command = assign_choices(doc.project(), &[(g1, psu), (t1, vfd)]).unwrap();
    doc.execute(command).unwrap();
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(doc.project().parts().count(), before + 2);
    let part_of = |d| {
        let id = doc.project().device(d).unwrap().parts[0];
        doc.project().part(id).unwrap().order_number.clone()
    };
    assert_eq!(part_of(g1), "GEN-PSU-24V10A");
    assert_eq!(part_of(t1), "GEN-VFD-4K0");
    // Choosing what is already there changes nothing.
    assert!(assign_choices(doc.project(), &[(g1, psu)]).is_none());
}

#[test]
fn the_generic_parts_cover_a_cooling_tower_cell() {
    let catalog = Catalog::starter();
    let generic: Vec<&CatalogPart> = catalog
        .parts()
        .iter()
        .filter(|p| p.manufacturer == "Generic")
        .collect();
    for role in [
        Role::Drive,
        Role::PowerSupply,
        Role::PlcCpu,
        Role::PlcIo,
        Role::NetSwitch,
        Role::Disconnector,
        Role::Lamp,
        Role::Fan,
        Role::Vent,
        Role::Enclosure,
        Role::Motor,
    ] {
        assert!(
            generic.iter().any(|p| part_role(p) == Some(role)),
            "no generic {}",
            role.label()
        );
    }
    for p in generic {
        // Honest labelling: authored here, typical values, never a maker datasheet.
        let origin = p.provenance.as_ref().expect("origin recorded");
        assert!(origin.licence_note.contains("Not manufacturer data"), "{p}");
        assert!(p.description.contains("(generic)"), "{p}");
        assert!(
            p.dimensions.is_some() || part_role(p) == Some(Role::Motor),
            "{p}"
        );
        for (key, source) in &p.sources {
            assert_ne!(*source, novo_parts::DataSource::MakerDatasheet, "{p} {key}");
        }
    }
    // Typical values are labelled as such: the efficiency is the IE3 class
    // minimum, and the enclosure ambient is a convention.
    for p in catalog
        .parts()
        .iter()
        .filter(|p| p.manufacturer == "Generic")
    {
        for key in ["efficiency_percent", "max_ambient_c"] {
            if p.data.contains_key(key) && part_role(p) != Some(Role::Vent) {
                assert_eq!(
                    p.sources.get(key),
                    Some(&novo_parts::DataSource::Estimated),
                    "{p} {key}"
                );
            }
        }
    }
    // The named Siemens motor carries no values that are not from Siemens.
    let motor = catalog.find("Siemens", "1LE1003-0EB42-2AA4").unwrap();
    assert!(!motor.data.contains_key("efficiency_percent"));
    assert!(!motor.data.contains_key("rated_current_a"));
}

fn role_of(description: &str) -> Option<Role> {
    let mut p = CatalogPart::new("Acme", "R-1");
    p.description = description.into();
    part_role(&p)
}

#[test]
fn a_part_is_named_by_what_it_is_not_by_the_words_around_it() {
    assert_eq!(
        role_of("Ethernet switch for PLC panels, 5 ports"),
        Some(Role::NetSwitch)
    );
    assert_eq!(
        role_of("Signal lamp for PLC outputs, 24 V"),
        Some(Role::Lamp)
    );
    assert_eq!(role_of("PLC I/O module, 16 inputs"), Some(Role::PlcIo));
    assert_eq!(role_of("PLC CPU compact"), Some(Role::PlcCpu));
    assert_eq!(
        role_of("Three phase motor, fan-cooled, 4 kW"),
        Some(Role::Motor)
    );
    assert_eq!(role_of("Induction motor with fan cover"), Some(Role::Motor));
    assert_eq!(role_of("Axial fan 230 V AC"), Some(Role::Fan));
}

#[test]
fn proposals_for_a_big_catalog_stay_fast() {
    use std::time::Instant;
    let (mut doc, gate) = project();
    doc.transact("Many devices", |tx| {
        for i in 0..100 {
            tx.add_device(DeviceTag::parse(&format!("-K{}", 100 + i)).unwrap(), "")?;
        }
        Ok(())
    })
    .unwrap();
    let _ = gate;
    let mut catalog = Catalog::new();
    for i in 0..3000 {
        let mut p = CatalogPart::new("Acme", format!("C-{i}"));
        p.letter_codes = vec!["K".into()];
        p.description = format!("Contactor {i}, 3 pole, coil 24 V DC");
        catalog.upsert(p);
    }
    let start = Instant::now();
    let proposals = propose_parts(&catalog, doc.project(), None);
    let took = start.elapsed();
    assert!(proposals.len() >= 100);
    // The part facts are read once per part, not once per device and part
    // (that was 30 s for 20,000 parts and 300 devices).
    assert!(took.as_secs_f64() < 3.0, "{took:?}");
}

#[test]
fn a_real_maker_part_ranks_before_a_generic_one_at_equal_fit() {
    let mut cat = Catalog::new();
    for (maker, order) in [("Generic", "A-1"), ("Siemens", "B-1"), ("Acme", "C-1")] {
        let mut part = CatalogPart::new(maker, order);
        part.letter_codes = vec!["B".into()];
        cat.upsert(part);
    }
    let order: Vec<&str> = cat
        .suggest("B", "", 10)
        .into_iter()
        .map(|p| p.manufacturer.as_str())
        .collect();
    assert_eq!(order, ["Acme", "Siemens", "Generic"]);
}
