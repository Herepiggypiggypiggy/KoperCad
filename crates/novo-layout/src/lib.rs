//! Cabinet layout engine (WS-15). Pure geometry on a mounting plate:
//! footprints, collisions, clearances and fit. See `docs/design/cabinet-layout.md`.
//!
//! Millimetres, origin top left, x right, y down. Nothing here mutates a
//! project; the model storage and commands come later.

pub mod assembly;
pub mod check;
pub mod collide;
pub mod drill;
pub mod enclosure;
pub mod model;
pub mod plan;
pub mod plate_drawing;
pub mod rules;
pub mod sizing;
pub mod sizing_model;
pub mod state;
pub mod stock;
pub mod surface_rules;
pub mod surfaces;
pub mod swing;

/// Footprint used for a device whose part has no dimensions (marked guessed).
pub const DEFAULT_FOOTPRINT_MM: (f64, f64) = (45.0, 90.0);
/// Default edge-to-edge gap between devices.
pub const DEFAULT_CLEARANCE_MM: f64 = 10.0;

/// Axis-aligned rectangle on the plate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    pub fn area(&self) -> f64 {
        self.w * self.h
    }
    /// True when the interiors overlap (touching edges do not count).
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
    /// True when `o` lies completely inside `self`.
    pub fn contains(&self, o: &Rect) -> bool {
        o.x >= self.x && o.y >= self.y && o.right() <= self.right() && o.bottom() <= self.bottom()
    }
    /// Edge-to-edge distance; 0 when overlapping or touching.
    pub fn gap(&self, o: &Rect) -> f64 {
        let dx = (o.x - self.right()).max(self.x - o.right()).max(0.0);
        let dy = (o.y - self.bottom()).max(self.y - o.bottom()).max(0.0);
        dx.hypot(dy)
    }
}

/// Size of a device from its part; `guessed` when the part had no size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
    pub guessed: bool,
}

impl Size {
    /// Size from optional part dimensions; zero or missing falls back to the default.
    pub fn from_part(width: Option<f64>, height: Option<f64>) -> Size {
        match (width, height) {
            (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Size {
                width: w,
                height: h,
                guessed: false,
            },
            _ => Size {
                width: DEFAULT_FOOTPRINT_MM.0,
                height: DEFAULT_FOOTPRINT_MM.1,
                guessed: true,
            },
        }
    }
}

/// A side of a device in the plate's frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Above,
    Below,
    Left,
    Right,
}

/// Free space a part asks for on each side, in mm, as in its data sheet
/// (upright). `None` means the part says nothing for that side.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Margins {
    pub above: Option<f64>,
    pub below: Option<f64>,
    pub left: Option<f64>,
    pub right: Option<f64>,
}

impl Margins {
    pub fn is_empty(&self) -> bool {
        *self == Margins::default()
    }
}

/// A device placed on the plate. `rotated` is 90 degrees.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub rotated: bool,
    pub size: Size,
    /// Extra gap this device needs on every side, if more than the default.
    pub clearance: Option<f64>,
    /// Per-side gaps from the part's data; a side set here replaces
    /// `clearance` for that side.
    pub margins: Margins,
    /// The rail the device is mounted on, if any.
    pub rail: Option<u64>,
}

impl Placement {
    /// The gap this device needs on `side` (a side of the plate): its part's
    /// figure for that side, else `clearance`, else the default. A rotated
    /// device turns its sides with it, 90 degrees clockwise: what the data
    /// sheet calls above is then on the right.
    pub fn need(&self, side: Side) -> f64 {
        let m = &self.margins;
        let own = match (self.rotated, side) {
            (false, Side::Above) => m.above,
            (false, Side::Below) => m.below,
            (false, Side::Left) => m.left,
            (false, Side::Right) => m.right,
            (true, Side::Above) => m.left,
            (true, Side::Below) => m.right,
            (true, Side::Left) => m.below,
            (true, Side::Right) => m.above,
        };
        own.or(self.clearance).unwrap_or(DEFAULT_CLEARANCE_MM)
    }

    /// The largest gap this device asks for on any side.
    pub fn max_need(&self) -> f64 {
        [Side::Above, Side::Below, Side::Left, Side::Right]
            .into_iter()
            .map(|s| self.need(s))
            .fold(0.0, f64::max)
    }

    pub fn footprint(&self) -> Rect {
        let (w, h) = if self.rotated {
            (self.size.height, self.size.width)
        } else {
            (self.size.width, self.size.height)
        };
        Rect {
            x: self.x,
            y: self.y,
            w,
            h,
        }
    }
}

/// The mounting plate: usable area is the plate inside its margin.
#[derive(Debug, Clone, PartialEq)]
pub struct Plate {
    pub width: f64,
    pub height: f64,
    pub margin: f64,
}

impl Plate {
    pub fn usable(&self) -> Rect {
        Rect {
            x: self.margin,
            y: self.margin,
            w: (self.width - 2.0 * self.margin).max(0.0),
            h: (self.height - 2.0 * self.margin).max(0.0),
        }
    }
}

/// Ids of placements whose footprints overlap, each pair once, ordered.
pub fn collisions(placements: &[Placement]) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    for (i, a) in placements.iter().enumerate() {
        for b in &placements[i + 1..] {
            if a.footprint().overlaps(&b.footprint()) {
                out.push((a.id.min(b.id), a.id.max(b.id)));
            }
        }
    }
    out.sort();
    out
}

/// Ids of placements that leave the usable plate area.
pub fn outside(plate: &Plate, placements: &[Placement]) -> Vec<u64> {
    let usable = plate.usable();
    placements
        .iter()
        .filter(|p| !usable.contains(&p.footprint()))
        .map(|p| p.id)
        .collect()
}

/// Pairs that do not overlap but are closer than the gap they require, with
/// the actual gap. Side by side or one above the other, the requirement is
/// the larger of the two facing sides' needs ([`Placement::need`]); on a
/// diagonal it is the larger of the horizontal and vertical needs, against
/// the straight distance.
pub fn clearance_violations(placements: &[Placement]) -> Vec<(u64, u64, f64)> {
    let mut out = Vec::new();
    for (i, a) in placements.iter().enumerate() {
        for b in &placements[i + 1..] {
            let (fa, fb) = (a.footprint(), b.footprint());
            if fa.overlaps(&fb) {
                continue;
            }
            let dx = (fb.x - fa.right()).max(fa.x - fb.right());
            let dy = (fb.y - fa.bottom()).max(fa.y - fb.bottom());
            // Needs on the facing sides, horizontal then vertical.
            let (a_h, b_h) = if fb.x >= fa.right() {
                (a.need(Side::Right), b.need(Side::Left))
            } else {
                (a.need(Side::Left), b.need(Side::Right))
            };
            let (a_v, b_v) = if fb.y >= fa.bottom() {
                (a.need(Side::Below), b.need(Side::Above))
            } else {
                (a.need(Side::Above), b.need(Side::Below))
            };
            let (need, gap) = if dx > 0.0 && dy > 0.0 {
                (a_h.max(b_h).max(a_v.max(b_v)), fa.gap(&fb))
            } else if dx > 0.0 {
                (a_h.max(b_h), dx)
            } else {
                (a_v.max(b_v), dy.max(0.0))
            };
            if gap < need {
                out.push((a.id.min(b.id), a.id.max(b.id), gap));
            }
        }
    }
    out.sort_by_key(|v| (v.0, v.1));
    out
}

/// Plate usage: used and free area of the usable region (overlaps counted twice).
pub fn fits(plate: &Plate, placements: &[Placement]) -> (f64, f64) {
    let used: f64 = placements.iter().map(|p| p.footprint().area()).sum();
    (used, plate.usable().area() - used)
}

/// A horizontal DIN rail or duct on the plate; devices on it are lined up
/// along `length`, starting at `x`.
#[derive(Debug, Clone, PartialEq)]
pub struct Rail {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub length: f64,
    /// Runs down from `(x, y)` instead of to the right.
    pub vertical: bool,
}

/// Default duct cross-section in mm when the model has none yet.
pub const DEFAULT_DUCT_MM: f64 = 30.0;

/// A wiring duct on the plate. Like a rail, `(x, y)` is the start of its
/// centre line, running for `length` to the right, or down when `vertical`;
/// `width` is its size across the run, centred on that line.
#[derive(Debug, Clone, PartialEq)]
pub struct Duct {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub length: f64,
    pub width: f64,
    pub vertical: bool,
}

impl Duct {
    /// The plate area the duct covers.
    pub fn footprint(&self) -> Rect {
        if self.vertical {
            Rect {
                x: self.x - self.width / 2.0,
                y: self.y,
                w: self.width,
                h: self.length,
            }
        } else {
            Rect {
                x: self.x,
                y: self.y - self.width / 2.0,
                w: self.length,
                h: self.width,
            }
        }
    }
}

/// Length used on a rail: the footprint extents of its devices along the
/// rail (widths on a horizontal rail, heights on a vertical one) plus the
/// default gap between neighbours.
pub fn rail_used(rail: &Rail, placements: &[Placement]) -> f64 {
    let on: Vec<f64> = placements
        .iter()
        .filter(|p| p.rail == Some(rail.id))
        .map(|p| {
            let f = p.footprint();
            if rail.vertical { f.h } else { f.w }
        })
        .collect();
    let gaps = on.len().saturating_sub(1) as f64 * DEFAULT_CLEARANCE_MM;
    on.iter().sum::<f64>() + gaps
}

/// Rails whose devices need more length than the rail has, with the excess.
pub fn rail_overflows(rails: &[Rail], placements: &[Placement]) -> Vec<(u64, f64)> {
    rails
        .iter()
        .filter_map(|r| {
            let over = rail_used(r, placements) - r.length;
            (over > 0.0).then_some((r.id, over))
        })
        .collect()
}

/// What a duct lies over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DuctHit {
    /// A DIN rail runs under the duct for `by` mm of its length.
    Rail { id: u64, by: f64 },
    /// A device footprint overlaps the duct by `across` x `along` mm
    /// (across the duct, then along it).
    Device { id: u64, across: f64, along: f64 },
}

/// A duct that covers part of a rail or of a device: nothing can be mounted
/// there, and the wires of the duct would run through the device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuctOverlap {
    pub duct: u64,
    pub hit: DuctHit,
}

/// Ducts that lie over a rail or a device. A rail is its centre line, so a
/// duct that ends next to a rail, or a rail beside a duct, is clear; touching
/// edges do not count.
pub fn duct_overlaps(ducts: &[Duct], rails: &[Rail], placements: &[Placement]) -> Vec<DuctOverlap> {
    const EPS: f64 = 1e-6;
    let mut out = Vec::new();
    for d in ducts {
        let f = d.footprint();
        for r in rails {
            // The rail's line against the duct's rectangle.
            let (across, from, to, lo, hi) = if r.vertical {
                (r.x, r.y, r.y + r.length, f.y, f.bottom())
            } else {
                (r.y, r.x, r.x + r.length, f.x, f.right())
            };
            let (side_lo, side_hi) = if r.vertical {
                (f.x, f.right())
            } else {
                (f.y, f.bottom())
            };
            if across > side_lo + EPS && across < side_hi - EPS {
                let by = to.min(hi) - from.max(lo);
                if by > EPS {
                    out.push(DuctOverlap {
                        duct: d.id,
                        hit: DuctHit::Rail { id: r.id, by },
                    });
                }
            }
        }
        for p in placements {
            let o = p.footprint();
            let w = o.right().min(f.right()) - o.x.max(f.x);
            let h = o.bottom().min(f.bottom()) - o.y.max(f.y);
            if w > EPS && h > EPS {
                let (across, along) = if d.vertical { (w, h) } else { (h, w) };
                out.push(DuctOverlap {
                    duct: d.id,
                    hit: DuctHit::Device {
                        id: p.id,
                        across,
                        along,
                    },
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: u64, x: f64, y: f64, w: f64, h: f64) -> Placement {
        Placement {
            id,
            x,
            y,
            rotated: false,
            size: Size::from_part(Some(w), Some(h)),
            clearance: None,
            margins: Margins::default(),
            rail: None,
        }
    }

    #[test]
    fn touching_is_not_a_collision_overlap_is() {
        let v = [p(1, 0.0, 0.0, 50.0, 50.0), p(2, 50.0, 0.0, 50.0, 50.0)];
        assert!(collisions(&v).is_empty());
        let v = [p(1, 0.0, 0.0, 50.0, 50.0), p(2, 49.0, 0.0, 50.0, 50.0)];
        assert_eq!(collisions(&v), vec![(1, 2)]);
    }

    #[test]
    fn rotation_swaps_the_footprint() {
        let mut a = p(1, 0.0, 0.0, 20.0, 80.0);
        a.rotated = true;
        let f = a.footprint();
        assert_eq!((f.w, f.h), (80.0, 20.0));
    }

    #[test]
    fn missing_size_is_guessed_never_zero() {
        let s = Size::from_part(None, Some(30.0));
        assert!(s.guessed);
        assert_eq!((s.width, s.height), DEFAULT_FOOTPRINT_MM);
        assert!(!Size::from_part(Some(10.0), Some(10.0)).guessed);
    }

    #[test]
    fn clearance_uses_the_larger_requirement() {
        let mut a = p(1, 0.0, 0.0, 50.0, 50.0);
        let b = p(2, 0.0, 70.0, 50.0, 50.0);
        assert!(clearance_violations(&[a.clone(), b.clone()]).is_empty());
        a.clearance = Some(30.0);
        let v = clearance_violations(&[a, b]);
        assert_eq!(v, vec![(1, 2, 20.0)]);
    }

    #[test]
    fn part_margins_apply_to_the_facing_sides_only() {
        let mut drive = p(1, 0.0, 0.0, 50.0, 50.0);
        drive.margins.above = Some(100.0);
        drive.margins.below = Some(100.0);
        // 20 mm to the right: the drive asks nothing sideways.
        let right = p(2, 70.0, 0.0, 50.0, 50.0);
        assert!(clearance_violations(&[drive.clone(), right]).is_empty());
        // 20 mm below: its 100 mm below clears nothing.
        let below = p(3, 0.0, 70.0, 50.0, 50.0);
        assert_eq!(
            clearance_violations(&[drive.clone(), below.clone()]),
            vec![(1, 3, 20.0)]
        );
        // The neighbour's own figure for the facing side counts too.
        let mut above = p(4, 0.0, -70.0, 50.0, 50.0);
        above.margins.below = Some(40.0);
        let plain = p(5, 0.0, 70.0, 50.0, 50.0);
        assert_eq!(clearance_violations(&[above, plain.clone()]).len(), 0);
        let mut tall = p(6, 0.0, 0.0, 50.0, 50.0);
        tall.margins.below = Some(40.0);
        assert_eq!(clearance_violations(&[tall, plain]), vec![(5, 6, 20.0)]);
    }

    #[test]
    fn a_rotated_device_turns_its_margins() {
        let mut a = p(1, 0.0, 0.0, 20.0, 80.0);
        a.margins.above = Some(30.0);
        assert_eq!(a.need(Side::Above), 30.0);
        assert_eq!(a.need(Side::Right), DEFAULT_CLEARANCE_MM);
        a.rotated = true;
        assert_eq!(a.need(Side::Right), 30.0);
        assert_eq!(a.need(Side::Above), DEFAULT_CLEARANCE_MM);
        assert_eq!(a.max_need(), 30.0);
    }

    #[test]
    fn diagonal_gap_is_euclidean() {
        let a = Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        };
        let b = Rect {
            x: 13.0,
            y: 14.0,
            w: 10.0,
            h: 10.0,
        };
        assert_eq!(a.gap(&b), 5.0);
    }

    #[test]
    fn outside_respects_the_margin() {
        let plate = Plate {
            width: 200.0,
            height: 200.0,
            margin: 10.0,
        };
        let v = [
            p(1, 10.0, 10.0, 50.0, 50.0),
            p(2, 5.0, 10.0, 50.0, 50.0),
            p(3, 150.0, 150.0, 50.0, 50.0),
        ];
        assert_eq!(outside(&plate, &v), vec![2, 3]);
    }

    #[test]
    fn fits_reports_used_and_free() {
        let plate = Plate {
            width: 120.0,
            height: 120.0,
            margin: 10.0,
        };
        let (used, free) = fits(&plate, &[p(1, 10.0, 10.0, 50.0, 50.0)]);
        assert_eq!((used, free), (2500.0, 10000.0 - 2500.0));
    }

    #[test]
    fn rail_overflow_counts_gaps() {
        let rail = Rail {
            vertical: false,
            id: 7,
            x: 0.0,
            y: 0.0,
            length: 100.0,
        };
        let mut a = p(1, 0.0, 0.0, 45.0, 90.0);
        let mut b = p(2, 55.0, 0.0, 45.0, 90.0);
        a.rail = Some(7);
        b.rail = Some(7);
        assert_eq!(rail_used(&rail, &[a.clone(), b.clone()]), 100.0);
        assert!(rail_overflows(std::slice::from_ref(&rail), &[a.clone(), b.clone()]).is_empty());
        let mut c = p(3, 110.0, 0.0, 20.0, 90.0);
        c.rail = Some(7);
        assert_eq!(rail_overflows(&[rail], &[a, b, c]), vec![(7, 30.0)]);
    }

    #[test]
    fn vertical_rail_counts_heights() {
        let rail = Rail {
            id: 7,
            x: 0.0,
            y: 0.0,
            length: 100.0,
            vertical: true,
        };
        let mut a = p(1, 0.0, 0.0, 20.0, 40.0);
        let mut b = p(2, 0.0, 50.0, 60.0, 40.0);
        a.rail = Some(7);
        b.rail = Some(7);
        assert_eq!(rail_used(&rail, &[a.clone(), b.clone()]), 90.0);
        let mut c = p(3, 0.0, 90.0, 10.0, 30.0);
        c.rail = Some(7);
        assert_eq!(rail_overflows(&[rail], &[a, b, c]), vec![(7, 30.0)]);
    }
}
