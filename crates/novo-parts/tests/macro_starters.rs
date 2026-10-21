use novo_core::{BlockDefinitionId, Document, Point, Project, Record, Value};
use novo_parts::{Imported, MacroLibrary, starter_macros};
use novo_symbols::Library;

#[test]
fn starters_use_real_symbols_with_matching_pins() {
    let lib = Library::builtin().unwrap();
    for (_, file) in starter_macros() {
        let mut symbols = 0;
        for member in &file.definition.members {
            let Record::Symbol(s) = member else { continue };
            symbols += 1;
            let sym = lib
                .get(s.symbol.0.as_str())
                .unwrap_or_else(|| panic!("{}: unknown symbol {}", file.name, s.symbol.0));
            assert_eq!(
                s.pins.len(),
                sym.connections.len(),
                "{} {}",
                file.name,
                s.symbol.0
            );
            for (pin, c) in s.pins.iter().zip(&sym.connections) {
                assert_eq!(pin.name, c.id, "{} {}", file.name, s.symbol.0);
                assert_eq!(
                    (pin.offset.x, pin.offset.y),
                    (c.at[0], c.at[1]),
                    "{} {} {}",
                    file.name,
                    s.symbol.0,
                    c.id
                );
            }
        }
        assert!(symbols >= 3, "{} has {symbols} symbols", file.name);
    }
}

#[test]
fn every_starter_places_with_new_tags_and_filled_parameters() {
    for (_, file) in starter_macros() {
        let mut doc = Document::new(Project::new(""));
        doc.transact("place twice", |tx| {
            let page = tx.add_page("1")?;
            let mut definition = file.definition.clone();
            definition.id = BlockDefinitionId(9_000_000);
            let def = definition.id;
            tx.insert_record(Record::BlockDefinition(definition))?;
            tx.place_block(def, page, Point::new(0.0, 0.0), vec![])?;
            tx.place_block(def, page, Point::new(300.0, 0.0), vec![])?;
            Ok(())
        })
        .unwrap_or_else(|e| panic!("{}: {e}", file.name));
        let tags: Vec<String> = doc.project().devices().map(|d| d.tag.to_string()).collect();
        let unique: std::collections::BTreeSet<_> = tags.iter().collect();
        assert_eq!(
            unique.len(),
            tags.len(),
            "{}: tags repeat: {tags:?}",
            file.name
        );
    }
}

#[test]
fn motor_power_parameter_fills_the_description() {
    let (_, file) = starter_macros()
        .into_iter()
        .find(|(_, f)| f.name == "Motor starter DOL")
        .unwrap();
    assert_eq!(file.definition.parameters[0].default, Value::Float(2.2));
    assert!(
        file.definition
            .members
            .iter()
            .any(|m| matches!(m, Record::Device(d) if d.description == "Motor {power} kW"))
    );
}

#[test]
fn installing_starters_is_idempotent_and_keeps_user_edits() {
    let dir = tempfile::tempdir().unwrap();
    let lib = MacroLibrary::new(dir.path().join("Macros"), "user");
    let first = lib.install_starters().unwrap();
    assert!(first.iter().all(|r| matches!(r, Imported::Added(_))));
    assert_eq!(first.len(), 15);
    let p = "Starter/E-stop chain.kcmacro";
    let mut edited = lib.load(p).unwrap();
    edited.description = "mine".into();
    lib.replace(p, &edited).unwrap();
    let second = lib.install_starters().unwrap();
    assert!(second.iter().all(|r| matches!(r, Imported::Skipped(_))));
    assert_eq!(lib.load(p).unwrap().description, "mine");
    assert_eq!(lib.search("star delta").len(), 1, "words match apart");
    assert_eq!(lib.search("star-delta").len(), 1);
    assert!(lib.search("24 V").len() >= 2);
}

fn starter(name: &str) -> novo_parts::MacroFile {
    starter_macros()
        .into_iter()
        .map(|(_, f)| f)
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no starter {name}"))
}

fn count(file: &novo_parts::MacroFile, key: &str) -> usize {
    file.definition
        .members
        .iter()
        .filter(|m| matches!(m, Record::Symbol(s) if s.symbol.0.as_str() == key))
        .count()
}

#[test]
fn there_are_fifteen_distinct_starters_and_each_one_is_wired() {
    let all = starter_macros();
    assert_eq!(all.len(), 15);
    let names: std::collections::BTreeSet<_> = all.iter().map(|(_, f)| f.name.clone()).collect();
    assert_eq!(names.len(), 15, "names repeat");
    for (_, f) in &all {
        let wires = f
            .definition
            .members
            .iter()
            .filter(|m| matches!(m, Record::Wire(_)))
            .count();
        assert!(wires >= 2, "{} has {wires} wires", f.name);
        assert!(
            !f.description.is_empty() && !f.tags.is_empty(),
            "{}",
            f.name
        );
    }
}

#[test]
fn reversing_starter_swaps_two_phases_and_interlocks_both_ways() {
    let f = starter("Motor starter reversing");
    assert_eq!(count(&f, "iec/contactors/main_3p"), 2);
    assert_eq!(count(&f, "iec/coils/contactor"), 2);
    assert_eq!(count(&f, "iec/contacts/nc"), 2, "one interlock each way");
    assert_eq!(count(&f, "iec/contacts/no"), 2, "one self-hold each");
}

#[test]
fn crane_hoist_has_limits_brake_and_estop() {
    let f = starter("Crane hoist up/down");
    assert_eq!(count(&f, "iec/contacts/position_nc"), 2);
    assert_eq!(count(&f, "iec/coils/brake"), 1);
    assert_eq!(count(&f, "iec/operators/emergency_stop"), 1);
    assert_eq!(count(&f, "iec/contacts/no"), 2, "brake released by either");
}

#[test]
fn drives_and_circuits_are_found_by_their_everyday_words() {
    let dir = tempfile::tempdir().unwrap();
    let lib = MacroLibrary::new(dir.path().join("Macros"), "user");
    lib.install_starters().unwrap();
    for (word, name) in [
        ("reversing", "Motor starter reversing"),
        ("vfd", "Frequency converter drive"),
        ("soft starter", "Soft starter with bypass"),
        ("brake", "Motor with brake"),
        ("hoist", "Crane hoist up/down"),
        ("float", "Pump with float switch"),
        ("thermostat", "Heater with thermostat"),
        ("plc", "PLC input card"),
        ("latch", "Start/stop latch"),
        ("rcbo", "Single-phase supply with 24 V"),
        ("indication", "Contactor with indication"),
    ] {
        let hits = lib.search(word);
        assert!(
            hits.iter().any(|h| h.name == name),
            "{word} does not find {name}"
        );
    }
}

#[test]
fn no_starter_has_a_diagonal_wire_even_after_placing() {
    for (_, file) in starter_macros() {
        let mut doc = Document::new(Project::new(""));
        doc.transact("place", |tx| {
            let page = tx.add_page("1")?;
            let mut definition = file.definition.clone();
            definition.id = BlockDefinitionId(9_000_000);
            let def = definition.id;
            tx.insert_record(Record::BlockDefinition(definition))?;
            tx.place_block(def, page, Point::new(50.0, 70.0), vec![])?;
            Ok(())
        })
        .unwrap();
        let p = doc.project();
        let mut wires = 0;
        for w in p.wires() {
            wires += 1;
            let mut pts = vec![p.endpoint_position(w.from).expect("end")];
            pts.extend(w.route.iter().copied());
            pts.push(p.endpoint_position(w.to).expect("end"));
            for pair in pts.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                assert!(
                    (a.x - b.x).abs() < 1e-6 || (a.y - b.y).abs() < 1e-6,
                    "{}: diagonal wire from ({}, {}) to ({}, {})",
                    file.name,
                    a.x,
                    a.y,
                    b.x,
                    b.y
                );
            }
        }
        assert!(wires >= 2, "{} placed {wires} wires", file.name);
    }
}
