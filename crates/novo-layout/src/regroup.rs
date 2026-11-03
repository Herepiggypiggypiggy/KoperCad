//! Autogroup by wiring and by heat: regroups the devices that stand on
//! horizontal rails across the rows, so that devices that are wired to each
//! other stand close together, power and signal parts do not share a row,
//! and (heat variants) hot parts keep clear of the parts that mind heat.
//!
//! This is a heuristic search, not an optimum, and it says so:
//! - **Wiring** is judged by the straight-line (Manhattan) distance between
//!   the centres of two devices times the number of wires they share. That is
//!   a planning estimate of wire length, not a routed length: wires that run
//!   through ducts are longer. A net that touches many devices (PE, 0 V) is
//!   spread thin over its pairs and a net that touches more than
//!   [`MAX_NET_DEVICES`] devices is left out by the caller.
//! - **Heat** is judged by the coupling score of [`crate::heat`]: a planning
//!   kernel, not a temperature.
//! - **Rules stay in force.** The zone order of the rule set (top over middle
//!   over bottom, hot and sensitive parts included) is penalised hard, a row
//!   that mixes power and signal parts is penalised, and the result is
//!   checked with [`crate::check::check`]: a result with more rule
//!   violations than the layout has now is dropped.
//! - **It never makes the chosen score worse.** The search starts from the
//!   layout as it stands (and from a built grouping, if that is better) and
//!   only accepts a change that lowers the score. When nothing better is
//!   found, nothing moves and the notes say so.
//!
//! The result is a [`Plan`] like the other arrange tools: previewed, accepted
//! or cancelled, one undo step. Only devices on horizontal rails are moved;
//! the rest are obstacles. Rails and ducts are not changed.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::arrange::{
    GROUP_GAP_MM, RAIL_OVERHANG_MM, mv, need_between, rect_at, standing, start_of, titled, wanted,
};
use crate::heat::{self, HeatPlace};
use crate::plan::{Move, Plan, PlannedDuct};
use crate::rules::{Class, DUCT_HEIGHT_MM, RuleSet, Spec, Zone};
use crate::state::CabinetLayout;
use crate::{Placement, Rail, Rect};

/// A net that touches more devices than this (PE, 0 V, a supply bar) says
/// nothing about which two devices belong together; callers leave it out.
pub const MAX_NET_DEVICES: usize = 12;
/// Cost of a row that holds both power and signal parts, in mm of wire.
pub const MIXED_ROW_MM: f64 = 600.0;
/// Cost of one pair of devices that is out of the order of the zones.
pub const ZONE_ORDER_MM: f64 = 1500.0;
/// Room kept beside a heat source when the heat goal asks for it: the
/// largest of these that the row has room for.
pub const HEAT_PADS: [f64; 5] = [120.0, 90.0, 60.0, 30.0, 0.0];
/// The most layouts one run judges.
pub const MAX_TRIES: usize = 6000;
/// The score has to drop by at least this share to be worth a change.
const MIN_GAIN: f64 = 0.01;

/// What the regrouping is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goal {
    /// Short wire runs; power and signal parts on separate rows.
    Wiring,
    /// Hot parts spaced out and high, parts that mind heat low and clear.
    Heat,
    /// Both, weighed equally at the start.
    Both,
}

impl Goal {
    pub fn title(self) -> &'static str {
        match self {
            Goal::Wiring => "Autogroup by wiring",
            Goal::Heat => "Autogroup by heat",
            Goal::Both => "Autogroup by wiring and heat",
        }
    }

    fn wiring(self) -> bool {
        matches!(self, Goal::Wiring | Goal::Both)
    }

    fn heat(self) -> bool {
        matches!(self, Goal::Heat | Goal::Both)
    }
}

/// Wires shared by two devices: the weight is the number of nets they share
/// (a net of `k` devices counts `1 / (k - 1)` for each pair).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Link {
    pub a: u64,
    pub b: u64,
    pub weight: f64,
}

/// What the regrouping reads besides the layout.
pub struct Inputs<'a> {
    /// One per device of the enclosure (see `rules::project::specs`).
    pub specs: &'a [Spec],
    pub links: &'a [Link],
    pub rules: &'a RuleSet,
}

/// The numbers behind a result, for the preview and for tests.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    /// Weighted straight-line wire estimate between linked devices, mm.
    pub wire_before_mm: f64,
    pub wire_after_mm: f64,
    /// Heat coupling score (see [`crate::heat::coupling`]); lower is cooler.
    pub heat_before: f64,
    pub heat_after: f64,
    /// Rows that hold both power and signal parts.
    pub mixed_before: usize,
    pub mixed_after: usize,
    /// Devices that move.
    pub moved: usize,
    /// Layouts judged.
    pub tried: usize,
}

/// A row of the plate: a horizontal rail with the devices on it that the
/// run may not move.
struct Row<'a> {
    rail: &'a Rail,
    /// Where the first moving device starts.
    origin: f64,
    /// Devices that stay, by start along the rail.
    fixed: Vec<&'a Placement>,
}

type Order = Vec<Vec<u64>>;
type Rects = BTreeMap<u64, Rect>;

struct Costs {
    wire: f64,
    heat: f64,
    mixed: usize,
    zone: usize,
}

struct Ctx<'a> {
    layout: &'a CabinetLayout,
    goal: Goal,
    rows: Vec<Row<'a>>,
    placed: HashMap<u64, &'a Placement>,
    spec: HashMap<u64, &'a Spec>,
    class: HashMap<u64, Class>,
    zone: HashMap<u64, Zone>,
    /// Link weight of a pair, smaller id first.
    link: HashMap<(u64, u64), f64>,
    heat: HeatPlace,
    /// Weight of the heat score against millimetres of wire.
    lambda: f64,
    /// The specs of the devices on the plate, for the coupling score.
    plate_specs: Vec<Spec>,
    ducts: Vec<Rect>,
    sources: HashSet<u64>,
}

fn pair(a: u64, b: u64) -> (u64, u64) {
    (a.min(b), a.max(b))
}

impl<'a> Ctx<'a> {
    fn linked(&self, a: u64, b: u64) -> bool {
        self.link.contains_key(&pair(a, b))
    }

    fn width(&self, id: u64) -> f64 {
        let f = self.placed[&id].footprint();
        f.w
    }

    /// Packs the devices `order` onto row `r`, left to right, in the room
    /// the devices that stay leave; `None` when they do not fit.
    fn pack(&self, r: usize, order: &[u64], pad: f64) -> Option<Vec<(u64, f64)>> {
        let row = &self.rows[r];
        let run_end = row.rail.x + row.rail.length;
        let mut cursor = row.origin;
        let mut prev: Option<&Placement> = None;
        let mut out = Vec::with_capacity(order.len());
        for &id in order {
            let p = self.placed[&id];
            let w = self.width(id);
            let mut gap = 0.0;
            if let Some(q) = prev {
                gap = need_between(q, p, false);
                if self.goal.wiring() && !self.linked(q.id, p.id) {
                    gap = gap.max(GROUP_GAP_MM);
                }
                if pad > 0.0 && (self.sources.contains(&q.id) || self.sources.contains(&p.id)) {
                    gap = gap.max(pad);
                }
            }
            let mut pos = (cursor + gap).ceil();
            // Hop over the devices that stay.
            loop {
                let hit = row.fixed.iter().find(|f| {
                    let fr = f.footprint();
                    pos < fr.right() + need_between(f, p, false) - 0.01
                        && pos + w + need_between(p, f, false) > fr.x + 0.01
                });
                match hit {
                    Some(f) => {
                        pos = (f.footprint().right() + need_between(f, p, false)).ceil();
                    }
                    None => break,
                }
            }
            if pos + w > run_end + 0.01 {
                return None;
            }
            out.push((id, pos));
            cursor = pos + w;
            prev = Some(p);
        }
        Some(out)
    }

    /// The rectangles after the rows in `rows` are packed as `order` says,
    /// or `None` when one does not fit or a device would stand on another,
    /// on a duct or off the plate.
    fn realize(&self, order: &Order, rows: &[usize], base: &Rects) -> Option<Rects> {
        let mut now = base.clone();
        let mut touched = Vec::new();
        for &r in rows {
            // Heat: as much room around a heat source as the row has.
            let packed = if self.goal.heat() && !self.sources.is_empty() {
                HEAT_PADS
                    .iter()
                    .find_map(|&pad| self.pack(r, &order[r], pad))
            } else {
                self.pack(r, &order[r], 0.0)
            }?;
            for (id, pos) in packed {
                let p = self.placed[&id];
                let (x, y) = standing(p, self.rows[r].rail, pos);
                now.insert(id, rect_at(p, x, y));
                touched.push(id);
            }
        }
        let usable = self.layout.plate.usable();
        for &id in &touched {
            let rect = now[&id];
            if !usable.contains(&rect)
                || self.ducts.iter().any(|d| d.overlaps(&rect))
                || now
                    .iter()
                    .any(|(other, o)| *other != id && o.overlaps(&rect))
            {
                return None;
            }
        }
        Some(now)
    }

    fn costs(&self, rects: &Rects, order: &Order) -> Costs {
        let centre = |r: &Rect| (r.x + r.w / 2.0, r.y + r.h / 2.0);
        let mut wire = 0.0;
        for (&(a, b), &w) in &self.link {
            if let (Some(ra), Some(rb)) = (rects.get(&a), rects.get(&b)) {
                let (ca, cb) = (centre(ra), centre(rb));
                wire += w * ((ca.0 - cb.0).abs() + (ca.1 - cb.1).abs());
            }
        }
        let heat = if self.goal.heat() {
            let boxes: HashMap<u64, Rect> = rects.iter().map(|(k, v)| (*k, *v)).collect();
            heat::coupling(
                &self.plate_specs,
                &boxes,
                &self.heat,
                self.layout.plate.height,
            )
        } else {
            0.0
        };
        let mixed = self.mixed_rows(order);
        let mut zone = 0;
        let zoned: Vec<(Zone, f64)> = rects
            .iter()
            .filter_map(|(id, r)| Some((*self.zone.get(id)?, centre(r).1)))
            .collect();
        for (za, ya) in &zoned {
            for (zb, yb) in &zoned {
                if za < zb && *ya > yb + 1.0 {
                    zone += 1;
                }
            }
        }
        Costs {
            wire,
            heat,
            mixed,
            zone,
        }
    }

    /// Rows with both power and signal parts, the ones that stay included.
    fn mixed_rows(&self, order: &Order) -> usize {
        let mut count = 0;
        for (r, row) in self.rows.iter().enumerate() {
            let classes = order[r]
                .iter()
                .copied()
                .chain(row.fixed.iter().map(|p| p.id))
                .filter_map(|id| self.class.get(&id).copied());
            let (mut power, mut signal) = (false, false);
            for c in classes {
                match c {
                    Class::Power => power = true,
                    Class::Signal => signal = true,
                    Class::Mixed => {}
                }
            }
            if power && signal {
                count += 1;
            }
        }
        count
    }

    fn total(&self, c: &Costs) -> f64 {
        let (wire_w, heat_w) = match self.goal {
            Goal::Wiring => (1.0, 0.0),
            // Heat first, wiring only as a guard against a scattered result.
            Goal::Heat => (0.25, self.lambda),
            Goal::Both => (1.0, self.lambda),
        };
        let mixed = if self.goal.wiring() {
            c.mixed as f64 * MIXED_ROW_MM
        } else {
            0.0
        };
        wire_w * c.wire + heat_w * c.heat + mixed + c.zone as f64 * ZONE_ORDER_MM
    }
}

/// The groups the wiring suggests, as an order of devices on the rows: kinds
/// of wiring (zone, then power before signal) keep to their own rows, and
/// devices that share the most wires stand next to each other.
fn built(ctx: &Ctx<'_>, pool: &[u64]) -> Option<Order> {
    let class_rank = |id: u64| match ctx.class.get(&id).copied().unwrap_or(Class::Mixed) {
        Class::Power => 0u8,
        Class::Mixed => 1,
        Class::Signal => 2,
    };
    let bucket = |id: u64| {
        (
            ctx.zone.get(&id).copied().unwrap_or(Zone::Middle),
            if ctx.goal.wiring() { class_rank(id) } else { 0 },
        )
    };
    let widest = ctx
        .rows
        .iter()
        .map(|r| r.rail.length)
        .fold(0.0_f64, f64::max);
    // Clusters: strongest links first, inside one bucket, up to a row.
    let mut cluster_of: HashMap<u64, usize> =
        pool.iter().enumerate().map(|(i, &d)| (d, i)).collect();
    let mut members: Vec<Vec<u64>> = pool.iter().map(|&d| vec![d]).collect();
    let mut width: Vec<f64> = pool.iter().map(|&d| ctx.width(d)).collect();
    if ctx.goal.wiring() {
        let mut links: Vec<((u64, u64), f64)> = ctx
            .link
            .iter()
            .filter(|((a, b), _)| cluster_of.contains_key(a) && cluster_of.contains_key(b))
            .map(|(k, w)| (*k, *w))
            .collect();
        links.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(&y.0)));
        for ((a, b), _) in links {
            let (ca, cb) = (cluster_of[&a], cluster_of[&b]);
            if ca == cb || bucket(a) != bucket(b) || width[ca] + width[cb] > widest * 0.9 {
                continue;
            }
            let moved = std::mem::take(&mut members[cb]);
            width[ca] += width[cb];
            for d in &moved {
                cluster_of.insert(*d, ca);
            }
            members[ca].extend(moved);
        }
    }
    let mut clusters: Vec<Vec<u64>> = members.into_iter().filter(|m| !m.is_empty()).collect();
    // Inside a cluster: a chain that follows the links.
    for c in &mut clusters {
        c.sort_by(|a, b| {
            ctx.spec[a]
                .sort_key
                .cmp(&ctx.spec[b].sort_key)
                .then(a.cmp(b))
        });
        let mut left: Vec<u64> = std::mem::take(c);
        let mut chain = vec![left.remove(0)];
        while !left.is_empty() {
            let last = *chain.last().unwrap();
            let best = left
                .iter()
                .enumerate()
                .max_by(|(ia, a), (ib, b)| {
                    let wa = ctx.link.get(&pair(last, **a)).copied().unwrap_or(0.0);
                    let wb = ctx.link.get(&pair(last, **b)).copied().unwrap_or(0.0);
                    wa.total_cmp(&wb).then(ib.cmp(ia))
                })
                .map(|(i, _)| i)
                .unwrap();
            chain.push(left.remove(best));
        }
        *c = chain;
    }
    let loss = |c: &Vec<u64>| -> f64 { c.iter().filter_map(|d| ctx.spec[d].loss_w).sum::<f64>() };
    clusters.sort_by(|a, b| {
        bucket(a[0])
            .cmp(&bucket(b[0]))
            .then_with(|| {
                if ctx.goal.heat() {
                    // The hottest groups go first, so they stand highest.
                    loss(b).total_cmp(&loss(a))
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| ctx.spec[&a[0]].sort_key.cmp(&ctx.spec[&b[0]].sort_key))
            .then(a[0].cmp(&b[0]))
    });
    // Fill the rows from the top. A change of bucket starts a new row.
    let mut order: Order = vec![Vec::new(); ctx.rows.len()];
    let mut r = 0;
    let mut row_bucket: Option<(Zone, u8)> = None;
    for cluster in clusters {
        let b = bucket(cluster[0]);
        if row_bucket.is_some_and(|rb| rb != b) && !order[r].is_empty() {
            r += 1;
        }
        let whole = |order: &Order, r: usize| {
            let mut t = order[r].clone();
            t.extend(cluster.iter().copied());
            ctx.pack(r, &t, 0.0).is_some()
        };
        while r < ctx.rows.len() && !order[r].is_empty() && !whole(&order, r) {
            r += 1;
        }
        if r >= ctx.rows.len() {
            return None;
        }
        if whole(&order, r) {
            order[r].extend(cluster.iter().copied());
            row_bucket = Some(b);
            continue;
        }
        // Too long for one row: split it device by device.
        for d in cluster {
            loop {
                if r >= ctx.rows.len() {
                    return None;
                }
                let mut t = order[r].clone();
                t.push(d);
                if ctx.pack(r, &t, 0.0).is_some() {
                    order[r] = t;
                    break;
                }
                r += 1;
            }
        }
        row_bucket = Some(b);
    }
    Some(order)
}

/// Autogroup by wiring and/or heat across the rails. `ids` limits it to
/// those devices (empty: every device on a horizontal rail). Returns the
/// plan, whose notes carry the figures, and the figures themselves.
pub fn regroup(
    layout: &CabinetLayout,
    ids: &[u64],
    goal: Goal,
    input: &Inputs<'_>,
) -> (Plan, Report) {
    let mut plan = titled(goal.title());
    let mut report = Report::default();

    let spec: HashMap<u64, &Spec> = input.specs.iter().map(|s| (s.id, s)).collect();
    let heat = input.rules.heat.unwrap_or_default();
    let on_plate = layout.on_plate();
    let placed: HashMap<u64, &Placement> = layout
        .placements
        .iter()
        .filter(|p| !layout.off_plate.contains_key(&p.id))
        .map(|p| (p.id, p))
        .collect();

    // Rows: horizontal rails, top to bottom.
    let mut rails: Vec<&Rail> = layout.rails.iter().filter(|r| !r.vertical).collect();
    rails.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let rail_index: HashMap<u64, usize> =
        rails.iter().enumerate().map(|(i, r)| (r.id, i)).collect();

    // The devices to regroup and the ones that stay.
    let mut pool: Vec<u64> = Vec::new();
    let mut off_rail = 0usize;
    let mut nameless = 0usize;
    for p in &on_plate {
        if !wanted(ids, p.id) {
            continue;
        }
        match p.rail.and_then(|r| rail_index.get(&r)) {
            Some(_) if spec.contains_key(&p.id) => pool.push(p.id),
            Some(_) => nameless += 1,
            None => off_rail += 1,
        }
    }
    if off_rail > 0 {
        plan.notes.push(format!(
            "{off_rail} device(s) stand on no rail or on a vertical rail and were left; Autoplace \
             on rail puts them on one first."
        ));
    }
    if nameless > 0 {
        plan.notes.push(format!(
            "{nameless} device(s) have no part data and were left."
        ));
    }
    if pool.len() < 2 || rails.is_empty() {
        plan.notes
            .push("Fewer than two devices on rails: nothing to regroup.".into());
        return (plan, report);
    }
    let pool_set: HashSet<u64> = pool.iter().copied().collect();

    let mut rows: Vec<Row<'_>> = rails
        .iter()
        .map(|&rail| Row {
            rail,
            origin: rail.x + RAIL_OVERHANG_MM,
            fixed: Vec::new(),
        })
        .collect();
    let mut start_of_row: Vec<f64> = vec![f64::INFINITY; rows.len()];
    for p in &on_plate {
        let Some(&i) = p.rail.and_then(|r| rail_index.get(&r)) else {
            continue;
        };
        if pool_set.contains(&p.id) {
            let f = p.footprint();
            start_of_row[i] = start_of_row[i].min(start_of(&f, false));
        } else if let Some(pp) = placed.get(&p.id) {
            rows[i].fixed.push(pp);
        }
    }
    for (i, row) in rows.iter_mut().enumerate() {
        if start_of_row[i].is_finite() {
            row.origin = start_of_row[i].max(row.rail.x);
        }
        row.fixed
            .sort_by(|a, b| a.footprint().x.total_cmp(&b.footprint().x));
    }

    // Wiring data.
    let mut link = HashMap::new();
    for l in input.links {
        if l.a != l.b && placed.contains_key(&l.a) && placed.contains_key(&l.b) && l.weight > 0.0 {
            *link.entry(pair(l.a, l.b)).or_insert(0.0) += l.weight;
        }
    }
    let pool_linked = link
        .keys()
        .filter(|(a, b)| pool_set.contains(a) || pool_set.contains(b))
        .count();
    if goal.wiring() && pool_linked == 0 {
        plan.notes.push(
            "No wiring between these devices is known (draw the wires in the schematic first), \
             so there is nothing to group by."
                .into(),
        );
        if goal == Goal::Wiring {
            return (plan, report);
        }
    }

    // Kinds of wiring and zones, from the rules.
    let zone_override = heat::zone_override(input.rules, input.specs);
    let mut class = HashMap::new();
    let mut zone = HashMap::new();
    for s in input.specs {
        let (_, rule) = input.rules.rule_for_kind(&s.kind);
        class.insert(s.id, rule.class);
        zone.insert(s.id, zone_override.get(&s.id).copied().unwrap_or(rule.zone));
    }

    // Heat data.
    let sources: HashSet<u64> = input
        .specs
        .iter()
        .filter(|s| s.loss_w.is_some_and(|w| w >= heat.min_source_w))
        .map(|s| s.id)
        .collect();
    if goal.heat() && sources.is_empty() {
        plan.notes.push(format!(
            "No device states a power loss of {:.0} W or more, so there is no heat to arrange by.",
            heat.min_source_w
        ));
        if goal == Goal::Heat {
            return (plan, report);
        }
    }
    if goal.heat() && input.rules.heat.is_none() {
        plan.notes.push(
            "Heat is switched off in the rules; this run uses the default heat settings.".into(),
        );
    }

    let ducts: Vec<Rect> = layout.ducts.iter().map(|d| d.footprint()).collect();
    let plate_specs: Vec<Spec> = input
        .specs
        .iter()
        .filter(|s| placed.contains_key(&s.id))
        .cloned()
        .collect();

    let mut ctx = Ctx {
        layout,
        goal,
        rows,
        placed,
        spec,
        class,
        zone,
        link,
        heat,
        lambda: 1.0,
        plate_specs,
        ducts,
        sources,
    };

    // Where things stand now.
    let now: Rects = on_plate.iter().map(|p| (p.id, p.footprint())).collect();
    let mut current: Order = vec![Vec::new(); ctx.rows.len()];
    {
        let mut by_row: Vec<Vec<&Placement>> = vec![Vec::new(); ctx.rows.len()];
        for p in &on_plate {
            if pool_set.contains(&p.id) {
                by_row[rail_index[&p.rail.unwrap()]].push(p);
            }
        }
        for (i, list) in by_row.iter_mut().enumerate() {
            list.sort_by(|a, b| {
                a.footprint()
                    .x
                    .total_cmp(&b.footprint().x)
                    .then(a.id.cmp(&b.id))
            });
            current[i] = list.iter().map(|p| p.id).collect();
        }
    }
    let before = ctx.costs(&now, &current);
    report.wire_before_mm = before.wire;
    report.heat_before = before.heat;
    report.mixed_before = before.mixed;
    // Wire and heat count the same at the start.
    ctx.lambda = if before.heat > 0.0 && before.wire > 0.0 {
        before.wire / before.heat
    } else {
        1.0
    };
    let baseline = ctx.total(&before);

    // Start: the layout as it stands, packed, or the built grouping.
    let mut best_order = current.clone();
    let mut best_rects: Option<Rects> = None;
    let mut best_cost = f64::INFINITY;
    let all_rows: Vec<usize> = (0..ctx.rows.len()).collect();
    let mut tried = 0usize;
    let mut candidates: Vec<Order> = vec![current.clone()];
    if let Some(o) = built(&ctx, &pool) {
        candidates.push(o);
    }
    for o in candidates {
        tried += 1;
        if let Some(rects) = ctx.realize(&o, &all_rows, &now) {
            let c = ctx.total(&ctx.costs(&rects, &o));
            if c < best_cost {
                best_cost = c;
                best_order = o;
                best_rects = Some(rects);
            }
        }
    }
    let Some(mut best_rects_v) = best_rects.take() else {
        plan.notes.push(
            "The devices do not fit on the rails in a regrouped order; nothing was moved.".into(),
        );
        return (plan, report);
    };

    // Local search: swap two devices, or move one to another place in a row.
    let mut order = best_order;
    'passes: for _ in 0..8 {
        let mut improved = false;
        for &d in &pool {
            // Swaps with every later device.
            for &e in &pool {
                if e <= d {
                    continue;
                }
                if tried >= MAX_TRIES {
                    break 'passes;
                }
                let at = locate(&order, d).unwrap();
                let at_e = locate(&order, e).unwrap();
                let mut trial = order.clone();
                trial[at.0][at.1] = e;
                trial[at_e.0][at_e.1] = d;
                let rows: Vec<usize> = if at.0 == at_e.0 {
                    vec![at.0]
                } else {
                    vec![at.0, at_e.0]
                };
                tried += 1;
                if let Some(rects) = ctx.realize(&trial, &rows, &best_rects_v) {
                    let c = ctx.total(&ctx.costs(&rects, &trial));
                    if c < best_cost - 1e-6 {
                        best_cost = c;
                        order = trial;
                        best_rects_v = rects;
                        improved = true;
                        continue;
                    }
                }
            }
            // Moves to every other place. `order` changes when a move is
            // accepted, so `d` is looked up again for every row.
            for r in 0..ctx.rows.len() {
                let Some(at) = locate(&order, d) else {
                    break;
                };
                let len = order[r].len() - usize::from(r == at.0);
                for k in 0..=len {
                    if (r, k) == at {
                        continue;
                    }
                    if tried >= MAX_TRIES {
                        break 'passes;
                    }
                    let mut trial = order.clone();
                    trial[at.0].remove(at.1);
                    let k = k.min(trial[r].len());
                    trial[r].insert(k, d);
                    let rows: Vec<usize> = if at.0 == r { vec![r] } else { vec![at.0, r] };
                    tried += 1;
                    if let Some(rects) = ctx.realize(&trial, &rows, &best_rects_v) {
                        let c = ctx.total(&ctx.costs(&rects, &trial));
                        if c < best_cost - 1e-6 {
                            best_cost = c;
                            order = trial;
                            best_rects_v = rects;
                            improved = true;
                            // `d` stands elsewhere now: on to the next row.
                            break;
                        }
                    }
                }
            }
        }
        if !improved {
            break;
        }
    }
    report.tried = tried;

    let after = ctx.costs(&best_rects_v, &order);
    report.wire_after_mm = after.wire;
    report.heat_after = after.heat;
    report.mixed_after = after.mixed;

    if best_cost >= baseline * (1.0 - MIN_GAIN) {
        plan.notes.push(format!(
            "The devices already stand about as well as this search finds for {} ({tried} layouts \
             tried); nothing was moved.",
            match goal {
                Goal::Wiring => "wiring",
                Goal::Heat => "heat",
                Goal::Both => "wiring and heat",
            }
        ));
        return (plan, report);
    }

    // The result must not break more place rules than the layout does now.
    let ducts_planned: Vec<PlannedDuct> = layout
        .ducts
        .iter()
        .map(|d| PlannedDuct {
            x: d.x,
            y: d.y,
            length: d.length,
            vertical: d.vertical,
            width: d.width,
            height: DUCT_HEIGHT_MM,
            class: Class::Mixed,
        })
        .collect();
    let moved_to = |p: &Placement| -> Placement {
        let mut q = p.clone();
        if let Some(r) = best_rects_v.get(&p.id) {
            q.x = r.x;
            q.y = r.y;
        }
        q
    };
    let after_layout: Vec<Placement> = on_plate.iter().map(moved_to).collect();
    let rules_before = crate::check::check(&on_plate, &ducts_planned, input.specs, input.rules);
    let rules_after = crate::check::check(&after_layout, &ducts_planned, input.specs, input.rules);
    if rules_after.len() > rules_before.len() {
        plan.notes.push(format!(
            "The regrouped layout would break {} more place rule(s) than now, so nothing was moved.",
            rules_after.len() - rules_before.len()
        ));
        return (plan, report);
    }

    // The moves.
    let mut moves: Vec<Move> = Vec::new();
    for (r, ids) in order.iter().enumerate() {
        for id in ids {
            let p = ctx.placed[id];
            let rect = best_rects_v[id];
            let f = p.footprint();
            let rail = ctx.rows[r].rail;
            if (rect.x - f.x).abs() > 0.01 || (rect.y - f.y).abs() > 0.01 || p.rail != Some(rail.id)
            {
                moves.push(mv(p, rail, false, rect.x, rect.y));
            }
        }
    }
    report.moved = moves.len();
    plan.moves = moves;

    if goal.wiring() && pool_linked > 0 {
        plan.notes.push(format!(
            "Wiring: estimated wire between connected devices {} to {} (straight line, not routed); \
             rows mixing power and signal {} to {}.",
            metres(report.wire_before_mm),
            metres(report.wire_after_mm),
            report.mixed_before,
            report.mixed_after
        ));
    }
    if goal.heat() && report.heat_before > 0.0 {
        let change = 100.0 * (1.0 - report.heat_after / report.heat_before);
        plan.notes.push(if change >= 1.0 {
            format!("Heat: coupling score {change:.0}% lower (planning score, not a temperature).")
        } else if change > -1.0 {
            "Heat: coupling score about the same (planning score, not a temperature).".to_owned()
        } else {
            format!(
                "Heat: coupling score {:.0}% higher, traded for the other goal (planning score, \
                 not a temperature).",
                -change
            )
        });
    }
    (plan, report)
}

fn metres(mm_weighted: f64) -> String {
    format!("{:.1} m", mm_weighted / 1000.0)
}

fn locate(order: &Order, id: u64) -> Option<(usize, usize)> {
    order
        .iter()
        .enumerate()
        .find_map(|(r, row)| row.iter().position(|&d| d == id).map(|k| (r, k)))
}
