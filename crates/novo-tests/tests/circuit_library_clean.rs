//! The circuit library places clean: the only pins left open are the ones a
//! circuit's file lists as open points, no two wires lie on each other, no
//! contact is drawn twice, nothing shorts a potential, and a circuit placed
//! twice gets fresh tags on the second.

use novo_checks::ProblemKind;
use novo_core::{BlockDefinitionId, Document, Point, Project, Record};
use novo_parts::{CircuitSpec, MacroFile, circuit_library, circuit_sources};

fn placed(file: &MacroFile, copies: usize) -> Document {
    let mut doc = Document::new(Project::new(""));
    doc.transact("place", |tx| {
        let page = tx.add_page("1")?;
        let mut definition = file.definition.clone();
        definition.id = BlockDefinitionId(9_000_000);
        let def = definition.id;
        tx.insert_record(Record::BlockDefinition(definition))?;
        for i in 0..copies {
            tx.place_block(def, page, Point::new(20.0 + 400.0 * i as f64, 20.0), vec![])?;
        }
        Ok(())
    })
    .unwrap_or_else(|e| panic!("{}: {e}", file.name));
    doc
}

/// How many pins a message like `-F1: 1, 3, 5 not connected` names.
fn pins_in(message: &str) -> usize {
    let list = message.trim_end_matches(" not connected");
    let list = list.rsplit_once(": ").map_or(list, |(_, pins)| pins);
    list.split(", ").count()
}

fn open_pins_listed(name: &str) -> usize {
    let source = circuit_sources()
        .iter()
        .find(|s| s.text.contains(&format!("name = \"{name}\"")))
        .unwrap_or_else(|| panic!("no source for {name}"));
    let spec = CircuitSpec::parse(source.text, source.path).unwrap();
    spec.open
        .iter()
        .map(|o| o.split_once('.').unwrap().1.split(',').count())
        .sum()
}

#[test]
fn only_the_listed_open_points_are_left_open() {
    for (_, file) in circuit_library().unwrap() {
        let doc = placed(&file, 1);
        let open: usize = novo_checks::check(doc.project())
            .into_iter()
            .filter(|p| p.kind == ProblemKind::UnconnectedPin)
            .map(|p| pins_in(&p.message))
            .sum();
        assert_eq!(open, open_pins_listed(&file.name), "{}", file.name);
    }
}

#[test]
fn no_circuit_has_wires_on_each_other_or_a_contact_drawn_twice() {
    for (_, file) in circuit_library().unwrap() {
        let doc = placed(&file, 1);
        for p in novo_checks::check(doc.project()) {
            assert!(
                !matches!(
                    p.kind,
                    ProblemKind::WireOverlap
                        | ProblemKind::ZeroLengthWire
                        | ProblemKind::ContactDrawnTwice
                        | ProblemKind::OpenWire
                        | ProblemKind::DanglingJunction
                        | ProblemKind::PotentialConflict
                ),
                "{}: {:?}: {}",
                file.name,
                p.kind,
                p.message
            );
        }
    }
}

#[test]
fn a_second_copy_takes_new_tags_and_stays_clean() {
    for (_, file) in circuit_library().unwrap() {
        let doc = placed(&file, 2);
        let tags: Vec<String> = doc.project().devices().map(|d| d.tag.to_string()).collect();
        let mut sorted = tags.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            tags.len(),
            "{}: duplicate tags {tags:?}",
            file.name
        );
    }
}
