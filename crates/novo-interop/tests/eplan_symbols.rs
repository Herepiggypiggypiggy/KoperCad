//! The EPLAN symbol mapping table covers the KoperCAD library, round-trips,
//! and degrades to a generic box without losing pins.

use novo_interop::eplan::symbols::{
    Basis, EplanSymbol, SymbolKey, SymbolMapper, Table, Target, UserTable, generic_symbol,
};
use novo_interop::report::Report;
use novo_symbols::{Library, Pack, validate};

fn dev(function: &str, letters: &str, pins: &[&str]) -> EplanSymbol {
    EplanSymbol {
        key: None,
        function: (!function.is_empty()).then(|| function.to_string()),
        letters: (!letters.is_empty()).then(|| letters.to_string()),
        pins: pins.iter().map(|p| p.to_string()).collect(),
    }
}

#[test]
fn every_symbol_of_every_pack_has_a_row() {
    for pack in Pack::ALL {
        assert_eq!(
            SymbolMapper::new(pack).gaps(),
            Vec::<String>::new(),
            "{} symbols without an EPLAN mapping row; add them to table.toml",
            pack.id()
        );
    }
}

#[test]
fn every_row_names_a_real_symbol_with_valid_pin_aliases() {
    let table = Table::builtin();
    let lib = Library::shared(Pack::Iec);
    let mut seen = std::collections::HashSet::new();
    for e in &table.entries {
        assert!(seen.insert(&e.symbol), "{} is listed twice", e.symbol);
        let symbol = lib
            .get(&format!("iec/{}", e.symbol))
            .unwrap_or_else(|| panic!("{} is not a library symbol", e.symbol));
        assert!(!e.names.is_empty(), "{} has no names", e.symbol);
        assert!(
            e.names.iter().all(|n| !n.trim().is_empty()),
            "{} has an empty name",
            e.symbol
        );
        for l in &e.letters {
            assert!(
                l.len() == 1 && l.chars().all(|c| c.is_ascii_uppercase()),
                "{}: letter {l}",
                e.symbol
            );
        }
        if let Some(set) = &e.pins {
            let aliases = table
                .pinsets
                .get(set)
                .unwrap_or_else(|| panic!("{}: unknown pin set {set}", e.symbol));
            // An alias that targets a connection this symbol lacks is fine
            // (sets are shared); every set must hit at least one.
            assert!(
                aliases
                    .values()
                    .any(|t| symbol.connections.iter().any(|c| &c.id == t)),
                "{}: pin set {set} reaches no connection",
                e.symbol
            );
        }
    }
}

#[test]
fn every_symbol_round_trips_through_its_eplan_description() {
    for pack in Pack::ALL {
        let mapper = SymbolMapper::new(pack);
        let lib = Library::shared(pack);
        for symbol in lib.iter() {
            let out = mapper
                .to_eplan(&symbol.id)
                .unwrap_or_else(|| panic!("{} has no EPLAN description", symbol.id));
            assert_eq!(out.pins.len(), symbol.connections.len());
            let back = mapper.map(&EplanSymbol {
                key: None,
                function: Some(out.function.clone()),
                letters: out.letter.clone(),
                pins: out.pins.clone(),
            });
            assert_eq!(
                back.symbol_id(),
                Some(symbol.id.as_str()),
                "{} written as '{}' {:?} {:?} came back as {:?}",
                symbol.id,
                out.function,
                out.letter,
                out.pins,
                back.target
            );
            assert_eq!(back.basis, Basis::Name);
            assert!(back.pins.is_complete(), "{}: {:?}", symbol.id, back.pins);
            assert!(!back.pins.positional, "{}", symbol.id);
        }
    }
}

#[test]
fn every_name_in_the_table_finds_a_symbol_of_its_family() {
    // Not necessarily its own row (3 pole and 4 pole breakers share words),
    // but never a generic box and never a different kind of device.
    let table = Table::builtin();
    let mapper = SymbolMapper::new(Pack::Iec);
    let lib = Library::shared(Pack::Iec);
    for e in &table.entries {
        let want = lib.get(&format!("iec/{}", e.symbol)).unwrap();
        for name in &e.names {
            let r = mapper.map(&dev(name, "", &[]));
            let id = r
                .symbol_id()
                .unwrap_or_else(|| panic!("'{name}' ({}) became generic", e.symbol));
            let got = lib.get(id).unwrap();
            assert_eq!(
                got.function.split('.').next(),
                want.function.split('.').next(),
                "'{name}' ({}) mapped to {id}",
                e.symbol
            );
        }
    }
}

#[test]
fn pins_tell_pole_counts_apart() {
    let mapper = SymbolMapper::new(Pack::Iec);
    let pick = |pins: &[&str]| {
        mapper
            .map(&dev("Circuit breaker", "F", pins))
            .symbol_id()
            .map(str::to_string)
    };
    assert_eq!(pick(&["1", "2"]).as_deref(), Some("iec/protection/mcb_1p"));
    assert_eq!(
        pick(&["1", "2", "3", "4"]).as_deref(),
        Some("iec/protection/mcb_2p")
    );
    assert_eq!(
        pick(&["1", "2", "3", "4", "5", "6"]).as_deref(),
        Some("iec/protection/mcb_3p")
    );
    // No pins at all: the first row, no claim of certainty about poles.
    assert_eq!(pick(&[]).as_deref(), Some("iec/protection/mcb_1p"));
}

#[test]
fn power_pole_notation_maps_onto_numbered_pins() {
    let mapper = SymbolMapper::new(Pack::Iec);
    let r = mapper.map(&dev(
        "Hoofdcontacten",
        "K",
        &["L1", "T1", "L2", "T2", "L3", "T3"],
    ));
    assert_eq!(r.symbol_id(), Some("iec/contactors/main_3p"));
    assert_eq!(r.pins.connection("T2"), Some("4"));
    assert!(r.pins.is_complete());
}

#[test]
fn a_motor_with_a_second_pe_and_wire_colour_sensors_map() {
    let mapper = SymbolMapper::new(Pack::Iec);
    let drive = mapper.map(&dev(
        "Frequency converter",
        "T",
        &["L1", "L2", "L3", "PE", "U", "V", "W", "PE"],
    ));
    assert_eq!(drive.symbol_id(), Some("iec/drives/frequency_converter"));
    assert_eq!(drive.pins.connection("U"), Some("U"));
    assert!(drive.pins.is_complete(), "{:?}", drive.pins);
    let sensor = mapper.map(&dev("Inductieve sensor", "B", &["BN", "BU", "BK"]));
    assert_eq!(sensor.symbol_id(), Some("iec/sensors/inductive_pnp"));
    assert_eq!(sensor.pins.connection("BK"), Some("4"));
}

#[test]
fn tag_letters_and_pins_find_a_device_without_a_name() {
    let mapper = SymbolMapper::new(Pack::Iec);
    let r = mapper.map(&dev("", "K", &["13", "14"]));
    assert_eq!(r.symbol_id(), Some("iec/contacts/no"));
    assert_eq!(r.basis, Basis::Letters);
    let r = mapper.map(&dev("", "K", &["A1", "A2"]));
    assert_eq!(r.basis, Basis::Letters);
    assert!(r.symbol_id().unwrap().contains("coils/"));
    assert!(r.ambiguous, "relay and contactor coils both fit");
    // Letters without pins say nothing about the device.
    assert_eq!(mapper.map(&dev("", "K", &[])).basis, Basis::Generic);
}

#[test]
fn an_unknown_device_becomes_a_generic_box_that_keeps_its_pins() {
    let mapper = SymbolMapper::new(Pack::Iec);
    let d = dev(
        "Weighing cell amplifier",
        "A",
        &["1", "2", "PE", "S+", "S-"],
    );
    let r = mapper.map(&d);
    assert_eq!(r.basis, Basis::Generic);
    let Target::Generic(symbol) = &r.target else {
        panic!("expected a generic box");
    };
    assert_eq!(validate(symbol), vec![]);
    assert_eq!(symbol.connections.len(), 5);
    assert_eq!(symbol.name, "Weighing cell amplifier");
    for pin in &d.pins {
        assert!(r.pins.connection(pin).is_some(), "{pin} lost");
    }
}

#[test]
fn generic_boxes_are_valid_symbols_for_any_pin_count_and_duplicates() {
    for n in 0..=20 {
        let pins: Vec<String> = (0..n).map(|i| format!("{}", i % 7)).collect();
        let symbol = generic_symbol(&pins, "X");
        assert_eq!(validate(&symbol), vec![], "{n} pins");
        assert_eq!(symbol.connections.len(), n);
    }
    let blank = generic_symbol(&[" ".into(), "".into()], "");
    assert_eq!(validate(&blank), vec![]);
}

#[test]
fn the_report_counts_what_fell_back_and_what_lost_pins() {
    let mapper = SymbolMapper::new(Pack::Iec);
    let devices = vec![
        dev("Pushbutton", "S", &["13", "14"]),
        dev("Contactor coil", "K", &["A1", "A2"]),
        dev("Weighing cell amplifier", "A", &["1", "2"]),
        dev("Weighing cell amplifier", "A", &["1", "2"]),
        dev("Contactor coil", "K", &["A1", "A2", "A3"]),
    ];
    let (resolved, report) = mapper.map_all(&devices);
    assert_eq!(resolved.len(), 5);
    assert_eq!(report.total(), 5);
    assert_eq!(report.by_basis[&Basis::Name], 3);
    assert_eq!(report.by_basis[&Basis::Generic], 2);
    assert_eq!(report.unmapped.len(), 1);
    assert_eq!(report.unmapped[0].count, 2);
    assert_eq!(report.pin_issues.len(), 1);
    assert_eq!(report.pin_issues[0].pins, vec!["A3".to_string()]);

    let mut out = Report::default();
    report.write_to(&mut out);
    assert_eq!(out.count_of("devices carried as generic boxes"), 2);
    assert!(!out.is_lossless(), "the lost pin is reported");
    assert!(out.text().contains("Weighing cell amplifier"));
}

#[test]
fn the_users_symbol_table_is_learnt_saved_and_wins() {
    let mapper = SymbolMapper::new(Pack::Iec);
    let key = SymbolKey {
        set: 1,
        number: 4711,
        variant: 0,
    };
    let known = EplanSymbol {
        key: Some(key),
        function: Some("Schliesser".into()),
        letters: None,
        pins: vec!["13".into(), "14".into()],
    };
    let odd = EplanSymbol {
        key: Some(SymbolKey {
            set: 1,
            number: 99,
            variant: 2,
        }),
        function: Some("Weighing cell amplifier".into()),
        letters: None,
        pins: vec!["1".into()],
    };
    let (table, open) = mapper.learn([&known, &odd]);
    assert_eq!(table.len(), 1);
    assert_eq!(table.get(key), Some("contacts/no"));
    assert_eq!(open, vec![odd.clone()]);

    let saved = table.to_toml();
    let reloaded = UserTable::from_toml(&saved).unwrap();
    assert_eq!(reloaded, table);

    // The key now wins even when the name is useless, and in the other pack.
    for pack in Pack::ALL {
        let m = SymbolMapper::new(pack).with_user_table(reloaded.clone());
        let r = m.map(&EplanSymbol {
            key: Some(key),
            function: Some("???".into()),
            letters: None,
            pins: vec!["13".into(), "14".into()],
        });
        assert_eq!(r.basis, Basis::UserTable);
        assert_eq!(
            r.symbol_id(),
            Some(format!("{}/contacts/no", pack.id()).as_str())
        );
        // And the way back carries the user's own key.
        assert_eq!(m.to_eplan("contacts/no").unwrap().key, Some(key));
    }
    assert!(UserTable::new().is_empty(), "nothing ships in the table");
}

#[test]
fn a_users_own_symbol_is_written_from_its_own_text() {
    let mapper = SymbolMapper::new(Pack::Iec);
    let mut own = Library::shared(Pack::Iec)
        .get("iec/contacts/no")
        .unwrap()
        .clone();
    own.id = "acme/special/gate_contact".into();
    own.name = "Gate contact".into();
    let out = mapper.to_eplan_symbol(&own);
    assert!(out.generic);
    assert_eq!(out.function, "Gate contact");
    assert_eq!(out.pins, vec!["13".to_string(), "14".to_string()]);
    assert_eq!(out.letter.as_deref(), Some("K"));
    assert!(mapper.to_eplan("acme/special/gate_contact").is_none());
}

#[test]
fn text_matching_folds_umlauts_and_ignores_case_and_punctuation() {
    let mapper = SymbolMapper::new(Pack::Iec);
    for text in ["SCHÜTZSPULE", "Schütz-Spule", "  contactor   COIL, 3-pole "] {
        let r = mapper.map(&dev(text, "", &["A1", "A2"]));
        assert_eq!(r.symbol_id(), Some("iec/coils/contactor"), "{text}");
    }
    let r = mapper.map(&dev("Not-Aus", "S", &["11", "12"]));
    assert_eq!(r.symbol_id(), Some("iec/operators/emergency_stop"));
}

/// The devices a crane configurator project draws that used to become
/// boxes: each resolves to a symbol of KoperCAD's own.
#[test]
fn the_configurators_odd_devices_have_symbols_of_their_own() {
    let mapper = SymbolMapper::new(Pack::Iec);
    let id = |d: &EplanSymbol| {
        let r = mapper.map(d);
        assert_ne!(r.basis, Basis::Generic, "{}", d.label());
        r.symbol_id().unwrap().to_string()
    };
    assert_eq!(
        id(&dev("Device connection point", "", &["A1"])),
        "iec/connectivity/device_pin"
    );
    assert_eq!(
        id(&dev("Device connection point", "", &["13"])),
        "iec/connectivity/device_pin"
    );
    assert_eq!(
        id(&dev(
            "Insulated wire termination, general device",
            "",
            &["1"]
        )),
        "iec/connectivity/wire_end_insulated"
    );
    assert_eq!(
        id(&dev(
            "Terminal, general, 3 connection points",
            "X",
            &["1", "2", "3"]
        )),
        "iec/terminals/terminal_tap"
    );
    assert_eq!(
        id(&dev(
            "Female receptacle with PE, 5 connection points",
            "",
            &["L1", "L2", "L3", "N", "PE"]
        )),
        "iec/terminals/socket_3p_n_pe"
    );
    assert_eq!(
        id(&dev(
            "Switch, triple NO contact",
            "Q",
            &["1", "2", "3", "4", "5", "6"]
        )),
        "iec/switches/main_switch_3p"
    );
}

#[test]
fn the_variable_motor_resolves_by_its_pin_names_or_by_how_many_points_it_has() {
    let mapper = SymbolMapper::new(Pack::Iec);
    // Winding ends named 1U..2W plus earth: the 6 terminal motor, by name.
    let named = dev(
        "Motor with PE, variable",
        "M",
        &["1U", "1V", "1W", "2W", "2V", "2U", "PE"],
    );
    let r = mapper.map(&named);
    assert_eq!(r.symbol_id(), Some("iec/motors/motor_3ph_6t"));
    assert!(r.pins.is_complete() && !r.pins.positional);
    assert_eq!(r.pins.connection("1U"), Some("U1"));
    assert_eq!(r.pins.connection("2W"), Some("W2"));
    // No pin names at all: only the count tells the family apart.
    let bare = dev("Motor with PE, variable", "M", &[]);
    for (points, symbol) in [
        (4, "iec/motors/motor_3ph"),
        (6, "iec/motors/motor_3ph_pe_t2"),
        (7, "iec/motors/motor_3ph_6t"),
    ] {
        let r = mapper.map_fitting(&bare, points).unwrap();
        assert_eq!(r.symbol_id(), Some(symbol), "{points} points");
    }
    assert!(mapper.map_fitting(&bare, 5).is_none());
}

#[test]
fn a_family_by_point_count_never_falls_back_to_a_box() {
    let mapper = SymbolMapper::new(Pack::Iec);
    let female = dev("Female pin, 2 connection points", "X", &[]);
    assert_eq!(
        mapper.map_fitting(&female, 1).unwrap().symbol_id(),
        Some("iec/terminals/socket_contact")
    );
    let trafo = dev("Transformer, 5 connection points with PE", "T", &[]);
    assert_eq!(
        mapper.map_fitting(&trafo, 5).unwrap().symbol_id(),
        Some("iec/transformers/transformer_1ph_pe")
    );
    // A name nobody knows is not guessed.
    assert!(mapper.map_fitting(&dev("Black box", "U", &[]), 2).is_none());
}

#[test]
fn the_new_symbols_exist_in_both_packs_and_validate() {
    for pack in Pack::ALL {
        let lib = Library::shared(pack);
        for path in [
            "connectivity/device_pin",
            "connectivity/wire_end_insulated",
            "motors/motor_3ph_pe_t2",
            "terminals/plug_3p_n_pe",
            "terminals/plug_3p_pe",
            "terminals/plug_contact",
            "terminals/socket_3p_n_pe",
            "terminals/socket_3p_pe",
            "terminals/socket_contact",
            "terminals/terminal_tap",
            "transformers/transformer_1ph_pe",
        ] {
            let symbol = lib
                .get(&format!("{}/{path}", pack.id()))
                .unwrap_or_else(|| panic!("{path} missing in {}", pack.id()));
            assert_eq!(validate(symbol), vec![], "{}", symbol.id);
        }
    }
}
