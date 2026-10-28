//! Making a table from a receiver or pendant device that is already in a drawing: its pins,
//! what is written at them and what they are wired to.

use super::table::{Columns, Row, Table};

/// One pin of a device in the open drawing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DevicePin {
    /// Pin or terminal label, for example `13` or `COM`.
    pub name: String,
    /// Function text of the pin: the pin description, or the part's pin function.
    pub text: String,
    /// What the pin is wired to, for example `-K61:A1`; empty when open.
    pub target: String,
}

impl Table {
    /// The table of a device's pins. A pin with no text of its own is read from the label
    /// of the thing it feeds, which often names the motion (`-K_HOIST_UP`).
    pub fn from_pins(pins: &[DevicePin]) -> Table {
        let rows: Vec<Row> = pins
            .iter()
            .take(super::table::MAX_ROWS)
            .enumerate()
            .map(|(i, p)| Row {
                line: i + 1,
                terminal: p.name.clone(),
                function: if p.text.trim().is_empty() {
                    p.target.clone()
                } else {
                    p.text.clone()
                },
                step: String::new(),
                target: p.target.clone(),
            })
            .collect();
        Table {
            rows,
            columns: Columns {
                terminal: Some(0),
                function: Some(1),
                step: None,
                target: Some(2),
            },
            had_header: false,
            notes: vec![format!(
                "Read from {} pins of a device in the drawing",
                pins.len()
            )],
        }
    }
}
