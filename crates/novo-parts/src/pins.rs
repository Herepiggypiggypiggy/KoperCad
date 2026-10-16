//! Terminal positions of a part as project properties.
//!
//! A catalog part keeps where each terminal is (`PartTerminal::position_mm`,
//! origin at the back-left-bottom corner of the outline). The project copy of
//! a part (`novo_core::Part`) has only `properties`, so the positions travel
//! there as `pin.<marking>.x_mm`, `.y_mm`, `.z_mm`, and wire routing and the
//! 3D view read them with [`terminal_position`] without a catalog.

use novo_core::{Properties, Value};

use crate::catalog::CatalogPart;
use crate::schema::DataSource;

/// Key in `CatalogPart::sources` that says how terminal positions were found.
pub const POSITIONS_SOURCE_KEY: &str = "terminal_positions";
/// Property that says how the positions were found: `measured` (read from the
/// maker's model) or `estimated` (spread evenly along a side).
pub const POSITIONS_SOURCE_PROPERTY: &str = "pin_positions_source";

const PREFIX: &str = "pin.";

fn key(name: &str, axis: char) -> String {
    format!("{PREFIX}{name}.{axis}_mm")
}

/// Puts the known terminal positions of `part` into `properties`.
pub fn write_properties(part: &CatalogPart, properties: &mut Properties) {
    let mut any = false;
    for t in &part.terminals {
        if let Some(p) = t.position_mm {
            for (axis, v) in ['x', 'y', 'z'].into_iter().zip(p) {
                properties.insert(key(&t.name, axis), Value::Float(v));
            }
            any = true;
        }
    }
    if any {
        let how = match part.sources.get(POSITIONS_SOURCE_KEY) {
            Some(DataSource::Estimated) => "estimated",
            _ => "measured",
        };
        properties.insert(POSITIONS_SOURCE_PROPERTY.into(), Value::Text(how.into()));
    }
}

fn number(properties: &Properties, key: &str) -> Option<f64> {
    match properties.get(key)? {
        Value::Float(v) => Some(*v),
        Value::Int(v) => Some(*v as f64),
        _ => None,
    }
}

/// Where terminal `name` is on the part, in millimetres from the
/// back-left-bottom corner of its outline, or `None` when it is not known.
pub fn terminal_position(properties: &Properties, name: &str) -> Option<[f64; 3]> {
    Some([
        number(properties, &key(name, 'x'))?,
        number(properties, &key(name, 'y'))?,
        number(properties, &key(name, 'z'))?,
    ])
}

/// Whether the positions in `properties` are an even spacing rather than a
/// reading of the maker's model. `None` when there are no positions.
pub fn positions_estimated(properties: &Properties) -> Option<bool> {
    match properties.get(POSITIONS_SOURCE_PROPERTY)? {
        Value::Text(t) => Some(t == "estimated"),
        _ => None,
    }
}

/// Markings that have a position in `properties`, in name order.
pub fn placed_terminals(properties: &Properties) -> Vec<String> {
    properties
        .keys()
        .filter_map(|k| k.strip_prefix(PREFIX)?.strip_suffix(".x_mm"))
        .filter(|n| terminal_position(properties, n).is_some())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::PartTerminal;

    fn part() -> CatalogPart {
        let mut p = CatalogPart::new("M", "1");
        p.terminals = vec![
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
        p
    }

    #[test]
    fn positions_round_trip_through_properties() {
        let mut p = part();
        p.sources
            .insert(POSITIONS_SOURCE_KEY.into(), DataSource::MakerDatasheet);
        let mut props = Properties::new();
        write_properties(&p, &mut props);
        assert_eq!(terminal_position(&props, "A1"), Some([5.0, 60.0, 20.0]));
        assert_eq!(terminal_position(&props, "A2"), None);
        assert_eq!(placed_terminals(&props), vec!["A1".to_string()]);
        assert_eq!(positions_estimated(&props), Some(false));
    }

    #[test]
    fn estimated_positions_say_so_and_unknown_writes_nothing() {
        let mut p = part();
        p.sources
            .insert(POSITIONS_SOURCE_KEY.into(), DataSource::Estimated);
        let mut props = Properties::new();
        write_properties(&p, &mut props);
        assert_eq!(positions_estimated(&props), Some(true));
        let mut none = Properties::new();
        write_properties(&CatalogPart::new("M", "2"), &mut none);
        assert!(none.is_empty());
        assert_eq!(positions_estimated(&none), None);
    }
}
