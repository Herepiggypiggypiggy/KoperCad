//! The mounting view of a circuit: where each device of the circuit sits on
//! a reference mounting plate, how big a generic device of its kind is, and
//! which devices are not in the cabinet at all. Pure data kept in the macro
//! file's `mounting` field; nothing here touches a project.
//!
//! The sizes are typical values for a generic device. They are a guide for
//! sizing the plate and the rails, not a datasheet: the real size comes from
//! the part the user assigns to the device afterwards.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::macros::MacroFile;

/// The macro file field that carries the mounting view.
pub const MOUNTING_KEY: &str = "mounting";

/// How a device is mounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MountKind {
    /// Clipped on a 35 mm DIN rail of the plate.
    Din,
    /// Screwed on the plate.
    Plate,
    /// In a 22.5 mm hole of the door or the operator panel.
    Door,
    /// Not in the cabinet: on the machine.
    Field,
}

impl MountKind {
    /// Words for a list: "On the DIN rail", "On the door".
    pub fn label(self) -> &'static str {
        match self {
            MountKind::Din => "DIN rail",
            MountKind::Plate => "Plate",
            MountKind::Door => "Door",
            MountKind::Field => "On the machine",
        }
    }
}

/// The reference mounting plate, millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MountPlate {
    pub width: f64,
    pub height: f64,
}

/// A DIN rail on the reference plate; it runs to the right from `(x, y)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MountRail {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub length: f64,
}

/// One device of the circuit in the mounting view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MountItem {
    /// The device in the macro's definition (its template id), the same
    /// number a carried part uses.
    pub device: u64,
    pub tag: String,
    /// The device description as the definition has it; a `{name}` is a
    /// parameter, see [`MountItem::description_with`].
    pub description: String,
    pub kind: MountKind,
    /// Width, height, depth in millimetres; not for a field device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[f64; 3]>,
    /// Top left on the plate (or the door), millimetres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rail: Option<String>,
    /// Where a field device sits, in words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// What comes with the device and is not in the schematic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl MountItem {
    /// The rectangle on the plate: left, top, right, bottom.
    pub fn rect(&self) -> Option<[f64; 4]> {
        let ([x, y], [w, h, _]) = (self.at?, self.size?);
        Some([x, y, x + w, y + h])
    }

    /// The description with each `{name}` replaced by its value.
    pub fn description_with(&self, values: &[(String, String)]) -> String {
        values
            .iter()
            .fold(self.description.clone(), |text, (name, value)| {
                text.replace(&format!("{{{name}}}"), value)
            })
    }
}

/// The whole mounting view of one circuit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MountingView {
    pub plate: MountPlate,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rails: Vec<MountRail>,
    pub items: Vec<MountItem>,
}

impl MountingView {
    /// The devices on the plate or its rails.
    pub fn panel(&self) -> impl Iterator<Item = &MountItem> {
        self.items
            .iter()
            .filter(|i| matches!(i.kind, MountKind::Din | MountKind::Plate))
    }

    /// The devices in the door or operator panel.
    pub fn door(&self) -> impl Iterator<Item = &MountItem> {
        self.items.iter().filter(|i| i.kind == MountKind::Door)
    }

    /// The devices on the machine, outside the cabinet.
    pub fn field(&self) -> impl Iterator<Item = &MountItem> {
        self.items.iter().filter(|i| i.kind == MountKind::Field)
    }

    /// The smallest plate that holds every panel device with `margin`
    /// millimetres around, in width and height.
    pub fn needed_plate(&self, margin: f64) -> (f64, f64) {
        self.panel()
            .filter_map(MountItem::rect)
            .fold((0.0, 0.0), |(w, h), r| {
                (f64::max(w, r[2] + margin), f64::max(h, r[3] + margin))
            })
    }

    /// Problems with the layout itself: a device off the plate or off its
    /// rail, two devices on each other, a size or place missing. Empty for
    /// a sound view.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let (pw, ph) = (self.plate.width, self.plate.height);
        let mut seen: Vec<(&MountItem, [f64; 4])> = Vec::new();
        for item in &self.items {
            if item.kind == MountKind::Field {
                if item.rect().is_some() || item.rail.is_some() {
                    out.push(format!(
                        "{}: a field device has no place on the plate",
                        item.tag
                    ));
                }
                continue;
            }
            let Some(rect) = item.rect() else {
                out.push(format!("{}: size and place are needed", item.tag));
                continue;
            };
            let on_door = item.kind == MountKind::Door;
            if !on_door && (rect[0] < 0.0 || rect[1] < 0.0 || rect[2] > pw || rect[3] > ph) {
                out.push(format!("{}: off the {pw} x {ph} mm plate", item.tag));
            }
            if item.kind == MountKind::Din {
                match item
                    .rail
                    .as_ref()
                    .and_then(|r| self.rails.iter().find(|x| &x.id == r))
                {
                    None => out.push(format!("{}: names no rail of the plate", item.tag)),
                    Some(rail) => {
                        // The device sits on the rail: its middle line is the
                        // rail's, and it stays within the rail's length.
                        let mid = (rect[1] + rect[3]) / 2.0;
                        if (mid - rail.y).abs() > item.size.map_or(0.0, |s| s[1]) / 2.0
                            || rect[0] < rail.x
                            || rect[2] > rail.x + rail.length
                        {
                            out.push(format!("{}: not on rail {}", item.tag, rail.id));
                        }
                    }
                }
            }
            for (other, r) in &seen {
                let same_plane = on_door == (other.kind == MountKind::Door);
                if same_plane
                    && rect[0] < r[2]
                    && r[0] < rect[2]
                    && rect[1] < r[3]
                    && r[1] < rect[3]
                {
                    out.push(format!("{} and {} lie on each other", item.tag, other.tag));
                }
            }
            seen.push((item, rect));
        }
        out
    }
}

impl MacroFile {
    /// The mounting view the file carries, if it has one and it reads.
    pub fn mounting(&self) -> Option<MountingView> {
        serde_json::from_value(self.extra.get(MOUNTING_KEY)?.clone()).ok()
    }

    /// Sets (or with `None` clears) the mounting view. The file stays on
    /// the base format version: an older build keeps the field as it is.
    pub fn set_mounting(&mut self, mounting: Option<&MountingView>) {
        match mounting {
            Some(m) => {
                let value: Value = serde_json::to_value(m).expect("a mounting view serializes");
                self.extra.insert(MOUNTING_KEY.into(), value);
            }
            None => {
                self.extra.remove(MOUNTING_KEY);
            }
        }
    }
}
