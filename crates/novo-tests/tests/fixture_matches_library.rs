//! The Gate 1 fixture places symbols the way the editor will: by WS-05
//! library id, with that symbol's connection points. This checks the two
//! crates agree, so model-side tests exercise real symbols.

use novo_core::{Document, SymbolRole};
use novo_symbols::Library;
use novo_tests::motor_starter;

#[test]
fn every_placed_symbol_exists_with_the_same_connection_points() {
    let library = Library::builtin().expect("built-in library loads");
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    for placed in doc.project().symbols() {
        let key = placed.symbol.0.clone();
        let symbol = library
            .get(&key)
            .unwrap_or_else(|| panic!("`{key}` is not in the symbol library"));
        let library_pins: Vec<(&str, f64, f64)> = symbol
            .connections
            .iter()
            .map(|c| (c.id.as_str(), c.at[0], c.at[1]))
            .collect();
        let model_pins: Vec<(&str, f64, f64)> = placed
            .pins
            .iter()
            .map(|p| (p.name.as_str(), p.offset.x, p.offset.y))
            .collect();
        assert_eq!(model_pins, library_pins, "connection points of `{key}`");
    }
}

#[test]
fn terminal_bridges_match_the_library() {
    let library = Library::builtin().unwrap();
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let mut checked = 0;
    for placed in doc.project().symbols() {
        let SymbolRole::Terminal { .. } = placed.role else {
            continue;
        };
        let expected: Vec<Vec<u16>> = library
            .get(&placed.symbol.0.clone())
            .unwrap()
            .bridges()
            .into_iter()
            .map(|g| g.into_iter().map(|i| i as u16).collect())
            .collect();
        // The model joins a terminal's sides through its role; the library
        // states the same as a bridge.
        assert_eq!(expected, vec![vec![0, 1]]);
        checked += 1;
    }
    assert_eq!(checked, 4);
}
