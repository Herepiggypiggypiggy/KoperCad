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

/// How far a wire keeps off a device body when it goes round it, mm.
const GAP_MM: f64 = 10.0;
/// Added to the rank of an entry whose stub cannot avoid a device body.
const BLOCKED_MM: f64 = 1000.0;
/// Cost factor of running along a DIN rail instead of in a duct, so a duct
/// wins when both are equally long.
const RAIL_COST: f64 = 1.1;
/// How far a rail's wiring channel reaches past the rail's ends to meet a
/// duct, mm.
const RAIL_REACH_MM: f64 = 100.0;
/// Clear space between a row of devices and the channel along its rail, mm.
const CHANNEL_GAP_MM: f64 = 10.0;
/// With device bodies known, the stub round them is worked out for this many
/// of the ducts nearest a terminal (straight distance) and this many rail
/// channels, not for every run of the plate.
const NEAR_DUCTS: usize = 8;
const NEAR_RAILS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orient {
    H,
    V,
}

/// Where a wire can leave or enter a duct along its length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Openings {
    /// Anywhere along the duct (the default: a slotted duct whose slot pitch
    /// is not known, so no position is claimed).
    Anywhere,
    /// Through slots every `pitch` mm, the first `first` mm from the start of
    /// the duct: a wire enters at the slot nearest its terminal.
    Pitch { pitch: f64, first: f64 },
    /// Only at the two ends: a closed duct without slots.
    Ends,
}

/// The footprint of a device on the plate (corners, mm). `id` is the
/// caller's own number for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub id: u64,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Body {
    fn contains(&self, p: Point, tol: f64) -> bool {
        p.x >= self.x0 - tol && p.x <= self.x1 + tol && p.y >= self.y0 - tol && p.y <= self.y1 + tol
    }

    /// True when the segment passes through the inside of the body, a
    /// little shrunk so that running along an edge does not count.
    fn cuts(&self, a: Point, b: Point) -> bool {
        let m = 0.5;
        let (x0, y0, x1, y1) = (self.x0 + m, self.y0 + m, self.x1 - m, self.y1 - m);
        if x1 <= x0 || y1 <= y0 {
            return false;
        }
        a.x.max(b.x) > x0 && a.x.min(b.x) < x1 && a.y.max(b.y) > y0 && a.y.min(b.y) < y1
    }
}

/// A DIN rail with the devices on it: wires between its devices may run in
/// a channel along the rail, outside the row of devices.
#[derive(Debug, Clone, PartialEq)]
pub struct RailRow {
    pub id: u64,
    pub vertical: bool,
    /// Where the rail's centre line starts and how far it runs.
    pub start: Point,
    pub length: f64,
    /// The extent of the devices across the rail: the lowest and highest
    /// coordinate across the line (y for a horizontal rail) any device on
    /// it reaches.
    pub across: (f64, f64),
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
    /// A wiring channel along a DIN rail, not a duct: no cross-section, no
    /// fill. `side` tells the two channels of one rail apart.
    rail: bool,
    side: u8,
    openings: Openings,
}

impl Run {
    /// The position along the run a wire at `t` enters by: `t` itself, or
    /// the nearest slot.
    fn snap(&self, t: f64) -> f64 {
        let t = t.clamp(self.lo, self.hi);
        match self.openings {
            Openings::Anywhere => t,
            Openings::Pitch { pitch, first } if pitch > EPS && pitch.is_finite() => {
                let k = ((t - self.lo - first) / pitch).round().max(0.0);
                (self.lo + first + k * pitch).clamp(self.lo, self.hi)
            }
            Openings::Pitch { .. } => t,
            Openings::Ends => {
                if t - self.lo <= self.hi - t {
                    self.lo
                } else {
                    self.hi
                }
            }
        }
    }

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
    /// Device footprints: a wire's stub from its terminal goes round them.
    bodies: Vec<Body>,
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
#[derive(Debug, Clone)]
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
    /// The wire from the terminal to the duct when device bodies are
    /// known: the shortest way round them. `None` without bodies.
    path: Option<Vec<Point>>,
    /// No way from the terminal to the duct avoids a device body.
    blocked: bool,
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
                    rail: false,
                    side: 0,
                    openings: Openings::Anywhere,
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
                    rail: false,
                    side: 0,
                    openings: Openings::Anywhere,
                });
            } else {
                ignored.push(d.id);
            }
        }
        // Stable sort, then the first duct of a repeated id wins.
        runs.sort_by_key(|r| (r.id, r.side));
        let mut kept: Vec<Run> = Vec::with_capacity(runs.len());
        for r in runs {
            if kept
                .last()
                .is_some_and(|k| k.id == r.id && k.side == r.side)
            {
                ignored.push(r.id);
            } else {
                kept.push(r);
            }
        }
        ignored.sort();
        Network::from_runs(kept, ignored, Vec::new())
    }

    fn from_runs(runs: Vec<Run>, ignored: Vec<u64>, bodies: Vec<Body>) -> Network {
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
            bodies,
        }
    }

    /// The same network with slots on some ducts: `(duct id, openings)`.
    pub fn with_openings(mut self, openings: &[(u64, Openings)]) -> Network {
        for (id, o) in openings {
            if let Some(r) = self.runs.iter_mut().find(|r| r.id == *id && !r.rail) {
                r.openings = *o;
            }
        }
        self
    }

    /// The same network that knows the footprints of the devices on the
    /// plate: stubs go round them instead of across.
    pub fn with_bodies(mut self, bodies: Vec<Body>) -> Network {
        self.bodies = bodies;
        self
    }

    /// The same network with a wiring channel along each rail's row of
    /// devices (above and below it), so wires between devices on one rail
    /// run along it. A channel is not a duct: it has no fill and the route
    /// lists it in [`Route::rails`].
    pub fn with_rails(self, rails: &[RailRow]) -> Network {
        let Network {
            mut runs,
            mut ignored,
            bodies,
            ..
        } = self;
        for rail in rails {
            let (orient, start) = if rail.vertical {
                (Orient::V, rail.start.y)
            } else {
                (Orient::H, rail.start.x)
            };
            let channels = [
                (0u8, rail.across.0 - CHANNEL_GAP_MM),
                (1u8, rail.across.1 + CHANNEL_GAP_MM),
            ];
            for (side, fixed) in channels {
                if !(fixed.is_finite() && start.is_finite() && rail.length.is_finite()) {
                    ignored.push(rail.id);
                    continue;
                }
                runs.push(Run {
                    id: rail.id,
                    orient,
                    fixed,
                    lo: start - RAIL_REACH_MM,
                    hi: start + rail.length + RAIL_REACH_MM,
                    area: 0.0,
                    width: 0.0,
                    mid_z: 0.0,
                    rail: true,
                    side,
                    openings: Openings::Anywhere,
                });
            }
        }
        ignored.sort();
        runs.sort_by_key(|r| (r.id, r.side));
        Network::from_runs(runs, ignored, bodies)
    }

    /// Nearest entry points of an anchor, best first.
    fn entries(&self, a: &Anchor, rules: &Rules) -> Vec<Entry> {
        let known =
            !self.bodies.is_empty() || self.runs.iter().any(|r| r.openings != Openings::Anywhere);
        let mut out: Vec<Entry> = if known {
            self.nearest_entries(a)
        } else {
            (0..self.runs.len())
                .map(|i| self.entry_of(i, a, false, &[]))
                .collect()
        };
        out.sort_by(|x, y| {
            x.rank
                .total_cmp(&y.rank)
                .then(self.runs[x.run].id.cmp(&self.runs[y.run].id))
                .then(self.runs[x.run].side.cmp(&self.runs[y.run].side))
        });
        // A stub that would run across another duct is not a real way in;
        // keep such entries only when nothing else is left.
        let clear: Vec<Entry> = out
            .iter()
            .filter(|e| {
                let p = self.runs[e.run].at(e.t);
                let pts = e.path.clone().unwrap_or_else(|| stub(a, p));
                !self.stub_crosses(&pts, e.run)
            })
            .cloned()
            .collect();
        let pick = if clear.is_empty() { out } else { clear };
        // The nearest few ducts and, apart from them, the nearest rail
        // channels, so a channel along the row never crowds out the ducts.
        let (mut rails, mut ducts): (Vec<Entry>, Vec<Entry>) =
            pick.into_iter().partition(|e| self.runs[e.run].rail);
        // Prefer ducts within reach; keep the rest only when none is.
        for list in [&mut ducts, &mut rails] {
            if list.iter().any(|e| e.stub <= rules.max_stub_mm) {
                list.retain(|e| e.stub <= rules.max_stub_mm);
            }
        }
        ducts.truncate(CANDIDATES);
        rails.truncate(2);
        ducts.extend(rails);
        ducts.sort_by(|x, y| x.rank.total_cmp(&y.rank));
        ducts
    }

    /// The entries worth working out when device bodies are known: the stub
    /// round the devices is costly, so only the runs nearest the terminal by
    /// straight distance are tried (the nearest few ducts and rail channels);
    /// the rest only when every one of those is blocked.
    fn nearest_entries(&self, a: &Anchor) -> Vec<Entry> {
        let mut order: Vec<(f64, usize)> = self
            .runs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let t = r.snap(match r.orient {
                    Orient::H => a.pos.x,
                    Orient::V => a.pos.y,
                });
                (a.pos.manhattan(&r.at(t)), i)
            })
            .collect();
        order.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        let (mut ducts, mut rails) = (0, 0);
        let mut first = Vec::new();
        let mut rest = Vec::new();
        for (_, i) in order {
            let (n, cap) = if self.runs[i].rail {
                (&mut rails, NEAR_RAILS)
            } else {
                (&mut ducts, NEAR_DUCTS)
            };
            if *n < cap {
                *n += 1;
                first.push(i);
            } else {
                rest.push(i);
            }
        }
        // The bodies the terminal sits on or in: its own device (or two
        // touching ones), found once for all its entries.
        let own: Vec<usize> = (0..self.bodies.len())
            .filter(|&i| self.bodies[i].contains(a.pos, 1.0))
            .collect();
        let mut out: Vec<Entry> = first
            .into_iter()
            .map(|i| self.entry_of(i, a, true, &own))
            .collect();
        if out.iter().all(|e| e.blocked) {
            out.extend(rest.into_iter().map(|i| self.entry_of(i, a, true, &own)));
        }
        out
    }

    /// The entry of an anchor on run `i`. `known`: device bodies or slots
    /// are in play, so the stub is the real path round them.
    fn entry_of(&self, i: usize, a: &Anchor, known: bool, own: &[usize]) -> Entry {
        let r = &self.runs[i];
        let t = r.snap(match r.orient {
            Orient::H => a.pos.x,
            Orient::V => a.pos.y,
        });
        let p = r.at(t);
        if known {
            let (mut path, blocked) = self.stub_path(a, p, r, own);
            let mut t = t;
            // A stub that goes round a device meets the duct's line
            // before its end: enter there, not farther along.
            if r.openings == Openings::Anywhere && !blocked {
                let on_line = |q: &Point| match r.orient {
                    Orient::H => {
                        (q.y - r.fixed).abs() < EPS && q.x >= r.lo - EPS && q.x <= r.hi + EPS
                    }
                    Orient::V => {
                        (q.x - r.fixed).abs() < EPS && q.y >= r.lo - EPS && q.y <= r.hi + EPS
                    }
                };
                if let Some(k) = path.iter().skip(1).position(on_line).map(|k| k + 1) {
                    path.truncate(k + 1);
                    t = match r.orient {
                        Orient::H => path[k].x,
                        Orient::V => path[k].y,
                    };
                }
            }
            let stub = crate::polyline_length(&path);
            return Entry {
                run: i,
                t,
                stub,
                detour: 0.0,
                rank: stub + if blocked { BLOCKED_MM } else { 0.0 },
                path: Some(path),
                blocked,
            };
        }
        let back = match a.facing {
            Facing::Up => (p.y - a.pos.y).max(0.0),
            Facing::Down => (a.pos.y - p.y).max(0.0),
            Facing::Left => (p.x - a.pos.x).max(0.0),
            Facing::Right => (a.pos.x - p.x).max(0.0),
            Facing::Any => 0.0,
        };
        let sideways = matches!(
            (a.facing, r.orient),
            (Facing::Up | Facing::Down, Orient::V) | (Facing::Left | Facing::Right, Orient::H)
        );
        let detour = 2.0 * back.min(TURN_MM);
        let stub = a.pos.manhattan(&p) + detour;
        Entry {
            run: i,
            t,
            stub,
            detour,
            rank: stub + if sideways { SIDEWAYS_MM } else { 0.0 },
            path: None,
            blocked: false,
        }
    }

    /// The wire from a terminal to a point on a duct, round the device
    /// bodies: the shortest of the two ways with one bend and the ways that
    /// leave the terminal, step clear of its own device and go on. The flag
    /// is set when every way crosses a device.
    ///
    /// Into a duct with slots the wire arrives straight at the slot, across
    /// the duct (or along it at an end), never along the side of the duct.
    fn stub_path(&self, a: &Anchor, entry: Point, run: &Run, own: &[usize]) -> (Vec<Point>, bool) {
        let off = run.width / 2.0 + GAP_MM / 2.0;
        // The point just outside the slot the wire reaches first.
        let e = match (run.openings, run.orient) {
            (Openings::Anywhere, _) => entry,
            (Openings::Ends, Orient::H) => {
                let s = if entry.x <= run.lo + EPS { -1.0 } else { 1.0 };
                Point::new(entry.x + s * GAP_MM, entry.y)
            }
            (Openings::Ends, Orient::V) => {
                let s = if entry.y <= run.lo + EPS { -1.0 } else { 1.0 };
                Point::new(entry.x, entry.y + s * GAP_MM)
            }
            (_, Orient::H) => {
                let s = if a.pos.y < entry.y { -1.0 } else { 1.0 };
                Point::new(entry.x, entry.y + s * off)
            }
            (_, Orient::V) => {
                let s = if a.pos.x < entry.x { -1.0 } else { 1.0 };
                Point::new(entry.x + s * off, entry.y)
            }
        };
        let (mut path, blocked) = self.stub_path_to(a, e, own);
        if e != entry {
            path.push(entry);
        }
        (path, blocked)
    }

    fn stub_path_to(&self, a: &Anchor, e: Point, own: &[usize]) -> (Vec<Point>, bool) {
        let first = stub(a, e);
        let other = match a.facing {
            Facing::Up | Facing::Down => vec![a.pos, Point::new(e.x, a.pos.y), e],
            _ => vec![a.pos, Point::new(a.pos.x, e.y), e],
        };
        let mut candidates = vec![first.clone(), other];
        if let Some(&b) = own.first() {
            let b = self.bodies[b];
            let out = match a.facing {
                Facing::Up => Some(Point::new(a.pos.x, a.pos.y - GAP_MM)),
                Facing::Down => Some(Point::new(a.pos.x, a.pos.y + GAP_MM)),
                Facing::Left => Some(Point::new(a.pos.x - GAP_MM, a.pos.y)),
                Facing::Right => Some(Point::new(a.pos.x + GAP_MM, a.pos.y)),
                Facing::Any => None,
            };
            if let Some(out) = out {
                for side in [-GAP_MM, GAP_MM] {
                    candidates.push(match a.facing {
                        Facing::Up | Facing::Down => {
                            let x = if side < 0.0 { b.x0 } else { b.x1 } + side;
                            vec![a.pos, out, Point::new(x, out.y), Point::new(x, e.y), e]
                        }
                        _ => {
                            let y = if side < 0.0 { b.y0 } else { b.y1 } + side;
                            vec![a.pos, out, Point::new(out.x, y), Point::new(e.x, y), e]
                        }
                    });
                }
            }
        }
        let deep = own.iter().any(|&i| self.bodies[i].contains(a.pos, -1.0));
        // Only the bodies inside the box that holds every candidate can be
        // crossed by one of them.
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in candidates.iter().flatten() {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        let near: Vec<usize> = (0..self.bodies.len())
            .filter(|&i| {
                let b = &self.bodies[i];
                b.x1 >= x0 && b.x0 <= x1 && b.y1 >= y0 && b.y0 <= y1
            })
            .collect();
        let clear = |path: &[Point]| {
            path.windows(2).enumerate().all(|(k, w)| {
                near.iter().all(|&i| {
                    let inside_start = k == 0 && deep && own.contains(&i);
                    inside_start || !self.bodies[i].cuts(w[0], w[1])
                })
            })
        };
        let best = candidates
            .into_iter()
            .filter(|c| clear(c))
            .min_by(|x, y| crate::polyline_length(x).total_cmp(&crate::polyline_length(y)));
        match best {
            Some(path) => (path, false),
            None => (first, true),
        }
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
                if i == skip || r.rail {
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
        let entries_b = self.entries(&job.to, rules);
        for ea in self.entries(&job.from, rules) {
            for eb in &entries_b {
                let Some((cost, path)) = self.shortest(&ea, eb, rules, load) else {
                    continue;
                };
                let total = ea.rank + eb.rank + cost;
                if best.as_ref().is_none_or(|b| total < b.0 - EPS) {
                    best = Some((total, ea.clone(), eb.clone(), path));
                }
            }
        }
        let Some((_, ea, eb, path)) = best else {
            return free_air(job, rules);
        };

        let head = |anchor: &Anchor, e: &Entry| {
            e.path
                .clone()
                .unwrap_or_else(|| stub(anchor, self.runs[e.run].at(e.t)))
        };
        let mut points = head(&job.from, &ea);
        let mut runs = vec![ea.run];
        for (p, run) in &path {
            points.push(*p);
            runs.push(*run);
        }
        let mut tail = head(&job.to, &eb);
        tail.reverse();
        points.extend(tail);
        runs.push(eb.run);
        runs.dedup();
        let mut ducts: Vec<u64> = Vec::new();
        let mut rails: Vec<u64> = Vec::new();
        for &run in &runs {
            let r = &self.runs[run];
            let list = if r.rail { &mut rails } else { &mut ducts };
            if list.last() != Some(&r.id) {
                list.push(r.id);
            }
        }
        points.dedup_by(|a, b| a.manhattan(b) < EPS);
        merge_collinear(&mut points);

        let detour = ea.detour + eb.detour;
        let at_terminals = (job.from.z_mm.unwrap_or(0.0) + job.to.z_mm.unwrap_or(0.0)) / 2.0;
        let z_of = |p: Point| self.z_at(p, self.runs[ea.run].mid_z, at_terminals);
        let mut route = finish(job, Kind::Ducted, points, ducts, detour, rules, &z_of);
        route.rails = rails;
        route.blocked = ea.blocked || eb.blocked;
        route
    }

    /// Height a wire lies at at a point of the network: that of the first
    /// duct (lowest id) the point is on, `fallback` when none contains it.
    /// Along a rail's channel it is the terminals' own height.
    fn z_at(&self, p: Point, fallback: f64, along_rail: f64) -> f64 {
        self.runs
            .iter()
            .find(|r| {
                let (along, across) = match r.orient {
                    Orient::H => (p.x, p.y),
                    Orient::V => (p.y, p.x),
                };
                (across - r.fixed).abs() < EPS && along > r.lo - EPS && along < r.hi + EPS
            })
            .map_or(fallback, |r| if r.rail { along_rail } else { r.mid_z })
    }

    /// Cost multiplier of a run: 1 until it would pass 80% of its allowed
    /// fill with this wire added, then rising steeply.
    fn penalty(&self, run: usize, rules: &Rules, load: Load) -> f64 {
        let avoid = if load.avoid.contains(&self.runs[run].id) {
            AVOID_COST
        } else {
            1.0
        };
        let rail = if self.runs[run].rail { RAIL_COST } else { 1.0 };
        avoid * rail * self.fill_penalty(run, rules, load)
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
        let ra = self.runs.iter().find(|r| r.id == a && !r.rail)?;
        let rb = self.runs.iter().find(|r| r.id == b && !r.rail)?;
        Some(ra.gap_to(rb))
    }

    /// Ids of the ducts closer than `mm` (edge to edge) to any of `ducts`,
    /// those ducts included.
    pub fn ducts_within(&self, ducts: &[u64], mm: f64) -> Vec<u64> {
        let near: Vec<&Run> = self
            .runs
            .iter()
            .filter(|r| !r.rail && ducts.contains(&r.id))
            .collect();
        self.runs
            .iter()
            .filter(|r| !r.rail)
            .filter(|r| near.iter().any(|n| r.gap_to(n) < mm - EPS))
            .map(|r| r.id)
            .collect()
    }

    /// Number of ducts in the network (ignored ones are not counted).
    pub fn duct_count(&self) -> usize {
        self.runs.iter().filter(|r| !r.rail).count()
    }

    /// Number of runs, ducts and rail channels (the length of the
    /// per-run lists).
    pub(crate) fn run_count(&self) -> usize {
        self.runs.len()
    }

    /// True when the run at index `i` is a channel along a rail.
    pub(crate) fn is_rail(&self, i: usize) -> bool {
        self.runs[i].rail
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
        rails: Vec::new(),
        blocked: false,
        detour_mm: detour,
        rise_mm: rise,
        bends,
        length_mm: length,
    }
}
