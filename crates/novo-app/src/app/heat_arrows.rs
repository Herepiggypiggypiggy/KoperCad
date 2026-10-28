//! Airflow as arrows on the slice plane: a coarse grid of arrows for reading
//! the direction and size of the flow at the plane. Length, width and colour
//! all follow the air speed on the scale shared with the other Flow modes
//! (see `heat_flow`), so two planes can be compared by length.

use glam::Vec3;
use novo_cfd::CfdSnapshot;
use novo_heatview::cfd::{self, Axis};
use novo_view3d::OverlayLines;

use super::heat_3d::Place;
use super::heat_flow::{self, FlowView};

/// Arrows across the longer side of the plane at no strength and at full.
const ACROSS: (f64, f64) = (7.0, 20.0);
/// Shortest arrow, as a share of the grid spacing.
const MIN_LEN: f64 = 0.3;

type Sample = ([f64; 3], [f32; 3], f32);

/// The air on the plane at a coarse grid: the samples and the grid spacing in
/// mm. `across` is how many cells span the longer side.
fn sample(snap: &CfdSnapshot, axis: Axis, at: f32, across: f64) -> (Vec<Sample>, f64) {
    let edges = match axis {
        Axis::X => &snap.grid.x_edges_mm,
        Axis::Y => &snap.grid.y_edges_mm,
        Axis::Z => &snap.grid.z_edges_mm,
    };
    let (lo, hi) = (edges[0], edges[edges.len() - 1]);
    let offset = lo + f64::from(at.clamp(0.0, 1.0)) * (hi - lo);
    let w = cfd::whole_window(snap, axis);
    let step = (w.w.max(w.h) / across).max(1.0);
    let (nu, nv) = ((w.w / step).floor() as usize, (w.h / step).floor() as usize);
    let point = |u: f64, v: f64| match axis {
        Axis::Z => [u, v, offset],
        Axis::X => [offset, v, u],
        Axis::Y => [u, offset, v],
    };
    let mut samples = Vec::new();
    for r in 0..nv {
        for c in 0..nu {
            let p = point(
                w.u0 + (c as f64 + 0.5) * step,
                w.v0 + (r as f64 + 0.5) * step,
            );
            if !cfd::inside(snap, p) || cfd::solid_at(snap, p) {
                continue;
            }
            let v = cfd::velocity_at(snap, p);
            let speed = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            samples.push((p, v, speed));
        }
    }
    (samples, step)
}

/// Arrows at plane `at` (0 to 1 along `axis`), in the scene frame. Cells
/// inside solids get none; air under 3 % of `scale_top` gets none. A longer
/// arrow is faster air in m/s, not relative to the plane: `scale_top` is the
/// speed of an arrow one cell long. `view.strength` sets how many.
pub(crate) fn arrows(
    snap: &CfdSnapshot,
    place: Place,
    axis: Axis,
    at: f32,
    view: &FlowView,
    scale_top: f32,
) -> OverlayLines {
    let mut out = OverlayLines {
        width_px: 2.2,
        order: 3,
        ..Default::default()
    };
    if !snap.is_consistent() {
        return out;
    }
    let across = ACROSS.0 + (ACROSS.1 - ACROSS.0) * f64::from(view.strength.clamp(0.0, 1.0));
    let (samples, step) = sample(snap, axis, at, across);
    for (p, v, speed) in samples {
        if speed < heat_flow::QUIET_SHARE * scale_top {
            continue;
        }
        let share = f64::from((speed / scale_top).clamp(0.0, 1.0));
        let len = step * (MIN_LEN + (0.9 - MIN_LEN) * share);
        let dir = [
            f64::from(v[0] / speed),
            f64::from(v[1] / speed),
            f64::from(v[2] / speed),
        ];
        let at = |t: f64, side: [f64; 3], s: f64| {
            let q = [
                p[0] + dir[0] * len * t + side[0] * s,
                p[1] + dir[1] * len * t + side[1] * s,
                p[2] + dir[2] * len * t + side[2] * s,
            ];
            place.world(q)
        };
        // A direction across the arrow, in the plane when the air runs in it.
        let normal = match axis {
            Axis::X => [1.0, 0.0, 0.0],
            Axis::Y => [0.0, 1.0, 0.0],
            Axis::Z => [0.0, 0.0, 1.0],
        };
        let mut side = cross(dir, normal);
        if norm(side) < 0.2 {
            // Air along the plane normal: any perpendicular will do.
            side = cross(dir, [0.0, 0.0, 1.0]);
            if norm(side) < 0.2 {
                side = cross(dir, [1.0, 0.0, 0.0]);
            }
        }
        let n = norm(side);
        let side = [side[0] / n, side[1] / n, side[2] / n];
        let head = len * 0.28;
        let rgb = shade(speed, scale_top);
        let tip = at(0.5, [0.0; 3], 0.0);
        let tail = at(-0.5, [0.0; 3], 0.0);
        let wing = |s: f64| {
            let q = [
                tip_solver(p, dir, len, 0.5)[0] - dir[0] * head + side[0] * s,
                tip_solver(p, dir, len, 0.5)[1] - dir[1] * head + side[1] * s,
                tip_solver(p, dir, len, 0.5)[2] - dir[2] * head + side[2] * s,
            ];
            place.world(q)
        };
        let wings = [wing(head * 0.6), wing(-head * 0.6)];
        push(&mut out, &[tail, tip], rgb);
        push(&mut out, &[wings[0], tip, wings[1]], rgb);
    }
    out
}

fn tip_solver(p: [f64; 3], dir: [f64; 3], len: f64, t: f64) -> [f64; 3] {
    [
        p[0] + dir[0] * len * t,
        p[1] + dir[1] * len * t,
        p[2] + dir[2] * len * t,
    ]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// The colour of air at `speed` on the shared scale, fainter when slow.
fn shade(speed: f32, scale_top: f32) -> [u8; 4] {
    let c = heat_flow::colour(speed, scale_top);
    let share = (speed / scale_top.max(1e-6)).clamp(0.0, 1.0);
    [c[0], c[1], c[2], (150.0 + 105.0 * share) as u8]
}

fn push(out: &mut OverlayLines, pts: &[Vec3], rgba: [u8; 4]) {
    out.starts.push(out.positions.len() as u32);
    for p in pts {
        out.positions.push(*p);
        out.colors.push(rgba);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::heat_3d_tests::{place, snapshot};

    fn view(strength: f32) -> FlowView {
        FlowView {
            strength,
            ..Default::default()
        }
    }

    fn lengths(o: &OverlayLines) -> Vec<f32> {
        o.starts
            .chunks(2)
            .map(|c| (o.positions[c[0] as usize + 1] - o.positions[c[0] as usize]).length())
            .collect()
    }

    #[test]
    fn a_uniform_wind_gives_equal_arrows_pointing_along_it() {
        let o = arrows(&snapshot(), place(), Axis::Z, 0.8, &view(0.5), 1.0);
        assert!(!o.starts.is_empty() && o.starts.len().is_multiple_of(2));
        // The first polyline of an arrow runs tail to tip: +x in the solver is
        // +x in the scene.
        let s = o.starts[0] as usize;
        let d = o.positions[s + 1] - o.positions[s];
        assert!(d.x > 0.0 && d.y.abs() < 1e-3 && d.z.abs() < 1e-3, "{d:?}");
        let lens = lengths(&o);
        assert!(lens.iter().all(|l| (l - lens[0]).abs() < 1e-3));
    }

    #[test]
    fn length_is_the_speed_on_the_shared_scale_not_relative_to_the_plane() {
        let fast = arrows(&snapshot(), place(), Axis::Z, 0.8, &view(0.5), 1.0);
        let mut slow = snapshot();
        slow.velocity.iter_mut().for_each(|v| *v = [0.25, 0.0, 0.0]);
        // The same plane with a quarter of the speed: shorter arrows, not the
        // same arrows rescaled to the plane's fastest air.
        let calm = arrows(&slow, place(), Axis::Z, 0.8, &view(0.5), 1.0);
        assert!(lengths(&calm)[0] < 0.7 * lengths(&fast)[0]);
    }

    #[test]
    fn more_strength_gives_more_arrows() {
        let few = arrows(&snapshot(), place(), Axis::Z, 0.8, &view(0.0), 1.0);
        let many = arrows(&snapshot(), place(), Axis::Z, 0.8, &view(1.0), 1.0);
        assert!(many.starts.len() > 2 * few.starts.len());
    }

    #[test]
    fn no_arrows_inside_a_device_or_without_wind() {
        let snap = snapshot();
        // z at 10 mm of 60: the 20 mm cube sits at x, y < 20 there.
        let o = arrows(&snap, place(), Axis::Z, 0.17, &view(1.0), 1.0);
        let all = arrows(&snap, place(), Axis::Z, 0.8, &view(1.0), 1.0);
        assert!(o.starts.len() < all.starts.len());
        let mut calm = snapshot();
        calm.velocity.iter_mut().for_each(|v| *v = [0.0; 3]);
        assert!(
            arrows(&calm, place(), Axis::Z, 0.8, &view(1.0), 1.0)
                .positions
                .is_empty()
        );
    }

    #[test]
    fn air_under_three_percent_of_the_scale_is_not_drawn() {
        let mut slow = snapshot();
        slow.velocity.iter_mut().for_each(|v| *v = [0.02, 0.0, 0.0]);
        assert!(
            arrows(&slow, place(), Axis::Z, 0.8, &view(1.0), 1.0)
                .positions
                .is_empty()
        );
        assert!(
            !arrows(&slow, place(), Axis::Z, 0.8, &view(1.0), 0.1)
                .positions
                .is_empty()
        );
    }
}
