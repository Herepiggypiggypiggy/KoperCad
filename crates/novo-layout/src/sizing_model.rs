//! The bridge from the project to the sizing helper: the devices that
//! belong in an enclosure, read as [`SizingDevice`]s.
//!
//! Part keys (see `novo_parts`): `width_mm`, `height_mm`, `depth_mm`,
//! `power_loss_w`, `clearance_above_mm` / `_below_` / `_left_` / `_right_`,
//! and `load_factor` (a plain property, 0..1). A device property overrides
//! the same key on its parts. Values below zero or not numbers count as
//! missing. Power loss of a device with several parts is the sum over its
//! parts unless the device itself carries the key.

use novo_core::{Device, Part, Project, Properties, StructureId, Value};

use crate::model::device_size;
use crate::sizing::SizingDevice;

const DEPTH: &str = "depth_mm";
const LOSS: &str = "power_loss_w";
const LOAD: &str = "load_factor";
const CLEARANCES: [&str; 4] = [
    "clearance_above_mm",
    "clearance_below_mm",
    "clearance_left_mm",
    "clearance_right_mm",
];

fn number(properties: &Properties, key: &str) -> Option<f64> {
    match properties.get(key)? {
        Value::Float(v) => Some(*v),
        Value::Int(v) => Some(*v as f64),
        _ => None,
    }
    .filter(|v| v.is_finite() && *v >= 0.0)
}

fn parts<'a>(project: &'a Project, device: &Device) -> Vec<&'a Part> {
    device
        .parts
        .iter()
        .filter_map(|p| project.get::<Part>(*p))
        .collect()
}

/// The sizing view of one device.
pub fn sizing_device(project: &Project, device: &Device) -> SizingDevice {
    let size = device_size(project, device);
    let parts = parts(project, device);
    let first = |key: &str| {
        number(&device.properties, key)
            .or_else(|| parts.iter().find_map(|p| number(&p.properties, key)))
    };
    let loss = number(&device.properties, LOSS).or_else(|| {
        let values: Vec<f64> = parts
            .iter()
            .filter_map(|p| number(&p.properties, LOSS))
            .collect();
        (!values.is_empty()).then(|| values.iter().sum())
    });
    let clearance = CLEARANCES.iter().filter_map(|k| first(k)).reduce(f64::max);
    SizingDevice {
        width: size.width,
        height: size.height,
        depth: first(DEPTH).unwrap_or(0.0),
        power_loss: loss.unwrap_or(0.0),
        load_factor: first(LOAD).unwrap_or(1.0).min(1.0),
        clearance,
        guessed: size.guessed,
    }
}

/// The devices that take room on the plate of an enclosure: those that
/// belong in it (see [`crate::model::belonging`], the set `Place all` uses)
/// minus the ones mounted on the door or another surface. Devices in the
/// field are not in the enclosure at all.
pub fn sizing_devices(project: &Project, id: StructureId) -> Vec<SizingDevice> {
    let off = crate::model::off_plate_devices(project, id);
    crate::model::belonging(project, id)
        .into_iter()
        .filter(|d| !off.contains(&d.id.raw()))
        .map(|d| sizing_device(project, d))
        .collect()
}

/// What the sizing leaves out, for a line under the result: the tags of the
/// devices, sorted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LeftOut {
    /// Motors, valves and other devices outside the cabinet.
    pub field: Vec<String>,
    /// Devices on the door or another surface.
    pub door: Vec<String>,
}

/// Which devices of the enclosure `sizing_devices` does not count, and why.
pub fn left_out(project: &Project, id: StructureId) -> LeftOut {
    let off = crate::model::off_plate_devices(project, id);
    let tag = |d: &novo_core::Device| {
        d.tag
            .device_code()
            .map_or_else(|| d.tag.to_string(), |c| format!("-{c}"))
    };
    let mut out = LeftOut {
        field: crate::model::in_the_field(project, id)
            .into_iter()
            .map(tag)
            .collect(),
        door: crate::model::belonging(project, id)
            .into_iter()
            .filter(|d| off.contains(&d.id.raw()))
            .map(tag)
            .collect(),
    };
    out.field.sort_by(|a, b| novo_core::natural_cmp(a, b));
    out.door.sort_by(|a, b| novo_core::natural_cmp(a, b));
    out
}
