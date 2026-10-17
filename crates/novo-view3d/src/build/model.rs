//! The only place the scene builder reads the project: plate, rails, ducts,
//! placements, part depths and device tags, through queries only.

use std::sync::Arc;

use novo_checks::standards::Colour;
use novo_core::{
    CabinetPlacement, Device, DeviceKind, Netlist, PotentialKind, Project, Rail as ModelRail,
    RailKind, StructureId, Value, WireId,
};
use novo_layout::enclosure::{Enclosure, Hinge};
use novo_layout::model::{enclosure_of, load};
use novo_routing::Kind;
use novo_routing::model::{route_project, wire_connections};

use super::{
    Depth, DoorSpec, DuctShape, HingeSide, Inputs, Options, Shell, WireColour, WireInput, WireKind,
    from_layout,
};
use crate::appearance::{DeviceClass, KindHint};
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
            let hint = match device.kind {
                DeviceKind::General => KindHint::General,
                DeviceKind::TerminalStrip => KindHint::TerminalStrip,
                DeviceKind::Connector => KindHint::Connector,
                DeviceKind::Plc => KindHint::Plc,
            };
            inputs.classes.insert(
                p.device.raw(),
                DeviceClass::of(hint, device.tag.letter_code()),
            );
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

/// How a wire colour is drawn: IEC 60757 colours as insulation, the
/// protective green-yellow and the grounded white with a blue stripe as two
/// colours. Red, blue and green-yellow are the palette's wire colours
/// (3d-look.md section 3).
pub fn insulation(colour: Colour) -> WireColour {
    let one = |body| WireColour { body, stripe: None };
    match colour {
        Colour::Black => one([0x1B, 0x1B, 0x1E]),
        Colour::Brown => one([0x6B, 0x42, 0x26]),
        Colour::Red => one([0xB3, 0x3A, 0x3A]),
        Colour::Orange => one([0xE0, 0x7B, 0x22]),
        Colour::Yellow => one([0xE3, 0xC9, 0x3A]),
        Colour::Green => one([0x3F, 0x9A, 0x47]),
        Colour::Blue => one([0x2F, 0x6F, 0xD0]),
        Colour::Violet => one([0x7A, 0x4F, 0xB0]),
        Colour::Grey => one([0x8C, 0x8F, 0x94]),
        Colour::White => one([0xEC, 0xEA, 0xE5]),
        Colour::Pink => one([0xE7, 0x9A, 0xB5]),
        Colour::Turquoise => one([0x2B, 0xB3, 0xB3]),
        Colour::GreenYellow => WireColour {
            body: [0x7F, 0xB0, 0x20],
            stripe: Some([0xE3, 0xD3, 0x4A]),
        },
        Colour::WhiteBlue => WireColour {
            body: [0xEC, 0xEA, 0xE5],
            stripe: Some([0x2F, 0x6F, 0xD0]),
        },
    }
}

/// The colour a wire has in the schematic: its own colour, else the name of
/// its cable core when that is a colour code, else the colour its potential
/// has in 3d-look.md section 3: protective earth green-yellow, neutral and
/// 0 V blue, positive DC red. A line conductor stays plain: whether it is
/// power (black) or control (red) is not in the model. `None` when none of
/// these applies or the text is not a colour the standards checks know.
pub fn wire_colour(project: &Project, wire: WireId) -> Option<WireColour> {
    wire_colour_in(project, &project.netlist(), wire)
}

/// [`wire_colour`] with the netlist at hand, for many wires.
fn wire_colour_in(project: &Project, netlist: &Netlist, wire: WireId) -> Option<WireColour> {
    let w = project.wire(wire)?;
    let own = w.color.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let colour = match own {
        Some(text) => Colour::parse(text),
        None => {
            let named = project.core_of_wire(wire).and_then(|(cable, core)| {
                let core = project.cable(cable)?.cores.get(core)?;
                Colour::parse(&core.name)
            });
            named.or_else(|| {
                let potential = netlist.net(netlist.net_of_wire(w)?).potential()?;
                match project.potential(potential)?.kind {
                    PotentialKind::ProtectiveEarth => Some(Colour::GreenYellow),
                    PotentialKind::Neutral | PotentialKind::DcNegative => Some(Colour::Blue),
                    PotentialKind::DcPositive => Some(Colour::Red),
                    PotentialKind::Line | PotentialKind::Pen | PotentialKind::Signal => None,
                }
            })
        }
    };
    colour.map(insulation)
}

/// The routed wires that lie on the plate of `id`, ready for the scene. The
/// routes are the Cabinet's, so the 3D view and the wire list agree.
pub fn wires(project: &Project, id: StructureId) -> Vec<WireInput> {
    wires_with(project, id, &route_project(project))
}

/// [`wires`] from routes already made, so a screen that also reads lengths
/// routes the project once.
pub fn wires_with(
    project: &Project,
    id: StructureId,
    routed: &novo_routing::model::Routed,
) -> Vec<WireInput> {
    let Some((_, plan)) = routed.plans.iter().find(|(s, _)| *s == id) else {
        return Vec::new();
    };
    let netlist = project.netlist();
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
            colour: wire_colour_in(project, &netlist, WireId::from(r.job)),
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
