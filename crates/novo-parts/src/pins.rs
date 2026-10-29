//! Terminal positions of a part as project properties.
//!
//! A catalog part keeps where each terminal is (`PartTerminal::position_mm`,
//! origin at the back-left-bottom corner of the outline). The project copy of
//! a part (`novo_core::Part`) has only `properties`, so the positions travel
//! there as `pin.<marking>.x_mm`, `.y_mm`, `.z_mm`, and wire routing and the
//! 3D view read them with [`terminal_position`] without a catalog.

use novo_core::{Properties, Value};

use crate::catalog::CatalogPart;
use crate::schema::{DataSource, FerruleRule};

/// Key in `CatalogPart::sources` that says how terminal positions were found.
pub const POSITIONS_SOURCE_KEY: &str = "terminal_positions";
/// Property that says how the positions were found: `measured` (read from the
/// maker's model) or `estimated` (spread evenly along a side).
pub const POSITIONS_SOURCE_PROPERTY: &str = "pin_positions_source";

const PREFIX: &str = "pin.";

fn key(name: &str, axis: char) -> String {
    format!("{PREFIX}{name}.{axis}_mm")
}

/// Key in `CatalogPart::sources` that says where the wire range, strip
/// length, torque and ferrule data of the connection points came from.
pub const CONNECTION_SOURCE_KEY: &str = "connection_data";
/// Property that carries that source into the project: `maker`, `typed` or
/// `estimated`. Absent when the part states no connection data.
pub const CONNECTION_SOURCE_PROPERTY: &str = "pin_data_source";

/// What a part says about one connection point (`pin.<marking>.*`). Every
/// field is `None` when the part does not state it: nothing is guessed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ConnectionData {
    /// Smallest and largest conductor cross-section in mm².
    pub wire_mm2: Option<(f64, f64)>,
    /// Length to strip, in mm.
    pub strip_mm: Option<f64>,
    /// Smallest and largest tightening torque in N·m.
    pub torque_nm: Option<(f64, f64)>,
    /// Conductors the point takes.
    pub max_wires: Option<u8>,
    pub ferrule: Option<FerruleRule>,
}

impl ConnectionData {
    /// Whether the part states nothing about the point.
    pub fn is_empty(&self) -> bool {
        *self == ConnectionData::default()
    }
}

fn data_key(name: &str, field: &str) -> String {
    format!("{PREFIX}{name}.{field}")
}

fn ferrule_word(rule: FerruleRule) -> &'static str {
    match rule {
        FerruleRule::Required => "required",
        FerruleRule::Optional => "optional",
        FerruleRule::Forbidden => "forbidden",
    }
}

fn write_connection_data(part: &CatalogPart, properties: &mut Properties) {
    let mut any = false;
    for t in &part.terminals {
        let mut put = |field: &str, v: Value| {
            properties.insert(data_key(&t.name, field), v);
            any = true;
        };
        if let Some((lo, hi)) = t.wire_mm2 {
            put("wire_min_mm2", Value::Float(lo));
            put("wire_max_mm2", Value::Float(hi));
        }
        if let Some(v) = t.strip_mm {
            put("strip_mm", Value::Float(v));
        }
        if let Some((lo, hi)) = t.torque_nm {
            put("torque_min_nm", Value::Float(lo));
            put("torque_max_nm", Value::Float(hi));
        }
        if let Some(n) = t.max_wires {
            put("max_wires", Value::Int(i64::from(n)));
        }
        if let Some(rule) = t.ferrule {
            put("ferrule", Value::Text(ferrule_word(rule).into()));
        }
    }
    if any {
        let how = match part.sources.get(CONNECTION_SOURCE_KEY) {
            Some(DataSource::MakerDatasheet) => "maker",
            Some(DataSource::Typed) => "typed",
            Some(DataSource::Estimated) => "estimated",
            _ => "unknown",
        };
        properties.insert(CONNECTION_SOURCE_PROPERTY.into(), Value::Text(how.into()));
    }
}

/// What the part states about connection point `name`, read from the project
/// properties of a part. Values that are zero, negative or in the wrong order
/// count as not stated.
pub fn connection_data(properties: &Properties, name: &str) -> ConnectionData {
    let get = |field: &str| number(properties, &data_key(name, field)).filter(|v| *v > 0.0);
    let range = |lo: &str, hi: &str| match (get(lo), get(hi)) {
        (Some(a), Some(b)) if a <= b => Some((a, b)),
        _ => None,
    };
    ConnectionData {
        wire_mm2: range("wire_min_mm2", "wire_max_mm2"),
        strip_mm: get("strip_mm"),
        torque_nm: range("torque_min_nm", "torque_max_nm"),
        max_wires: get("max_wires")
            .filter(|n| n.fract() == 0.0 && *n <= 255.0)
            .map(|n| n as u8),
        ferrule: match properties.get(&data_key(name, "ferrule")) {
            Some(Value::Text(t)) => match t.as_str() {
                "required" => Some(FerruleRule::Required),
                "optional" => Some(FerruleRule::Optional),
                "forbidden" => Some(FerruleRule::Forbidden),
                _ => None,
            },
            _ => None,
        },
    }
}

/// Where the connection data of a part came from: `maker`, `typed`,
/// `estimated` or `unknown`; `None` when the part states none.
pub fn connection_source(properties: &Properties) -> Option<&str> {
    match properties.get(CONNECTION_SOURCE_PROPERTY)? {
        Value::Text(t) => Some(t.as_str()),
        _ => None,
    }
}

/// Puts the known terminal positions and connection data of `part` into
/// `properties`.
pub fn write_properties(part: &CatalogPart, properties: &mut Properties) {
    write_connection_data(part, properties);
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

    #[test]
    fn connection_data_round_trips_and_unstated_stays_unknown() {
        let mut p = part();
        p.terminals[0].wire_mm2 = Some((0.5, 2.5));
        p.terminals[0].strip_mm = Some(9.0);
        p.terminals[0].torque_nm = Some((0.4, 0.5));
        p.terminals[0].max_wires = Some(1);
        p.terminals[0].ferrule = Some(FerruleRule::Required);
        p.sources
            .insert(CONNECTION_SOURCE_KEY.into(), DataSource::MakerDatasheet);
        let mut props = Properties::new();
        write_properties(&p, &mut props);
        let d = connection_data(&props, "A1");
        assert_eq!(d.wire_mm2, Some((0.5, 2.5)));
        assert_eq!(d.strip_mm, Some(9.0));
        assert_eq!(d.torque_nm, Some((0.4, 0.5)));
        assert_eq!(d.max_wires, Some(1));
        assert_eq!(d.ferrule, Some(FerruleRule::Required));
        assert_eq!(connection_source(&props), Some("maker"));
        assert!(connection_data(&props, "A2").is_empty());
    }

    #[test]
    fn nonsense_connection_values_count_as_not_stated() {
        let mut props = Properties::new();
        props.insert("pin.1.wire_min_mm2".into(), Value::Float(4.0));
        props.insert("pin.1.wire_max_mm2".into(), Value::Float(1.0));
        props.insert("pin.1.max_wires".into(), Value::Int(0));
        props.insert("pin.1.ferrule".into(), Value::Text("maybe".into()));
        assert!(connection_data(&props, "1").is_empty());
    }
}
