//! Devices mounted on the walls, roof, floor and extra plates, drawn in the
//! 3D view as boxes standing out from their surface into the enclosure.
//! Devices on the mounting plate and the door are scene items and are not
//! repeated here. Overlay data only (picking and the camera fit ignore it);
//! shown with the surfaces (Layers > Surfaces, or a surface picked in the
//! Cabinet header).

use glam::Vec3;
use novo_layout::state::CabinetLayout;
use novo_layout::surfaces::MAIN_PLATE;
use novo_view3d::{Overlay, OverlayLines, OverlayMesh, PlateFrame};

use super::cabinet_surfaces::{SurfaceRow, SurfaceSummary};

/// Depth of a device that has no stored depth, as in the scene (guessed).
const DEFAULT_DEPTH_MM: f64 = 60.0;
/// Body and outline colours (sRGB), copper like the picked surface.
const BODY: [u8; 4] = [196, 140, 92, 255];
const BODY_PICKED: [u8; 4] = [232, 160, 96, 255];
const EDGE: [u8; 4] = [60, 40, 24, 255];

/// The overlays for the devices on surfaces other than the plate and the
/// door. `stored` is the stored layout (positions and depths), `summary`
/// says which surface each device is on.
pub(super) fn overlays(
    summary: &SurfaceSummary,
    stored: &CabinetLayout,
    picked: Option<&str>,
    frame: PlateFrame,
) -> Vec<Overlay> {
    let Some(plate) = summary.row(MAIN_PLATE) else {
        return Vec::new();
    };
    let origin = plate.origin;
    let world = |p: [f64; 3]| frame.world(p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]);
    let mut bodies = OverlayMesh {
        order: 12,
        ..OverlayMesh::default()
    };
    let mut edges = OverlayLines {
        width_px: 1.0,
        order: 13,
        ..OverlayLines::default()
    };
    for p in &stored.placements {
        let Some(row) = summary
            .assigned
            .get(&p.id)
            .and_then(|n| summary.row(n))
            .filter(|r| r.name != MAIN_PLATE && r.name != "door")
        else {
            continue;
        };
        let depth = stored
            .depths
            .get(&p.id)
            .copied()
            .unwrap_or(DEFAULT_DEPTH_MM);
        let fp = p.footprint();
        let corners = box_corners(row, fp.x, fp.y, fp.w, fp.h, depth).map(world);
        let colour = if picked == Some(row.name.as_str()) {
            BODY_PICKED
        } else {
            BODY
        };
        push_box(&mut bodies, &mut edges, corners, colour);
    }
    let mut out = Vec::new();
    if !bodies.indices.is_empty() {
        out.push(Overlay::Mesh(bodies));
        out.push(Overlay::Lines(edges));
    }
    out
}

/// The eight corners of a device box on `row`: the four of the footprint on
/// the surface, then the four `depth` mm out along the normal, in the same
/// order around.
fn box_corners(row: &SurfaceRow, x: f64, y: f64, w: f64, h: f64, depth: f64) -> [[f64; 3]; 8] {
    let at = |a: f64, b: f64, c: f64| {
        let mut p = row.origin;
        for (i, v) in p.iter_mut().enumerate() {
            *v += row.u[i] * a + row.v[i] * b + row.normal[i] * c;
        }
        p
    };
    [
        at(x, y, 0.0),
        at(x + w, y, 0.0),
        at(x + w, y + h, 0.0),
        at(x, y + h, 0.0),
        at(x, y, depth),
        at(x + w, y, depth),
        at(x + w, y + h, depth),
        at(x, y + h, depth),
    ]
}

fn push_box(mesh: &mut OverlayMesh, lines: &mut OverlayLines, c: [Vec3; 8], colour: [u8; 4]) {
    let base = mesh.positions.len() as u32;
    for p in c {
        mesh.positions.push(p);
        mesh.colors.push(colour);
    }
    // Both windings: the faces are shaded flat and the box can be seen from
    // either side of a wall, so no face is culled by a wrong turn.
    for [a, b, d, e] in [
        [0, 1, 2, 3],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ] {
        mesh.indices
            .extend([a, b, d, a, d, e, a, d, b, a, e, d].map(|i| base + i));
    }
    for path in [
        &[0usize, 1, 2, 3, 0][..],
        &[4, 5, 6, 7, 4][..],
        &[0, 4][..],
        &[1, 5][..],
        &[2, 6][..],
        &[3, 7][..],
    ] {
        lines.starts.push(lines.positions.len() as u32);
        for &i in path {
            lines.positions.push(c[i]);
            lines.colors.push(EDGE);
        }
    }
}
