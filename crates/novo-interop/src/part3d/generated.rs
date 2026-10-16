//! Parts with no model file whose shape is standard: DIN rails and wiring
//! ducts. They are drawn from their dimensions instead of as plain boxes,
//! so a cabinet view shows the top-hat profile of a rail with its slots and
//! the comb fingers of a duct.
//!
//! The result is an ordinary [`PartGeometry`] (levels of detail in part
//! space: x along the part, y across it, z the height from the plate) with
//! `MeshOrigin::Box`, which is what a part without a model reports. The
//! slots and fingers are left out of the detailed level when they would
//! not fit its triangle budget. Dimensions are the usual catalog values for
//! planning (EN 60715 top-hat rails; slot and finger sizes vary by maker).

use super::decimate::weld;
use super::math::*;
use super::mesh::{MeshBudget, MeshOrigin, PartGeometry, Quality, levels};

/// Top-hat rails of EN 60715.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailProfile {
    /// 35 mm wide, 7.5 mm high, 1.0 mm sheet.
    Ts35x7_5,
    /// 35 mm wide, 15 mm high, 1.5 mm sheet.
    Ts35x15,
}

impl RailProfile {
    /// Reads names as makers write them: "TS 35x7.5", "NS 35/7,5", "35x15".
    pub fn from_name(name: &str) -> Option<RailProfile> {
        // "35x7.5", "35/7,5" and "TS 35 x 7.5" all reduce to "3575".
        let digits: String = name.chars().filter(char::is_ascii_digit).collect();
        if digits.starts_with("3575") {
            Some(RailProfile::Ts35x7_5)
        } else if digits.starts_with("3515") {
            Some(RailProfile::Ts35x15)
        } else {
            None
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            RailProfile::Ts35x7_5 => "TS 35x7.5",
            RailProfile::Ts35x15 => "TS 35x15",
        }
    }

    /// (overall height, sheet thickness) in millimetres; the width is 35.
    pub fn height_and_thickness(self) -> (f64, f64) {
        match self {
            RailProfile::Ts35x7_5 => (7.5, 1.0),
            RailProfile::Ts35x15 => (15.0, 1.5),
        }
    }
}

pub const RAIL_WIDTH_MM: f64 = 35.0;
/// Outside width of the raised part of a top-hat rail.
const HAT_OUTER_MM: f64 = 27.0;
/// Slots of a perforated rail: width along the rail, pitch.
pub const RAIL_SLOT_MM: f64 = 5.2;
pub const RAIL_SLOT_PITCH_MM: f64 = 25.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailSpec {
    pub profile: RailProfile,
    pub length_mm: f64,
    /// Slots every 25 mm across the top of the hat (a perforated rail).
    pub slotted: bool,
}

/// Wiring duct: an open channel with a comb of fingers on both sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuctSpec {
    /// Outside width across the duct.
    pub width_mm: f64,
    /// Height of the walls above the plate.
    pub height_mm: f64,
    pub length_mm: f64,
    /// Cut the walls into fingers (a slotted duct).
    pub fingers: bool,
}

impl DuctSpec {
    pub const WALL_MM: f64 = 2.0;
    pub const FINGER_MM: f64 = 5.0;
    pub const SLOT_MM: f64 = 5.0;
}

type P2 = [f64; 2];

/// Triangles with a vertex list; extruded along x.
#[derive(Default)]
struct Prism {
    pos: Vec<V3>,
    tris: Vec<[u32; 3]>,
}

impl Prism {
    fn vertex(&mut self, p: V3) -> u32 {
        self.pos.push(p);
        (self.pos.len() - 1) as u32
    }

    /// The side wall for polygon edge `a -> b` (counter-clockwise polygon in
    /// (y, z), seen from +x) between `x0` and `x1`.
    fn wall(&mut self, a: P2, b: P2, x0: f64, x1: f64) {
        let a0 = self.vertex([x0, a[0], a[1]]);
        let a1 = self.vertex([x1, a[0], a[1]]);
        let b0 = self.vertex([x0, b[0], b[1]]);
        let b1 = self.vertex([x1, b[0], b[1]]);
        self.tris.push([a0, b1, a1]);
        self.tris.push([a0, b0, b1]);
    }

    fn walls(&mut self, poly: &[P2], x0: f64, x1: f64) {
        for i in 0..poly.len() {
            self.wall(poly[i], poly[(i + 1) % poly.len()], x0, x1);
        }
    }

    /// A flat face of a counter-clockwise polygon at `x`, looking along
    /// +x (`towards_plus`) or -x.
    fn cap(&mut self, poly: &[P2], x: f64, towards_plus: bool) {
        let flat: Vec<f64> = poly.iter().flat_map(|p| [p[0], p[1]]).collect();
        let Ok(idx) = earcutr::earcut(&flat, &[], 2) else {
            return;
        };
        let ids: Vec<u32> = poly.iter().map(|p| self.vertex([x, p[0], p[1]])).collect();
        for t in idx.as_chunks::<3>().0 {
            let (a, b, c) = (ids[t[0]], ids[t[1]], ids[t[2]]);
            // earcut may return either winding; fix it from the polygon's
            // own orientation in (y, z), where counter-clockwise looks +x.
            let (p, q, r) = (poly[t[0]], poly[t[1]], poly[t[2]]);
            let ccw = (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]) > 0.0;
            if ccw == towards_plus {
                self.tris.push([a, b, c]);
            } else {
                self.tris.push([a, c, b]);
            }
        }
    }
}

fn finish(prism: Prism, quality: Quality, longest: f64) -> (Vec<super::mesh::MeshData>, usize) {
    let (pos, tris) = weld(&prism.pos, &prism.tris, 1e-6);
    let n = tris.len();
    (levels(pos, tris, MeshBudget::for_size(longest, quality)), n)
}

/// A rail along x, centred, 35 mm across (y), standing `height` above the
/// plate (z).
pub fn din_rail(spec: &RailSpec, quality: Quality) -> PartGeometry {
    let len = spec.length_mm.max(1.0);
    let budget = MeshBudget::for_size(len, quality).max_triangles as usize;
    let mut prism = rail_prism(spec, len, spec.slotted);
    if prism.tris.len() > budget {
        prism = rail_prism(spec, len, false);
    }
    let (lods, _) = finish(prism, quality, len);
    PartGeometry {
        lods,
        origin: MeshOrigin::Box,
    }
}

fn rail_prism(spec: &RailSpec, len: f64, slotted: bool) -> Prism {
    let (h, t) = spec.profile.height_and_thickness();
    let (w2, yo) = (RAIL_WIDTH_MM / 2.0, HAT_OUTER_MM / 2.0);
    let yi = yo - t;
    let full: Vec<P2> = vec![
        [-w2, 0.0],
        [-yi, 0.0],
        [-yi, h - t],
        [yi, h - t],
        [yi, 0.0],
        [w2, 0.0],
        [w2, t],
        [yo, t],
        [yo, h],
        [yi, h],
        [-yi, h],
        [-yo, h],
        [-yo, t],
        [-w2, t],
    ];
    // Points that a neighbouring face also has are listed even where they
    // are collinear, so that adjoining faces share their vertices.
    let left: Vec<P2> = vec![
        [-w2, 0.0],
        [-yi, 0.0],
        [-yi, h - t],
        [-yi, h],
        [-yo, h],
        [-yo, t],
        [-w2, t],
    ];
    let right: Vec<P2> = vec![
        [yi, 0.0],
        [w2, 0.0],
        [w2, t],
        [yo, t],
        [yo, h],
        [yi, h],
        [yi, h - t],
    ];
    let bite: Vec<P2> = vec![[-yi, h - t], [yi, h - t], [yi, h], [-yi, h]];

    let (lo, hi) = (-len / 2.0, len / 2.0);
    // Slot centres: one pitch in from each end, every pitch.
    let mut slots: Vec<(f64, f64)> = Vec::new();
    if slotted {
        let mut c = lo + RAIL_SLOT_PITCH_MM;
        while c + RAIL_SLOT_MM / 2.0 <= hi - RAIL_SLOT_PITCH_MM / 2.0 {
            slots.push((c - RAIL_SLOT_MM / 2.0, c + RAIL_SLOT_MM / 2.0));
            c += RAIL_SLOT_PITCH_MM;
        }
    }
    let mut p = Prism::default();
    p.cap(&full, lo, false);
    let mut x = lo;
    for &(s0, s1) in &slots {
        p.walls(&full, x, s0);
        p.cap(&bite, s0, true);
        p.walls(&left, s0, s1);
        p.walls(&right, s0, s1);
        p.cap(&bite, s1, false);
        x = s1;
    }
    p.walls(&full, x, hi);
    p.cap(&full, hi, true);
    p
}

/// A duct along x, centred, `width` across (y) and `height` above the plate.
pub fn duct(spec: &DuctSpec, quality: Quality) -> PartGeometry {
    let len = spec.length_mm.max(1.0);
    let budget = MeshBudget::for_size(len, quality).max_triangles as usize;
    let mut prism = duct_prism(spec, len, spec.fingers);
    if prism.tris.len() > budget {
        prism = duct_prism(spec, len, false);
    }
    let (lods, _) = finish(prism, quality, len);
    PartGeometry {
        lods,
        origin: MeshOrigin::Box,
    }
}

fn duct_prism(spec: &DuctSpec, len: f64, fingers: bool) -> Prism {
    let tw = DuctSpec::WALL_MM;
    let hw = spec.width_mm.max(2.0 * tw + 1.0) / 2.0;
    let h = spec.height_mm.max(tw + 1.0);
    let full: Vec<P2> = vec![
        [-hw, 0.0],
        [hw, 0.0],
        [hw, tw],
        [hw, h],
        [hw - tw, h],
        [hw - tw, tw],
        [-hw + tw, tw],
        [-hw + tw, h],
        [-hw, h],
        [-hw, tw],
    ];
    let base: Vec<P2> = vec![
        [-hw, 0.0],
        [hw, 0.0],
        [hw, tw],
        [hw - tw, tw],
        [-hw + tw, tw],
        [-hw, tw],
    ];
    let wall_r: Vec<P2> = vec![[hw - tw, tw], [hw, tw], [hw, h], [hw - tw, h]];
    let wall_l: Vec<P2> = vec![[-hw, tw], [-hw + tw, tw], [-hw + tw, h], [-hw, h]];

    let (lo, hi) = (-len / 2.0, len / 2.0);
    let (fw, sw) = (DuctSpec::FINGER_MM, DuctSpec::SLOT_MM);
    let mut slots: Vec<(f64, f64)> = Vec::new();
    if fingers && len >= 2.0 * fw + sw {
        // A finger at each end, then slot and finger alternately; the last
        // finger takes the remainder.
        let n = ((len - fw) / (fw + sw)).floor() as usize;
        for k in 0..n {
            let s0 = lo + fw + k as f64 * (fw + sw);
            slots.push((s0, s0 + sw));
        }
    }
    let mut p = Prism::default();
    p.cap(&full, lo, false);
    let mut x = lo;
    for &(s0, s1) in &slots {
        p.walls(&full, x, s0);
        p.cap(&wall_r, s0, true);
        p.cap(&wall_l, s0, true);
        p.walls(&base, s0, s1);
        p.cap(&wall_r, s1, false);
        p.cap(&wall_l, s1, false);
        x = s1;
    }
    p.walls(&full, x, hi);
    p.cap(&full, hi, true);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_names_as_makers_write_them() {
        for n in ["TS35x7.5", "TS 35 x 7,5", "NS 35/7,5", "35x7.5"] {
            assert_eq!(
                RailProfile::from_name(n),
                Some(RailProfile::Ts35x7_5),
                "{n}"
            );
        }
        for n in ["TS 35x15", "NS 35/15", "35x15x1.5"] {
            assert_eq!(RailProfile::from_name(n), Some(RailProfile::Ts35x15), "{n}");
        }
        assert_eq!(RailProfile::from_name("TS 15x5.5"), None);
        assert_eq!(RailProfile::from_name(""), None);
    }
}
