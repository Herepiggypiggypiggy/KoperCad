//! Placing a row by press and drag (docs/DESIGN.md §4, "Placing a row").
//!
//! With a symbol on the pointer, a press and drag places evenly spaced
//! copies as far as the pointer stretches. These are the pure pieces: the
//! drag state, the pitch, the count and the stop before a collision.

use novo_core::{DeviceId, PageId, Placement, Point, Project, SymbolRole};
use novo_symbols::Symbol;

use crate::catalog::Catalog;

/// The most symbols one drag places.
pub const MAX_COUNT: usize = 50;

/// Two positions this close along an axis do not overlap (touching is fine).
const TOUCH: f64 = 1e-6;

/// A row drag in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct RowDrag {
    /// Where the button went down, on the grid: the first symbol.
    pub press: Point,
    /// The axis the row runs along; switches only past the hysteresis.
    pub horizontal: bool,
    /// Grid steps added to (or taken from) the default pitch.
    pub extra_steps: i32,
    /// Where the pointer is now.
    pub to: Point,
}

impl RowDrag {
    pub fn new(press: Point) -> Self {
        RowDrag {
            press,
            horizontal: true,
            extra_steps: 0,
            to: press,
        }
    }

    /// Follows the pointer; the axis flips only when the other distance
    /// leads by more than `hysteresis`.
    pub fn follow(&mut self, pointer: Point, hysteresis: f64) {
        self.to = pointer;
        let (dx, dy) = (
            (pointer.x - self.press.x).abs(),
            (pointer.y - self.press.y).abs(),
        );
        if self.horizontal && dy > dx + hysteresis {
            self.horizontal = false;
        } else if !self.horizontal && dx > dy + hysteresis {
            self.horizontal = true;
        }
    }

    /// Signed distance of the pointer from the press along the row's axis.
    fn along(&self) -> f64 {
        if self.horizontal {
            self.to.x - self.press.x
        } else {
            self.to.y - self.press.y
        }
    }
}

/// Distance between neighbouring poles of a multi-pole symbol, which is also
/// the pitch of a terminal strip, in millimetres.
pub const POLE_PITCH: f64 = 8.0;

/// Library ids of one device in 1 to 4 poles, in pole order.
const POLE_FAMILIES: [&[&str]; 3] = [
    &[
        "iec/protection/mcb_1p",
        "iec/protection/mcb_2p",
        "iec/protection/mcb_3p",
        "iec/protection/mcb_4p",
    ],
    &[
        "iec/protection/fuse",
        "iec/protection/fuse_2p",
        "iec/protection/fuse_3p",
        "iec/protection/fuse_4p",
    ],
    &[
        "iec/contactors/main_1p",
        "iec/contactors/main_2p",
        "iec/contactors/main_3p",
        "iec/contactors/main_4p",
    ],
];

/// The 1 to 4 pole variants of a 1-pole symbol, or `None` for any other
/// symbol.
pub fn pole_family(symbol: &str) -> Option<&'static [&'static str]> {
    POLE_FAMILIES.iter().copied().find(|f| f[0] == symbol)
}

/// A drag with a 1-pole symbol in hand: the multi-pole variant to place
/// instead of a row of single poles, and where its first pole goes.
#[derive(Debug, Clone, PartialEq)]
pub struct PoleRow {
    /// Library id of the 2 to 4 pole variant.
    pub symbol: &'static str,
    pub origin: Point,
    pub poles: usize,
}

/// Turns a row drag of a 1-pole symbol along its pole axis into one
/// multi-pole symbol, one pole per [`POLE_PITCH`] dragged. `None` when the
/// symbol has no variants, the drag runs across the pole axis, or it has not
/// reached the second pole yet.
pub fn pole_row(symbol: &str, placement: &Placement, row: &RowDrag) -> Option<PoleRow> {
    let family = pole_family(symbol)?;
    let axis = placement.apply(Point::new(1.0, 0.0)) - placement.apply(Point::ZERO);
    // The poles of a symbol sit side by side along its x axis.
    if (axis.x.abs() > 0.5) != row.horizontal {
        return None;
    }
    let along = (row.to.x - row.press.x) * axis.x + (row.to.y - row.press.y) * axis.y;
    let poles = ((along.abs() / POLE_PITCH).floor() as usize + 1).min(family.len());
    if poles < 2 {
        return None;
    }
    // Dragging backwards grows the symbol the other way.
    let back = if along < 0.0 {
        (poles - 1) as f64 * POLE_PITCH
    } else {
        0.0
    };
    Some(PoleRow {
        symbol: family[poles - 1],
        origin: Point::new(row.press.x - axis.x * back, row.press.y - axis.y * back),
        poles,
    })
}

/// The page-space box `(min, max)` of `symbol` placed as `placement`.
pub fn bounds_on_page(symbol: &Symbol, placement: &Placement) -> (Point, Point) {
    let (lo, hi) = symbol.bounds();
    let corners = [
        placement.apply(Point::new(lo[0], lo[1])),
        placement.apply(Point::new(hi[0], lo[1])),
        placement.apply(Point::new(lo[0], hi[1])),
        placement.apply(Point::new(hi[0], hi[1])),
    ];
    let min = |f: fn(&Point) -> f64| corners.iter().map(f).fold(f64::INFINITY, f64::min);
    let max = |f: fn(&Point) -> f64| corners.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
    (
        Point::new(min(|p| p.x), min(|p| p.y)),
        Point::new(max(|p| p.x), max(|p| p.y)),
    )
}

/// The spacing of the terminals of `strip` already on `page` in the row the
/// press is on (same line across the row's axis), so a new row of terminals
/// lines up with the strip it joins. `None` with fewer than two there.
pub fn strip_pitch(
    project: &Project,
    page: PageId,
    strip: DeviceId,
    row: &RowDrag,
    grid: f64,
) -> Option<f64> {
    let across = if row.horizontal {
        row.press.y
    } else {
        row.press.x
    };
    let mut at: Vec<f64> = project
        .page_contents(page)
        .symbols
        .iter()
        .filter_map(|id| project.symbol(*id))
        .filter(|s| match &s.role {
            SymbolRole::Terminal { terminals } => terminals
                .iter()
                .any(|t| project.terminal(*t).is_some_and(|t| t.strip == strip)),
            _ => false,
        })
        .filter_map(|s| {
            let o = s.placement.origin;
            let (a, c) = if row.horizontal {
                (o.x, o.y)
            } else {
                (o.y, o.x)
            };
            ((c - across).abs() < grid / 2.0).then_some(a)
        })
        .collect();
    at.sort_by(f64::total_cmp);
    at.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let pitch = at
        .windows(2)
        .map(|w| w[1] - w[0])
        .fold(f64::INFINITY, f64::min);
    pitch.is_finite().then_some(pitch)
}

/// The distance between neighbours: the symbol's extent along the axis plus
/// one grid step (plus the user's `extra_steps`), rounded up to the grid.
pub fn pitch(
    symbol: &Symbol,
    placement: &Placement,
    row: &RowDrag,
    grid: f64,
    strip_pitch: Option<f64>,
) -> f64 {
    if let Some(strip) = strip_pitch {
        return (strip + f64::from(row.extra_steps) * grid).max(grid);
    }
    let (lo, hi) = bounds_on_page(symbol, placement);
    let extent = if row.horizontal {
        hi.x - lo.x
    } else {
        hi.y - lo.y
    };
    let steps = ((extent / grid).ceil() as i32 + 1 + row.extra_steps).max(1);
    f64::from(steps) * grid
}

fn overlaps(a: (Point, Point), b: (Point, Point)) -> bool {
    a.0.x < b.1.x - TOUCH && b.0.x < a.1.x - TOUCH && a.0.y < b.1.y - TOUCH && b.0.y < a.1.y - TOUCH
}

/// Where the symbols of the row go, and the first position that was left
/// out because it would overlap another device (if the row stopped short).
pub fn positions(
    project: &Project,
    catalog: &Catalog,
    page: PageId,
    symbol: &Symbol,
    placement: Placement,
    row: &RowDrag,
    pitch: f64,
) -> (Vec<Point>, Option<Point>) {
    let along = row.along();
    let sign = if along < 0.0 { -1.0 } else { 1.0 };
    let wanted = ((along.abs() / pitch).floor() as usize + 1).min(MAX_COUNT);
    let others: Vec<(Point, Point)> = project
        .page_contents(page)
        .symbols
        .iter()
        .filter_map(|id| project.symbol(*id))
        .filter_map(|s| {
            catalog
                .get(&s.symbol.0)
                .map(|def| bounds_on_page(def, &s.placement))
        })
        .collect();
    let mut out = Vec::new();
    for k in 0..wanted {
        let step = k as f64 * pitch * sign;
        let origin = if row.horizontal {
            Point::new(row.press.x + step, row.press.y)
        } else {
            Point::new(row.press.x, row.press.y + step)
        };
        let here = bounds_on_page(
            symbol,
            &Placement {
                origin,
                ..placement
            },
        );
        if others.iter().any(|o| overlaps(here, *o)) {
            return (out, Some(origin));
        }
        out.push(origin);
    }
    (out, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn the_axis_flips_only_past_the_hysteresis() {
        let mut row = RowDrag::new(p(0.0, 0.0));
        row.follow(p(20.0, 5.0), 8.0);
        assert!(row.horizontal);
        // The vertical distance leads by 6: still horizontal.
        row.follow(p(10.0, 16.0), 8.0);
        assert!(row.horizontal);
        // By 14: it switches, and back only past 8 again.
        row.follow(p(10.0, 24.0), 8.0);
        assert!(!row.horizontal);
        row.follow(p(30.0, 24.0), 8.0);
        assert!(!row.horizontal);
        row.follow(p(40.0, 24.0), 8.0);
        assert!(row.horizontal);
    }
}
