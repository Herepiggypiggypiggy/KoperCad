//! A fan inside the domain (lane H8): a disk of faces across one axis that
//! pushes the air with the maker's straight-line curve.
//!
//! It is the interior twin of the fan patch of [`crate::openings`]. On the
//! faces of the disk (those of the grid plane nearest the fan, whose centre
//! lies within the radius) the pressure jump of the curve acts as a momentum
//! source, spread evenly over the width of the face control volume:
//!
//! `a = sign (p_max / rho) (1 - sign u / u_free) / width`,  `u_free = Q_free / A`
//!
//! `A` is the area the grid gives the disk (not `pi r^2`), so the curve is
//! met on the area actually used. The pressure projection takes the part of
//! the force the surrounding air cannot follow, which makes the fan work
//! against the loop it sits in. The drag part is applied semi-implicitly, so
//! the step may be as large as the convection allows.
//!
//! Model, not measurement: a disk of faces is a momentum patch, not a rotor.
//! It has no swirl, no hub, no stall region (the curve is a straight line)
//! and the jet leaves it as a block of the disk's area.

use super::mesh::{Mesh, other_axes};

/// A fan across the grid, blowing along one axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InteriorFan {
    /// Axis the air is pushed along (0 x, 1 y, 2 z).
    pub axis: usize,
    /// `+1.0` pushes towards the max side of the axis, `-1.0` the other way.
    pub sign: f64,
    /// Centre of the disk, metres from the origin of the mesh.
    pub centre: [f64; 3],
    pub radius_m: f64,
    /// Maker curve: flow at zero pressure and pressure at zero flow (already
    /// reduced for the installation by the caller).
    pub free_flow_m3s: f64,
    pub max_pressure_pa: f64,
}

/// The faces of one fan on a mesh: indices into the velocity array of its
/// axis, with the area of each.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FanFaces {
    pub faces: Vec<(usize, f64)>,
    pub area: f64,
    /// Width of the face control volume along the axis, m.
    pub width: f64,
}

impl InteriorFan {
    pub(crate) fn resolve(&self, mesh: &Mesh) -> Option<FanFaces> {
        let a = self.axis;
        if a > 2 || mesh.n[a] < 2 {
            return None;
        }
        // Nearest interior grid plane to the centre.
        let s = (1..mesh.n[a])
            .min_by(|&p, &q| {
                let dp = (mesh.edges[a][p] - self.centre[a]).abs();
                let dq = (mesh.edges[a][q] - self.centre[a]).abs();
                dp.total_cmp(&dq)
            })
            .unwrap();
        let (b, c) = other_axes(a);
        let mut faces = Vec::new();
        let mut area = 0.0;
        for kc in 0..mesh.n[c] {
            for kb in 0..mesh.n[b] {
                let (db, dc) = (
                    mesh.centre[b][kb] - self.centre[b],
                    mesh.centre[c][kc] - self.centre[c],
                );
                if db * db + dc * dc > self.radius_m * self.radius_m {
                    continue;
                }
                let mut ix = [0usize; 3];
                ix[a] = s;
                ix[b] = kb;
                ix[c] = kc;
                let am = mesh.dx[b][kb] * mesh.dx[c][kc];
                faces.push((mesh.idx(a, ix), am));
                area += am;
            }
        }
        if faces.is_empty() {
            return None;
        }
        let width = 0.5 * (mesh.dx[a][s - 1] + mesh.dx[a][s]);
        Some(FanFaces { faces, area, width })
    }
}

/// The scenario's circulation fans (`Params::circulation_fans`) as fans of the
/// grid, in the frame of the 3D mesh: `x` right, `y` down, `z` from the back
/// of the interior (the plate face is at `plate_thickness_mm`) to the door.
/// The curve is reduced by the installed factor, as the 2D engine does.
/// A fan without a flow or a pressure is refused with its index.
pub fn interior_fans_from_scenario(
    scn: &novo_thermal::Scenario,
) -> Result<Vec<InteriorFan>, crate::openings::OpeningError> {
    use novo_thermal::BlowDirection as B;
    let e = &scn.enclosure;
    let t = scn.params.plate_thickness_mm;
    let air_depth = (e.interior_depth_mm() - t).max(1.0);
    scn.params
        .circulation_fans
        .iter()
        .enumerate()
        .map(|(i, f)| {
            if f.q_max_m3s() <= 0.0 || f.p_max_pa() <= 0.0 || f.diameter_mm <= 0.0 {
                return Err(crate::openings::OpeningError::BadFan(i));
            }
            let (axis, sign) = match f.direction {
                B::Right => (0, 1.0),
                B::Left => (0, -1.0),
                B::Down => (1, 1.0),
                B::Up => (1, -1.0),
                B::ToDoor => (2, 1.0),
                B::ToPlate => (2, -1.0),
            };
            let z = t + f.z_mm.unwrap_or(0.5 * air_depth);
            Ok(InteriorFan {
                axis,
                sign,
                centre: [f.x_mm * 1e-3, f.y_mm * 1e-3, z * 1e-3],
                radius_m: 0.5 * f.diameter_mm * 1e-3,
                free_flow_m3s: f.q_max_m3s(),
                max_pressure_pa: f.p_max_pa(),
            })
        })
        .collect()
}
