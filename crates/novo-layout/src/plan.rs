//! The `place all` planner: proposes rails and positions for devices on an
//! empty plate. It only proposes; accepting the plan is one undo step.

use crate::rules::Class;
use crate::{DEFAULT_CLEARANCE_MM, Plate, Rail, Size};

/// Vertical space left between rail rows for wiring ducts.
pub const ROW_GAP_MM: f64 = 40.0;

/// A device to place. Devices are placed by `sort_key` (letter code, then
/// page order), so contactors end up next to contactors.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: u64,
    pub size: Size,
    pub sort_key: String,
}

/// One proposed position (top left), mounted on `rail`: the rail's line runs
/// through the middle of the device (see [`crate::Rail`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Proposed {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub rail: u64,
}

/// A wiring duct the planner proposes between two rows. `(x, y)` is the
/// start of its centre line; it runs `length` to the right, or down when
/// `vertical`. `width` lies in the plate, `height` stands off it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedDuct {
    pub x: f64,
    pub y: f64,
    pub length: f64,
    pub vertical: bool,
    pub width: f64,
    pub height: f64,
    /// The wiring it serves; power and signal ducts are kept apart.
    pub class: Class,
}

/// The planner's proposal.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    pub rails: Vec<Rail>,
    /// Ducts between the rows (rules-based plans only).
    pub ducts: Vec<PlannedDuct>,
    pub placements: Vec<Proposed>,
    /// Devices that do not fit on the plate (too wide, or no room for another row).
    pub unplaced: Vec<u64>,
}

/// Fills rails left to right inside the usable plate area, starting a new
/// rail when the row is full. Rail ids count up from `first_rail_id`.
pub fn place_all(plate: &Plate, candidates: &[Candidate], first_rail_id: u64) -> Plan {
    place_all_below(plate, candidates, first_rail_id, 0.0)
}

/// Like [`place_all`], but the first rail starts at `top` or lower, so a
/// plan can sit below devices that are already placed.
pub fn place_all_below(
    plate: &Plate,
    candidates: &[Candidate],
    first_rail_id: u64,
    top: f64,
) -> Plan {
    let usable = plate.usable();
    let mut order: Vec<&Candidate> = candidates.iter().collect();
    order.sort_by(|a, b| a.sort_key.cmp(&b.sort_key).then(a.id.cmp(&b.id)));

    let mut plan = Plan::default();
    let (mut x, mut y, mut row_h) = (usable.x, usable.y.max(top), 0.0_f64);
    let mut rail_id = first_rail_id;
    let mut row_open = false;
    let mut full = false;

    for c in order {
        let (w, h) = (c.size.width, c.size.height);
        if full || w > usable.w || h > usable.h {
            plan.unplaced.push(c.id);
            continue;
        }
        let needed = if row_open {
            w + DEFAULT_CLEARANCE_MM
        } else {
            w
        };
        if row_open && x - usable.x + needed > usable.w {
            y += row_h + ROW_GAP_MM;
            x = usable.x;
            row_h = 0.0;
            row_open = false;
            rail_id += 1;
        }
        if y + h > usable.y + usable.h {
            full = true;
            plan.unplaced.push(c.id);
            continue;
        }
        if !row_open {
            plan.rails.push(Rail {
                id: rail_id,
                x: usable.x,
                y,
                length: usable.w,
                vertical: false,
            });
            row_open = true;
        } else {
            x += DEFAULT_CLEARANCE_MM;
        }
        plan.placements.push(Proposed {
            id: c.id,
            x,
            y,
            rail: rail_id,
        });
        x += w;
        row_h = row_h.max(h);
    }
    // Each rail runs through the middle of its row, behind its devices, which
    // are centred on it: the rail was created at the row's top edge.
    let heights: Vec<(u64, f64)> = candidates.iter().map(|c| (c.id, c.size.height)).collect();
    let height_of = |id: u64| heights.iter().find(|(i, _)| *i == id).map_or(0.0, |h| h.1);
    for rail in &mut plan.rails {
        let on: Vec<usize> = (0..plan.placements.len())
            .filter(|&i| plan.placements[i].rail == rail.id)
            .collect();
        let row_h = on
            .iter()
            .map(|&i| height_of(plan.placements[i].id))
            .fold(0.0, f64::max);
        for &i in &on {
            let h = height_of(plan.placements[i].id);
            plan.placements[i].y += (row_h - h) / 2.0;
        }
        rail.y += row_h / 2.0;
    }
    plan
}
