//! Open boundaries: free openings, vents and louvres, fans (lane CFD-C2).
//!
//! An opening is a patch on one of the six domain faces where the wall is
//! replaced by a pressure condition against still outside air. Per boundary
//! face with outward normal velocity `u` (kinematic pressure, so divided by
//! the density):
//!
//! `p_face = r u + p_jump`
//!
//! - **Free**: `p_jump = 0`, `r = |u| / 2` for inflow (Bernoulli: the air
//!   accelerates from rest), `r = 0` for outflow.
//! - **Vent**: a sharp-edged orifice, `dp = (1/2) K u^2` with
//!   `K = 1 / (Cd phi)^2` (`phi` the free area share, `Cd` the discharge
//!   coefficient). This is the law `novo-thermal` uses, so both models agree
//!   on what a louvre is.
//! - **Fan**: the maker curve, a straight line from (0 flow, `max_pressure`)
//!   to (`free_flow`, 0 pressure). The slope is a resistance
//!   `dp_max A / Q_free` per face, the intercept a pressure jump of the
//!   sign of the fan direction. It acts as a distributed momentum source
//!   over the patch (the flow is taken as evenly spread over it).
//!
//! The pressure is measured against outside air at the same height, so the
//! buoyancy reference of the solver must be the outside temperature
//! ([`crate::flow::FlowConfig::t_ref`]); then the stack effect needs no extra
//! term.
//!
//! Where the user finds it: Cabinet view, Heat layer: vents and fans placed
//! on the enclosure (the same ones the 2D preview uses) become these patches
//! in Simulate. App side: lane CFD-F.

mod set;

pub use set::{OpenFace, OpenSet};

use novo_thermal::{FanMode, Opening, Placement, Scenario};

use crate::flow::Face;

/// What the opening does to the air going through it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpenKind {
    /// No loss besides the inflow acceleration.
    Free,
    /// Orifice loss `dp = K u^2 / 2` on the face-averaged velocity.
    Loss { k: f64 },
    /// A fan with a straight-line curve.
    Fan {
        intake: bool,
        free_flow_m3s: f64,
        max_pressure_pa: f64,
    },
}

impl OpenKind {
    /// Orifice loss of a vent with free area share `free_area` and discharge
    /// coefficient `cd`.
    pub fn vent(free_area: f64, cd: f64) -> OpenKind {
        let a = (free_area * cd).max(1e-6);
        OpenKind::Loss { k: 1.0 / (a * a) }
    }
}

/// A rectangle on a domain face. `lo` and `hi` are along the two axes
/// tangential to the face in ascending axis order (x then y for a z face,
/// x then z for a y face, y then z for an x face), metres from the origin of
/// the mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenPatch {
    pub face: Face,
    pub lo: [f64; 2],
    pub hi: [f64; 2],
    pub kind: OpenKind,
    /// Temperature of the air that comes in, degrees C. `None` = the
    /// outside temperature of the solver configuration.
    pub inflow_c: Option<f64>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum OpeningError {
    #[error("opening {0} lies outside the enclosure")]
    Outside(usize),
    #[error("vent {0} has no free area")]
    NoFreeArea(usize),
    #[error("fan {0} has no flow or no pressure")]
    BadFan(usize),
}

/// The scenario's vents and fans as patches of the interior (metres).
/// Vents first, then fans, in scenario order; the error names the index in
/// that order.
///
/// Mapping (frame of `novo-thermal`: `x` right, `y` down, `z` from the plate
/// to the door): top and bottom walls are the `y` faces, left and right the
/// `x` faces, the front the `z` max face (door). On the side walls the
/// opening is centred in the depth.
pub fn patches_from_scenario(scn: &Scenario) -> Result<Vec<OpenPatch>, OpeningError> {
    let e = &scn.enclosure;
    let dom = [
        e.interior_width_mm(),
        e.interior_height_mm(),
        e.interior_depth_mm(),
    ];
    let cd = scn.params.discharge_coefficient;
    let mut out = Vec::new();
    for (i, v) in scn.vents.iter().enumerate() {
        if v.free_area_fraction <= 0.0 {
            return Err(OpeningError::NoFreeArea(i));
        }
        out.push(patch(
            &v.opening,
            OpenKind::vent(v.free_area_fraction.min(1.0), cd),
            dom,
            i,
        )?);
    }
    let nv = scn.vents.len();
    for (i, f) in scn.fans.iter().enumerate() {
        if f.free_flow_m3h <= 0.0 || f.max_pressure_pa <= 0.0 {
            return Err(OpeningError::BadFan(nv + i));
        }
        out.push(patch(
            &f.opening,
            OpenKind::Fan {
                intake: f.mode == FanMode::Intake,
                free_flow_m3s: f.free_flow_m3h / 3600.0,
                max_pressure_pa: f.max_pressure_pa,
            },
            dom,
            nv + i,
        )?);
    }
    Ok(out)
}

fn patch(
    o: &Opening,
    kind: OpenKind,
    dom: [f64; 3],
    index: usize,
) -> Result<OpenPatch, OpeningError> {
    let span = |c: f64, w: f64, len: f64| -> (f64, f64) {
        ((c - 0.5 * w).max(0.0), (c + 0.5 * w).min(len))
    };
    let centred = |w: f64, len: f64| span(0.5 * len, w, len);
    // (face, tangential spans in mm)
    let (face, a, b) = match o.placement {
        Placement::Top { x_mm } => (
            Face::YMin,
            span(x_mm, o.width_mm, dom[0]),
            centred(o.height_mm, dom[2]),
        ),
        Placement::Bottom { x_mm } => (
            Face::YMax,
            span(x_mm, o.width_mm, dom[0]),
            centred(o.height_mm, dom[2]),
        ),
        Placement::Left { y_mm } => (
            Face::XMin,
            span(y_mm, o.width_mm, dom[1]),
            centred(o.height_mm, dom[2]),
        ),
        Placement::Right { y_mm } => (
            Face::XMax,
            span(y_mm, o.width_mm, dom[1]),
            centred(o.height_mm, dom[2]),
        ),
        Placement::Front { x_mm, y_mm } => (
            Face::ZMax,
            span(x_mm, o.width_mm, dom[0]),
            span(y_mm, o.height_mm, dom[1]),
        ),
    };
    if a.1 <= a.0 || b.1 <= b.0 {
        return Err(OpeningError::Outside(index));
    }
    Ok(OpenPatch {
        face,
        lo: [a.0 * 1e-3, b.0 * 1e-3],
        hi: [a.1 * 1e-3, b.1 * 1e-3],
        kind,
        inflow_c: None,
    })
}
