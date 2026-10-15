//! The duct graph and shortest-path search.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use crate::{Anchor, Duct, Facing, Job, Kind, Point, Route, Rules, polyline_length};

const EPS: f64 = 1e-6;
/// How many nearest ducts per anchor are tried as the entry.
const CANDIDATES: usize = 3;

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
}

impl Run {
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
    /// Ids of ducts that were not horizontal, vertical, finite or had no length.
    pub ignored: Vec<u64>,
}

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
                });
            } else if dx < EPS {
                runs.push(Run {
                    id: d.id,
                    orient: Orient::V,
                    fixed: d.a.x,
                    lo: d.a.y.min(d.b.y),
                    hi: d.a.y.max(d.b.y),
                });
            } else {
                ignored.push(d.id);
            }
        }
        runs.sort_by_key(|r| r.id);
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
                Entry {
                    run: i,
                    t,
                    stub: a.pos.manhattan(&p) + 2.0 * back,
                    detour: 2.0 * back,
                }
            })
            .collect();
        out.sort_by(|x, y| {
            x.stub
                .total_cmp(&y.stub)
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
        let mut best: Option<(f64, Entry, Entry, Path)> = None;
        for ea in self.entries(&job.from, rules) {
            for eb in self.entries(&job.to, rules) {
                let Some((cost, path)) = self.shortest(&ea, &eb) else {
                    continue;
                };
                let total = ea.stub + eb.stub + cost;
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
        finish(job, Kind::Ducted, points, ducts, detour, rules)
    }

    /// Shortest way along the ducts between two entries. Returns the cost
    /// and the points after the first entry with the run each step lies on.
    fn shortest(&self, a: &Entry, b: &Entry) -> Option<(f64, Path)> {
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
                let len = w[1] - w[0];
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
    finish(job, Kind::FreeAir, points, Vec::new(), 0.0, rules)
}

fn finish(
    job: &Job,
    kind: Kind,
    points: Vec<Point>,
    ducts: Vec<u64>,
    detour: f64,
    rules: &Rules,
) -> Route {
    let length = polyline_length(&points) + detour + 2.0 * rules.slack_mm;
    Route {
        job: job.id,
        kind,
        points,
        ducts,
        detour_mm: detour,
        length_mm: length,
    }
}
