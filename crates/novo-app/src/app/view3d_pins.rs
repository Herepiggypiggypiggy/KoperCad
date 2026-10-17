//! Connection points in the 3D view: a small marker on every device where
//! the part says a terminal is (Parts window > Connection points). Measured
//! and typed points are green, estimated ones amber. Overlay data only; the
//! scene, picking and the camera fit ignore it. Shown with 3D view > Layers
//! > Connection points.

use glam::Vec3;
use novo_core::{Device, DeviceId, Project};
use novo_view3d::scene::ItemKind;
use novo_view3d::{Overlay, OverlayMesh, Scene};

/// Marker colours (sRGB, straight alpha).
const MEASURED: [u8; 4] = [90, 200, 120, 255];
const ESTIMATED: [u8; 4] = [235, 170, 60, 255];
/// Radius of a marker, mm.
const RADIUS_MM: f32 = 2.0;

/// One marker per terminal with a position, in the scene frame.
pub(super) fn overlays(project: &Project, scene: &Scene) -> Vec<Overlay> {
    let mut mesh = OverlayMesh {
        order: 5,
        ..OverlayMesh::default()
    };
    for item in scene.items.iter().filter(|i| i.id.kind == ItemKind::Device) {
        let Some(device) = project.get::<Device>(DeviceId::from(item.id.raw)) else {
            continue;
        };
        let to_world = scene.world_matrix(item);
        for part in &device.parts {
            let Some(part) = project.get::<novo_core::Part>(*part) else {
                continue;
            };
            let colour = if novo_parts::positions_estimated(&part.properties) == Some(true) {
                ESTIMATED
            } else {
                MEASURED
            };
            for name in novo_parts::placed_terminals(&part.properties) {
                let Some([x, y, z]) = novo_parts::terminal_position(&part.properties, &name) else {
                    continue;
                };
                // The box is centred in x and y, and stands off the plate in
                // z; the part's frame starts at its back-left-bottom corner.
                let local = Vec3::new(
                    x as f32 - item.size.x * 0.5,
                    y as f32 - item.size.y * 0.5,
                    z as f32,
                );
                octahedron(&mut mesh, to_world.transform_point3(local), colour);
            }
        }
    }
    if mesh.indices.is_empty() {
        Vec::new()
    } else {
        vec![Overlay::Mesh(mesh)]
    }
}

fn octahedron(mesh: &mut OverlayMesh, c: Vec3, colour: [u8; 4]) {
    let base = mesh.positions.len() as u32;
    for d in [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ] {
        mesh.positions.push(c + d * RADIUS_MM);
        mesh.colors.push(colour);
    }
    for (a, b, d) in [
        (0, 2, 4),
        (2, 1, 4),
        (1, 3, 4),
        (3, 0, 4),
        (2, 0, 5),
        (1, 2, 5),
        (3, 1, 5),
        (0, 3, 5),
    ] {
        mesh.indices.extend([base + a, base + b, base + d]);
    }
}
