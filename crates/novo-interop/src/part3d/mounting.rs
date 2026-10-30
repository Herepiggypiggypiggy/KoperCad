//! Fixing holes and footprint of a part from its STEP model.
//!
//! [`super::pins`] reads what a wire meets; this reads what a drill meets:
//! the round holes in the face that sits on the mounting plate (screw
//! holes in a flange, a motor starter's fixing lugs, a power supply's
//! tapped holes). The result is the data a hand-drilling plan needs: where
//! the holes are relative to the part's outline, and how big.
//!
//! Positions are in the mounting-plate frame of the part: origin at the
//! bottom-left corner of its outline as seen from the door, x to the right,
//! y up, millimetres. [`MountingScan::on_plate`] puts them on a plate for a
//! placement and a number of quarter turns.
//!
//! It is a reading of geometry. The footprint is the part's outline
//! (width by height), not the exact contact area of the back; the holes are
//! only those that open in the back face within a tolerance of the outline,
//! round or short slots between 2.5 and 14 mm; a blind or tapped hole is
//! reported like a through hole, because the model alone cannot say which a
//! maker intended. Check the data sheet before drilling.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use super::build::{BuildError, StepOptions, step_soup};
use super::math::unit;
use super::mesh::{Orientation, chord_tolerance, derive_xf};
use super::pins::CandidateKind;

/// Smallest and largest hole taken as a fixing hole, mm.
pub const HOLE_DIAMETER_MM: (f64, f64) = (2.5, 14.0);

/// One fixing hole in the back face of a part.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountingHole {
    /// Centre from the bottom-left of the outline, mm.
    pub x_mm: f64,
    pub y_mm: f64,
    /// Diameter of the circle of equal area, mm.
    pub diameter_mm: f64,
    /// False for a slot or other shape, where the diameter is an average.
    pub round: bool,
}

/// What a model says about how it is fixed.
#[derive(Debug, Clone, PartialEq)]
pub struct MountingScan {
    /// Outline against the mounting plate: width, height, mm.
    pub footprint_mm: [f64; 2],
    /// Depth of the part off the plate, mm.
    pub depth_mm: f64,
    pub orientation: Orientation,
    /// Holes sorted bottom to top, then left to right.
    pub holes: Vec<MountingHole>,
}

/// A hole on a plate, in the plate's frame (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateHole {
    pub x_mm: f64,
    pub y_mm: f64,
    pub diameter_mm: f64,
}

impl MountingScan {
    /// The holes on a plate when the part's bottom-left outline corner is at
    /// `origin_mm` and the part is turned `quarter_turns` counter-clockwise
    /// (seen from the door) about that corner. The turned outline then
    /// extends from the origin like the turned rectangle does.
    pub fn on_plate(&self, origin_mm: [f64; 2], quarter_turns: i32) -> Vec<PlateHole> {
        let [w, h] = self.footprint_mm;
        let turns = quarter_turns.rem_euclid(4);
        self.holes
            .iter()
            .map(|p| {
                let (x, y) = match turns {
                    0 => (p.x_mm, p.y_mm),
                    // 90 deg CCW about the corner, then shifted back into +x.
                    1 => (h - p.y_mm, p.x_mm),
                    2 => (w - p.x_mm, h - p.y_mm),
                    _ => (p.y_mm, w - p.x_mm),
                };
                PlateHole {
                    x_mm: origin_mm[0] + x,
                    y_mm: origin_mm[1] + y,
                    diameter_mm: p.diameter_mm,
                }
            })
            .collect()
    }

    /// The footprint on a plate after the same quarter turns.
    pub fn footprint_turned(&self, quarter_turns: i32) -> [f64; 2] {
        let [w, h] = self.footprint_mm;
        if quarter_turns.rem_euclid(2) == 1 {
            [h, w]
        } else {
            [w, h]
        }
    }
}

/// Reads the fixing holes of a STEP model. `size_mm` (width, height, depth
/// from the catalog) fixes the axes exactly as the render mesh does.
pub fn scan_mounting(
    bytes: &[u8],
    size_mm: Option<[f64; 3]>,
    cancel: &AtomicBool,
    deadline: Option<Instant>,
) -> Result<MountingScan, BuildError> {
    if bytes.len() > super::MAX_STEP_BYTES {
        return Err(BuildError::TooComplex);
    }
    let text = String::from_utf8_lossy(bytes);
    let guess = size_mm.map_or(60.0, |s| s.iter().cloned().fold(0.0, f64::max));
    let opts = StepOptions {
        chord_tol_mm: chord_tolerance(guess).max(0.05),
        deadline,
        ..StepOptions::default()
    };
    let (soup, _) = step_soup(&text, &opts, cancel)?;
    let (xf, orientation, size) = derive_xf(&soup, size_mm);
    // The back face sits on the plate: z = 0 in part space.
    let back_tol = 0.5_f64.max(0.01 * size[2]);
    let mut holes: Vec<MountingHole> = Vec::new();
    for raw in &soup.features {
        if raw.kind != CandidateKind::Opening
            || raw.size_mm < HOLE_DIAMETER_MM.0
            || raw.size_mm > HOLE_DIAMETER_MM.1
        {
            continue;
        }
        let c = xf.apply(raw.centre);
        let a = unit(xf.apply_dir(raw.axis));
        if a[2].abs() < 0.9 || c[2].abs() > back_tol {
            continue;
        }
        let (x, y) = (c[0] + size[0] / 2.0, c[1] + size[1] / 2.0);
        if x < 0.0 || y < 0.0 || x > size[0] || y > size[1] {
            continue;
        }
        // Stepped and counterbored holes give several faces on one axis;
        // keep the smallest diameter, which is what the bolt passes.
        if let Some(h) = holes
            .iter_mut()
            .find(|h| (h.x_mm - x).hypot(h.y_mm - y) < 0.6)
        {
            if raw.size_mm < h.diameter_mm {
                h.diameter_mm = raw.size_mm;
                h.round = raw.round;
            }
            continue;
        }
        holes.push(MountingHole {
            x_mm: x,
            y_mm: y,
            diameter_mm: raw.size_mm,
            round: raw.round,
        });
    }
    // Rows are told apart at 0.1 mm, so rounding noise does not reorder a row.
    let row = |h: &MountingHole| (h.y_mm * 10.0).round();
    holes.sort_by(|a, b| row(a).total_cmp(&row(b)).then(a.x_mm.total_cmp(&b.x_mm)));
    Ok(MountingScan {
        footprint_mm: [size[0], size[1]],
        depth_mm: size[2],
        orientation,
        holes,
    })
}
