//! The only place the scene builder reads the project: plate, rails, ducts,
//! placements, part depths and device tags, through queries only.

use std::sync::Arc;

use novo_core::{
    CabinetPlacement, Device, Project, Rail as ModelRail, RailKind, StructureId, Value,
};
use novo_layout::enclosure::{Enclosure, Hinge};
use novo_layout::model::{enclosure_of, load};
use novo_routing::Kind;
use novo_routing::model::{route_project, wire_connections};

use super::{
    Depth, DoorSpec, DuctShape, HingeSide, Inputs, Options, Shell, WireInput, WireKind, from_layout,
};
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

/// The shell of an enclosure: the depth from the plate surface to the inside
/// of the door and the door's hinge side, closed. The default shell while the
/// enclosure has no interior yet.
pub fn shell(project: &Project, id: StructureId) -> Shell {
    match enclosure_of(project, id) {
        Some(e) => shell_of(&e),
        None => Shell::default(),
    }
}

fn shell_of(e: &Enclosure) -> Shell {
    Shell {
        depth: (e.depth - e.plate_setback).max(0.0),
        door: e.door.map(|d| DoorSpec {
            hinge: match d.hinge {
                Hinge::Left => HingeSide::Left,
                Hinge::Right => HingeSide::Right,
            },
            open_deg: 0.0,
        }),
    }
}

/// How far the enclosure's door opens, in degrees; `None` without a door.
pub fn door_limit_deg(project: &Project, id: StructureId) -> Option<f64> {
    let door = enclosure_of(project, id)?.door?;
    Some(door.max_angle.clamp(0.0, 180.0))
}

/// The routed wires that lie on the plate of `id`, ready for the scene. The
/// routes are the Cabinet's, so the 3D view and the wire list agree.
pub fn wires(project: &Project, id: StructureId) -> Vec<WireInput> {
    let routed = route_project(project);
    let Some((_, plan)) = routed.plans.iter().find(|(s, _)| *s == id) else {
        return Vec::new();
    };
    let diameters: std::collections::HashMap<u64, Option<f64>> = wire_connections(project)
        .into_iter()
        .map(|c| (c.id, c.diameter_mm))
        .collect();
    plan.routes
        .iter()
        .map(|r| WireInput {
            id: r.job,
            kind: match r.kind {
                Kind::Ducted => WireKind::Ducted,
                Kind::FreeAir => WireKind::FreeAir,
            },
            points: r.points.iter().map(|p| (p.x, p.y)).collect(),
            ducts: r.ducts.clone(),
            diameter_mm: diameters.get(&r.job).copied().flatten(),
        })
        .collect()
}

/// The builder options of an enclosure: its own shell, and its routed wires
/// when `with_wires`.
pub fn options(project: &Project, id: StructureId, with_wires: bool) -> Options {
    Options {
        shell: shell(project, id),
        wires: if with_wires {
            wires(project, id)
        } else {
            Vec::new()
        },
    }
}
