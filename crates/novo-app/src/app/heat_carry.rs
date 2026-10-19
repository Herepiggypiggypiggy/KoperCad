//! Heat flow in the 3D view: where the heat goes, not only where the air goes.
//! Lines and moving dashes follow the heat the air carries away from hot
//! devices (`rho * cp * (T - ambient) * u`), brighter where more heat moves,
//! and arrows inside the plate, rails and devices show conduction
//! (`-k grad T`). The numbers come from `novo_heatview::flux`: derived from
//! the solved temperature and velocity, without surface radiation.

use glam::Vec3;
use novo_cfd::CfdSnapshot;
use novo_heatview::cfd::Trace;
use novo_heatview::flux;
use novo_view3d::OverlayLines;

use super::heat_3d::{Flow, FlowLine, Place};

/// Colour of the heat layer: warm, apart from the blue airflow lines.
pub(crate) const RGB: [u8; 3] = [255, 150, 50];
/// Colour of the moving dashes.
pub(crate) const TRACER_RGB: [u8; 3] = [255, 232, 170];
/// Blocks per axis to seed lines from: the strongest air cell of each.
const BLOCKS: [usize; 3] = [8, 6, 4];
/// Blocks whose strongest flux is under this share of the top get no line.
const FLOOR_SHARE: f32 = 0.04;
/// Lines are followed while the flux is above this share of the top.
const FOLLOW_SHARE: f32 = 0.01;
/// Most conduction arrows, strongest first, and the weakest share kept.
const MAX_ARROWS: usize = 300;
const ARROW_FLOOR: f32 = 0.15;

/// The heat-flow layer of a result.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Carry {
    /// Lines along the heat carried by the air; the value on each point is
    /// the flux there, W/m2.
    pub flow: Flow,
    /// Arrows for conduction in solids.
    pub conduction: OverlayLines,
    /// Strongest flux in the air and in solids, W/m2.
    pub top_air: f32,
    pub top_solid: f32,
}

/// Traces the layer once per result.
pub(crate) fn carry(snap: &CfdSnapshot, ambient: f64, place: Place) -> Carry {
    if !snap.is_consistent() {
        return Carry::default();
    }
    let field = flux::flux_field(snap, ambient);
    let top_air = flux::top_air_flux(snap, &field);
    let mut out = Carry {
        top_air,
        ..Default::default()
    };
    if top_air > 0.0 {
        let seeds = flux::seeds(snap, &field, BLOCKS, FLOOR_SHARE);
        let lines = flux::lines(
            snap,
            &field,
            &seeds,
            Trace::default(),
            top_air * FOLLOW_SHARE,
        );
        for line in lines.into_iter().filter(|l| l.len() >= 2) {
            let points: Vec<Vec3> = line.iter().map(|p| place.world(*p)).collect();
            let strength: Vec<f32> = line
                .iter()
                .map(|p| flux::magnitude(flux::flux_at(snap, &field, *p)))
                .collect();
            let mut along = vec![0.0_f32];
            for w in points.windows(2) {
                along.push(along[along.len() - 1] + w[0].distance(w[1]));
            }
            out.flow.top = out
                .flow
                .top
                .max(strength.iter().copied().fold(0.0, f32::max));
            out.flow.lines.push(FlowLine {
                points,
                speed: strength,
                along,
            });
        }
    }
    out.top_solid = conduction_arrows(snap, &field, place, &mut out.conduction);
    out
}

/// Arrows in the strongest conducting solid cells; returns the top flux.
fn conduction_arrows(
    snap: &CfdSnapshot,
    field: &[[f32; 3]],
    place: Place,
    out: &mut OverlayLines,
) -> f32 {
    let g = &snap.grid;
    let mut cells: Vec<(f32, usize)> = (0..g.cells())
        .filter(|n| snap.material[*n] != 0)
        .map(|n| (flux::magnitude(field[n]), n))
        .filter(|(m, _)| *m > 0.0)
        .collect();
    let top = cells.iter().map(|c| c.0).fold(0.0, f32::max);
    if top <= 0.0 {
        return 0.0;
    }
    cells.retain(|(m, _)| *m >= ARROW_FLOOR * top);
    cells.sort_by(|a, b| b.0.total_cmp(&a.0));
    cells.truncate(MAX_ARROWS);
    out.width_px = 1.8;
    out.order = 4;
    for (m, n) in cells {
        let (i, j, k) = (n % g.nx, (n / g.nx) % g.ny, n / (g.nx * g.ny));
        let size = [
            g.x_edges_mm[i + 1] - g.x_edges_mm[i],
            g.y_edges_mm[j + 1] - g.y_edges_mm[j],
            g.z_edges_mm[k + 1] - g.z_edges_mm[k],
        ];
        let reach = size[0].min(size[1]).min(size[2]);
        let share = f64::from(m / top);
        let len = reach * (0.5 + 1.0 * share);
        let q = field[n].map(f64::from);
        let d = [
            q[0] / f64::from(m),
            q[1] / f64::from(m),
            q[2] / f64::from(m),
        ];
        let c = g.centre_mm(i, j, k);
        let a = (110.0 + 145.0 * share) as u8;
        arrow(out, place, c, d, len, [RGB[0], RGB[1], RGB[2], a]);
    }
    top
}

/// An arrow centred on `c` (solver frame, mm) along unit vector `d`.
fn arrow(out: &mut OverlayLines, place: Place, c: [f64; 3], d: [f64; 3], len: f64, rgba: [u8; 4]) {
    let at = |t: f64| {
        [
            c[0] + d[0] * len * t,
            c[1] + d[1] * len * t,
            c[2] + d[2] * len * t,
        ]
    };
    let side = perpendicular(d);
    let head = len * 0.3;
    let tip = at(0.5);
    let wing = |s: f64| {
        [
            tip[0] - d[0] * head + side[0] * s,
            tip[1] - d[1] * head + side[1] * s,
            tip[2] - d[2] * head + side[2] * s,
        ]
    };
    let mut push = |pts: &[[f64; 3]]| {
        out.starts.push(out.positions.len() as u32);
        for p in pts {
            out.positions.push(place.world(*p));
            out.colors.push(rgba);
        }
    };
    push(&[at(-0.5), tip]);
    push(&[wing(head * 0.6), tip, wing(-head * 0.6)]);
}

fn perpendicular(d: [f64; 3]) -> [f64; 3] {
    // Cross with the axis least aligned with `d`.
    let axis = if d[0].abs() <= d[1].abs() && d[0].abs() <= d[2].abs() {
        [1.0, 0.0, 0.0]
    } else if d[1].abs() <= d[2].abs() {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let c = [
        d[1] * axis[2] - d[2] * axis[1],
        d[2] * axis[0] - d[0] * axis[2],
        d[0] * axis[1] - d[1] * axis[0],
    ];
    let n = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt().max(1e-12);
    [c[0] / n, c[1] / n, c[2] / n]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::heat_3d_tests::{place, snapshot};

    #[test]
    fn warm_wind_gives_lines_that_run_with_the_wind() {
        // T = 20 + x/10 with a 1 m/s wind along +x: heat moves towards +x.
        let c = carry(&snapshot(), 20.0, place());
        assert!(c.top_air > 0.0 && !c.flow.lines.is_empty());
        for l in &c.flow.lines {
            let d = *l.points.last().unwrap() - l.points[0];
            assert!(d.x > 0.0 && d.y.abs() < 1e-3 && d.z.abs() < 1e-3, "{d:?}");
            assert_eq!(l.points.len(), l.speed.len());
        }
    }

    #[test]
    fn stronger_flux_where_the_air_is_hotter() {
        let c = carry(&snapshot(), 20.0, place());
        // rho cp (T - 20) u at the hot end (about 10 K, 1 m/s).
        let want = (flux_scale() * 9.5) as f32;
        assert!(
            c.top_air > want * 0.8 && c.top_air < want * 1.2,
            "{}",
            c.top_air
        );
    }

    fn flux_scale() -> f64 {
        flux::AIR_DENSITY * flux::AIR_CP
    }

    #[test]
    fn air_at_ambient_has_no_heat_flow() {
        let mut snap = snapshot();
        snap.temperature_c.iter_mut().for_each(|t| *t = 25.0);
        let c = carry(&snap, 25.0, place());
        assert!(c.flow.lines.is_empty());
        assert_eq!(c.top_air, 0.0);
    }

    #[test]
    fn the_device_conducts_and_arrows_point_down_its_gradient() {
        // The analytic device is uniform in material but T = 20 + x/10 runs
        // through it: conduction runs towards -x... heat goes from hot to
        // cold, and T rises with x, so the arrows point towards -x.
        let c = carry(&snapshot(), 20.0, place());
        assert!(c.top_solid > 0.0);
        assert!(!c.conduction.starts.is_empty());
        let s = c.conduction.starts[0] as usize;
        let d = c.conduction.positions[s + 1] - c.conduction.positions[s];
        assert!(d.x < 0.0 && d.y.abs() < 1e-3 && d.z.abs() < 1e-3, "{d:?}");
        assert!(c.conduction.is_valid());
    }
}
