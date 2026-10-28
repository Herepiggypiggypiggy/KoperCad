//! The duct graph and shortest-path search.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use crate::{Anchor, Duct, Facing, Job, Kind, Point, Point3, Route, Rules, polyline_length};

const EPS: f64 = 1e-6;
/// How many nearest ducts per anchor are tried as the entry.
const CANDIDATES: usize = 3;
/// How far a wire leaves a terminal that faces away from its duct before it
/// turns round, mm. A terminal on top of a device with the duct below it
/// does not send its wire all the way up and back: it leaves a short way and
/// runs down beside the device. The detour is twice this (up and back), not
/// twice the whole distance to the duct.
const TURN_MM: f64 = 40.0;
/// Added to the rank of an entry a wire reaches by running sideways along
/// its device row (a terminal on the top or bottom edge going to a vertical
/// duct, or on a side edge going to a horizontal one): that stub passes the
/// neighbouring devices, so a duct on the terminal's own side is preferred
/// unless it is much farther.
const SIDEWAYS_MM: f64 = 150.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orient {
    H,
    V,
}

/// One duct as a run along an axis: `fixed` is y for H and x for V, the run
/// spans `lo..=hi` along the other axis.
#[derive(Debug, Clone)]
struct Run {
    id: u64,
    orient: Orient,
    fixed: f64,
    lo: f64,
    hi: f64,
    /// Cross-section, mm squared.
    area: f64,
    /// Width across the run, mm (0 when unknown): the clearance is measured
    /// between duct edges.
    width: f64,
    /// Height above the plate the wires lie at: half the duct height, 0
    /// while the duct has no cross-section.
    mid_z: f64,
}

impl Run {
    /// The duct as a rectangle `(x0, y0, x1, y1)`: its centre line widened by
    /// half the width on each side.
    fn rect(&self) -> (f64, f64, f64, f64) {
        let h = self.width / 2.0;
        match self.orient {
            Orient::H => (self.lo, self.fixed - h, self.hi, self.fixed + h),
            Orient::V => (self.fixed - h, self.lo, self.fixed + h, self.hi),
        }
    }

    /// Distance between the edges of two ducts, 0 when they overlap.
    fn gap_to(&self, other: &Run) -> f64 {
        let (a, b) = (self.rect(), other.rect());
        let dx = (a.0 - b.2).max(b.0 - a.2).max(0.0);
        let dy = (a.1 - b.3).max(b.1 - a.3).max(0.0);
        dx.hypot(dy)
    }

    fn at(&self, t: f64) -> Point {
        match self.orient {
            Orient::H => Point::new(t, self.fixed),
            Orient::V => Point::new(self.fixed, t),
        }
    }
}

/// The ducts of one cabinet, ready for routing.
#[derive(Debug, Clone)]
pub struct Network {
    runs: Vec<Run>,
    /// Per run, the positions along it where it is joined to another duct.
    splits: Vec<Vec<f64>>,
    /// Ids of ducts that were not horizontal, vertical, finite, had no length,
    /// or repeated an earlier duct's id.
    pub ignored: Vec<u64>,
}

/// How full the ducts already are while a wire is being routed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Load<'a> {
    /// Ducts this job should stay out of if it can (ids): they cost
    /// [`AVOID_COST`] times as much, so it still gets through when they are
    /// the only way.
    pub avoid: &'a [u64],
    /// Ducts this job must not use at all (ids), for a hard clearance.
    pub forbid: &'a [u64],
    /// Used cross-section per run, mm squared.
    pub used: &'a [f64],
    /// Cross-section of the wire being routed.
    pub wire_area: f64,
}

/// Fill ratio (of the allowed share of the cross-section) above which
/// a duct starts to cost extra.
const PENALTY_FROM: f64 = 0.8;
/// Longest a duct's cost can be stretched, so a full duct stays routable.
const PENALTY_CAP: f64 = 20.0;
/// Cost multiplier of a duct the wire is asked to keep out of.
const AVOID_COST: f64 = 12.0;

/// Points along the ducts, each with the run it lies on.
type Path = Vec<(Point, usize)>;

/// An entry point of an anchor on a run.
#[derive(Debug, Clone, Copy)]
struct Entry {
    run: usize,
    t: f64,
    /// Stub length plus the detour for a terminal facing away.
    stub: f64,
    detour: f64,
    /// What the entry is ranked by: the stub plus [`SIDEWAYS_MM`] when the
    /// wire has to run along its device row to reach a duct that stands
    /// across its direction. Only steers the choice; lengths use `stub`.
    rank: f64,
}

impl Network {
    pub fn new(ducts: &[Duct]) -> Network {
        let mut ignored = Vec::new();
        let mut runs = Vec::new();
        for d in ducts {
            let (dx, dy) = ((d.a.x - d.b.x).abs(), (d.a.y - d.b.y).abs());
            let finite = [d.a.x, d.a.y, d.b.x, d.b.y].iter().all(|v| v.is_finite());
            if !finite || (dx < EPS && dy < EPS) {
                ignored.push(d.id);
            } else if dy < EPS {
                runs.push(Run {
                    id: d.id,
                    orient: Orient::H,
                    fixed: d.a.y,
                    lo: d.a.x.min(d.b.x),
                    hi: d.a.x.max(d.b.x),
                    area: d.width * d.height,
                    width: if d.width.is_finite() {
                        d.width.max(0.0)
                    } else {
                        0.0
                    },
                    mid_z: mid_z(d),
                });
            } else if dx < EPS {
                runs.push(Run {
                    id: d.id,
                    orient: Orient::V,
                    fixed: d.a.x,
                    lo: d.a.y.min(d.b.y),
                    hi: d.a.y.max(d.b.y),
                    area: d.width * d.height,
                    width: if d.width.is_finite() {
                        d.width.max(0.0)
                    } else {
                        0.0
                    },
                    mid_z: mid_z(d),
                });
            } else {
                ignored.push(d.id);
            }
        }
        // Stable sort, then the first duct of a repeated id wins.
        runs.sort_by_key(|r| r.id);
        let mut kept: Vec<Run> = Vec::with_capacity(runs.len());
        for r in runs {
            if kept.last().is_some_and(|k| k.id == r.id) {
                ignored.push(r.id);
            } else {
                kept.push(r);
            }
        }
        let runs = kept;
        ignored.sort();

        let mut splits: Vec<Vec<f64>> = runs.iter().map(|r| vec![r.lo, r.hi]).collect();
        for (i, a) in runs.iter().enumerate() {
            for b in runs.iter() {
                if std::ptr::eq(a, b) {
                    continue;
                }
                if a.orient != b.orient {
                    // a's split where b crosses or touches it
                    if b.fixed >= a.lo - EPS
                        && b.fixed <= a.hi + EPS
                        && a.fixed >= b.lo - EPS
                        && a.fixed <= b.hi + EPS
                    {
                        splits[i].push(b.fixed);
                    }
                } else if (a.fixed - b.fixed).abs() < EPS {
                    for t in [b.lo, b.hi] {
                        if t >= a.lo - EPS && t <= a.hi + EPS {
                            splits[i].push(t);
                        }
                    }
                }
            }
        }
        Network {
            runs,
            splits,
            ignored,
        }
    }

    /// Nearest entry points of an anchor, best first.
    fn entries(&self, a: &Anchor, rules: &Rules) -> Vec<Entry> {
        let mut out: Vec<Entry> = self
            .runs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let t = match r.orient {
                    Orient::H => a.pos.x.clamp(r.lo, r.hi),
                    Orient::V => a.pos.y.clamp(r.lo, r.hi),
                };
                let p = r.at(t);
                let back = match a.facing {
                    Facing::Up => (p.y - a.pos.y).max(0.0),
                    Facing::Down => (a.pos.y - p.y).max(0.0),
                    Facing::Left => (p.x - a.pos.x).max(0.0),
                    Facing::Right => (a.pos.x - p.x).max(0.0),
                    Facing::Any => 0.0,
                };
                let sideways = matches!(
                    (a.facing, r.orient),
                    (Facing::Up | Facing::Down, Orient::V)
                        | (Facing::Left | Facing::Right, Orient::H)
                );
                let detour = 2.0 * back.min(TURN_MM);
                let stub = a.pos.manhattan(&p) + detour;
                Entry {
                    run: i,
                    t,
                    stub,
                    detour,
                    rank: stub + if sideways { SIDEWAYS_MM } else { 0.0 },
                }
            })
            .collect();
        out.sort_by(|x, y| {
            x.rank
                .total_cmp(&y.rank)
                .then(self.runs[x.run].id.cmp(&self.runs[y.run].id))
        });
        // A stub that would run across another duct is not a real way in;
        // keep such entries only when nothing else is left.
        let clear: Vec<Entry> = out
            .iter()
            .copied()
            .filter(|e| !self.stub_crosses(&stub(a, self.runs[e.run].at(e.t)), e.run))
            .collect();
        let mut pick = if clear.is_empty() { out } else { clear };
        // Prefer ducts within reach; keep the rest only when none is.
        if pick.iter().any(|e| e.stub <= rules.max_stub_mm) {
            pick.retain(|e| e.stub <= rules.max_stub_mm);
        }
        pick.truncate(CANDIDATES);
        pick
    }

    /// True when the polyline crosses, or runs along, a duct other than `skip`.
    fn stub_crosses(&self, pts: &[Point], skip: usize) -> bool {
        pts.windows(2).any(|w| {
            let (lo_x, hi_x) = (w[0].x.min(w[1].x), w[0].x.max(w[1].x));
            let (lo_y, hi_y) = (w[0].y.min(w[1].y), w[0].y.max(w[1].y));
            if hi_x - lo_x < EPS && hi_y - lo_y < EPS {
                return false;
            }
            self.runs.iter().enumerate().any(|(i, r)| {
                if i == skip {
                    return false;
                }
                // Overlap of the segment's box with the run, grown by EPS,
                // where the segment is a line: both are axis-aligned boxes.
                let (rx0, rx1, ry0, ry1) = match r.orient {
                    Orient::H => (r.lo, r.hi, r.fixed, r.fixed),
                    Orient::V => (r.fixed, r.fixed, r.lo, r.hi),
                };
                let (ox0, ox1) = (lo_x.max(rx0), hi_x.min(rx1));
                let (oy0, oy1) = (lo_y.max(ry0), hi_y.min(ry1));
                if ox1 < ox0 - EPS || oy1 < oy0 - EPS {
                    return false;
                }
                // Touching only at the segment's own end points is not a crossing.
                let hit = Point::new(ox0, oy0);
                let at_end = w.iter().any(|p| p.manhattan(&hit) < EPS)
                    && (ox1 - ox0).abs() < EPS
                    && (oy1 - oy0).abs() < EPS;
                !at_end
            })
        })
    }

    /// Routes one job: through the ducts when both anchors reach one
    /// connected set of ducts, otherwise in free air.
    pub fn route(&self, job: &Job, rules: &Rules) -> Route {
        let used = vec![0.0; self.runs.len()];
        self.route_loaded(
            job,
            rules,
            Load {
                used: &used,
                wire_area: 0.0,
                avoid: &[],
                forbid: &[],
            },
        )
    }

    /// Like [`Network::route`], with the ducts' current fill steering the
    /// wire away from ducts that would pass their limit.
    pub(crate) fn route_loaded(&self, job: &Job, rules: &Rules, load: Load) -> Route {
        let mut best: Option<(f64, Entry, Entry, Path)> = None;
        for ea in self.entries(&job.from, rules) {
            for eb in self.entries(&job.to, rules) {
                let Some((cost, path)) = self.shortest(&ea, &eb, rules, load) else {
                    continue;
                };
                let total = ea.rank + eb.rank + cost;
                if best.as_ref().is_none_or(|b| total < b.0 - EPS) {
                    best = Some((total, ea, eb, path));
                }
            }
        }
        let Some((_, ea, eb, path)) = best else {
            return free_air(job, rules);
        };

        let mut points = stub(&job.from, self.runs[ea.run].at(ea.t));
        let mut ducts = vec![self.runs[ea.run].id];
        for (p, run) in &path {
            points.push(*p);
            ducts.push(self.runs[*run].id);
        }
        let mut tail = stub(&job.to, self.runs[eb.run].at(eb.t));
        tail.reverse();
        points.extend(tail);
        ducts.push(self.runs[eb.run].id);
        ducts.dedup();
        points.dedup_by(|a, b| a.manhattan(b) < EPS);
        merge_collinear(&mut points);

        let detour = ea.detour + eb.detour;
        let z_of = |p: Point| self.z_at(p, self.runs[ea.run].mid_z);
        finish(job, Kind::Ducted, points, ducts, detour, rules, &z_of)
    }

    /// Height a wire lies at at a point of the network: that of the first
    /// duct (lowest id) the point is on, `fallback` when none contains it.
    fn z_at(&self, p: Point, fallback: f64) -> f64 {
        self.runs
            .iter()
            .find(|r| {
                let (along, across) = match r.orient {
                    Orient::H => (p.x, p.y),
                    Orient::V => (p.y, p.x),
                };
                (across - r.fixed).abs() < EPS && along > r.lo - EPS && along < r.hi + EPS
            })
            .map_or(fallback, |r| r.mid_z)
    }

    /// Cost multiplier of a run: 1 until it would pass 80% of its allowed
    /// fill with this wire added, then rising steeply.
    fn penalty(&self, run: usize, rules: &Rules, load: Load) -> f64 {
        let avoid = if load.avoid.contains(&self.runs[run].id) {
            AVOID_COST
        } else {
            1.0
        };
        avoid * self.fill_penalty(run, rules, load)
    }

    fn fill_penalty(&self, run: usize, rules: &Rules, load: Load) -> f64 {
        if !self.sized(run) {
            return 1.0;
        }
        let cap = self.runs[run].area * rules.fill_limit;
        let after = (load.used[run] + load.wire_area) / cap;
        if !after.is_finite() {
            return PENALTY_CAP;
        }
        (1.0 + 4.0 * (after - PENALTY_FROM).max(0.0) / (1.0 - PENALTY_FROM)).min(PENALTY_CAP)
    }

    /// True when the duct has a known cross-section. An unsized duct is
    /// neither penalised nor counted as overfull; it is reported instead.
    pub(crate) fn sized(&self, run: usize) -> bool {
        let a = self.runs[run].area;
        a.is_finite() && a > 0.0
    }

    /// Gap between the edges of two ducts, mm: 0 where they touch, cross or
    /// are the same duct. `None` when either id is not a duct of the network.
    pub fn duct_gap(&self, a: u64, b: u64) -> Option<f64> {
        let ra = self.runs.iter().find(|r| r.id == a)?;
        let rb = self.runs.iter().find(|r| r.id == b)?;
        Some(ra.gap_to(rb))
    }

    /// Ids of the ducts closer than `mm` (edge to edge) to any of `ducts`,
    /// those ducts included.
    pub fn ducts_within(&self, ducts: &[u64], mm: f64) -> Vec<u64> {
        let near: Vec<&Run> = self.runs.iter().filter(|r| ducts.contains(&r.id)).collect();
        self.runs
            .iter()
            .filter(|r| near.iter().any(|n| r.gap_to(n) < mm - EPS))
            .map(|r| r.id)
            .collect()
    }

    /// Number of ducts in the network (ignored ones are not counted).
    pub fn duct_count(&self) -> usize {
        self.runs.len()
    }

    /// Index of a duct by id, in id order.
    pub(crate) fn run_index(&self, id: u64) -> Option<usize> {
        self.runs.binary_search_by_key(&id, |r| r.id).ok()
    }

    /// (id, cross-section) of the duct at index `i`.
    pub(crate) fn run_info(&self, i: usize) -> (u64, f64) {
        (self.runs[i].id, self.runs[i].area)
    }

    /// Shortest way along the ducts between two entries. Returns the cost
    /// and the points after the first entry with the run each step lies on.
    fn shortest(&self, a: &Entry, b: &Entry, rules: &Rules, load: Load) -> Option<(f64, Path)> {
        let mut nodes: BTreeMap<(i64, i64), usize> = BTreeMap::new();
        let mut pts: Vec<Point> = Vec::new();
        let mut adj: Vec<Vec<(usize, f64, usize)>> = Vec::new();
        let mut node = |p: Point, pts: &mut Vec<Point>, adj: &mut Vec<Vec<_>>| -> usize {
            let key = ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64);
            *nodes.entry(key).or_insert_with(|| {
                pts.push(p);
                adj.push(Vec::new());
                pts.len() - 1
            })
        };

        for (i, r) in self.runs.iter().enumerate() {
            if load.forbid.contains(&r.id) {
                continue;
            }
            let mut ts = self.splits[i].clone();
            for e in [a, b] {
                if e.run == i {
                    ts.push(e.t);
                }
            }
            ts.sort_by(f64::total_cmp);
            ts.dedup_by(|x, y| (*x - *y).abs() < EPS);
            for w in ts.windows(2) {
                let (p, q) = (r.at(w[0]), r.at(w[1]));
                let (u, v) = (node(p, &mut pts, &mut adj), node(q, &mut pts, &mut adj));
                let len = (w[1] - w[0]) * self.penalty(i, rules, load);
                adj[u].push((v, len, i));
                adj[v].push((u, len, i));
            }
        }

        let start = node(self.runs[a.run].at(a.t), &mut pts, &mut adj);
        let goal = node(self.runs[b.run].at(b.t), &mut pts, &mut adj);

        let mut dist = vec![f64::INFINITY; pts.len()];
        let mut prev: Vec<Option<(usize, usize)>> = vec![None; pts.len()];
        let mut heap = BinaryHeap::new();
        dist[start] = 0.0;
        heap.push(State {
            cost: 0.0,
            node: start,
        });
        while let Some(State { cost, node: u }) = heap.pop() {
            if cost > dist[u] {
                continue;
            }
            if u == goal {
                break;
            }
            for &(v, len, run) in &adj[u] {
                let c = cost + len;
                if c < dist[v] - EPS {
                    dist[v] = c;
                    prev[v] = Some((u, run));
                    heap.push(State { cost: c, node: v });
                }
            }
        }
        if !dist[goal].is_finite() {
            return None;
        }
        let mut path = Vec::new();
        let mut at = goal;
        while let Some((u, run)) = prev[at] {
            path.push((pts[at], run));
            at = u;
        }
        // The first entry itself lies on the first entry's run.
        path.push((pts[start], a.run));
        path.reverse();
        Some((dist[goal], path))
    }
}

/// Min-heap entry: lowest cost first, lowest node id on equal cost.
#[derive(Debug, PartialEq)]
struct State {
    cost: f64,
    node: usize,
}

impl Eq for State {}

impl Ord for State {
    fn cmp(&self, o: &Self) -> Ordering {
        o.cost.total_cmp(&self.cost).then(o.node.cmp(&self.node))
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Points from an anchor to a duct entry: one leg along the facing axis,
/// then the other.
fn stub(a: &Anchor, entry: Point) -> Vec<Point> {
    let corner = match a.facing {
        Facing::Up | Facing::Down => Point::new(a.pos.x, entry.y),
        _ => Point::new(entry.x, a.pos.y),
    };
    vec![a.pos, corner, entry]
}

/// Drops points that lie on a straight run between their neighbours, so
/// the polyline keeps only its bends and ends.
fn merge_collinear(points: &mut Vec<Point>) {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for &p in points.iter() {
        while out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            // Only a point between its neighbours goes; a point where the
            // wire turns back on itself is a real end of a run.
            let between =
                |lo: f64, mid: f64, hi: f64| mid >= lo.min(hi) - EPS && mid <= lo.max(hi) + EPS;
            let same_x =
                (a.x - b.x).abs() < EPS && (b.x - p.x).abs() < EPS && between(a.y, b.y, p.y);
            let same_y =
                (a.y - b.y).abs() < EPS && (b.y - p.y).abs() < EPS && between(a.x, b.x, p.x);
            if same_x || same_y {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    *points = out;
}

fn free_air(job: &Job, rules: &Rules) -> Route {
    let (a, b) = (job.from.pos, job.to.pos);
    let corner = match job.from.facing {
        Facing::Up | Facing::Down => Point::new(a.x, b.y),
        _ => Point::new(b.x, a.y),
    };
    let mut points = vec![a, corner, b];
    points.dedup_by(|x, y| x.manhattan(y) < EPS);
    let z = job.from.z_mm.unwrap_or(0.0).max(job.to.z_mm.unwrap_or(0.0));
    finish(job, Kind::FreeAir, points, Vec::new(), 0.0, rules, &|_| z)
}

fn mid_z(d: &Duct) -> f64 {
    if d.height.is_finite() && d.height > 0.0 {
        d.height / 2.0
    } else {
        0.0
    }
}

/// Builds the route with its third dimension. `z_of` gives the height the
/// wire lies at for a point inside the network (the duct's, or the
/// anchors' own height in free air); the ends use their terminals' height,
/// so the wire rises or falls between a terminal and its duct.
fn finish(
    job: &Job,
    kind: Kind,
    points: Vec<Point>,
    ducts: Vec<u64>,
    detour: f64,
    rules: &Rules,
    z_of: &dyn Fn(Point) -> f64,
) -> Route {
    let n = points.len();
    let z: Vec<f64> = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == 0 {
                job.from.z_mm.unwrap_or(0.0)
            } else if i + 1 == n {
                job.to.z_mm.unwrap_or(0.0)
            } else {
                z_of(*p)
            }
        })
        .collect();
    let points3: Vec<Point3> = points
        .iter()
        .zip(&z)
        .map(|(p, z)| Point3::new(p.x, p.y, *z))
        .collect();
    let rise: f64 = z.windows(2).map(|w| (w[0] - w[1]).abs()).sum();
    let bends = points
        .windows(3)
        .filter(|w| {
            let (a, b, c) = (w[0], w[1], w[2]);
            ((b.x - a.x).abs() > EPS) != ((c.x - b.x).abs() > EPS)
        })
        .count();
    let length = polyline_length(&points)
        + detour
        + rise
        + bends as f64 * rules.bend_mm
        + 2.0 * rules.slack_mm;
    Route {
        job: job.id,
        kind,
        points,
        points3,
        ducts,
        detour_mm: detour,
        rise_mm: rise,
        bends,
        length_mm: length,
    }
}
