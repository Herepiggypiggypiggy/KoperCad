//! Where a device sits on a DIN rail, and where a loose device snaps onto one.
//!
//! A device clipped on a TS 35 rail has the rail behind its middle, so the
//! rail's centre line runs through the centre of the device across the run
//! (the planning rule: real devices differ by a few millimetres, the part
//! data's own height decides the middle). Along the run a device stands
//! fully on the rail, and neighbours sit side by side with the rule set's gap.

use crate::{DEFAULT_CLEARANCE_MM, Placement, Rail};

/// How far the centre of a device may be from a rail's line and still snap
/// onto it, mm.
pub const MOUNT_REACH_MM: f64 = 15.0;
/// How close a device's side gets to the end of its rail or to a neighbour
/// before it lines up with it, mm.
pub const ALONG_SNAP_MM: f64 = 6.0;

/// The top edge (or left edge, on a vertical rail) of a device of the given
/// extent across the rail run, when the rail's line is at `line`.
pub fn rail_mount_top(line: f64, extent: f64) -> f64 {
    line - extent / 2.0
}

/// Where a device lands on a rail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mount {
    pub rail: u64,
    /// Top left of the device's footprint.
    pub at: (f64, f64),
    /// Distance from the rail's start to the device's near edge along the run.
    pub offset: f64,
}

/// Snaps the footprint `size` (width, height as it lies on the plate) with
/// its top left at `at` onto the nearest rail whose line passes within
/// [`MOUNT_REACH_MM`] of the device's centre and whose run the device overlaps.
/// The device is pulled onto the line across the run, kept on the rail along
/// it, and lined up with the rail's end or a neighbour on the same rail when
/// within [`ALONG_SNAP_MM`] (`gap` apart from a neighbour). `others` are the
/// placements on the plate; `device` itself is ignored among them.
pub fn snap_to_rail(
    rails: &[Rail],
    others: &[Placement],
    device: u64,
    size: (f64, f64),
    at: (f64, f64),
    gap: f64,
) -> Option<Mount> {
    let (w, h) = size;
    let mut best: Option<(f64, Mount)> = None;
    for r in rails {
        // Across the run the device is centred on the line; along it, the
        // device must overlap the run.
        let (across_extent, along_extent) = if r.vertical { (w, h) } else { (h, w) };
        let (line, start, along_at, across_at) = if r.vertical {
            (r.x, r.y, at.1, at.0)
        } else {
            (r.y, r.x, at.0, at.1)
        };
        let off_line = (across_at + across_extent / 2.0 - line).abs();
        if off_line > MOUNT_REACH_MM
            || along_at + along_extent <= start
            || along_at >= start + r.length
        {
            continue;
        }
        let mut along = along_at;
        // Stand on the rail: inside its run when the device is shorter than it.
        if along_extent <= r.length {
            along = along.clamp(start, start + r.length - along_extent);
        }
        let mut targets = vec![start];
        for p in others.iter().filter(|p| p.id != device) {
            if !on_same_line(p, r) {
                continue;
            }
            let f = p.footprint();
            let (lo, hi) = if r.vertical {
                (f.y, f.bottom())
            } else {
                (f.x, f.right())
            };
            targets.push(hi + gap);
            targets.push(lo - gap - along_extent);
        }
        if let Some(t) = targets
            .into_iter()
            .filter(|t| (t - along).abs() <= ALONG_SNAP_MM)
            .min_by(|a, b| (a - along).abs().total_cmp(&(b - along).abs()))
        {
            along = t;
        }
        let across = rail_mount_top(line, across_extent);
        let mount = Mount {
            rail: r.id,
            at: if r.vertical {
                (across, along)
            } else {
                (along, across)
            },
            offset: along - start,
        };
        if best.is_none_or(|(d, _)| off_line < d) {
            best = Some((off_line, mount));
        }
    }
    best.map(|(_, m)| m)
}

/// A neighbour counts as on the rail when it is mounted on it, or sits on its
/// line (a device from an older project that was never recorded on it).
fn on_same_line(p: &Placement, r: &Rail) -> bool {
    if p.rail == Some(r.id) {
        return true;
    }
    p.rail.is_none() && off_the_line(p, r).is_some_and(|d| d < 1.0)
}

/// How far the centre of a placed device is from the line of rail `r`, across
/// the run. `None` when the device lies beside the rail's run.
pub fn off_the_line(p: &Placement, r: &Rail) -> Option<f64> {
    let f = p.footprint();
    let (centre, lo, hi, line, start) = if r.vertical {
        (f.x + f.w / 2.0, f.y, f.bottom(), r.x, r.y)
    } else {
        (f.y + f.h / 2.0, f.x, f.right(), r.y, r.x)
    };
    (hi > start && lo < start + r.length).then_some((centre - line).abs())
}

/// The rail a footprint `size` with its top left at `at` is mounted on as it
/// stands: the rail whose line runs through its centre (within half a
/// millimetre) and whose run it lies on completely, with the distance from
/// the rail's start. A device the snap has not put there (or has been put
/// beside a rail by hand) is on no rail.
pub fn mounted_on(rails: &[Rail], size: (f64, f64), at: (f64, f64)) -> Option<(u64, f64)> {
    const TOLERANCE: f64 = 0.5;
    let (w, h) = size;
    rails
        .iter()
        .filter_map(|r| {
            let (across, along, extent, line, start) = if r.vertical {
                (at.0 + w / 2.0, at.1, h, r.x, r.y)
            } else {
                (at.1 + h / 2.0, at.0, w, r.y, r.x)
            };
            let off = (across - line).abs();
            let on = along >= start - TOLERANCE && along + extent <= start + r.length + TOLERANCE;
            (off <= TOLERANCE && on).then_some((off, r.id, along - start))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id, offset)| (id, offset))
}

/// The gap neighbours get when one is lined up by hand: the engine default.
pub const SNAP_GAP_MM: f64 = DEFAULT_CLEARANCE_MM / 2.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Margins, Size};

    fn rail() -> Rail {
        Rail {
            id: 7,
            x: 50.0,
            y: 400.0,
            length: 300.0,
            vertical: false,
        }
    }

    fn placed(id: u64, x: f64, y: f64, w: f64, h: f64, rail: Option<u64>) -> Placement {
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
    fn the_centre_of_the_device_snaps_onto_the_rail_line() {
        // A 90 mm device: its top is 45 above the line.
        let m = snap_to_rail(&[rail()], &[], 1, (45.0, 90.0), (100.0, 360.0), 0.0).unwrap();
        assert_eq!(m.rail, 7);
        assert_eq!(m.at, (100.0, 355.0));
        assert_eq!(m.offset, 50.0);
    }

    #[test]
    fn a_device_with_its_top_on_the_line_is_too_far_away() {
        // The old convention: the top edge on the line is 45 mm off the centre.
        assert!(snap_to_rail(&[rail()], &[], 1, (45.0, 90.0), (100.0, 400.0), 0.0).is_none());
    }

    #[test]
    fn short_and_tall_devices_share_the_line() {
        let a = snap_to_rail(&[rail()], &[], 1, (18.0, 90.0), (60.0, 356.0), 0.0).unwrap();
        let b = snap_to_rail(&[rail()], &[], 2, (45.0, 50.0), (60.0, 376.0), 0.0).unwrap();
        assert_eq!(a.at.1 + 45.0, 400.0);
        assert_eq!(b.at.1 + 25.0, 400.0);
    }

    #[test]
    fn beside_the_run_there_is_no_snap() {
        assert!(snap_to_rail(&[rail()], &[], 1, (45.0, 90.0), (400.0, 355.0), 0.0).is_none());
        assert!(snap_to_rail(&[rail()], &[], 1, (45.0, 90.0), (0.0, 355.0), 0.0).is_none());
    }

    #[test]
    fn the_device_stays_on_the_rail_at_the_ends() {
        let m = snap_to_rail(&[rail()], &[], 1, (45.0, 90.0), (320.0, 355.0), 0.0).unwrap();
        assert_eq!(m.at.0, 305.0, "right end of the rail minus the width");
        let m = snap_to_rail(&[rail()], &[], 1, (45.0, 90.0), (30.0, 355.0), 0.0).unwrap();
        assert_eq!(m.at.0, 50.0);
    }

    #[test]
    fn a_device_lines_up_beside_its_neighbour() {
        let n = placed(2, 50.0, 355.0, 45.0, 90.0, Some(7));
        let m = snap_to_rail(
            &[rail()],
            std::slice::from_ref(&n),
            1,
            (18.0, 90.0),
            (98.0, 355.0),
            5.0,
        )
        .unwrap();
        assert_eq!(m.at.0, 100.0, "right edge 95 plus the gap");
        assert_eq!(m.offset, 50.0);
        // Far from it: left where it is.
        let m = snap_to_rail(&[rail()], &[n], 1, (18.0, 90.0), (160.0, 355.0), 5.0).unwrap();
        assert_eq!(m.at.0, 160.0);
    }

    #[test]
    fn the_device_itself_is_no_neighbour() {
        let me = placed(1, 100.0, 355.0, 45.0, 90.0, Some(7));
        let m = snap_to_rail(&[rail()], &[me], 1, (45.0, 90.0), (103.0, 355.0), 5.0).unwrap();
        assert_eq!(m.at.0, 103.0);
    }

    #[test]
    fn a_vertical_rail_takes_the_device_centre_on_its_x() {
        let v = Rail {
            id: 9,
            x: 300.0,
            y: 100.0,
            length: 400.0,
            vertical: true,
        };
        let m = snap_to_rail(&[v], &[], 1, (45.0, 90.0), (280.0, 200.0), 0.0).unwrap();
        assert_eq!(m.at, (277.5, 200.0));
        assert_eq!(m.offset, 100.0);
    }

    #[test]
    fn the_nearest_line_wins() {
        let near = Rail {
            id: 8,
            y: 390.0,
            ..rail()
        };
        let m = snap_to_rail(&[rail(), near], &[], 1, (45.0, 90.0), (100.0, 345.0), 0.0).unwrap();
        assert_eq!(m.rail, 8);
    }

    #[test]
    fn a_device_is_mounted_where_the_line_runs_through_its_centre() {
        let rails = [rail()];
        assert_eq!(
            mounted_on(&rails, (45.0, 90.0), (100.0, 355.0)),
            Some((7, 50.0))
        );
        // Off the line, hanging past the end, or beside the run: not mounted.
        assert_eq!(mounted_on(&rails, (45.0, 90.0), (100.0, 400.0)), None);
        assert_eq!(mounted_on(&rails, (45.0, 90.0), (320.0, 355.0)), None);
        assert_eq!(mounted_on(&rails, (45.0, 90.0), (400.0, 355.0)), None);
    }

    #[test]
    fn off_the_line_measures_the_centre() {
        let p = placed(1, 100.0, 355.0, 45.0, 90.0, Some(7));
        assert_eq!(off_the_line(&p, &rail()), Some(0.0));
        let q = placed(2, 100.0, 400.0, 45.0, 90.0, Some(7));
        assert_eq!(off_the_line(&q, &rail()), Some(45.0));
        let far = placed(3, 500.0, 355.0, 45.0, 90.0, Some(7));
        assert_eq!(off_the_line(&far, &rail()), None);
    }
}
