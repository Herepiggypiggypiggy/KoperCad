//! The Heat layer in the 3D view: a slice plane coloured by temperature, an
//! iso-temperature shell and streamlines, built from the solver result as
//! overlays in the scene frame (see `novo_view3d::overlay`). Pure geometry;
//! `view3d.rs` hands the result to the view when something changed.

use glam::Vec3;
use novo_cfd::CfdSnapshot;
use novo_heatview::cfd::{self, Axis, Plane, Trace};
use novo_heatview::range::Range;
use novo_theme::heat;
use novo_view3d::{Overlay, OverlayLines, OverlayMesh, PlateFrame};

/// What the 3D view shows of the result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Heat3d {
    pub slice: bool,
    pub axis: Axis,
    /// Position of the slice plane along its axis, 0 to 1.
    pub slice_at: f32,
    pub iso: bool,
    /// Temperature of the shell between ambient (0) and the hottest cell (1).
    pub iso_at: f32,
    pub lines: bool,
}

impl Default for Heat3d {
    fn default() -> Self {
        Heat3d {
            slice: true,
            axis: Axis::Z,
            slice_at: 0.25,
            iso: false,
            iso_at: 0.6,
            lines: false,
        }
    }
}

/// Where the solver frame sits in the scene: the plate corner in the solver
/// frame (mm) and the plate height for the y flip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Place {
    pub plate_x: f64,
    pub plate_y: f64,
    pub frame: PlateFrame,
}

impl Place {
    fn world(&self, p: [f64; 3]) -> Vec3 {
        self.frame
            .world(p[0] - self.plate_x, p[1] - self.plate_y, p[2])
    }
}

/// Opacity of the slice and of the shell.
const SLICE_ALPHA: u8 = 190;
const SHELL_ALPHA: u8 = 110;
/// Most cells across a slice.
const SLICE_CELLS: f64 = 120.0;

/// Temperature scale of a result: ambient to the hottest cell.
pub(crate) fn range(snap: &CfdSnapshot, ambient: f64) -> Range {
    let hi = f64::from(snap.max_temperature_c()).max(ambient + 1.0);
    Range { lo: ambient, hi }
}

/// All overlays for `settings`; empty when the result cannot be sliced.
pub(crate) fn overlays(
    snap: &CfdSnapshot,
    ambient: f64,
    place: Place,
    settings: &Heat3d,
) -> Vec<Overlay> {
    if !snap.is_consistent() {
        return Vec::new();
    }
    let range = range(snap, ambient);
    let mut out = Vec::new();
    if settings.slice
        && let Some(m) = slice_mesh(snap, place, range, settings.axis, settings.slice_at)
    {
        out.push(Overlay::Mesh(m));
    }
    if settings.iso {
        let level = range.lo + f64::from(settings.iso_at.clamp(0.0, 1.0)) * (range.hi - range.lo);
        let m = shell_mesh(snap, place, range, level as f32);
        if !m.indices.is_empty() {
            out.push(Overlay::Mesh(m));
        }
    }
    if settings.lines {
        let l = stream_lines(snap, place);
        if !l.positions.is_empty() {
            out.push(Overlay::Lines(l));
        }
    }
    out
}

fn extent(snap: &CfdSnapshot, axis: Axis) -> (f64, f64) {
    let e = match axis {
        Axis::X => &snap.grid.x_edges_mm,
        Axis::Y => &snap.grid.y_edges_mm,
        Axis::Z => &snap.grid.z_edges_mm,
    };
    (e[0], e[e.len() - 1])
}

/// The plane at `at` (0 to 1 along `axis`) as coloured quads; cells inside
/// solids are left out so devices stay visible.
pub(crate) fn slice_mesh(
    snap: &CfdSnapshot,
    place: Place,
    range: Range,
    axis: Axis,
    at: f32,
) -> Option<OverlayMesh> {
    let (lo, hi) = extent(snap, axis);
    let offset = lo + f64::from(at.clamp(0.0, 1.0)) * (hi - lo);
    let window = cfd::whole_window(snap, axis);
    let cell = (window.w.max(window.h) / SLICE_CELLS).max(4.0);
    let s = cfd::slice(
        snap,
        Plane {
            axis,
            offset_mm: offset,
        },
        window,
        cell,
    )?;
    let g = s.temperature.grid;
    let point = |u: f64, v: f64| match axis {
        Axis::Z => [u, v, offset],
        Axis::X => [offset, v, u],
        Axis::Y => [u, offset, v],
    };
    let mut m = OverlayMesh {
        translucent: true,
        order: 0,
        ..Default::default()
    };
    for r in 0..g.rows {
        for c in 0..g.cols {
            if s.solid[r * g.cols + c] {
                continue;
            }
            let rgb = heat::ramp(range.t(f64::from(s.temperature.values[r * g.cols + c])));
            let colour = [rgb[0], rgb[1], rgb[2], SLICE_ALPHA];
            let (u0, v0) = (
                window.u0 + c as f64 * g.cell_w,
                window.v0 + r as f64 * g.cell_h,
            );
            let (u1, v1) = (u0 + g.cell_w, v0 + g.cell_h);
            let base = m.positions.len() as u32;
            for (u, v) in [(u0, v0), (u1, v0), (u1, v1), (u0, v1)] {
                m.positions.push(place.world(point(u, v)));
                m.colors.push(colour);
            }
            m.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    (!m.indices.is_empty()).then_some(m)
}

/// The surface at `level` degrees as a translucent shell in the ramp colour
/// of that temperature.
pub(crate) fn shell_mesh(
    snap: &CfdSnapshot,
    place: Place,
    range: Range,
    level: f32,
) -> OverlayMesh {
    let rgb = heat::ramp(range.t(f64::from(level)));
    let mut m = OverlayMesh {
        translucent: true,
        order: 1,
        ..Default::default()
    };
    for tri in cfd::iso_surface(snap, level) {
        for p in tri {
            m.indices.push(m.positions.len() as u32);
            m.positions
                .push(place.world([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]));
            m.colors.push([rgb[0], rgb[1], rgb[2], SHELL_ALPHA]);
        }
    }
    m
}

/// Streamlines from a lattice of seeds in front of the devices; faster air
/// is more opaque.
pub(crate) fn stream_lines(snap: &CfdSnapshot, place: Place) -> OverlayLines {
    const SEEDS: usize = 6;
    let (x0, x1) = extent(snap, Axis::X);
    let (y0, y1) = extent(snap, Axis::Y);
    let z = cfd::front_plane_z(snap);
    let mut seeds = Vec::new();
    for j in 0..SEEDS {
        for i in 0..SEEDS {
            let f = |k: usize| (k as f64 + 0.5) / SEEDS as f64;
            seeds.push([x0 + f(i) * (x1 - x0), y0 + f(j) * (y1 - y0), z]);
        }
    }
    let lines = cfd::streamlines(snap, &seeds, Trace::default());
    let speed = |p: [f64; 3]| {
        let v = cfd::velocity_at(snap, p);
        f64::from(v[0].hypot(v[1]).hypot(v[2]))
    };
    let top = lines
        .iter()
        .flatten()
        .map(|p| speed(*p))
        .fold(0.0_f64, f64::max)
        .max(1e-9);
    let mut out = OverlayLines {
        width_px: 1.8,
        order: 2,
        ..Default::default()
    };
    for line in lines.iter().filter(|l| l.len() >= 2) {
        out.starts.push(out.positions.len() as u32);
        for p in line {
            let a = (60.0 + 195.0 * speed(*p) / top).clamp(60.0, 255.0) as u8;
            out.positions.push(place.world(*p));
            out.colors.push([110, 200, 255, a]);
        }
    }
    out
}
