//! Editing a rail or duct on the plate by its ends: drag an end to make it
//! shorter or longer, trim it at a crossing rail or duct or at a point, and
//! read off what the cut leaves of a stock bar.
//!
//! Pure geometry on a [`CabinetLayout`]; the app turns the answer into one
//! undo step. Millimetres. An end moves along the run only; the other end
//! stays where it is, so a hand-cut piece is described by its start and its
//! length, as in the cut list.
//!
//! Rules every answer follows:
//! - an end snaps to the nearest of the plate grid, the usable edge of the
//!   plate, an end or side edge of another rail or duct, and an edge of a
//!   device beside it (within `tol`), and says what it snapped to;
//! - a rail never gets shorter than the devices on it, nor longer than the
//!   plate;
//! - a duct stops at the edge of another duct, a rail band or a device it
//!   would run into, and says what stopped it, so ducts never overlap;
//! - the length is at least [`MIN_LENGTH_MM`].

use crate::state::CabinetLayout;
use crate::stock::Stock;
use crate::{DEFAULT_DUCT_MM, RAIL_BAND_MM, Rect};

/// The shortest piece a rail or duct can be cut to.
pub const MIN_LENGTH_MM: f64 = 10.0;

/// Which end of a rail or duct: its start (left, or top when vertical) or
/// its finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Start,
    Finish,
}

/// What an end snapped to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapTo {
    Grid,
    /// The usable edge of the plate.
    Plate,
    /// An end or side edge of another rail or duct.
    Part(u64),
    /// The edge of a device.
    Device(u64),
}

/// What stopped an end from going further.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stop {
    /// Another duct, rail or device is in the way (its id).
    Part(u64),
    Device(u64),
    /// The usable edge of the plate.
    Plate,
    /// The devices on the rail (a rail is no shorter than they need).
    Devices,
    /// The shortest cut, [`MIN_LENGTH_MM`].
    Shortest,
}

/// A rail or duct as its start `(x, y)` and length, with what the answer
/// snapped to or was stopped by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edit {
    pub x: f64,
    pub y: f64,
    pub length: f64,
    pub snap: Option<SnapTo>,
    pub stop: Option<Stop>,
}

/// A rail or duct seen from along its run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Item {
    pub id: u64,
    pub duct: bool,
    pub vertical: bool,
    pub x: f64,
    pub y: f64,
    pub length: f64,
    /// Size across the run (35 mm band for a rail).
    pub across: f64,
}

impl Item {
    /// Start of the run along its axis.
    pub fn start(&self) -> f64 {
        if self.vertical { self.y } else { self.x }
    }

    pub fn finish(&self) -> f64 {
        self.start() + self.length
    }

    /// The centre line across the run.
    pub fn line(&self) -> f64 {
        if self.vertical { self.x } else { self.y }
    }

    /// The plate area it covers (a rail as its band).
    pub fn rect(&self) -> Rect {
        let half = self.across / 2.0;
        if self.vertical {
            Rect {
                x: self.x - half,
                y: self.y,
                w: self.across,
                h: self.length,
            }
        } else {
            Rect {
                x: self.x,
                y: self.y - half,
                w: self.length,
                h: self.across,
            }
        }
    }

    /// The same item with its run `(start, length)` along its axis.
    pub fn with_run(&self, start: f64, length: f64) -> Item {
        let mut i = *self;
        if self.vertical {
            i.y = start;
        } else {
            i.x = start;
        }
        i.length = length;
        i
    }
}

/// Every rail and duct of the layout.
pub fn items(layout: &CabinetLayout) -> Vec<Item> {
    let rails = layout.rails.iter().map(|r| Item {
        id: r.id,
        duct: false,
        vertical: r.vertical,
        x: r.x,
        y: r.y,
        length: r.length,
        across: RAIL_BAND_MM,
    });
    let ducts = layout.ducts.iter().map(|d| Item {
        id: d.id,
        duct: true,
        vertical: d.vertical,
        x: d.x,
        y: d.y,
        length: d.length,
        across: if d.width > 0.0 {
            d.width
        } else {
            DEFAULT_DUCT_MM
        },
    });
    rails.chain(ducts).collect()
}

/// The item with this id.
pub fn item(layout: &CabinetLayout, id: u64) -> Option<Item> {
    items(layout).into_iter().find(|i| i.id == id)
}

/// The run of a rectangle along an axis, and its extent across.
fn span(r: &Rect, vertical: bool) -> ((f64, f64), (f64, f64)) {
    if vertical {
        ((r.y, r.bottom()), (r.x, r.right()))
    } else {
        ((r.x, r.right()), (r.y, r.bottom()))
    }
}

/// What lies in the way of an item along its run: other ducts for a duct or
/// a rail, and for a duct also the other rails' bands and the devices.
/// `(id, kind, rect)`.
fn obstacles(layout: &CabinetLayout, me: &Item) -> Vec<(Stop, Rect)> {
    let mut out = Vec::new();
    for o in items(layout) {
        if o.id == me.id {
            continue;
        }
        // A rail may lie under nothing but a duct stops at a rail band too;
        // a rail only stops at ducts.
        if me.duct || o.duct {
            out.push((Stop::Part(o.id), o.rect()));
        }
    }
    if me.duct {
        for p in &layout.placements {
            out.push((Stop::Device(p.id), p.footprint()));
        }
    }
    out
}

/// Moves one end of `id` to `target` (mm along its run) and returns the
/// piece that results. Snaps within `tol`; `None` when there is no such
/// item.
pub fn resize_end(
    layout: &CabinetLayout,
    id: u64,
    end: End,
    target: f64,
    tol: f64,
) -> Option<Edit> {
    resize(layout, id, end, target, tol, true)
}

/// A typed length for `id`, its start staying: the same limits as a drag
/// (plate, other ducts, devices on a rail, shortest cut) but no snapping, so
/// the number typed is the number cut.
pub fn set_length(layout: &CabinetLayout, id: u64, length: f64) -> Option<Edit> {
    let me = item(layout, id)?;
    resize(layout, id, End::Finish, me.start() + length, 0.0, false)
}

fn resize(
    layout: &CabinetLayout,
    id: u64,
    end: End,
    target: f64,
    tol: f64,
    to_grid: bool,
) -> Option<Edit> {
    let me = item(layout, id)?;
    let (fixed, dir) = match end {
        End::Start => (me.finish(), -1.0),
        End::Finish => (me.start(), 1.0),
    };
    let usable = layout.plate.usable();
    let ((lo, hi), _) = span(&usable, me.vertical);

    // Snap: the nearest candidate within tolerance, else the grid.
    let mut best: Option<(f64, f64, SnapTo)> = None;
    let mut offer = |v: f64, to: SnapTo| {
        let d = (v - target).abs();
        if d <= tol && best.is_none_or(|(bd, ..)| d < bd) {
            best = Some((d, v, to));
        }
    };
    offer(lo, SnapTo::Plate);
    offer(hi, SnapTo::Plate);
    let band = me.across / 2.0;
    let (a0, a1) = (me.line() - band, me.line() + band);
    for o in items(layout).iter().filter(|o| o.id != me.id) {
        let ((s0, s1), _) = span(&o.rect(), me.vertical);
        offer(s0, SnapTo::Part(o.id));
        offer(s1, SnapTo::Part(o.id));
    }
    for p in &layout.placements {
        let f = p.footprint();
        let ((s0, s1), (c0, c1)) = span(&f, me.vertical);
        if c0 < a1 && a0 < c1 {
            offer(s0, SnapTo::Device(p.id));
            offer(s1, SnapTo::Device(p.id));
        }
    }
    let grid = layout.grid;
    let (mut at, mut snap) = match best {
        Some((_, v, to)) => (v, Some(to)),
        None if to_grid && grid > 0.0 => ((target / grid).round() * grid, Some(SnapTo::Grid)),
        None => (target, None),
    };
    let mut stop = None;

    // Limits on the moving end, from the fixed end outwards.
    let mut limit_out = if dir > 0.0 { hi } else { lo };
    let mut out_stop = Stop::Plate;
    for (kind, r) in obstacles(layout, &me) {
        let ((s0, s1), (c0, c1)) = span(&r, me.vertical);
        if !(c0 < a1 - 1e-6 && a0 + 1e-6 < c1) {
            continue;
        }
        // Only obstacles wholly beyond the fixed end block the way.
        if dir > 0.0 && s0 >= fixed - 1e-6 && s0 < limit_out {
            limit_out = s0;
            out_stop = kind;
        } else if dir < 0.0 && s1 <= fixed + 1e-6 && s1 > limit_out {
            limit_out = s1;
            out_stop = kind;
        }
    }
    if (at - limit_out) * dir > 0.0 {
        at = limit_out;
        stop = Some(out_stop);
        snap = None;
    }

    // The devices on a rail set how short it can be.
    if !me.duct {
        let on: Vec<(f64, f64)> = layout
            .placements
            .iter()
            .filter(|p| p.rail == Some(me.id))
            .map(|p| span(&p.footprint(), me.vertical).0)
            .collect();
        if !on.is_empty() {
            let first = on.iter().map(|s| s.0).fold(f64::MAX, f64::min);
            let last = on.iter().map(|s| s.1).fold(f64::MIN, f64::max);
            if dir > 0.0 && at < last {
                at = last;
                stop = Some(Stop::Devices);
                snap = None;
            } else if dir < 0.0 && at > first {
                at = first;
                stop = Some(Stop::Devices);
                snap = None;
            }
        }
    }
    // The shortest cut.
    if (at - fixed) * dir < MIN_LENGTH_MM {
        at = fixed + dir * MIN_LENGTH_MM;
        stop = Some(Stop::Shortest);
        snap = None;
    }

    let (start, finish) = if dir > 0.0 { (fixed, at) } else { (at, fixed) };
    let run = me.with_run(start, finish - start);
    Some(Edit {
        x: run.x,
        y: run.y,
        length: run.length,
        snap,
        stop,
    })
}

/// A rail or duct moved as a whole: where its start lands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moved {
    pub x: f64,
    pub y: f64,
    pub snap: Option<SnapTo>,
    /// What it was pushed back from (it would have lain over it).
    pub stop: Option<Stop>,
}

/// Where the start of `id` lands when dragged to `(x, y)`: an edge of the
/// piece snaps to the plate edge, an edge of another rail or duct or of a
/// device within `tol` (else the start goes to the grid), it stays on the
/// usable area, and a duct is slid to the nearest free place instead of
/// lying over another duct, a rail or a device. A rail takes its devices with
/// it, so devices are no obstacle to a rail.
pub fn move_to(layout: &CabinetLayout, id: u64, x: f64, y: f64, tol: f64) -> Option<Moved> {
    let me = item(layout, id)?;
    let usable = layout.plate.usable();
    let r = me.rect();
    // The start of the piece is `(x, y)`; the rectangle hangs off it.
    let (off_x, off_y) = (r.x - me.x, r.y - me.y);
    let rect_at = |x: f64, y: f64| Rect {
        x: x + off_x,
        y: y + off_y,
        w: r.w,
        h: r.h,
    };
    let want = rect_at(x, y);
    let mut snap = None;
    let mut out = [x, y];
    #[allow(clippy::needless_range_loop)] // `axis` also picks the rectangle side
    for axis in 0..2 {
        let (lo, hi) = if axis == 0 {
            (want.x, want.right())
        } else {
            (want.y, want.bottom())
        };
        let mut best: Option<(f64, f64, SnapTo)> = None;
        let mut offer = |edge: f64, to_edge: f64, to: SnapTo| {
            let shift = to_edge - edge;
            if shift.abs() <= tol && best.is_none_or(|(d, ..)| shift.abs() < d) {
                best = Some((shift.abs(), shift, to));
            }
        };
        let (u0, u1) = if axis == 0 {
            (usable.x, usable.right())
        } else {
            (usable.y, usable.bottom())
        };
        offer(lo, u0, SnapTo::Plate);
        offer(hi, u1, SnapTo::Plate);
        let mut edges: Vec<(f64, f64, SnapTo)> = Vec::new();
        for o in items(layout).iter().filter(|o| o.id != id) {
            let b = o.rect();
            let (a, c) = if axis == 0 {
                (b.x, b.right())
            } else {
                (b.y, b.bottom())
            };
            edges.push((a, c, SnapTo::Part(o.id)));
        }
        for p in &layout.placements {
            let b = p.footprint();
            let (a, c) = if axis == 0 {
                (b.x, b.right())
            } else {
                (b.y, b.bottom())
            };
            edges.push((a, c, SnapTo::Device(p.id)));
        }
        for (a, c, to) in edges {
            // Its far edge to my near edge and the other way round (flush),
            // and edge to edge (lined up).
            offer(hi, a, to);
            offer(lo, c, to);
            offer(lo, a, to);
            offer(hi, c, to);
        }
        match best {
            Some((_, shift, to)) => {
                out[axis] += shift;
                snap = snap.or(Some(to));
            }
            None if layout.grid > 0.0 => {
                out[axis] = (out[axis] / layout.grid).round() * layout.grid;
                snap = snap.or(Some(SnapTo::Grid));
            }
            None => {}
        }
    }
    // Keep it on the usable area.
    let fit = |v: f64, lo_edge: f64, hi_edge: f64, off: f64, size: f64| {
        v.clamp(lo_edge - off, (hi_edge - size - off).max(lo_edge - off))
    };
    out[0] = fit(out[0], usable.x, usable.right(), off_x, r.w);
    out[1] = fit(out[1], usable.y, usable.bottom(), off_y, r.h);

    // Never over another duct, rail band or (for a duct) device.
    let blockers = obstacles(layout, &me);
    let hit = |x: f64, y: f64| {
        let c = rect_at(x, y);
        blockers
            .iter()
            .find(|(_, b)| c.overlaps(b))
            .map(|(k, _)| *k)
    };
    let mut stop = None;
    if let Some(first) = hit(out[0], out[1]) {
        // Slide to the nearest free edge of what is in the way.
        let mut best: Option<(f64, f64, f64)> = None;
        for (_, b) in &blockers {
            let c = rect_at(out[0], out[1]);
            if !c.overlaps(b) {
                continue;
            }
            for (nx, ny) in [
                (b.x - r.w - off_x, out[1]),
                (b.right() - off_x, out[1]),
                (out[0], b.y - r.h - off_y),
                (out[0], b.bottom() - off_y),
            ] {
                let in_plate = {
                    let c = rect_at(nx, ny);
                    c.x >= usable.x - 1e-6
                        && c.y >= usable.y - 1e-6
                        && c.right() <= usable.right() + 1e-6
                        && c.bottom() <= usable.bottom() + 1e-6
                };
                if in_plate && hit(nx, ny).is_none() {
                    let d = (nx - out[0]).abs() + (ny - out[1]).abs();
                    if best.is_none_or(|(bd, ..)| d < bd) {
                        best = Some((d, nx, ny));
                    }
                }
            }
        }
        match best {
            Some((_, nx, ny)) => {
                out = [nx, ny];
                stop = Some(first);
            }
            None => {
                out = [me.x, me.y];
                stop = Some(first);
            }
        }
        snap = None;
    }
    Some(Moved {
        x: out[0],
        y: out[1],
        snap,
        stop,
    })
}

/// A cut: the piece that stays and how much was cut off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trim {
    pub edit: Edit,
    /// Which end was cut.
    pub end: End,
    /// Length removed, mm.
    pub cut_off: f64,
}

/// Cuts `id` at `at` (mm along its run), removing the part beyond it at the
/// chosen `end`. `None` when the cut would leave less than the shortest
/// piece, would not remove anything, or would cut into the devices on a rail.
pub fn trim_at(layout: &CabinetLayout, id: u64, end: End, at: f64) -> Option<Trim> {
    let me = item(layout, id)?;
    let run = match end {
        End::Finish => (me.start(), at),
        End::Start => (at, me.finish()),
    };
    let length = run.1 - run.0;
    let cut_off = me.length - length;
    if cut_off <= 1e-6 || length < MIN_LENGTH_MM {
        return None;
    }
    if !me.duct {
        for p in layout.placements.iter().filter(|p| p.rail == Some(id)) {
            let (s0, s1) = span(&p.footprint(), me.vertical).0;
            if s0 < run.0 - 1e-6 || s1 > run.1 + 1e-6 {
                return None;
            }
        }
    }
    let kept = me.with_run(run.0, length);
    Some(Trim {
        edit: Edit {
            x: kept.x,
            y: kept.y,
            length,
            snap: None,
            stop: None,
        },
        end,
        cut_off,
    })
}

/// Trims `end` of `id` back to the nearest rail or duct that crosses it
/// (the way a CAD trim cuts at the next line): the part from that crossing
/// to the end is removed. `None` when nothing crosses between the ends.
pub fn trim_to_crossing(layout: &CabinetLayout, id: u64, end: End) -> Option<Trim> {
    let me = item(layout, id)?;
    let (a0, a1) = (me.line() - me.across / 2.0, me.line() + me.across / 2.0);
    // The cut of every item that runs across this one and reaches its line:
    // at its near edge, inside the run.
    let cuts = items(layout)
        .into_iter()
        .filter(|o| o.id != id && o.vertical != me.vertical)
        .filter_map(|o| {
            let ((s0, s1), (c0, c1)) = span(&o.rect(), me.vertical);
            if !(c0 < a1 && a0 < c1) {
                return None;
            }
            let cut = match end {
                End::Finish => s0,
                End::Start => s1,
            };
            (cut > me.start() + MIN_LENGTH_MM && cut < me.finish() - MIN_LENGTH_MM).then_some(cut)
        });
    // The crossing nearest the end that is cut.
    let at = match end {
        End::Finish => cuts.reduce(f64::max)?,
        End::Start => cuts.reduce(f64::min)?,
    };
    trim_at(layout, id, end, at)
}

/// What a length is cut from: whole bars, and what is left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarNote {
    /// Whole bars the piece needs (more than one: it is spliced).
    pub bars: usize,
    /// Length left on the last bar after the cut and the kerf; 0 for an
    /// exact bar.
    pub offcut: f64,
    /// True when the offcut is long enough to keep.
    pub kept: bool,
}

/// The bar a piece of `length` is cut from by itself: one bar when it fits,
/// else whole bars plus a remainder. The cut list (`stock::optimise`) shares
/// bars between pieces; this is the single-piece reading for the inspector.
pub fn bar_note(length: f64, stock: &Stock) -> BarNote {
    if length <= 0.0 || stock.length <= 0.0 {
        return BarNote {
            bars: 0,
            offcut: 0.0,
            kept: false,
        };
    }
    let exact = (length - stock.length).abs() < 1e-6;
    let mut bars = (length / stock.length).ceil().max(1.0) as usize;
    if length > stock.length && (length / stock.length).fract() < 1e-9 {
        bars = (length / stock.length) as usize;
    }
    let used = length - (bars as f64 - 1.0) * stock.length;
    let offcut = if exact || used >= stock.length {
        0.0
    } else {
        (stock.length - used - stock.kerf).max(0.0)
    };
    BarNote {
        bars,
        offcut,
        kept: offcut >= stock.min_offcut,
    }
}

/// The standard lengths a bar is cut into at the workshop: a full bar, a
/// half, a quarter and a fifth (2000, 1000, 500, 400 mm for a 2 m bar).
pub fn stock_lengths(stock: &Stock) -> Vec<f64> {
    [1.0, 0.5, 0.25, 0.2]
        .iter()
        .map(|f| stock.length * f)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duct, Margins, Placement, Plate, Rail, Size};

    fn layout() -> CabinetLayout {
        let mut l = CabinetLayout::new(Plate {
            width: 600.0,
            height: 800.0,
            margin: 10.0,
        });
        l.grid = 5.0;
        l
    }

    fn duct(id: u64, x: f64, y: f64, length: f64, vertical: bool) -> Duct {
        Duct {
            id,
            x,
            y,
            length,
            width: 30.0,
            vertical,
        }
    }

    fn dev(id: u64, x: f64, y: f64, w: f64, h: f64, rail: Option<u64>) -> Placement {
        Placement {
            id,
            x,
            y,
            rotated: false,
            size: Size::from_part(Some(w), Some(h)),
            clearance: None,
            margins: Margins::default(),
            rail,
        }
    }

    #[test]
    fn dragging_the_finish_end_keeps_the_start_and_snaps_to_the_grid() {
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 200.0, false));
        let e = resize_end(&l, 1, End::Finish, 303.0, 2.0).unwrap();
        assert_eq!((e.x, e.y, e.length), (20.0, 100.0, 285.0));
    }

    #[test]
    fn dragging_the_start_end_keeps_the_finish_in_place() {
        let mut l = layout();
        l.ducts.push(duct(1, 100.0, 100.0, 200.0, false));
        let e = resize_end(&l, 1, End::Start, 50.0, 2.0).unwrap();
        assert_eq!((e.x, e.length), (50.0, 250.0));
        assert_eq!(e.x + e.length, 300.0);
    }

    #[test]
    fn an_end_snaps_to_another_duct_and_to_a_device_edge() {
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 100.0, false));
        l.ducts.push(duct(2, 333.0, 300.0, 100.0, true));
        let e = resize_end(&l, 1, End::Finish, 321.0, 18.0).unwrap();
        // The vertical duct's left edge is 333 - 15 = 318.
        assert_eq!(e.snap, Some(SnapTo::Part(2)));
        assert_eq!(e.x + e.length, 318.0);
        let mut l = layout();
        l.rails.push(Rail {
            id: 7,
            x: 20.0,
            y: 100.0,
            length: 200.0,
            vertical: false,
        });
        l.placements.push(dev(9, 120.0, 90.0, 45.0, 20.0, Some(7)));
        let e = resize_end(&l, 7, End::Finish, 168.0, 5.0).unwrap();
        // Not below the device's right edge (165): it snaps there and stops.
        assert_eq!(e.x + e.length, 165.0);
    }

    #[test]
    fn a_rail_is_never_cut_shorter_than_its_devices() {
        let mut l = layout();
        l.rails.push(Rail {
            id: 7,
            x: 20.0,
            y: 100.0,
            length: 300.0,
            vertical: false,
        });
        l.placements.push(dev(9, 120.0, 90.0, 45.0, 20.0, Some(7)));
        let e = resize_end(&l, 7, End::Finish, 60.0, 2.0).unwrap();
        assert_eq!(e.stop, Some(Stop::Devices));
        assert_eq!(e.x + e.length, 165.0);
        let e = resize_end(&l, 7, End::Start, 200.0, 2.0).unwrap();
        assert_eq!(e.stop, Some(Stop::Devices));
        assert_eq!(e.x, 120.0);
        assert!(trim_at(&l, 7, End::Finish, 150.0).is_none());
        assert!(trim_at(&l, 7, End::Finish, 200.0).is_some());
    }

    #[test]
    fn a_duct_stops_at_the_next_duct_and_never_overlaps_it() {
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 100.0, false));
        l.ducts.push(duct(2, 200.0, 100.0, 100.0, false));
        let e = resize_end(&l, 1, End::Finish, 400.0, 2.0).unwrap();
        assert_eq!(e.stop, Some(Stop::Part(2)));
        assert_eq!(e.x + e.length, 200.0);
        let e = resize_end(&l, 2, End::Start, 0.0, 2.0).unwrap();
        assert_eq!(e.stop, Some(Stop::Part(1)));
        assert_eq!(e.x, 120.0);
        // A duct on another row is no obstacle.
        l.ducts.push(duct(3, 20.0, 300.0, 100.0, false));
        let e = resize_end(&l, 3, End::Finish, 580.0, 2.0).unwrap();
        assert_eq!(e.stop, None);
        assert_eq!(e.x + e.length, 580.0);
    }

    #[test]
    fn a_duct_stops_at_a_device_and_at_the_plate_edge() {
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 100.0, false));
        l.placements.push(dev(9, 250.0, 90.0, 45.0, 40.0, None));
        let e = resize_end(&l, 1, End::Finish, 500.0, 2.0).unwrap();
        assert_eq!(e.stop, Some(Stop::Device(9)));
        assert_eq!(e.x + e.length, 250.0);
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 100.0, false));
        let e = resize_end(&l, 1, End::Finish, 900.0, 2.0).unwrap();
        assert_eq!(e.x + e.length, 590.0);
        let e = resize_end(&l, 1, End::Start, -50.0, 2.0).unwrap();
        assert_eq!(e.x, 10.0);
    }

    #[test]
    fn the_shortest_cut_is_ten_millimetres() {
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 100.0, false));
        let e = resize_end(&l, 1, End::Finish, 0.0, 2.0).unwrap();
        assert_eq!(e.length, MIN_LENGTH_MM);
        assert_eq!(e.stop, Some(Stop::Shortest));
    }

    #[test]
    fn vertical_ducts_resize_along_y() {
        let mut l = layout();
        l.ducts.push(duct(1, 100.0, 50.0, 300.0, true));
        let e = resize_end(&l, 1, End::Start, 100.0, 2.0).unwrap();
        assert_eq!((e.x, e.y, e.length), (100.0, 100.0, 250.0));
    }

    #[test]
    fn trim_to_crossing_cuts_at_the_near_edge_of_the_crossing_duct() {
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 400.0, false));
        // A vertical duct at x 250 (edges 235..265) crossing the row.
        l.ducts.push(duct(2, 250.0, 40.0, 300.0, true));
        let t = trim_to_crossing(&l, 1, End::Finish).unwrap();
        assert_eq!(t.edit.length, 215.0);
        assert_eq!(t.cut_off, 185.0);
        let t = trim_to_crossing(&l, 1, End::Start).unwrap();
        assert_eq!(t.edit.x, 265.0);
        assert_eq!(t.edit.length, 155.0);
        // Nothing crosses the vertical duct's own row other than the duct.
        l.ducts.remove(1);
        assert!(trim_to_crossing(&l, 1, End::Finish).is_none());
    }

    #[test]
    fn trim_at_a_point_reports_what_was_cut_off() {
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 400.0, false));
        let t = trim_at(&l, 1, End::Finish, 220.0).unwrap();
        assert_eq!((t.edit.length, t.cut_off), (200.0, 200.0));
        let t = trim_at(&l, 1, End::Start, 120.0).unwrap();
        assert_eq!((t.edit.x, t.edit.length, t.cut_off), (120.0, 300.0, 100.0));
        assert!(trim_at(&l, 1, End::Finish, 500.0).is_none());
        assert!(trim_at(&l, 1, End::Finish, 25.0).is_none());
    }

    #[test]
    fn moving_a_duct_snaps_flush_and_never_lies_over_another() {
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 100.0, false));
        l.ducts.push(duct(2, 20.0, 200.0, 100.0, false));
        // Dragged to just above duct 2's top edge (185): snaps flush to it.
        let m = move_to(&l, 1, 20.0, 168.0, 6.0).unwrap();
        assert_eq!((m.x, m.y), (20.0, 170.0));
        assert_eq!(m.snap, Some(SnapTo::Part(2)));
        // Dragged over it: slid to the nearest free edge.
        let m = move_to(&l, 1, 20.0, 195.0, 2.0).unwrap();
        assert_eq!(m.stop, Some(Stop::Part(2)));
        assert_eq!((m.x, m.y), (20.0, 170.0));
        // Onto the plate edge.
        let m = move_to(&l, 1, 3.0, 100.0, 2.0).unwrap();
        assert_eq!(m.x, 10.0);
    }

    #[test]
    fn a_duct_does_not_land_on_a_device_but_a_rail_may_carry_its_devices() {
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 100.0, false));
        l.placements.push(dev(9, 300.0, 90.0, 45.0, 40.0, None));
        let m = move_to(&l, 1, 280.0, 105.0, 2.0).unwrap();
        assert_eq!(m.stop, Some(Stop::Device(9)));
        l.rails.push(Rail {
            id: 7,
            x: 20.0,
            y: 400.0,
            length: 200.0,
            vertical: false,
        });
        l.placements.push(dev(10, 40.0, 390.0, 45.0, 20.0, Some(7)));
        let m = move_to(&l, 7, 20.0, 405.0, 2.0).unwrap();
        assert_eq!(m.stop, None);
        assert_eq!((m.x, m.y), (20.0, 405.0));
    }

    #[test]
    fn a_typed_length_is_cut_exactly_unless_something_is_in_the_way() {
        let mut l = layout();
        l.ducts.push(duct(1, 20.0, 100.0, 100.0, false));
        l.ducts.push(duct(2, 400.0, 100.0, 100.0, false));
        let e = set_length(&l, 1, 333.0).unwrap();
        assert_eq!((e.length, e.stop), (333.0, None));
        let e = set_length(&l, 1, 500.0).unwrap();
        assert_eq!((e.length, e.stop), (380.0, Some(Stop::Part(2))));
    }

    #[test]
    fn a_bar_note_names_the_offcut() {
        let stock = Stock::default();
        let n = bar_note(1360.0, &stock);
        assert_eq!((n.bars, n.offcut, n.kept), (1, 638.0, true));
        let n = bar_note(1950.0, &stock);
        assert_eq!((n.bars, n.offcut, n.kept), (1, 48.0, false));
        let n = bar_note(2000.0, &stock);
        assert_eq!((n.bars, n.offcut), (1, 0.0));
        let n = bar_note(2500.0, &stock);
        assert_eq!((n.bars, n.offcut), (2, 1498.0));
        assert_eq!(stock_lengths(&stock), vec![2000.0, 1000.0, 500.0, 400.0]);
    }
}
