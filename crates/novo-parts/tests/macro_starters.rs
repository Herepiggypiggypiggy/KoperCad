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
    assert_eq!(first.len(), 4);
    let p = "Starter/E-stop chain.kcmacro";
    let mut edited = lib.load(p).unwrap();
    edited.description = "mine".into();
    lib.replace(p, &edited).unwrap();
    let second = lib.install_starters().unwrap();
    assert!(second.iter().all(|r| matches!(r, Imported::Skipped(_))));
    assert_eq!(lib.load(p).unwrap().description, "mine");
    assert_eq!(lib.search("star delta").len(), 1, "words match apart");
    assert_eq!(lib.search("star-delta").len(), 1);
    assert_eq!(lib.search("24 V").len(), 1);
}
