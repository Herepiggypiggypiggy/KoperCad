//! The fallback symbol for a device with no mapping: a labelled box that
//! keeps every EPLAN pin as a connection, so wires and nets survive.

use novo_symbols::{
    Align, Anchor, Behavior, Connection, Direction, FORMAT_VERSION, Graphic, Stroke, Symbol,
};

/// Spacing of the pins; a multiple of the 4 mm connection grid.
const PITCH: f64 = 8.0;

/// A box with `pins` as connections, half on top and half below. `label` is
/// the device text (the EPLAN function name or tag letters) shown inside and
/// used as the symbol name. The id is `import/generic/box_<n>p`; it is not
/// in any library, so the importer stores the symbol in the project.
pub fn generic_symbol(pins: &[String], label: &str) -> Symbol {
    let top = pins.len().div_ceil(2);
    let bottom = pins.len() - top;
    let width = (top.max(bottom).max(1) as f64) * PITCH;

    let mut ids: Vec<String> = Vec::new();
    for pin in pins {
        let mut id = pin.trim().to_string();
        if id.is_empty() {
            id = (ids.len() + 1).to_string();
        }
        while ids.contains(&id) {
            id.push('\'');
        }
        ids.push(id);
    }

    let mut connections = Vec::new();
    let mut graphics = vec![Graphic::Rect {
        at: [-PITCH / 2.0, 4.0],
        size: [width, 8.0],
        fill: false,
        stroke: Stroke::Solid,
    }];
    for (i, id) in ids.iter().enumerate() {
        let (x, y_out, y_in, dir) = if i < top {
            (i as f64 * PITCH, 0.0, 4.0, Direction::Up)
        } else {
            ((i - top) as f64 * PITCH, 16.0, 12.0, Direction::Down)
        };
        graphics.push(Graphic::Line {
            from: [x, y_out],
            to: [x, y_in],
            stroke: Stroke::Solid,
        });
        connections.push(Connection {
            id: id.clone(),
            at: [x, y_out],
            dir,
            role: None,
            optional: false,
        });
    }
    let text: String = label
        .chars()
        .filter(|c| c.is_ascii_graphic())
        .take(6)
        .collect();
    if !text.is_empty() {
        graphics.push(Graphic::Text {
            at: [width / 2.0 - PITCH / 2.0, 8.0],
            text,
            size: 2.5,
            align: Align::Center,
        });
    }

    Symbol {
        format: FORMAT_VERSION,
        id: format!("import/generic/box_{}p", pins.len()),
        name: if label.trim().is_empty() {
            "Imported device".to_string()
        } else {
            label.trim().to_string()
        },
        description: "Stand-in for an EPLAN device that has no KoperCAD symbol yet.".into(),
        standard: "IEC 60617".into(),
        category: "generic".into(),
        function: "generic.device".into(),
        keywords: vec!["generic".into(), "imported".into()],
        aliases: Vec::new(),
        favorite: false,
        connections,
        anchors: vec![Anchor {
            role: "tag".into(),
            at: [-PITCH / 2.0 - 2.0, 8.0],
            align: Align::Right,
        }],
        bridges: Vec::new(),
        graphics,
        behavior: Behavior {
            kind: "none".into(),
            params: toml::Table::new(),
        },
    }
}
