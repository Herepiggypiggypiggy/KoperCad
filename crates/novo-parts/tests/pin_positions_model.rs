//! A catalog part's terminal positions travel into the project part.

use novo_parts::{CatalogPart, PartTerminal, terminal_position};

#[test]
fn to_model_carries_terminal_positions_and_from_model_keeps_them() {
    let mut part = CatalogPart::new("ACME", "C1");
    part.terminals = vec![
        PartTerminal {
            name: "A1".into(),
            position_mm: Some([5.0, 60.0, 20.0]),
            ..Default::default()
        },
        PartTerminal {
            name: "A2".into(),
            ..Default::default()
        },
    ];
    let model = part.to_model(novo_core::PartId::from(1));
    assert_eq!(
        terminal_position(&model.properties, "A1"),
        Some([5.0, 60.0, 20.0])
    );
    assert_eq!(terminal_position(&model.properties, "A2"), None);
    let back = CatalogPart::from_model(&model).to_model(novo_core::PartId::from(1));
    assert_eq!(
        terminal_position(&back.properties, "A1"),
        Some([5.0, 60.0, 20.0])
    );
}
