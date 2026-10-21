//! Checks on a routed wire that the cut length does not need: whether the
//! wire can take the corners of its path, and whether it passes close to a
//! device that gives off heat.
//!
//! Both are **models**, not measurements. A route is a centre line through
//! ducts of a stated width; a wire has an outer diameter and a minimum bend
//! radius of a stated multiple of it. Nothing here knows how stiff a given
//! cable is, how a duct corner is really cut, or how hot a device surface
//! gets. The caller says so where it shows the result.

use crate::{Duct, Point, Route};

/// A corner of the path that is tighter than the wire's minimum bend radius.
#[derive(Debug, Clone, PartialEq)]
pub struct TightBend {
    /// The corner on the plate.
    pub at: Point,
    /// Radius the wire needs, mm (factor times diameter).
    pub needed_mm: f64,
    /// Largest radius the two ducts leave at this corner, mm.
    pub available_mm: f64,
    /// The two ducts meeting at the corner.
    pub ducts: (u64, u64),
    /// How high the taller of the two stands off the plate, mm: where a mark
    /// must sit to be seen above them.
    pub top_mm: f64,
}

const ON_LINE_MM: f64 = 0.5;
/// Samples along the arc when testing it against the ducts.
const ARC_STEPS: usize = 48;
/// Upper bound of the search for the available radius, mm.
const MAX_RADIUS_MM: f64 = 5000.0;

fn is_horizontal(d: &Duct) -> bool {
    (d.a.y - d.b.y).abs() < ON_LINE_MM
}

/// Whether `p` lies on the duct's middle line, ends included.
fn on_duct(d: &Duct, p: Point) -> bool {
    if is_horizontal(d) {
        let (lo, hi) = (d.a.x.min(d.b.x), d.a.x.max(d.b.x));
        (p.y - d.a.y).abs() <= d.width / 2.0 + ON_LINE_MM
            && p.x >= lo - ON_LINE_MM
            && p.x <= hi + ON_LINE_MM
    } else {
        let (lo, hi) = (d.a.y.min(d.b.y), d.a.y.max(d.b.y));
        (p.x - d.a.x).abs() <= d.width / 2.0 + ON_LINE_MM
            && p.y >= lo - ON_LINE_MM
            && p.y <= hi + ON_LINE_MM
    }
}

/// Largest radius of a circular arc, tangent to both middle lines, that stays
/// inside the union of two ducts meeting at a right angle. `along_u` is the
/// half width of the duct whose axis is `u` (it limits how far the arc may
/// stray in `v`), `along_v` the other. Found by bisection: a larger radius
/// never fits where a smaller one does not.
fn available_radius(half_u: f64, half_v: f64) -> f64 {
    let fits = |r: f64| -> bool {
        (0..=ARC_STEPS).all(|i| {
            let t = std::f64::consts::FRAC_PI_2 * i as f64 / ARC_STEPS as f64;
            // Offsets from the corner along u and v.
            let a = r * (1.0 - t.cos());
            let b = r * (1.0 - t.sin());
            // Inside the duct along u while the offset across it (b) is within
            // its half width; inside the duct along v while a is.
            b <= half_u + 1e-9 || a <= half_v + 1e-9
        })
    };
    let (mut lo, mut hi) = (0.0, MAX_RADIUS_MM);
    if fits(hi) {
        return hi;
    }
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Corners of `route` where two ducts meet at a right angle and the arc a
/// wire of `diameter_mm` needs (`factor` times its diameter) does not fit
/// the ducts. Corners in free air or at a terminal are not checked: nothing
/// limits the arc there but the person laying the wire. A duct without a
/// width is skipped (it is reported as unsized by the plan).
pub fn tight_bends(route: &Route, ducts: &[Duct], diameter_mm: f64, factor: f64) -> Vec<TightBend> {
    let needed = diameter_mm * factor;
    if !(needed.is_finite() && needed > 0.0) {
        return Vec::new();
    }
    let used: Vec<&Duct> = ducts
        .iter()
        .filter(|d| route.ducts.contains(&d.id) && d.width > 0.0)
        .collect();
    let p = &route.points;
    let mut out = Vec::new();
    for i in 1..p.len().saturating_sub(1) {
        let (a, b, c) = (p[i - 1], p[i], p[i + 1]);
        let (din_h, dout_h) = (
            (a.y - b.y).abs() < ON_LINE_MM,
            (b.y - c.y).abs() < ON_LINE_MM,
        );
        let flat_in = (a.x - b.x).abs() < ON_LINE_MM && din_h;
        let flat_out = (b.x - c.x).abs() < ON_LINE_MM && dout_h;
        if flat_in || flat_out || din_h == dout_h {
            continue; // zero-length leg or no turn
        }
        let here: Vec<&&Duct> = used.iter().filter(|d| on_duct(d, b)).collect();
        let horizontal = here.iter().find(|d| is_horizontal(d));
        let vertical = here.iter().find(|d| !is_horizontal(d));
        let (Some(h), Some(v)) = (horizontal, vertical) else {
            continue;
        };
        // u is the axis of the leg arriving at the corner.
        let (half_u, half_v) = if din_h {
            (h.width / 2.0, v.width / 2.0)
        } else {
            (v.width / 2.0, h.width / 2.0)
        };
        let available = available_radius(half_u, half_v);
        if available + 1e-6 < needed {
            out.push(TightBend {
                at: b,
                needed_mm: needed,
                available_mm: available,
                ducts: (h.id, v.id),
                top_mm: h.height.max(v.height),
            });
        }
    }
    out
}

/// A device that gives off heat, as a rectangle on the plate.
#[derive(Debug, Clone, PartialEq)]
pub struct Hot {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Power loss in watts.
    pub loss_w: f64,
}

/// A wire that passes close to a hot device.
#[derive(Debug, Clone, PartialEq)]
pub struct NearHot {
    pub device: u64,
    /// Shortest distance between the routed centre line and the outline, mm.
    pub gap_mm: f64,
    pub loss_w: f64,
}

/// Distance from an axis-aligned segment to a rectangle (0 when they touch).
fn gap_to_rect(a: Point, b: Point, hot: &Hot) -> f64 {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    let dx = (hot.x - x1).max(x0 - (hot.x + hot.w)).max(0.0);
    let dy = (hot.y - y1).max(y0 - (hot.y + hot.h)).max(0.0);
    dx.hypot(dy)
}

/// Hot devices the route passes closer than `min_gap_mm`, nearest first.
/// Devices below `min_loss_w` are not hot, and the devices in `own` (the
/// ones the wire is connected to) are left out: a wire ends at them.
pub fn near_hot(
    route: &Route,
    hot: &[Hot],
    own: &[u64],
    min_gap_mm: f64,
    min_loss_w: f64,
) -> Vec<NearHot> {
    // A route of one point is a point.
    let pts = &route.points;
    let mut out: Vec<NearHot> = hot
        .iter()
        .filter(|h| h.loss_w >= min_loss_w && !own.contains(&h.id))
        .filter_map(|h| {
            let gap = if pts.len() == 1 {
                gap_to_rect(pts[0], pts[0], h)
            } else {
                pts.windows(2)
                    .map(|w| gap_to_rect(w[0], w[1], h))
                    .fold(f64::INFINITY, f64::min)
            };
            (gap < min_gap_mm).then_some(NearHot {
                device: h.id,
                gap_mm: gap,
                loss_w: h.loss_w,
            })
        })
        .collect();
    out.sort_by(|a, b| a.gap_mm.total_cmp(&b.gap_mm).then(a.device.cmp(&b.device)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Kind, Point3};

    fn duct(id: u64, a: (f64, f64), b: (f64, f64), width: f64) -> Duct {
        Duct {
            id,
            a: Point::new(a.0, a.1),
            b: Point::new(b.0, b.1),
            width,
            height: 60.0,
        }
    }

    fn route(points: &[(f64, f64)], ducts: &[u64]) -> Route {
        let points: Vec<Point> = points.iter().map(|p| Point::new(p.0, p.1)).collect();
        Route {
            job: 0,
            kind: Kind::Ducted,
            points3: points.iter().map(|p| Point3::new(p.x, p.y, 0.0)).collect(),
            points,
            ducts: ducts.to_vec(),
            detour_mm: 0.0,
            rise_mm: 0.0,
            bends: 1,
            length_mm: 0.0,
        }
    }

    fn corner(width: f64) -> (Vec<Duct>, Route) {
        let ducts = vec![
            duct(1, (0.0, 100.0), (300.0, 100.0), width),
            duct(2, (300.0, 100.0), (300.0, 400.0), width),
        ];
        let r = route(&[(0.0, 100.0), (300.0, 100.0), (300.0, 400.0)], &[1, 2]);
        (ducts, r)
    }

    #[test]
    fn a_thin_wire_takes_a_corner_of_wide_ducts() {
        let (ducts, r) = corner(60.0);
        // 3 mm wire, 6 x diameter = 18 mm.
        assert!(tight_bends(&r, &ducts, 3.0, 6.0).is_empty());
    }

    #[test]
    fn a_thick_cable_does_not_fit_a_corner_of_narrow_ducts() {
        let (ducts, r) = corner(25.0);
        let found = tight_bends(&r, &ducts, 20.0, 6.0);
        assert_eq!(found.len(), 1);
        let b = &found[0];
        assert_eq!(b.at, Point::new(300.0, 100.0));
        assert_eq!(b.needed_mm, 120.0);
        assert!(
            b.available_mm > 12.5 && b.available_mm < 120.0,
            "{}",
            b.available_mm
        );
        assert_eq!(b.ducts, (1, 2));
        assert_eq!(b.top_mm, 60.0, "the ducts of the fixture stand 60 mm");
    }

    #[test]
    fn the_available_radius_grows_with_the_duct_width() {
        let r = |w: f64| available_radius(w / 2.0, w / 2.0);
        assert!(r(25.0) < r(40.0));
        assert!(r(40.0) < r(80.0));
        // Two 40 mm ducts: the arc may stray 20 mm in both ways, which
        // leaves r (1 - 1/sqrt2) <= 20 at the middle, about 68 mm.
        assert!((r(40.0) - 20.0 / (1.0 - std::f64::consts::FRAC_1_SQRT_2)).abs() < 0.5);
    }

    #[test]
    fn a_corner_in_free_air_is_not_checked() {
        let (ducts, _) = corner(25.0);
        let r = route(&[(0.0, 0.0), (300.0, 0.0), (300.0, 400.0)], &[]);
        assert!(tight_bends(&r, &ducts, 20.0, 6.0).is_empty());
    }

    #[test]
    fn a_straight_path_has_no_bend() {
        let ducts = vec![duct(1, (0.0, 100.0), (300.0, 100.0), 25.0)];
        let r = route(&[(0.0, 100.0), (150.0, 100.0), (300.0, 100.0)], &[1]);
        assert!(tight_bends(&r, &ducts, 20.0, 6.0).is_empty());
    }

    #[test]
    fn an_unsized_duct_is_skipped() {
        let (ducts, r) = corner(0.0);
        assert!(tight_bends(&r, &ducts, 20.0, 6.0).is_empty());
    }

    fn hot(id: u64, x: f64, y: f64, loss_w: f64) -> Hot {
        Hot {
            id,
            x,
            y,
            w: 50.0,
            h: 50.0,
            loss_w,
        }
    }

    #[test]
    fn a_wire_beside_a_hot_device_is_found_with_its_gap() {
        let r = route(&[(0.0, 100.0), (300.0, 100.0)], &[1]);
        // Device top edge at y = 110: 10 mm below the line.
        let found = near_hot(&r, &[hot(7, 100.0, 110.0, 30.0)], &[], 20.0, 5.0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].device, 7);
        assert!((found[0].gap_mm - 10.0).abs() < 1e-9);
    }

    #[test]
    fn far_cool_or_own_devices_are_left_out() {
        let r = route(&[(0.0, 100.0), (300.0, 100.0)], &[1]);
        let devices = [
            hot(1, 100.0, 200.0, 30.0), // 100 mm away
            hot(2, 100.0, 110.0, 2.0),  // close but cool
            hot(3, 100.0, 110.0, 30.0), // close and hot, but the wire's own
        ];
        assert!(near_hot(&r, &devices, &[3], 20.0, 5.0).is_empty());
    }

    #[test]
    fn nearest_first() {
        let r = route(&[(0.0, 100.0), (300.0, 100.0)], &[1]);
        let devices = [hot(1, 0.0, 115.0, 30.0), hot(2, 100.0, 105.0, 30.0)];
        let found = near_hot(&r, &devices, &[], 20.0, 5.0);
        assert_eq!(found.iter().map(|f| f.device).collect::<Vec<_>>(), [2, 1]);
    }
}
