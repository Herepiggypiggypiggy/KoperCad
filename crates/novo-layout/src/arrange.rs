//! Arrange tools for devices that are already on the plate (or about to be):
//! autodistance, autoplace on a rail, automatic rails, automatic ducts and
//! autogroup. Each one reads a [`CabinetLayout`] and returns a [`Plan`] that
//! the Cabinet previews like a Place all plan: nothing is stored until it is
//! accepted, and accepting is one undo step.
//!
//! These are plain rules, not a search. A tool moves only what it was asked
//! to move (the selection, or everything when nothing is selected), never
//! moves a device onto another one, a duct or off the plate, and says in
//! [`Plan::notes`] what it left alone and why. Positions are whole
//! millimetres and gaps are rounded up, so a gap never ends up under the
//! figure the part asks for. They know nothing about heat, wiring or
//! terminals; the heat-aware Place all does.
//!
//! The rail passes through the middle of the devices on it (see
//! [`crate::Rail`]), so a device on a horizontal rail has its vertical centre
//! on the rail's line.

use std::collections::BTreeMap;

use crate::plan::{Candidate, Move, Plan, PlannedDuct};
use crate::rules::{Class, kind_of};
use crate::{DEFAULT_DUCT_MM, Placement, RAIL_BAND_MM, Rail, Rect, Side};

/// Space kept between two groups of different kinds on a rail, if the parts
/// ask for less.
pub const GROUP_GAP_MM: f64 = 20.0;
/// A rail made by [`make_rails`] reaches this far past its outer devices.
pub const RAIL_OVERHANG_MM: f64 = 10.0;
/// The shortest rail [`make_rails`] makes.
pub const MIN_RAIL_MM: f64 = 50.0;
/// Free space kept between a duct and the devices of the rows beside it.
pub const DUCT_SIDE_MM: f64 = 10.0;

/// How [`distribute`] spaces the devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spacing {
    /// Close up to the smallest gap each pair asks for, from the first one.
    Tight,
    /// Equal gaps between the first and the last, which stay where they are.
    Even,
}

fn start_of(r: &Rect, vertical: bool) -> f64 {
    if vertical { r.y } else { r.x }
}

fn end_of(r: &Rect, vertical: bool) -> f64 {
    if vertical { r.bottom() } else { r.right() }
}

/// The gap `a` (before) and `b` (after) ask for along a run.
fn need_between(a: &Placement, b: &Placement, vertical: bool) -> f64 {
    if vertical {
        a.need(Side::Below).max(b.need(Side::Above))
    } else {
        a.need(Side::Right).max(b.need(Side::Left))
    }
}

/// Top left of `p` moved along its rail to `at`, keeping where it stands
/// across the rail (an older plan may have its rails off the middle of the
/// devices; Centre rails does that, not these tools).
fn slid(p: &Placement, rail: &Rail, at: f64) -> (f64, f64) {
    if rail.vertical { (p.x, at) } else { (at, p.y) }
}

/// Top left of `p` standing on `rail` with its start `at` along the run.
fn standing(p: &Placement, rail: &Rail, at: f64) -> (f64, f64) {
    let f = p.footprint();
    if rail.vertical {
        (rail.x - f.w / 2.0, at)
    } else {
        (at, rail.y - f.h / 2.0)
    }
}

fn rect_at(p: &Placement, x: f64, y: f64) -> Rect {
    let f = p.footprint();
    Rect {
        x,
        y,
        w: f.w,
        h: f.h,
    }
}

/// The devices of the layout on the mounting plate, with the plan's moves
/// applied: what a tool checks its next move against.
struct World<'a> {
    layout: &'a CabinetLayout,
    now: BTreeMap<u64, Rect>,
}

use crate::state::CabinetLayout;

impl<'a> World<'a> {
    fn new(layout: &'a CabinetLayout) -> Self {
        World {
            layout,
            now: layout
                .on_plate()
                .iter()
                .map(|p| (p.id, p.footprint()))
                .collect(),
        }
    }

    /// True when `rect` stands on the plate, clear of every other device and
    /// of the ducts. `me` is left out of the comparison.
    fn free(&self, me: u64, rect: &Rect) -> bool {
        // Where the device stands in the stored layout: a duct it already
        // overlaps there is not a reason to refuse the move (an older plan
        // may have one), and a device that does not move is always fine.
        let old = self
            .layout
            .placements
            .iter()
            .find(|p| p.id == me)
            .map(|p| p.footprint());
        if old.is_some_and(|o| (o.x - rect.x).abs() < 0.01 && (o.y - rect.y).abs() < 0.01) {
            return true;
        }
        let usable = self.layout.plate.usable();
        let was = |r: &Rect| old.is_some_and(|o| o.overlaps(r));
        (usable.contains(rect) || old.is_some_and(|o| !usable.contains(&o)))
            && !self
                .now
                .iter()
                .any(|(id, other)| *id != me && other.overlaps(rect) && !was(other))
            && !self
                .layout
                .ducts
                .iter()
                .any(|d| d.footprint().overlaps(rect) && !was(&d.footprint()))
    }

    fn set(&mut self, id: u64, rect: Rect) {
        self.now.insert(id, rect);
    }
}

fn wanted(ids: &[u64], id: u64) -> bool {
    ids.is_empty() || ids.contains(&id)
}

fn titled(title: &str) -> Plan {
    Plan {
        title: Some(title.to_owned()),
        ..Plan::default()
    }
}

fn mv(p: &Placement, rail: &Rail, new_rail: bool, x: f64, y: f64) -> Move {
    let offset = if rail.vertical {
        y - rail.y
    } else {
        x - rail.x
    };
    Move {
        id: p.id,
        x,
        y,
        rotated: p.rotated,
        rail: rail.id,
        new_rail,
        offset,
    }
}

/// The devices on each rail that a tool acts on, in order along the rail.
fn on_rails<'a>(layout: &'a CabinetLayout, ids: &[u64]) -> Vec<(&'a Rail, Vec<&'a Placement>)> {
    let mut out = Vec::new();
    for rail in &layout.rails {
        let mut on: Vec<&Placement> = layout
            .placements
            .iter()
            .filter(|p| p.rail == Some(rail.id) && !layout.off_plate.contains_key(&p.id))
            .filter(|p| wanted(ids, p.id))
            .collect();
        on.sort_by(|a, b| {
            start_of(&a.footprint(), rail.vertical)
                .total_cmp(&start_of(&b.footprint(), rail.vertical))
                .then(a.id.cmp(&b.id))
        });
        if !on.is_empty() {
            out.push((rail, on));
        }
    }
    out
}

/// Autodistance: spaces the devices on each rail. `ids` limits it to those
/// devices (empty: every device on a rail). A rail whose devices would run
/// off its end, or into something else, is left alone and named in the notes.
pub fn distribute(layout: &CabinetLayout, ids: &[u64], spacing: Spacing) -> Plan {
    let mut plan = titled(match spacing {
        Spacing::Tight => "Autodistance: close up",
        Spacing::Even => "Autodistance: even gaps",
    });
    let mut world = World::new(layout);
    for (rail, on) in on_rails(layout, ids) {
        if on.len() < 2 {
            continue;
        }
        let v = rail.vertical;
        let first = start_of(&on[0].footprint(), v);
        let mut at = Vec::with_capacity(on.len());
        match spacing {
            Spacing::Tight => {
                let mut pos = first;
                for (i, p) in on.iter().enumerate() {
                    if i > 0 {
                        pos = (pos + need_between(on[i - 1], p, v)).ceil();
                    }
                    at.push(pos);
                    pos += end_of(&p.footprint(), v) - start_of(&p.footprint(), v);
                }
            }
            Spacing::Even => {
                if on.len() < 3 {
                    continue;
                }
                let last = on[on.len() - 1].footprint();
                let widths: f64 = on[..on.len() - 1]
                    .iter()
                    .map(|p| end_of(&p.footprint(), v) - start_of(&p.footprint(), v))
                    .sum();
                let gaps = (on.len() - 1) as f64;
                let gap = ((start_of(&last, v) - first - widths) / gaps).floor();
                let wanted_gap = (1..on.len())
                    .map(|i| need_between(on[i - 1], on[i], v))
                    .fold(0.0, f64::max);
                if gap < wanted_gap {
                    plan.notes.push(format!(
                        "Rail row of {} devices: the room between the first and last is {:.0} mm \
                         for {:.0} mm of gaps; close up instead.",
                        on.len(),
                        start_of(&last, v) - first - widths,
                        wanted_gap * gaps
                    ));
                    continue;
                }
                let mut pos = first;
                for (i, p) in on.iter().enumerate() {
                    if i == on.len() - 1 {
                        at.push(start_of(&last, v));
                    } else {
                        at.push(pos);
                        pos += end_of(&p.footprint(), v) - start_of(&p.footprint(), v) + gap;
                    }
                }
            }
        }
        // Check the whole rail before storing any of it.
        let run_end = if v { rail.y } else { rail.x } + rail.length;
        let mut trial = World {
            layout,
            now: world.now.clone(),
        };
        // The devices of this rail are about to stand somewhere new.
        for p in &on {
            trial.now.remove(&p.id);
        }
        let mut moves = Vec::new();
        let mut ok = true;
        for (p, &pos) in on.iter().zip(&at) {
            let (x, y) = slid(p, rail, pos);
            let r = rect_at(p, x, y);
            if end_of(&r, v) > run_end + 0.01 || !trial.free(p.id, &r) {
                ok = false;
                break;
            }
            trial.set(p.id, r);
            let f = p.footprint();
            if (x - f.x).abs() > 0.01 || (y - f.y).abs() > 0.01 {
                moves.push(mv(p, rail, false, x, y));
            }
        }
        if !ok {
            plan.notes.push(format!(
                "{} devices on one rail were not moved: they would run past the end of the rail \
                 or into another device.",
                on.len()
            ));
            continue;
        }
        world = trial;
        plan.moves.extend(moves);
    }
    plan
}

/// Autogroup: orders the devices on each rail by their sort key (letter
/// code, then number) so that devices of one kind stand together, and keeps
/// [`GROUP_GAP_MM`] (or more, if the parts ask) between two kinds. `keys`
/// gives the sort key of each device; a device with none keeps its place at
/// the end of its rail.
pub fn group(layout: &CabinetLayout, ids: &[u64], keys: &BTreeMap<u64, String>) -> Plan {
    let mut plan = titled("Autogroup");
    let mut world = World::new(layout);
    for (rail, on) in on_rails(layout, ids) {
        if on.len() < 2 {
            continue;
        }
        let v = rail.vertical;
        let first = start_of(&on[0].footprint(), v);
        let mut order: Vec<&Placement> = on.clone();
        order.sort_by(|a, b| {
            let (ka, kb) = (keys.get(&a.id), keys.get(&b.id));
            // Without a key the old order stands, behind the keyed devices.
            ka.is_none()
                .cmp(&kb.is_none())
                .then_with(|| ka.cmp(&kb))
                .then_with(|| start_of(&a.footprint(), v).total_cmp(&start_of(&b.footprint(), v)))
        });
        let kind = |p: &Placement| keys.get(&p.id).map(|k| kind_of(k)).unwrap_or_default();
        let mut pos = first;
        let mut trial = World {
            layout,
            now: world.now.clone(),
        };
        // The devices of this rail are about to stand somewhere new.
        for p in &on {
            trial.now.remove(&p.id);
        }
        let mut moves = Vec::new();
        let mut ok = true;
        let run_end = if v { rail.y } else { rail.x } + rail.length;
        for (i, p) in order.iter().enumerate() {
            if i > 0 {
                let mut gap = need_between(order[i - 1], p, v);
                if kind(order[i - 1]) != kind(p) {
                    gap = gap.max(GROUP_GAP_MM);
                }
                pos = (pos + gap).ceil();
            }
            let (x, y) = slid(p, rail, pos);
            let r = rect_at(p, x, y);
            if end_of(&r, v) > run_end + 0.01 || !trial.free(p.id, &r) {
                ok = false;
                break;
            }
            trial.set(p.id, r);
            let f = p.footprint();
            if (x - f.x).abs() > 0.01 || (y - f.y).abs() > 0.01 {
                moves.push(mv(p, rail, false, x, y));
            }
            pos += end_of(&f, v) - start_of(&f, v);
        }
        if ok {
            world = trial;
            plan.moves.extend(moves);
        } else {
            plan.notes.push(format!(
                "{} devices on one rail were not regrouped: with the group gaps they would not \
                 fit on the rail.",
                on.len()
            ));
        }
    }
    plan
}

/// Autoplace on rail: puts each device on the nearest rail that has room,
/// at the free place on it closest to where the device is now. `ids` are the
/// devices to place (empty: every placed device that is on no rail);
/// `unplaced` are devices that are not on the plate yet, which may also be
/// asked for by id. A device that fits on no rail stays where it is and is
/// named in the notes.
pub fn onto_rails(layout: &CabinetLayout, ids: &[u64], unplaced: &[Candidate]) -> Plan {
    let mut plan = titled("Autoplace on rail");
    if layout.rails.is_empty() {
        plan.notes
            .push("There is no rail on the plate yet. Use Auto rails first.".into());
        return plan;
    }
    let mut world = World::new(layout);
    // The devices to place, with their size and where they are now.
    let mut todo: Vec<(Placement, (f64, f64))> = Vec::new();
    for p in layout.on_plate() {
        if wanted(ids, p.id) && (p.rail.is_none() || !ids.is_empty()) {
            let f = p.footprint();
            todo.push((p.clone(), (f.x + f.w / 2.0, f.y + f.h / 2.0)));
        }
    }
    for c in unplaced {
        if ids.contains(&c.id) && !layout.placements.iter().any(|p| p.id == c.id) {
            let p = Placement {
                id: c.id,
                x: 0.0,
                y: 0.0,
                rotated: false,
                size: c.size,
                clearance: None,
                margins: crate::Margins::default(),
                rail: None,
            };
            let u = layout.plate.usable();
            todo.push((p, (u.x, u.y)));
        }
    }
    todo.sort_by(|a, b| {
        a.1.1
            .total_cmp(&b.1.1)
            .then(a.1.0.total_cmp(&b.1.0))
            .then(a.0.id.cmp(&b.0.id))
    });
    let mut on_rail: BTreeMap<u64, Vec<Placement>> = BTreeMap::new();
    for p in &layout.placements {
        if let Some(r) = p.rail
            && !layout.off_plate.contains_key(&p.id)
            && !todo.iter().any(|(t, _)| t.id == p.id)
        {
            on_rail.entry(r).or_default().push(p.clone());
        }
    }
    for (p, (cx, cy)) in todo {
        let mut best: Option<(f64, &Rail, f64)> = None;
        for rail in &layout.rails {
            let v = rail.vertical;
            let (along, across) = if v { (cy, cx) } else { (cx, cy) };
            let line = if v { rail.x } else { rail.y };
            let distance = (line - across).abs();
            let width = {
                let f = p.footprint();
                end_of(&f, v) - start_of(&f, v)
            };
            let desired = along - width / 2.0;
            let others = on_rail.get(&rail.id).map(Vec::as_slice).unwrap_or(&[]);
            if let Some(at) = free_slot(&world, rail, &p, others, desired) {
                let cost = distance + (at - desired).abs() * 0.25;
                if best.is_none_or(|b| cost < b.0) {
                    best = Some((cost, rail, at));
                }
            }
        }
        match best {
            Some((_, rail, at)) => {
                let (x, y) = standing(&p, rail, at);
                world.set(p.id, rect_at(&p, x, y));
                let mut placed = p.clone();
                placed.x = x;
                placed.y = y;
                on_rail.entry(rail.id).or_default().push(placed);
                plan.moves.push(mv(&p, rail, false, x, y));
            }
            None => {
                plan.unplaced.push(p.id);
                plan.notes
                    .push("A device does not fit on any rail with its gaps.".into());
            }
        }
    }
    plan.notes.dedup();
    plan
}

/// The start along `rail` closest to `desired` where `p` fits between the
/// devices already on it, keeping their gaps, and stands clear of
/// everything else.
fn free_slot(
    world: &World<'_>,
    rail: &Rail,
    p: &Placement,
    others: &[Placement],
    desired: f64,
) -> Option<f64> {
    let v = rail.vertical;
    let f = p.footprint();
    let width = end_of(&f, v) - start_of(&f, v);
    let (run_start, run_end) = if v {
        (rail.y, rail.y + rail.length)
    } else {
        (rail.x, rail.x + rail.length)
    };
    let mut candidates = vec![run_start, run_end - width, desired];
    for o in others {
        let of = o.footprint();
        candidates.push((end_of(&of, v) + need_between(o, p, v)).ceil());
        candidates.push((start_of(&of, v) - need_between(p, o, v) - width).floor());
    }
    candidates
        .into_iter()
        .filter(|&c| c >= run_start - 0.01 && c + width <= run_end + 0.01)
        .filter(|&c| {
            others.iter().all(|o| {
                let of = o.footprint();
                c >= end_of(&of, v) + need_between(o, p, v) - 0.01
                    || c + width + need_between(p, o, v) <= start_of(&of, v) + 0.01
            })
        })
        .filter(|&c| {
            let (x, y) = standing(p, rail, c);
            world.free(p.id, &rect_at(p, x, y))
        })
        .min_by(|a, b| (a - desired).abs().total_cmp(&(b - desired).abs()))
}

/// Auto rails: gives devices that stand on no rail one, a horizontal rail
/// per row of devices. Rows are found by the vertical centres of the
/// devices; the rail runs through the middle of the row, reaches
/// [`RAIL_OVERHANG_MM`] past the outer devices (cut to the whole 5 mm, as a
/// hacksaw cut is) and stays on the plate. The devices of a row are lined up
/// on it. A row that would put its rail over an existing rail, a duct or a
/// device that is not part of the row is left alone and named in the notes.
pub fn make_rails(layout: &CabinetLayout, ids: &[u64]) -> Plan {
    let mut plan = titled("Auto rails");
    let mut world = World::new(layout);
    let mut loose: Vec<&Placement> = layout
        .placements
        .iter()
        .filter(|p| p.rail.is_none() && !layout.off_plate.contains_key(&p.id) && wanted(ids, p.id))
        .collect();
    loose.sort_by(|a, b| {
        let (fa, fb) = (a.footprint(), b.footprint());
        (fa.y + fa.h / 2.0)
            .total_cmp(&(fb.y + fb.h / 2.0))
            .then(fa.x.total_cmp(&fb.x))
    });
    // Rows: a device joins the row when its centre lies within half the
    // smaller height of the row's centre.
    let mut rows: Vec<Vec<&Placement>> = Vec::new();
    for p in loose {
        let f = p.footprint();
        let cy = f.y + f.h / 2.0;
        match rows.last_mut() {
            Some(row)
                if {
                    let centre = row
                        .iter()
                        .map(|q| {
                            let g = q.footprint();
                            g.y + g.h / 2.0
                        })
                        .sum::<f64>()
                        / row.len() as f64;
                    let smallest = row.iter().map(|q| q.footprint().h).fold(f.h, f64::min);
                    (cy - centre).abs() <= smallest / 2.0
                } =>
            {
                row.push(p)
            }
            _ => rows.push(vec![p]),
        }
    }
    let usable = layout.plate.usable();
    let mut next_id = layout
        .rails
        .iter()
        .map(|r| r.id)
        .chain(layout.ducts.iter().map(|d| d.id))
        .max()
        .unwrap_or(0)
        + 1;
    let mut made: Vec<Rail> = Vec::new();
    'rows: for row in rows {
        let line = (row
            .iter()
            .map(|p| {
                let f = p.footprint();
                f.y + f.h / 2.0
            })
            .sum::<f64>()
            / row.len() as f64)
            .round();
        let from = row
            .iter()
            .map(|p| p.footprint().x)
            .fold(f64::INFINITY, f64::min);
        let to = row
            .iter()
            .map(|p| p.footprint().right())
            .fold(f64::NEG_INFINITY, f64::max);
        let start = (from - RAIL_OVERHANG_MM).max(usable.x).floor();
        let mut end = (to + RAIL_OVERHANG_MM).min(usable.right());
        end = (start + ((end - start) / 5.0).ceil() * 5.0)
            .max(start + MIN_RAIL_MM)
            .min(usable.right());
        let rail = Rail {
            id: next_id,
            x: start,
            y: line,
            length: end - start,
            vertical: false,
        };
        let band = Rect {
            x: rail.x,
            y: rail.y - RAIL_BAND_MM / 2.0,
            w: rail.length,
            h: RAIL_BAND_MM,
        };
        let band_taken = layout
            .rails
            .iter()
            .chain(made.iter())
            .any(|r| rail_band(r).overlaps(&band))
            || layout.ducts.iter().any(|d| d.footprint().overlaps(&band));
        if band_taken {
            plan.notes.push(format!(
                "A row of {} devices at y = {line:.0} mm lies over a rail or duct; use Autoplace \
                 on rail to put them on it.",
                row.len()
            ));
            continue;
        }
        // Line the devices up on the rail, then check them.
        let mut trial = World {
            layout,
            now: world.now.clone(),
        };
        let mut moves = Vec::new();
        for p in &row {
            let f = p.footprint();
            let (x, y) = standing(p, &rail, f.x);
            let r = rect_at(p, x, y);
            if !trial.free(p.id, &r) || x < rail.x - 0.01 || r.right() > rail.x + rail.length {
                plan.notes.push(format!(
                    "A row at y = {line:.0} mm was left alone: lining it up on a rail would put a \
                     device into another one or off the plate."
                ));
                continue 'rows;
            }
            trial.set(p.id, r);
            moves.push(mv(p, &rail, true, x, y));
        }
        world = trial;
        next_id += 1;
        plan.moves.extend(moves);
        made.push(rail);
    }
    plan.rails = made;
    plan
}

fn rail_band(r: &Rail) -> Rect {
    if r.vertical {
        Rect {
            x: r.x - RAIL_BAND_MM / 2.0,
            y: r.y,
            w: RAIL_BAND_MM,
            h: r.length,
        }
    } else {
        Rect {
            x: r.x,
            y: r.y - RAIL_BAND_MM / 2.0,
            w: r.length,
            h: RAIL_BAND_MM,
        }
    }
}

/// Auto ducts: puts a wiring duct in the free band between each pair of
/// neighbouring horizontal rails, centred between the devices of the upper
/// row and the lower one, as long as the two rows overlap along the plate
/// and the band is wide enough for the duct and [`DUCT_SIDE_MM`] on both
/// sides. A band that has a duct already, or is too narrow, is named in the
/// notes. The duct is of class "mixed"; the Rails window sets the class.
pub fn add_ducts(layout: &CabinetLayout) -> Plan {
    let mut plan = titled("Auto ducts");
    let mut rails: Vec<&Rail> = layout.rails.iter().filter(|r| !r.vertical).collect();
    rails.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let extent = |rail: &Rail, upper: bool| -> f64 {
        let own = layout
            .placements
            .iter()
            .filter(|p| p.rail == Some(rail.id))
            .map(|p| {
                let f = p.footprint();
                if upper { f.bottom() } else { f.y }
            });
        let band = if upper {
            rail.y + RAIL_BAND_MM / 2.0
        } else {
            rail.y - RAIL_BAND_MM / 2.0
        };
        own.chain(std::iter::once(band))
            .fold(band, |a, b| if upper { a.max(b) } else { a.min(b) })
    };
    for pair in rails.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let x0 = a.x.min(b.x);
        let x1 = (a.x + a.length).max(b.x + b.length);
        let overlap = (a.x + a.length).min(b.x + b.length) - a.x.max(b.x);
        if overlap <= 0.0 || (b.y - a.y) < 1.0 {
            continue;
        }
        let top = extent(a, true);
        let bottom = extent(b, false);
        let free = bottom - top;
        let band = Rect {
            x: x0,
            y: top,
            w: x1 - x0,
            h: free.max(0.0),
        };
        if layout.ducts.iter().any(|d| d.footprint().overlaps(&band))
            || plan.ducts.iter().any(|d| duct_rect(d).overlaps(&band))
        {
            plan.notes.push(format!(
                "Between the rails at y = {:.0} and {:.0} mm there is a duct already.",
                a.y, b.y
            ));
            continue;
        }
        let need = DEFAULT_DUCT_MM + 2.0 * DUCT_SIDE_MM;
        if free < need {
            plan.notes.push(format!(
                "Between the rails at y = {:.0} and {:.0} mm there are {:.0} mm free; a duct \
                 needs {need:.0} mm.",
                a.y,
                b.y,
                free.max(0.0)
            ));
            continue;
        }
        let duct = PlannedDuct {
            x: x0,
            y: ((top + bottom) / 2.0).round(),
            length: x1 - x0,
            vertical: false,
            width: DEFAULT_DUCT_MM,
            height: 60.0,
            class: Class::Mixed,
        };
        if !layout.plate.usable().contains(&duct_rect(&duct)) {
            plan.notes.push(format!(
                "A duct between the rails at y = {:.0} and {:.0} mm would leave the plate.",
                a.y, b.y
            ));
            continue;
        }
        plan.ducts.push(duct);
    }
    plan
}

fn duct_rect(d: &PlannedDuct) -> Rect {
    if d.vertical {
        Rect {
            x: d.x - d.width / 2.0,
            y: d.y,
            w: d.width,
            h: d.length,
        }
    } else {
        Rect {
            x: d.x,
            y: d.y - d.width / 2.0,
            w: d.length,
            h: d.width,
        }
    }
}
