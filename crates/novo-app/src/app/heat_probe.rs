//! The probe in the 3D view: click the air (on the slice, or through the
//! enclosure) and read the temperature and the air speed there. Pure
//! geometry on the solver result; `view3d.rs` feeds it the pointer ray.

use glam::Vec3;
use novo_cfd::CfdSnapshot;
use novo_heatview::cfd::{self, Axis};
use novo_view3d::{Overlay, OverlayLines, Ray};

use super::heat_3d::Place;

/// What the probe read at one point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Reading {
    /// The point in the solver frame, mm.
    pub at: [f64; 3],
    pub temperature_c: f64,
    /// Rise over the ambient air, K.
    pub rise_k: f64,
    /// Air speed, m/s.
    pub speed: f64,
    /// Vertical part of the air velocity, m/s, positive upward in the view.
    pub up: f64,
}

impl Reading {
    /// One line for the readout.
    pub(crate) fn text(&self) -> String {
        let dir = if self.speed < 0.005 {
            String::from("still")
        } else if self.up.abs() > 0.5 * self.speed {
            format!(
                "{} {:.2} m/s",
                if self.up > 0.0 { "rising" } else { "falling" },
                self.speed
            )
        } else {
            format!("{:.2} m/s", self.speed)
        };
        format!(
            "{:.1} °C ({:+.1} K), air {dir}  at x {:.0}, y {:.0}, z {:.0} mm",
            self.temperature_c, self.rise_k, self.at[0], self.at[1], self.at[2]
        )
    }
}

/// The probe's state in the 3D view.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct ProbeState {
    pub on: bool,
    /// The probed point in the solver frame, mm.
    pub at: Option<[f64; 3]>,
    /// What the overlays were last built with: result id and point.
    pub shown: Option<(u64, Option<[f64; 3]>)>,
}

/// The scene ray in the solver frame (origin, direction).
fn to_solver(place: Place, ray: &Ray) -> ([f64; 3], [f64; 3]) {
    let o = ray.origin;
    let d = ray.dir;
    (
        [
            f64::from(o.x) + place.plate_x,
            place.plate_y + f64::from(place.frame.height - o.y),
            f64::from(o.z),
        ],
        [f64::from(d.x), -f64::from(d.y), f64::from(d.z)],
    )
}

fn axis_index(axis: Axis) -> usize {
    match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    }
}

fn bounds(snap: &CfdSnapshot) -> ([f64; 3], [f64; 3]) {
    let g = &snap.grid;
    let e = |v: &Vec<f64>| (v[0], v[v.len() - 1]);
    let (x, y, z) = (e(&g.x_edges_mm), e(&g.y_edges_mm), e(&g.z_edges_mm));
    ([x.0, y.0, z.0], [x.1, y.1, z.1])
}

/// Where the ray meets the air: on the slice plane when one is shown and the
/// ray crosses it in the air, otherwise the first air the ray reaches inside
/// the enclosure. `slice` is the plane's axis and offset in mm.
pub(crate) fn locate(
    snap: &CfdSnapshot,
    place: Place,
    ray: &Ray,
    slice: Option<(Axis, f64)>,
) -> Option<[f64; 3]> {
    if !snap.is_consistent() {
        return None;
    }
    let (o, d) = to_solver(place, ray);
    let at = |t: f64| [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
    if let Some((axis, offset)) = slice {
        let i = axis_index(axis);
        if d[i].abs() > 1e-9 {
            let t = (offset - o[i]) / d[i];
            let p = at(t);
            if t >= 0.0 && cfd::inside(snap, p) && !cfd::solid_at(snap, p) {
                return Some(p);
            }
        }
    }
    // March through the box the result covers.
    let (lo, hi) = bounds(snap);
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for i in 0..3 {
        if d[i].abs() < 1e-12 {
            if o[i] < lo[i] || o[i] > hi[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[i] - o[i]) / d[i], (hi[i] - o[i]) / d[i]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    if t0 > t1 {
        return None;
    }
    let step = 2.0;
    let mut t = t0;
    while t <= t1 {
        let p = at(t);
        if cfd::inside(snap, p) && !cfd::solid_at(snap, p) {
            return Some(p);
        }
        t += step;
    }
    None
}

/// The temperature and air speed at `at`.
pub(crate) fn read(snap: &CfdSnapshot, ambient: f64, at: [f64; 3]) -> Reading {
    let temperature_c = f64::from(cfd::temperature_at(snap, at));
    let v = cfd::velocity_at(snap, at);
    let speed = v.iter().map(|c| f64::from(*c).powi(2)).sum::<f64>().sqrt();
    Reading {
        at,
        temperature_c,
        rise_k: temperature_c - ambient,
        speed,
        // The solver's y runs down the plate.
        up: -f64::from(v[1]),
    }
}

/// A small white cross at the probed point.
pub(crate) fn marker(place: Place, at: [f64; 3]) -> Overlay {
    const ARM: f64 = 12.0;
    let mut l = OverlayLines {
        width_px: 2.0,
        order: 5,
        ..Default::default()
    };
    for axis in 0..3 {
        l.starts.push(l.positions.len() as u32);
        for s in [-ARM, ARM] {
            let mut p = at;
            p[axis] += s;
            let w: Vec3 = place
                .frame
                .world(p[0] - place.plate_x, p[1] - place.plate_y, p[2]);
            l.positions.push(w);
            l.colors.push([255, 255, 255, 255]);
        }
    }
    Overlay::Lines(l)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::heat_3d_tests::{place, snapshot};

    fn down_at(x: f32, y: f32) -> Ray {
        Ray {
            origin: Vec3::new(x, y, 200.0),
            dir: Vec3::NEG_Z,
        }
    }

    #[test]
    fn a_click_on_the_slice_reads_the_field_there() {
        let (snap, place) = (snapshot(), place());
        // Scene (50, 40) is solver x 60, y 30.
        let p = locate(&snap, place, &down_at(50.0, 40.0), Some((Axis::Z, 40.0))).unwrap();
        assert!(
            (p[0] - 60.0).abs() < 1e-6 && (p[1] - 30.0).abs() < 1e-6 && (p[2] - 40.0).abs() < 1e-6
        );
        let r = read(&snap, 20.0, p);
        assert!((r.temperature_c - 26.0).abs() < 0.05, "{r:?}");
        assert!((r.rise_k - 6.0).abs() < 0.05);
        assert!((r.speed - 1.0).abs() < 1e-6 && r.up.abs() < 1e-6);
        assert!(r.text().contains("26.0 °C"));
    }

    #[test]
    fn without_a_slice_the_ray_stops_in_the_first_air() {
        let (snap, place) = (snapshot(), place());
        let p = locate(&snap, place, &down_at(50.0, 40.0), None).unwrap();
        assert!((p[2] - 60.0).abs() <= 2.0, "{p:?}");
    }

    #[test]
    fn a_slice_inside_a_device_falls_through_to_the_air_above_it() {
        let (snap, place) = (snapshot(), place());
        // Scene (5, 55) is solver (15, 15): inside the device up to z 20.
        let p = locate(&snap, place, &down_at(5.0, 55.0), Some((Axis::Z, 10.0))).unwrap();
        assert!(p[2] >= 20.0, "{p:?}");
    }

    #[test]
    fn a_ray_beside_the_enclosure_reads_nothing() {
        let (snap, place) = (snapshot(), place());
        assert!(locate(&snap, place, &down_at(500.0, 40.0), None).is_none());
    }

    #[test]
    fn the_marker_is_three_crossed_lines() {
        let Overlay::Lines(l) = marker(place(), [60.0, 30.0, 40.0]) else {
            panic!("lines")
        };
        assert_eq!(l.starts.len(), 3);
        assert_eq!(l.positions.len(), 6);
    }
}
