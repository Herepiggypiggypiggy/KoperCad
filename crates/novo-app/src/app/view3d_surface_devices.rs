//! Devices mounted on the walls, roof, floor and extra plates in the 3D view:
//! scene items standing out from their surface into the enclosure, so they
//! can be picked, selected and dragged like plate devices. A drag reads in
//! the surface's own coordinates (the item carries its surface as the drag
//! plane) and ends as a move of the device's placement, which keeps its
//! surface. Devices on the mounting plate and the door are built by
//! novo-view3d and are not repeated here. Reach it: put a device on a surface
//! (Cabinet, Inspector, On surface), open the 3D view, drag it with the mouse.

use glam::{Mat3, Quat, Vec3};
use novo_layout::Rect;
use novo_layout::state::CabinetLayout;
use novo_layout::surfaces::MAIN_PLATE;
use novo_view3d::build::{Depth, Inputs};
use novo_view3d::drag_plane::DragPlane;
use novo_view3d::scene::{Item, ItemId, ItemKind, Mark, Pose, Scene, Shape};
use novo_view3d::widget::ItemDrag;
use novo_view3d::{Overlay, OverlayLines, OverlayMesh, PlateFrame};

use super::cabinet_surfaces::{SurfaceRow, SurfaceSummary};

/// Ghost colours (sRGB) for the place a dragged device would land.
const GHOST: [u8; 4] = [232, 160, 96, 110];
const GHOST_EDGE: [u8; 4] = [232, 160, 96, 255];

/// The surface a placement is on, when it is not the plate or the door
/// (those are built by novo-view3d).
fn surface_of(summary: &SurfaceSummary, device: u64) -> Option<&SurfaceRow> {
    summary
        .assigned
        .get(&device)
        .and_then(|n| summary.row(n))
        .filter(|r| r.name != MAIN_PLATE && r.name != "door")
}

/// A point of the enclosure frame (y down, the plate's corner somewhere
/// else) in the scene's world.
fn world(frame: &PlateFrame, plate_origin: [f64; 3], p: [f64; 3]) -> Vec3 {
    frame.world(
        p[0] - plate_origin[0],
        p[1] - plate_origin[1],
        p[2] - plate_origin[2],
    )
}

/// Direction of the enclosure frame in the scene's world (y is flipped).
fn dir(v: [f64; 3]) -> Vec3 {
    Vec3::new(v[0] as f32, -v[1] as f32, v[2] as f32)
}

/// The point of `row` at surface (`x`, `y`) lifted `lift` mm into the
/// enclosure, in the enclosure frame.
fn at(row: &SurfaceRow, x: f64, y: f64, lift: f64) -> [f64; 3] {
    let mut p = row.origin;
    for (i, c) in p.iter_mut().enumerate() {
        *c += row.u[i] * x + row.v[i] * y + row.normal[i] * lift;
    }
    p
}

/// Adds the devices on walls, roof, floor and extra plates to `scene`.
pub(super) fn push(
    scene: &mut Scene,
    summary: &SurfaceSummary,
    stored: &CabinetLayout,
    inputs: &Inputs,
) {
    let Some(plate) = summary.row(MAIN_PLATE) else {
        return;
    };
    let origin = plate.origin;
    let frame = scene.frame;
    let mut placements: Vec<_> = stored.placements.iter().collect();
    placements.sort_by_key(|p| p.id);
    for p in placements {
        let Some(row) = surface_of(summary, p.id) else {
            continue;
        };
        let depth = inputs
            .depths
            .get(&p.id)
            .copied()
            .unwrap_or_else(|| Depth::from_part(None));
        let fp = p.footprint();
        // The box grows along its local +z (out of the surface, into the
        // enclosure) and is centred on its footprint. Local y is the
        // surface's "up", so a wall device reads upright. When the surface's
        // axes turn the other way round local x is flipped; the box is
        // symmetric in x, so only its orientation changes, not its place.
        let u = dir(row.u);
        let up = -dir(row.v);
        let out = dir(row.normal);
        let lx = if u.cross(up).dot(out) < 0.0 { -u } else { u };
        let mut item = Item::new(
            ItemId::new(ItemKind::Device, p.id),
            Shape::Box,
            Pose {
                translation: world(
                    &frame,
                    origin,
                    at(row, fp.x + fp.w / 2.0, fp.y + fp.h / 2.0, 0.0),
                ),
                rotation: Quat::from_mat3(&Mat3::from_cols(lx, up, out)),
            },
            Vec3::new(fp.w as f32, fp.h as f32, depth.mm as f32),
        );
        if p.size.guessed || depth.guessed {
            item.mark = Mark::GuessedSize;
        }
        item.label = inputs.labels.get(&p.id).cloned();
        item.tint = inputs.classes.get(&p.id).map(|c| c.tint());
        item.drag_plane = Some(DragPlane {
            origin: world(&frame, origin, at(row, 0.0, 0.0, 0.0)),
            u,
            v: dir(row.v),
        });
        scene.push(item);
    }
}

/// A surface device being dragged: where it would land.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SurfaceDrag {
    pub device: u64,
    /// Where its corner would be on the surface, mm.
    pub at: (f64, f64),
    pub rect: Rect,
    pub surface: String,
    pub depth: f64,
    /// The button was released this frame.
    pub done: bool,
}

/// The landing of a drag of `drag` if it moves a device on a surface other
/// than the plate and the door. The corner is kept inside the surface's
/// usable area and rounded to whole millimetres.
pub(super) fn landing(
    summary: &SurfaceSummary,
    stored: &CabinetLayout,
    inputs: &Inputs,
    drag: &ItemDrag,
) -> Option<SurfaceDrag> {
    if drag.id.kind != ItemKind::Device {
        return None;
    }
    let row = surface_of(summary, drag.id.raw)?;
    let p = stored.placements.iter().find(|p| p.id == drag.id.raw)?;
    let fp = p.footprint();
    let usable = row.plate.usable();
    let clamp = |v: f64, lo: f64, hi: f64| v.min(hi.max(lo)).max(lo);
    let x = clamp(
        (fp.x + drag.delta.0).round(),
        usable.x,
        usable.right() - fp.w,
    );
    let y = clamp(
        (fp.y + drag.delta.1).round(),
        usable.y,
        usable.bottom() - fp.h,
    );
    let depth = inputs
        .depths
        .get(&p.id)
        .copied()
        .unwrap_or_else(|| Depth::from_part(None))
        .mm;
    Some(SurfaceDrag {
        device: p.id,
        at: (x, y),
        rect: Rect {
            x,
            y,
            w: fp.w,
            h: fp.h,
        },
        surface: row.name.clone(),
        depth,
        done: drag.done,
    })
}

/// The box a dragged device would take, drawn while the button is down.
pub(super) fn ghost(
    summary: &SurfaceSummary,
    land: &SurfaceDrag,
    frame: PlateFrame,
) -> Vec<Overlay> {
    let (Some(plate), Some(row)) = (summary.row(MAIN_PLATE), summary.row(&land.surface)) else {
        return Vec::new();
    };
    let r = land.rect;
    let corner = |x: f64, y: f64, lift: f64| world(&frame, plate.origin, at(row, x, y, lift));
    let c = [
        corner(r.x, r.y, 0.0),
        corner(r.x + r.w, r.y, 0.0),
        corner(r.x + r.w, r.y + r.h, 0.0),
        corner(r.x, r.y + r.h, 0.0),
        corner(r.x, r.y, land.depth),
        corner(r.x + r.w, r.y, land.depth),
        corner(r.x + r.w, r.y + r.h, land.depth),
        corner(r.x, r.y + r.h, land.depth),
    ];
    let mut mesh = OverlayMesh {
        translucent: true,
        order: 12,
        ..OverlayMesh::default()
    };
    mesh.positions.extend(c);
    mesh.colors.extend([GHOST; 8]);
    for [a, b, d, e] in [
        [0u32, 1, 2, 3],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ] {
        mesh.indices.extend([a, b, d, a, d, e, a, d, b, a, e, d]);
    }
    let mut lines = OverlayLines {
        width_px: 1.5,
        order: 13,
        ..OverlayLines::default()
    };
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
            lines.colors.push(GHOST_EDGE);
        }
    }
    vec![Overlay::Mesh(mesh), Overlay::Lines(lines)]
}
