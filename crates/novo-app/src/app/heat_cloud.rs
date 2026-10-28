//! The Heat cloud: the whole cabinet volume drawn as a stack of see-through
//! slices coloured by air temperature, so heat reads in 3D in one view. Air
//! near ambient is invisible and hotter air is more opaque; the slices are
//! cut across the axis the camera looks along most and drawn far to near, so
//! the blend is right without a volume renderer (cheap on integrated
//! graphics: only warm cells make quads, and the stack is rebuilt only when
//! the main viewing direction changes, not per frame).
//!
//! It shows the solver's air temperature as it is on the slices; between
//! slices nothing is interpolated, so a thin plume narrower than the slice
//! gap can be under-drawn. More layers cost more quads.

use glam::Vec3;
use novo_cfd::CfdSnapshot;
use novo_heatview::cfd::{self, Axis, Plane};
use novo_heatview::range::Range;
use novo_theme::heat;
use novo_view3d::OverlayMesh;

use super::heat_3d::Place;

/// Slices through the volume by default, and the range the user can pick.
pub(crate) const LAYERS: usize = 16;
pub(crate) const LAYERS_RANGE: std::ops::RangeInclusive<u8> = 4..=40;
/// Cells across the longer side of one slice.
const CELLS: f64 = 48.0;
/// Air whose share of the air's own span (ambient to the hot end of the air)
/// is below this is not drawn.
const FLOOR: f32 = 0.04;
/// Most opaque a single layer gets at full opacity and full heat. The
/// stack of layers adds up, so the full setting has to be plainly visible.
const LAYER_ALPHA: f32 = 0.55;
/// The hot end of the cloud's opacity is this share of the air cells (by
/// temperature): a plume in a few cells does not wash the rest out.
const HOT_END_SHARE: f64 = 0.97;
/// The shaping exponent at the lowest and the highest contrast setting: below
/// 1 lifts slightly warm air, above 1 keeps only the hot air.
const GAMMA_FLAT: f32 = 1.6;
const GAMMA_STRONG: f32 = 0.4;
/// Default of the contrast setting, 0 to 1.
pub(crate) const CONTRAST: f32 = 0.6;

/// The hot end of the cloud's own scale: the hot end of the air (not of the
/// devices, which are far above it), at least 1 K over `lo`. Colour and
/// opacity both run from `lo` to here, so a cabinet whose air is 8 K over
/// ambient shows that.
pub(crate) fn air_hot_end(snap: &CfdSnapshot, lo: f64) -> f64 {
    let mut air: Vec<f32> = snap
        .material
        .iter()
        .zip(&snap.temperature_c)
        .filter(|(m, _)| **m == 0)
        .map(|(_, t)| *t)
        .filter(|t| t.is_finite())
        .collect();
    if air.is_empty() {
        return lo + 1.0;
    }
    air.sort_by(f32::total_cmp);
    let at = ((air.len() - 1) as f64 * HOT_END_SHARE).round() as usize;
    f64::from(air[at]).max(lo + 1.0)
}

/// The way the camera looks, reduced to the axis the stack is cut across and
/// the side the camera is on (solver frame, `y` down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CloudView {
    pub axis: Axis,
    /// The camera is on the high side of `axis`.
    pub high_side: bool,
}

impl Default for CloudView {
    fn default() -> Self {
        // Looking at the plate from the door side.
        CloudView {
            axis: Axis::Z,
            high_side: true,
        }
    }
}

impl CloudView {
    /// The view for a camera whose unit vector from target to eye is `eye`
    /// in the scene frame, keeping `prev` unless another axis leads clearly
    /// (a camera near a diagonal does not make the stack flip back and forth).
    pub(crate) fn from_eye(eye: Vec3, prev: CloudView) -> CloudView {
        // Scene y is up, solver y is down.
        let v = [eye.x, -eye.y, eye.z];
        let comp = |a: Axis| match a {
            Axis::X => v[0],
            Axis::Y => v[1],
            Axis::Z => v[2],
        };
        let mut best = prev.axis;
        for a in [Axis::X, Axis::Y, Axis::Z] {
            if comp(a).abs() > comp(best).abs() * 1.15 {
                best = a;
            }
        }
        CloudView {
            axis: best,
            high_side: comp(best) >= 0.0,
        }
    }
}

/// The stack as one translucent mesh, far slices first. `opacity` is 0 to 1;
/// `contrast` (0 to 1) shapes how fast opacity rises with the air temperature.
pub(crate) fn mesh(
    snap: &CfdSnapshot,
    place: Place,
    range: Range,
    view: CloudView,
    opacity: f32,
    layers: usize,
    contrast: f32,
) -> Option<OverlayMesh> {
    if !snap.is_consistent() || layers == 0 || opacity <= 0.0 {
        return None;
    }
    let hot_end = air_hot_end(snap, range.lo);
    let gamma = GAMMA_FLAT + (GAMMA_STRONG - GAMMA_FLAT) * contrast.clamp(0.0, 1.0);
    let axis = view.axis;
    let edges = match axis {
        Axis::X => &snap.grid.x_edges_mm,
        Axis::Y => &snap.grid.y_edges_mm,
        Axis::Z => &snap.grid.z_edges_mm,
    };
    let (lo, hi) = (edges[0], edges[edges.len() - 1]);
    let window = cfd::whole_window(snap, axis);
    let cell = (window.w.max(window.h) / CELLS).max(2.0);
    let mut m = OverlayMesh {
        translucent: true,
        order: -1,
        ..Default::default()
    };
    // Each layer stands for 1/layers of the depth; fewer layers need more
    // opacity each to keep the total the same.
    let per_layer = LAYER_ALPHA * (LAYERS as f32 / layers as f32).min(2.0);
    for n in 0..layers {
        // Far to near: the camera on the high side sees low offsets last.
        let n = if view.high_side { n } else { layers - 1 - n };
        let offset = lo + (n as f64 + 0.5) / layers as f64 * (hi - lo);
        let Some(s) = cfd::slice(
            snap,
            Plane {
                axis,
                offset_mm: offset,
            },
            window,
            cell,
        ) else {
            continue;
        };
        let g = s.temperature.grid;
        let point = |u: f64, v: f64| match axis {
            Axis::Z => [u, v, offset],
            Axis::X => [offset, v, u],
            Axis::Y => [u, offset, v],
        };
        for r in 0..g.rows {
            for c in 0..g.cols {
                let i = r * g.cols + c;
                if s.solid[i] {
                    continue;
                }
                let temp = f64::from(s.temperature.values[i]);
                // Opacity follows the air's own span; the colour follows the scale.
                let share = ((temp - range.lo) / (hot_end - range.lo)) as f32;
                if share < FLOOR {
                    continue;
                }
                // Eases in from the floor so the cloud has no hard edge.
                let k = ((share - FLOOR) / (1.0 - FLOOR)).clamp(0.0, 1.0);
                let a = (opacity.clamp(0.0, 1.0) * per_layer * k.powf(gamma) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                if a == 0 {
                    continue;
                }
                // The cloud has its own scale, ambient to the hot end of the air
                // (its key sits in the Heat strip): on the shared scale, which
                // runs up to the devices, ordinary warm air is a dark purple on
                // a dark background and cannot be told apart.
                let rgb = heat::ramp(share.clamp(0.0, 1.0));
                let colour = [rgb[0], rgb[1], rgb[2], a];
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
    }
    (!m.indices.is_empty()).then_some(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::heat_3d_tests::{place, snapshot};

    const RANGE: Range = Range { lo: 20.0, hi: 30.0 };

    fn view(axis: Axis, high_side: bool) -> CloudView {
        CloudView { axis, high_side }
    }

    #[test]
    fn warm_air_is_drawn_and_ambient_air_is_not() {
        // T = 20 + x/10: the left tenth is under the floor.
        let m = mesh(
            &snapshot(),
            place(),
            RANGE,
            view(Axis::Z, true),
            1.0,
            8,
            CONTRAST,
        )
        .unwrap();
        assert!(m.is_valid() && m.translucent);
        // Scene x = solver x - 10. The coldest cells, at ambient, are left out.
        assert!(m.positions.iter().all(|p| p.x > -10.0 + 1.0));
        assert!(m.positions.iter().any(|p| p.x > 80.0));
    }

    #[test]
    fn hotter_air_is_more_opaque_and_the_ramp_colour_follows() {
        let m = mesh(
            &snapshot(),
            place(),
            RANGE,
            view(Axis::Z, true),
            1.0,
            4,
            CONTRAST,
        )
        .unwrap();
        let alpha_at = |x: f32| {
            let i = m.positions.iter().position(|p| p.x > x).unwrap();
            m.colors[i]
        };
        assert!(alpha_at(70.0)[3] > alpha_at(10.0)[3]);
        assert_ne!(alpha_at(70.0)[..3], alpha_at(10.0)[..3]);
    }

    #[test]
    fn opacity_scales_the_alpha_and_zero_draws_nothing() {
        let s = snapshot();
        let top = |o: f32| {
            mesh(&s, place(), RANGE, view(Axis::Z, true), o, 4, CONTRAST)
                .unwrap()
                .colors
                .iter()
                .map(|c| c[3])
                .max()
                .unwrap()
        };
        assert!(top(1.0) > top(0.4));
        assert!(mesh(&s, place(), RANGE, view(Axis::Z, true), 0.0, 4, CONTRAST).is_none());
    }

    #[test]
    fn the_strongest_setting_is_plainly_visible_and_contrast_lifts_warm_air() {
        let s = snapshot();
        let strongest = |contrast: f32| {
            mesh(&s, place(), RANGE, view(Axis::Z, true), 1.0, 16, contrast).unwrap()
        };
        let top = strongest(CONTRAST)
            .colors
            .iter()
            .map(|c| c[3])
            .max()
            .unwrap();
        // One layer at full heat is over a quarter opaque at the top setting.
        assert!(top >= 70, "{top}");
        // Cooler air (the left third of the plate) is more opaque with more contrast.
        let cool_alpha = |contrast: f32| -> u32 {
            let m = strongest(contrast);
            m.positions
                .iter()
                .zip(&m.colors)
                .filter(|(p, _)| p.x < 30.0)
                .map(|(_, c)| u32::from(c[3]))
                .sum()
        };
        assert!(cool_alpha(1.0) > cool_alpha(0.0));
    }

    #[test]
    fn opacity_is_stretched_over_the_air_not_the_devices() {
        // A cabinet whose air is a few kelvin over ambient with a device far
        // above it: the hot end of the opacity is the air's, not the device's.
        let s = snapshot();
        let hot = air_hot_end(&s, 20.0);
        assert!(hot >= 21.0 && hot <= f64::from(s.max_temperature_c()));
    }

    #[test]
    fn layers_are_drawn_far_to_near() {
        let s = snapshot();
        let depth = |high| {
            let m = mesh(&s, place(), RANGE, view(Axis::Z, high), 1.0, 6, CONTRAST).unwrap();
            // z of each quad's first vertex, in draw order.
            let z: Vec<f32> = m.positions.chunks(4).map(|q| q[0].z).collect();
            z
        };
        // Camera on the door side (high z): the plate side (low z) is far.
        let from_door = depth(true);
        assert!(from_door.windows(2).all(|w| w[0] <= w[1] + 1e-4));
        let from_plate = depth(false);
        assert!(from_plate.windows(2).all(|w| w[0] >= w[1] - 1e-4));
    }

    #[test]
    fn the_stack_axis_follows_the_camera_with_some_stickiness() {
        let prev = CloudView::default();
        // Looking from the door (+z), a little up.
        let v = CloudView::from_eye(Vec3::new(0.2, 0.3, 0.9).normalize(), prev);
        assert_eq!(v, view(Axis::Z, true));
        // From the right side.
        let v = CloudView::from_eye(Vec3::new(0.95, 0.1, 0.2).normalize(), prev);
        assert_eq!(v, view(Axis::X, true));
        // Scene up is solver -y: a camera above sees the stack along -y.
        let v = CloudView::from_eye(Vec3::new(0.0, 1.0, 0.1).normalize(), prev);
        assert_eq!(v, view(Axis::Y, false));
        // Near a diagonal the old axis stays.
        let v = CloudView::from_eye(Vec3::new(0.7, 0.0, 0.72).normalize(), prev);
        assert_eq!(v.axis, Axis::Z);
    }
}
