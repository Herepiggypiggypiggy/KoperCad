//! Panel-building data of a part: power loss, mounting type and the free
//! space the maker asks for around it. Stored as plain part properties
//! (see [`keys`](crate::keys)), so a project file and a catalog carry them
//! without any new model type. See `docs/PRO-PANEL-GAP-ANALYSIS.md` (R0, R1).

use novo_core::{Properties, Value};

use crate::catalog::keys;

/// How a part is fixed in the cabinet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mounting {
    /// Clips onto a DIN rail.
    DinRail,
    /// Screwed to the plate.
    Screw,
    /// Sits flat on the plate (adhesive, magnet, no fixing holes).
    Plate,
    /// Mounted in a door cut-out (displays, selectors, pushbuttons).
    DoorCutout,
}

impl Mounting {
    pub const ALL: [Mounting; 4] = [
        Mounting::DinRail,
        Mounting::Screw,
        Mounting::Plate,
        Mounting::DoorCutout,
    ];

    /// The stored value: `din_rail`, `screw`, `plate`, `door_cutout`.
    pub fn as_str(self) -> &'static str {
        match self {
            Mounting::DinRail => "din_rail",
            Mounting::Screw => "screw",
            Mounting::Plate => "plate",
            Mounting::DoorCutout => "door_cutout",
        }
    }

    /// The name shown to people: `DIN rail`, `Screw`, `Plate`, `Door cut-out`.
    pub fn label(self) -> &'static str {
        match self {
            Mounting::DinRail => "DIN rail",
            Mounting::Screw => "Screw",
            Mounting::Plate => "Plate",
            Mounting::DoorCutout => "Door cut-out",
        }
    }

    /// Reads the stored value or what a person or a maker file would type
    /// (`DIN rail`, `Hutschiene`, `door cut-out`, `Schraubmontage`).
    pub fn parse(text: &str) -> Option<Mounting> {
        let t: String = text
            .trim()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        Some(match t.as_str() {
            "dinrail" | "rail" | "din" | "tsrail" | "hutschiene" | "tragschiene" | "dinrail35"
            | "dinrails" | "dinrijl" | "rijl" => Mounting::DinRail,
            "screw" | "screws" | "schraube" | "schraubmontage" | "schroef" | "bolted" => {
                Mounting::Screw
            }
            "plate" | "mountingplate" | "montageplatte" | "montageplaat" | "flat" => {
                Mounting::Plate
            }
            "doorcutout" | "doorcutouts" | "door" | "cutout" | "tuer" | "tur" | "deur"
            | "frontmounting" | "frontplate" => Mounting::DoorCutout,
            _ => return None,
        })
    }
}

/// Free space a part wants on each side, in mm, in the part's own frame
/// (upright, as drawn in the data sheet).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Clearances {
    pub above_mm: Option<f64>,
    pub below_mm: Option<f64>,
    pub left_mm: Option<f64>,
    pub right_mm: Option<f64>,
}

impl Clearances {
    pub fn is_empty(&self) -> bool {
        *self == Clearances::default()
    }
}

/// The panel-building data found in a part's properties.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PanelData {
    /// Power loss at rated load in W.
    pub power_loss_w: Option<f64>,
    pub mounting: Option<Mounting>,
    pub clearances: Clearances,
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Float(v) => Some(*v),
        Value::Int(v) => Some(*v as f64),
        Value::Text(t) => crate::parse_number(t),
        Value::Bool(_) => None,
    }
    .filter(|v| v.is_finite() && *v >= 0.0)
}

impl PanelData {
    /// The data of a device: its own values win over its part's, field by
    /// field (editing a part row in the property sheet sets the device's
    /// own value, so a derated device keeps its figure).
    pub fn of_device(device: &Properties, part: Option<&Properties>) -> PanelData {
        let own = PanelData::of_properties(device);
        let Some(part) = part.map(PanelData::of_properties) else {
            return own;
        };
        PanelData {
            power_loss_w: own.power_loss_w.or(part.power_loss_w),
            mounting: own.mounting.or(part.mounting),
            clearances: Clearances {
                above_mm: own.clearances.above_mm.or(part.clearances.above_mm),
                below_mm: own.clearances.below_mm.or(part.clearances.below_mm),
                left_mm: own.clearances.left_mm.or(part.clearances.left_mm),
                right_mm: own.clearances.right_mm.or(part.clearances.right_mm),
            },
        }
    }

    /// Reads the panel keys; values that are negative or not numbers count
    /// as missing.
    pub fn of_properties(properties: &Properties) -> PanelData {
        let num = |key: &str| properties.get(key).and_then(number);
        PanelData {
            power_loss_w: num(keys::POWER_LOSS_W),
            mounting: match properties.get(keys::MOUNTING) {
                Some(Value::Text(t)) => Mounting::parse(t),
                _ => None,
            },
            clearances: Clearances {
                above_mm: num(keys::CLEARANCE_ABOVE_MM),
                below_mm: num(keys::CLEARANCE_BELOW_MM),
                left_mm: num(keys::CLEARANCE_LEFT_MM),
                right_mm: num(keys::CLEARANCE_RIGHT_MM),
            },
        }
    }
}

/// Whether `key` holds a non-negative number (power loss, clearances).
pub fn is_panel_number_key(key: &str) -> bool {
    matches!(
        key,
        keys::POWER_LOSS_W
            | keys::CLEARANCE_ABOVE_MM
            | keys::CLEARANCE_BELOW_MM
            | keys::CLEARANCE_LEFT_MM
            | keys::CLEARANCE_RIGHT_MM
    )
}

/// Names older files and maker sheets use for the panel keys.
const LEGACY_KEYS: &[(&str, &str)] = &[
    ("power_loss", keys::POWER_LOSS_W),
    ("loss_w", keys::POWER_LOSS_W),
    ("power_dissipation_w", keys::POWER_LOSS_W),
    ("power_dissipation", keys::POWER_LOSS_W),
    ("dissipation_w", keys::POWER_LOSS_W),
    ("verlustleistung_w", keys::POWER_LOSS_W),
    ("mounting_type", keys::MOUNTING),
    ("montage", keys::MOUNTING),
    ("mounting_distance_above_mm", keys::CLEARANCE_ABOVE_MM),
    ("mounting_distance_below_mm", keys::CLEARANCE_BELOW_MM),
    ("clearance_top_mm", keys::CLEARANCE_ABOVE_MM),
    ("clearance_bottom_mm", keys::CLEARANCE_BELOW_MM),
];

/// Moves legacy panel keys to the current names and writes the mounting
/// type in its stored form. A current key already present wins over a
/// legacy one. Returns whether anything changed.
pub fn migrate_properties(properties: &mut Properties) -> bool {
    let mut changed = false;
    for (old, new) in LEGACY_KEYS {
        // A loss written as text with a unit ("15 W") is left for a maker
        // recipe to read; only plain numbers are renamed here.
        if *new == keys::POWER_LOSS_W && matches!(properties.get(*old), Some(Value::Text(_))) {
            continue;
        }
        if let Some(value) = properties.remove(*old) {
            changed = true;
            properties.entry((*new).to_string()).or_insert(value);
        }
    }
    if let Some(Value::Text(t)) = properties.get(keys::MOUNTING)
        && let Some(m) = Mounting::parse(t)
        && t != m.as_str()
    {
        properties.insert(keys::MOUNTING.into(), Value::Text(m.as_str().into()));
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounting_reads_what_people_type() {
        assert_eq!(Mounting::parse("DIN rail"), Some(Mounting::DinRail));
        assert_eq!(Mounting::parse(" Hutschiene "), Some(Mounting::DinRail));
        assert_eq!(Mounting::parse("door cut-out"), Some(Mounting::DoorCutout));
        assert_eq!(Mounting::parse("glue"), None);
        for m in Mounting::ALL {
            assert_eq!(Mounting::parse(m.as_str()), Some(m));
            assert_eq!(Mounting::parse(m.label()), Some(m));
        }
    }

    #[test]
    fn negative_or_text_values_count_as_missing() {
        let mut p = Properties::new();
        p.insert(keys::POWER_LOSS_W.into(), Value::Float(-1.0));
        p.insert(keys::CLEARANCE_ABOVE_MM.into(), Value::Int(30));
        p.insert(keys::CLEARANCE_LEFT_MM.into(), Value::Text("2,5".into()));
        let d = PanelData::of_properties(&p);
        assert_eq!(d.power_loss_w, None);
        assert_eq!(d.clearances.above_mm, Some(30.0));
        assert_eq!(d.clearances.left_mm, Some(2.5));
    }

    #[test]
    fn migration_renames_and_normalizes() {
        let mut p = Properties::new();
        p.insert("power_dissipation_w".into(), Value::Float(4.5));
        p.insert("mounting_type".into(), Value::Text("DIN rail".into()));
        p.insert(keys::POWER_LOSS_W.into(), Value::Float(9.0));
        assert!(migrate_properties(&mut p));
        assert_eq!(p.get(keys::POWER_LOSS_W), Some(&Value::Float(9.0)));
        assert_eq!(p.get(keys::MOUNTING), Some(&Value::Text("din_rail".into())));
        assert!(!p.contains_key("power_dissipation_w"));
        assert!(!migrate_properties(&mut p));
    }
}
