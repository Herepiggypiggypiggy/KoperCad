//! Cabinet layout engine (WS-15). Pure geometry on a mounting plate:
//! footprints, collisions, clearances and fit. See `docs/design/cabinet-layout.md`.
//!
//! Millimetres, origin top left, x right, y down. Nothing here mutates a
//! project; the model storage and commands come later.

pub mod check;
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
    /// The rail the device is mounted on, if any.
    pub rail: Option<u64>,
}

impl Placement {
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

/// Pairs that do not overlap but are closer than the larger of their
/// required gaps (default [`DEFAULT_CLEARANCE_MM`]), with the actual gap.
pub fn clearance_violations(placements: &[Placement]) -> Vec<(u64, u64, f64)> {
    let mut out = Vec::new();
    for (i, a) in placements.iter().enumerate() {
        for b in &placements[i + 1..] {
            let (fa, fb) = (a.footprint(), b.footprint());
            if fa.overlaps(&fb) {
                continue;
            }
            let need = a
                .clearance
                .unwrap_or(DEFAULT_CLEARANCE_MM)
                .max(b.clearance.unwrap_or(DEFAULT_CLEARANCE_MM));
            let gap = fa.gap(&fb);
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
