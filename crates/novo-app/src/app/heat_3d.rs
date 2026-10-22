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
    /// Arrows on the slice plane instead of (or besides) streamlines.
    pub arrows: bool,
    /// Colour the device bodies by the air temperature in front of them.
    pub devices: bool,
    /// Pinned labels (tag, temperature, margin) on the hottest devices.
    pub labels: bool,
    /// The whole volume as see-through slices coloured by temperature.
    pub cloud: bool,
    /// Opacity of the cloud, 0 to 1.
    pub cloud_opacity: f32,
    /// Slices in the cloud: more is smoother and costs more to draw.
    pub cloud_layers: u8,
    /// Which way the cloud is cut, following the camera (not a user choice).
    pub cloud_view: super::heat_cloud::CloudView,
    /// Lines and dashes along the flow of heat (see `heat_carry`).
    pub carry: bool,
    /// The temperature scale the person set, °C (from, to); `None` is the
    /// automatic scale, ambient to the hottest cell.
    pub scale: Option<(f64, f64)>,
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
            arrows: false,
            devices: false,
            labels: true,
            cloud: false,
            cloud_opacity: 0.6,
            cloud_layers: super::heat_cloud::LAYERS as u8,
            cloud_view: super::heat_cloud::CloudView::default(),
            carry: false,
            scale: None,
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
    pub(super) fn world(&self, p: [f64; 3]) -> Vec3 {
        self.frame
            .world(p[0] - self.plate_x, p[1] - self.plate_y, p[2])
    }
}

/// Opacity of the slice and of the shell.
const SLICE_ALPHA: u8 = 190;
const SHELL_ALPHA: u8 = 110;
/// Most cells across a slice.
const SLICE_CELLS: f64 = 120.0;

/// How far in front of a device face the air is sampled, mm.
const DEVICE_SAMPLE_MM: f32 = 4.0;

/// The scene with every device body coloured by the air temperature at the
/// middle of its front face, on the same scale as the slice. A device the
/// solver frame does not contain keeps its own colour. The revision changes
/// with `salt` (the result id) so the view redraws.
pub(crate) fn tint_devices(
    scene: &novo_view3d::Scene,
    snap: &CfdSnapshot,
    ambient: f64,
    place: Place,
    scale: Option<(f64, f64)>,
    salt: u64,
) -> novo_view3d::Scene {
    let range = range_for(snap, ambient, scale);
    let mut out = scene.clone();
    out.revision = scene
        .revision
        .wrapping_add(salt.wrapping_mul(0x9E37_79B9) | 1);
    if !snap.is_consistent() {
        return out;
    }
    for item in &mut out.items {
        if item.id.kind != novo_view3d::ItemKind::Device {
            continue;
        }
        let front = glam::Vec3::new(0.0, 0.0, item.size.z + DEVICE_SAMPLE_MM);
        let w = scene.world_matrix(item).transform_point3(front);
        // The inverse of `Place::world`.
        let p = [
            f64::from(w.x) + place.plate_x,
            place.plate_y + f64::from(place.frame.height - w.y),
            f64::from(w.z),
        ];
        if !cfd::inside(snap, p) {
            continue;
        }
        let rgb = heat::ramp(range.t(f64::from(cfd::temperature_at(snap, p))));
        item.tint = Some(rgb);
    }
    out
}

/// Temperature scale of a result: ambient to the hottest cell.
pub(crate) fn range(snap: &CfdSnapshot, ambient: f64) -> Range {
    let hi = f64::from(snap.max_temperature_c()).max(ambient + 1.0);
    Range { lo: ambient, hi }
}

/// The scale in use: the person's when it is a usable span (finite, at least
/// 1 K wide), else the automatic one.
pub(crate) fn range_for(snap: &CfdSnapshot, ambient: f64, scale: Option<(f64, f64)>) -> Range {
    match scale {
        Some((lo, hi)) if lo.is_finite() && hi.is_finite() && hi >= lo + 1.0 => Range { lo, hi },
        _ => range(snap, ambient),
    }
}

/// All overlays for `settings`; empty when the result cannot be sliced.
pub(crate) fn overlays(
    snap: &CfdSnapshot,
    ambient: f64,
    place: Place,
    settings: &Heat3d,
    flow: &Flow,
) -> Vec<Overlay> {
    if !snap.is_consistent() {
        return Vec::new();
    }
    let range = range_for(snap, ambient, settings.scale);
    let mut out = Vec::new();
    if settings.cloud
        && let Some(m) = super::heat_cloud::mesh(
            snap,
            place,
            range,
            settings.cloud_view,
            settings.cloud_opacity,
            usize::from(settings.cloud_layers),
        )
    {
        out.push(Overlay::Mesh(m));
    }
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
        let l = stream_lines(flow);
        if !l.positions.is_empty() {
            out.push(Overlay::Lines(l));
        }
    }
    if settings.arrows {
        let l = super::heat_arrows::arrows(snap, place, settings.axis, settings.slice_at);
        if !l.positions.is_empty() {
            out.push(Overlay::Lines(l));
        }
    }
    out
}

/// The cell edges of the result along `axis`, mm.
pub(crate) fn slice_edges(snap: &CfdSnapshot, axis: Axis) -> &[f64] {
    match axis {
        Axis::X => &snap.grid.x_edges_mm,
        Axis::Y => &snap.grid.y_edges_mm,
        Axis::Z => &snap.grid.z_edges_mm,
    }
}

/// The centre of the cell next to the one holding `mm` (`dir` is 1 or -1),
/// on a grid that may be graded; at either end it stays on the last cell.
pub(crate) fn step_cell(edges: &[f64], mm: f64, dir: i32) -> f64 {
    let n = edges.len().saturating_sub(1).max(1);
    let here = edges.windows(2).position(|w| mm < w[1]).unwrap_or(n - 1);
    let to = (here as i64 + i64::from(dir.signum())).clamp(0, n as i64 - 1) as usize;
    0.5 * (edges[to] + edges[to + 1])
}

fn extent(snap: &CfdSnapshot, axis: Axis) -> (f64, f64) {
    let e = match axis {
        Axis::X => &snap.grid.x_edges_mm,
        Axis::Y => &snap.grid.y_edges_mm,
        Axis::Z => &snap.grid.z_edges_mm,
    };
    (e[0], e[e.len() - 1])
}

/// Where the plane at `at` (0 to 1 along `axis`) sits in the solver frame, mm.
pub(crate) fn slice_offset(snap: &CfdSnapshot, axis: Axis, at: f32) -> f64 {
    let (lo, hi) = extent(snap, axis);
    lo + f64::from(at.clamp(0.0, 1.0)) * (hi - lo)
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
    let offset = slice_offset(snap, axis, at);
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

/// One traced line in the scene frame with the air speed at each point.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FlowLine {
    pub points: Vec<Vec3>,
    pub speed: Vec<f32>,
    /// Distance from the start to each point, mm.
    pub along: Vec<f32>,
}

/// The streamlines of a result, traced once and drawn as lines and tracers.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Flow {
    pub lines: Vec<FlowLine>,
    /// Fastest air on any line, m/s.
    pub top: f32,
}

/// Traces streamlines from a lattice of seeds in front of the devices.
#[cfg(test)]
pub(crate) fn flow(snap: &CfdSnapshot, place: Place) -> Flow {
    flow_from(snap, place, &[])
}

/// As [`flow`], with more seeds in the solver frame, mm: the air coming in at
/// the openings, so it can be followed from the vent through the cabinet.
pub(crate) fn flow_from(snap: &CfdSnapshot, place: Place, extra: &[[f64; 3]]) -> Flow {
    const SEEDS: usize = 6;
    if !snap.is_consistent() {
        return Flow::default();
    }
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
    seeds.extend(extra.iter().copied().filter(|p| cfd::inside(snap, *p)));
    let mut out = Flow::default();
    for line in cfd::streamlines(snap, &seeds, Trace::default()) {
        if line.len() < 2 {
            continue;
        }
        let points: Vec<Vec3> = line.iter().map(|p| place.world(*p)).collect();
        let speed: Vec<f32> = line
            .iter()
            .map(|p| {
                let v = cfd::velocity_at(snap, *p);
                v[0].hypot(v[1]).hypot(v[2])
            })
            .collect();
        let mut along = vec![0.0_f32];
        for w in points.windows(2) {
            along.push(along[along.len() - 1] + w[0].distance(w[1]));
        }
        out.top = out.top.max(speed.iter().copied().fold(0.0, f32::max));
        out.lines.push(FlowLine {
            points,
            speed,
            along,
        });
    }
    out
}

/// The colour of air at `t` (0 to 1) of the fastest air: still air is a
/// medium blue, the fastest is near white, through cyan and mint. It stays
/// clear of the temperature ramp (purple to yellow) so the two never read as
/// one scale, and every step is readable on the dark and on the light view.
pub(crate) fn speed_rgb(t: f32) -> [u8; 3] {
    const STOPS: [(f32, [f32; 3]); 4] = [
        (0.0, [60.0, 120.0, 255.0]),
        (0.35, [30.0, 205.0, 240.0]),
        (0.7, [120.0, 240.0, 160.0]),
        (1.0, [250.0, 255.0, 235.0]),
    ];
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    let k = STOPS
        .windows(2)
        .position(|w| t <= w[1].0)
        .unwrap_or(STOPS.len() - 2);
    let (a, b) = (STOPS[k], STOPS[k + 1]);
    let f = (t - a.0) / (b.0 - a.0);
    let mix = |i: usize| (a.1[i] + (b.1[i] - a.1[i]) * f).round() as u8;
    [mix(0), mix(1), mix(2)]
}

/// Distance between arrowheads on a line, mm, and the size of one.
const ARROW_EVERY_MM: f32 = 130.0;
const ARROW_LEN_MM: f32 = 15.0;
/// Air slower than this share of the fastest gets no arrowheads.
const ARROW_MIN_SHARE: f32 = 0.04;

/// The streamlines as polylines coloured by air speed, with an arrowhead
/// every so often showing the way the air goes.
pub(crate) fn stream_lines(flow: &Flow) -> OverlayLines {
    let top = flow.top.max(1e-9);
    let mut out = OverlayLines {
        width_px: 2.6,
        order: 2,
        ..Default::default()
    };
    let colour = |v: f32| {
        let c = speed_rgb(v / top);
        [
            c[0],
            c[1],
            c[2],
            (150.0 + 105.0 * v / top).clamp(150.0, 255.0) as u8,
        ]
    };
    // The lines first, then the arrowheads: one polyline each.
    let mut heads: Vec<([Vec3; 3], [u8; 4])> = Vec::new();
    for line in &flow.lines {
        out.starts.push(out.positions.len() as u32);
        for (p, v) in line.points.iter().zip(&line.speed) {
            out.positions.push(*p);
            out.colors.push(colour(*v));
        }
        let len = line.along.last().copied().unwrap_or(0.0);
        let mut s = 0.5 * ARROW_EVERY_MM;
        while s < len {
            let (a, b) = (at(line, (s - 3.0).max(0.0)), at(line, (s + 3.0).min(len)));
            let v = line.speed[line
                .along
                .partition_point(|x| *x <= s)
                .min(line.speed.len() - 1)];
            let dir = (b - a).normalize_or_zero();
            if dir != Vec3::ZERO && v >= ARROW_MIN_SHARE * top {
                let tip = at(line, s);
                let up = if dir.y.abs() > 0.95 { Vec3::X } else { Vec3::Y };
                let side = dir.cross(up).normalize_or_zero() * (0.45 * ARROW_LEN_MM);
                let back = -dir * ARROW_LEN_MM;
                heads.push(([tip + back + side, tip, tip + back - side], colour(v)));
            }
            s += ARROW_EVERY_MM;
        }
    }
    for (points, c) in heads {
        out.starts.push(out.positions.len() as u32);
        for p in points {
            out.positions.push(p);
            out.colors.push(c);
        }
    }
    out
}

/// Lines in `rgb`, more opaque where the value on the line is higher.
pub(crate) fn stream_lines_in(flow: &Flow, rgb: [u8; 3], order: i32) -> OverlayLines {
    let top = flow.top.max(1e-9);
    let mut out = OverlayLines {
        width_px: 1.8,
        order,
        ..Default::default()
    };
    for line in &flow.lines {
        out.starts.push(out.positions.len() as u32);
        for (p, v) in line.points.iter().zip(&line.speed) {
            let a = (60.0 + 195.0 * v / top).clamp(60.0, 255.0) as u8;
            out.positions.push(*p);
            out.colors.push([rgb[0], rgb[1], rgb[2], a]);
        }
    }
    out
}

/// Particles per line and the share of a line a tracer's tail covers.
const TRACERS_PER_LINE: usize = 3;
const TAIL_SHARE: f32 = 0.12;
/// Longest tail, mm.
const TAIL_MAX_MM: f32 = 40.0;

/// Where on a line (0 to 1) a particle is at `time` seconds: faster lines
/// cycle faster, particles of one line are spread evenly.
fn tracer_phase(line: &FlowLine, top: f32, time: f64, k: usize) -> f32 {
    let mean = line.speed.iter().sum::<f32>() / line.speed.len() as f32;
    let rate = 0.12 + 0.4 * (mean / top.max(1e-9));
    ((time * f64::from(rate)) as f32 + k as f32 / TRACERS_PER_LINE as f32).rem_euclid(1.0)
}

/// The point at distance `s` along the line.
fn at(line: &FlowLine, s: f32) -> Vec3 {
    let i = line
        .along
        .partition_point(|a| *a <= s)
        .clamp(1, line.points.len() - 1);
    let (a, b) = (line.along[i - 1], line.along[i]);
    let t = if b > a {
        ((s - a) / (b - a)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    line.points[i - 1].lerp(line.points[i], t)
}

/// Bright dashes moving downwind along the streamlines at `time` seconds.
pub(crate) fn tracers(flow: &Flow, time: f64) -> OverlayLines {
    tracers_in(flow, time, [235, 250, 255], 3)
}

/// Dashes in `rgb` moving along the lines of `flow`.
pub(crate) fn tracers_in(flow: &Flow, time: f64, rgb: [u8; 3], order: i32) -> OverlayLines {
    let mut out = OverlayLines {
        width_px: 3.6,
        order,
        ..Default::default()
    };
    for line in &flow.lines {
        let len = line.along.last().copied().unwrap_or(0.0);
        if len <= 0.0 {
            continue;
        }
        let tail = (len * TAIL_SHARE).min(TAIL_MAX_MM);
        for k in 0..TRACERS_PER_LINE {
            let head = tracer_phase(line, flow.top, time, k) * len;
            let from = (head - tail).max(0.0);
            out.starts.push(out.positions.len() as u32);
            let mut pts = vec![at(line, from)];
            pts.extend(
                line.points
                    .iter()
                    .zip(&line.along)
                    .filter(|(_, a)| **a > from && **a < head)
                    .map(|(p, _)| *p),
            );
            pts.push(at(line, head));
            let n = pts.len();
            for (i, p) in pts.into_iter().enumerate() {
                // Fades towards the tail.
                let a = (255.0 * (i + 1) as f32 / n as f32) as u8;
                out.positions.push(p);
                out.colors.push([rgb[0], rgb[1], rgb[2], a]);
            }
        }
    }
    out
}
