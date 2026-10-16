//! The only place the scene builder reads the project: plate, rails, ducts,
//! placements, part depths and device tags, through queries only.

use std::sync::Arc;

use novo_core::{
    CabinetPlacement, Device, Project, Rail as ModelRail, RailKind, StructureId, Value,
};
use novo_layout::model::load;

use super::{Depth, DuctShape, Inputs, Options, from_layout};
use crate::scene::Scene;

const DEPTH_KEY: &str = "depth_mm";

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Float(v) => Some(*v),
        Value::Int(v) => Some(*v as f64),
        _ => None,
    }
}

/// The depth of a device from the first of its parts that has one.
pub fn device_depth(project: &Project, device: &Device) -> Depth {
    for part in &device.parts {
        if let Some(part) = project.get::<novo_core::Part>(*part)
            && let Some(d) = part.properties.get(DEPTH_KEY).and_then(number)
        {
            return Depth::from_part(Some(d));
        }
    }
    Depth::from_part(None)
}

/// Depths, labels and ducts of an enclosure.
pub fn inputs(project: &Project, id: StructureId) -> Inputs {
    let mut inputs = Inputs::default();
    for p in project
        .all::<CabinetPlacement>()
        .filter(|p| p.structure == id)
    {
        if let Some(device) = project.get::<Device>(p.device) {
            inputs
                .depths
                .insert(p.device.raw(), device_depth(project, device));
            if let Some(code) = device.tag.device_code() {
                inputs
                    .labels
                    .insert(p.device.raw(), Arc::from(format!("-{code}")));
            }
        }
    }
    for r in project.all::<ModelRail>().filter(|r| r.structure == id) {
        if r.kind == RailKind::Duct {
            inputs.ducts.push(DuctShape {
                id: r.id.raw(),
                x: r.x,
                y: r.y,
                length: r.length,
                vertical: r.vertical,
                width: r.width,
                height: r.height,
            });
        }
    }
    inputs
}

/// The scene of an enclosure, `None` while it has no plate. Build it again
/// after every edit and hand it to [`super::refresh`].
pub fn scene(project: &Project, id: StructureId, options: &Options) -> Option<Scene> {
    let layout = load(project, id)?;
    Some(from_layout(&layout, &inputs(project, id), options))
}
