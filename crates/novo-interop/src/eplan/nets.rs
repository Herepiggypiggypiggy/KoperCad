//! Net reconstruction for imported EPLAN data: the nets EPLAN draws, rebuilt
//! without EPLAN.
//!
//! EPLAN stores few explicit connections. Most of a drawing is connection
//! points that face each other with nothing between them, drawn line
//! segments, nodes (corners and T joints) and interruption points that pair
//! up by name. This module turns that geometry into nets by EPLAN's own
//! rules, on a neutral [`ConnGraph`] that the PXF reader fills (no PXF type
//! and no `novo-core` type appears here), and cross-checks the result
//! against KoperCAD's own net resolution ([`compare`]).
//!
//! The rules (source: `docs/design/eplan-nets.md`, which lists where each
//! came from):
//!
//! 1. A connection point that faces another across a free straight run is
//!    joined to it, over any distance. Ports are on one line when the other
//!    coordinate agrees within 0.2 mm. The run connects when the nearest
//!    port faces back; any other port met first, facing or not, stops it.
//!    Symbol bodies, texts and lines do not stop a run (the observed model;
//!    an optional [`Blocker`] exists for houses that want it). Two
//!    interruption points never join this way, and ports of one symbol
//!    ignore each other.
//! 2. A drawn [`ConnLine`] joins what its two ends touch. Ends that sit on
//!    one position join each other. A node that lies on a line's interior
//!    joins that line (a T joint); a plain point on a line's interior does
//!    not, and two crossing lines do not join.
//! 3. A node is one point with several legs; each leg is a ray as in rule 1.
//! 4. A terminal is one conductor: points sharing a `conductor` value are
//!    one net.
//! 5. Interruption points with the same full name pair up two by two in
//!    `seq` order; the pair is one net. An odd one stays unpaired, and a
//!    name used more than twice is reported as ambiguous (KoperCAD would
//!    join all of them, EPLAN pairs them).
//! 6. Explicit [`bridges`](ConnGraph::bridges) join two points (a jumper,
//!    a device-internal link the caller knows about).
//!
//! Contacts never join nets here, as in KoperCAD (whether a contact
//! conducts is simulation state).
//!
//! Positions are millimetres, y up, snapped to a micrometre grid: two
//! positions within a micrometre are one position.
//!
//! ```
//! use novo_interop::eplan::nets::*;
//!
//! // Two pins facing each other across a gap: one net.
//! let mut g = ConnGraph::default();
//! g.points.push(ConnPoint::pin("-K1:A1", 0, 0.0, 0.0, Dir::Right));
//! g.points.push(ConnPoint::pin("-H1:X1", 0, 40.0, 0.0, Dir::Left));
//! let nets = rebuild_nets(&g);
//! assert_eq!(nets.nets.len(), 1);
//! assert!(nets.issues.is_empty());
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::{Endpoint, Netlist};

/// A direction a connection point or node leg faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

impl Dir {
    pub fn opposite(self) -> Dir {
        match self {
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
        }
    }

    fn step(self) -> (f64, f64) {
        match self {
            Dir::Left => (-1.0, 0.0),
            Dir::Right => (1.0, 0.0),
            Dir::Up => (0.0, 1.0),
            Dir::Down => (0.0, -1.0),
        }
    }
}

/// Identifies one connection point across the import, stable and unique per
/// point (typically `=func+loc-device:pin`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PinKey(pub String);

impl From<&str> for PinKey {
    fn from(key: &str) -> Self {
        PinKey(key.to_owned())
    }
}

/// A position in millimetres, y up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl Pt {
    pub fn new(x: f64, y: f64) -> Pt {
        Pt { x, y }
    }
}

/// A connection point of a symbol, or a node.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnPoint {
    pub key: PinKey,
    /// The page's index in page order (the caller numbers pages 0, 1, ...).
    pub page: u32,
    pub at: Pt,
    /// The directions it connects in: one for a pin, the legs for a node,
    /// empty for a point that connects only through drawn lines.
    pub facing: Vec<Dir>,
    /// A node (corner or T joint): one point with several legs.
    pub node: bool,
    /// Points sharing a value are one conductor (the two sides of a terminal).
    pub conductor: Option<String>,
    /// The symbol the point belongs to; a run ignores points of its own
    /// symbol.
    pub owner: Option<String>,
}

impl ConnPoint {
    /// A pin facing one direction.
    pub fn pin(key: &str, page: u32, x: f64, y: f64, facing: Dir) -> ConnPoint {
        ConnPoint {
            key: key.into(),
            page,
            at: Pt::new(x, y),
            facing: vec![facing],
            node: false,
            conductor: None,
            owner: None,
        }
    }

    /// A node with the given legs.
    pub fn node(key: &str, page: u32, x: f64, y: f64, legs: &[Dir]) -> ConnPoint {
        ConnPoint {
            key: key.into(),
            page,
            at: Pt::new(x, y),
            facing: legs.to_vec(),
            node: true,
            conductor: None,
            owner: None,
        }
    }

    pub fn with_owner(mut self, owner: &str) -> ConnPoint {
        self.owner = Some(owner.to_owned());
        self
    }

    pub fn with_conductor(mut self, conductor: &str) -> ConnPoint {
        self.conductor = Some(conductor.to_owned());
        self
    }
}

/// A drawn connection segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnLine {
    pub page: u32,
    pub from: Pt,
    pub to: Pt,
}

/// An optional obstacle a connection run does not pass through. The crane
/// repo's observed EPLAN model has none (symbol bodies do not stop a run), so
/// the reader normally leaves this empty; it is there for houses that treat
/// a body as a stop. It must not contain the pins that start the run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blocker {
    pub page: u32,
    pub min: Pt,
    pub max: Pt,
}

/// An interruption point; the point itself is in [`ConnGraph::points`].
#[derive(Debug, Clone, PartialEq)]
pub struct Interruption {
    pub point: PinKey,
    /// The full name (`=func+loc-name`) that pairs it with another.
    pub name: String,
    /// Page order, then position within the page: pairs form in this order.
    pub seq: u32,
}

/// Everything net reconstruction reads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConnGraph {
    pub points: Vec<ConnPoint>,
    pub lines: Vec<ConnLine>,
    pub blockers: Vec<Blocker>,
    pub interruptions: Vec<Interruption>,
    pub bridges: Vec<(PinKey, PinKey)>,
}

/// One reconstructed net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconNet {
    /// Its connection points (nodes are not listed), in input order.
    pub pins: Vec<PinKey>,
    /// The pages it appears on, sorted.
    pub pages: Vec<u32>,
}

/// Something the reconstruction found wrong or unclear in the drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetIssue {
    /// An interruption point without a partner.
    UnpairedInterruption { name: String, point: PinKey },
    /// A name used by more than two interruption points: EPLAN pairs them
    /// two by two, KoperCAD joins all of them.
    AmbiguousInterruption { name: String, count: usize },
    /// A node leg that reaches nothing: no line, no facing point.
    DanglingLeg { node: PinKey, dir: Dir },
    /// A drawn line end that touches nothing.
    DanglingLineEnd { page: u32, x_um: i64, y_um: i64 },
    /// A pin that faces a direction but ended up alone in its net.
    OpenPin { pin: PinKey },
    /// A port faces another that does not point back: the run is stopped
    /// there and joins nothing.
    BlockedRun { from: PinKey, stopped_by: PinKey },
    /// A drawn line that is not horizontal or vertical: counted, not used.
    SkewLine { page: u32 },
    /// Two points with one key; the first is used.
    DuplicateKey { key: PinKey },
    /// An interruption or bridge naming a key that is not a point.
    UnknownPin { key: PinKey },
}

/// The result of [`rebuild_nets`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconNets {
    pub nets: Vec<ReconNet>,
    pub issues: Vec<NetIssue>,
    /// The straight runs and coincident points that joined two points, in
    /// the order found: the wires EPLAN implies, as a graph over the points.
    pub runs: Vec<(PinKey, PinKey)>,
    by_pin: HashMap<PinKey, usize>,
}

impl ReconNets {
    /// The index into [`nets`](ReconNets::nets) of the net a point is in.
    pub fn net_of(&self, key: &PinKey) -> Option<usize> {
        self.by_pin.get(key).copied()
    }

    /// Whether two points are in one net.
    pub fn connected(&self, a: &PinKey, b: &PinKey) -> bool {
        matches!((self.net_of(a), self.net_of(b)), (Some(x), Some(y)) if x == y)
    }

    /// Links that give every net as a chain of its points: what an importer
    /// draws as wires to make KoperCAD's own nets equal these. A net of one
    /// point gives none.
    pub fn spanning_links(&self) -> Vec<(PinKey, PinKey)> {
        self.nets
            .iter()
            .flat_map(|net| net.pins.windows(2).map(|w| (w[0].clone(), w[1].clone())))
            .collect()
    }

    /// Nets of two or more points.
    pub fn connected_nets(&self) -> usize {
        self.nets.iter().filter(|n| n.pins.len() > 1).count()
    }
}

struct Dsu(Vec<usize>);

impl Dsu {
    fn new(n: usize) -> Dsu {
        Dsu((0..n).collect())
    }

    fn find(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a.max(b)] = a.min(b);
        }
    }
}

/// Two ports are on one line when the other coordinate agrees within this
/// (0.2 mm, micrometres).
const TOL_UM: i64 = 200;

type Cell = (u32, i64, i64);

fn um(v: f64) -> i64 {
    (v * 1000.0).round() as i64
}

fn cell(page: u32, p: Pt) -> Cell {
    (page, um(p.x), um(p.y))
}

/// Whether `p` lies on the open interior of the segment `a`-`b`.
fn on_interior(p: Pt, a: Pt, b: Pt) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-12 {
        return false;
    }
    let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2;
    let (cx, cy) = (a.x + t * dx, a.y + t * dy);
    let off = ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt();
    let len = len2.sqrt();
    off < 1e-3 && t * len > 1e-3 && (1.0 - t) * len > 1e-3
}

/// Whether the segment from `a` to `b` passes through the interior of `r`.
fn crosses(blocker: &Blocker, a: Pt, b: Pt) -> bool {
    let shrink = 1e-3;
    let (min, max) = (
        Pt::new(blocker.min.x + shrink, blocker.min.y + shrink),
        Pt::new(blocker.max.x - shrink, blocker.max.y - shrink),
    );
    if min.x >= max.x || min.y >= max.y {
        return false;
    }
    let (lo_x, hi_x) = (a.x.min(b.x), a.x.max(b.x));
    let (lo_y, hi_y) = (a.y.min(b.y), a.y.max(b.y));
    // Straight runs are axis aligned: overlap of the boxes decides.
    lo_x < max.x && hi_x > min.x && lo_y < max.y && hi_y > min.y
}

/// Rebuilds the nets of `graph` by the rules in the module docs.
pub fn rebuild_nets(graph: &ConnGraph) -> ReconNets {
    let axis = |l: &ConnLine| {
        (l.from.x - l.to.x).abs() * 1000.0 <= TOL_UM as f64
            || (l.from.y - l.to.y).abs() * 1000.0 <= TOL_UM as f64
    };
    if !graph.lines.iter().all(axis) {
        // Skew lines are drawn only: count them and rebuild without them.
        let mut straight = graph.clone();
        straight.lines.retain(axis);
        let mut nets = rebuild_nets(&straight);
        for l in graph.lines.iter().filter(|l| !axis(l)) {
            nets.issues.push(NetIssue::SkewLine { page: l.page });
        }
        return nets;
    }
    let mut issues = Vec::new();
    let mut runs: Vec<(PinKey, PinKey)> = Vec::new();
    let n = graph.points.len();
    let mut index: HashMap<&PinKey, usize> = HashMap::with_capacity(n);
    for (i, p) in graph.points.iter().enumerate() {
        if index.entry(&p.key).or_insert(i) != &i {
            issues.push(NetIssue::DuplicateKey { key: p.key.clone() });
        }
    }
    // Elements: points first, then both ends of each line.
    let line_end = |l: usize, end: usize| n + 2 * l + end;
    let mut dsu = Dsu::new(n + 2 * graph.lines.len());

    // Rule 2 and coincident points: group by position.
    let mut at: BTreeMap<Cell, Vec<usize>> = BTreeMap::new();
    for (i, p) in graph.points.iter().enumerate() {
        at.entry(cell(p.page, p.at)).or_default().push(i);
    }
    for (l, line) in graph.lines.iter().enumerate() {
        at.entry(cell(line.page, line.from))
            .or_default()
            .push(line_end(l, 0));
        at.entry(cell(line.page, line.to))
            .or_default()
            .push(line_end(l, 1));
    }
    let mut lone_line_ends = Vec::new();
    for (&(page, x, y), members) in &at {
        let touches = |i: &usize| *i >= n || graph.points[*i].node;
        if members.iter().any(touches) {
            // A line end or node: everything at the position is joined.
            for w in members.windows(2) {
                dsu.union(w[0], w[1]);
                if w[0] < n && w[1] < n {
                    runs.push((
                        graph.points[w[0]].key.clone(),
                        graph.points[w[1]].key.clone(),
                    ));
                }
            }
        } else {
            // Bare pins join only when they face each other.
            for (a, &i) in members.iter().enumerate() {
                for &j in &members[a + 1..] {
                    let faces = graph.points[i]
                        .facing
                        .iter()
                        .any(|d| graph.points[j].facing.contains(&d.opposite()));
                    if faces {
                        dsu.union(i, j);
                    }
                }
            }
        }
        if let [only] = members.as_slice()
            && *only >= n
        {
            lone_line_ends.push((page, x, y));
        }
    }
    // A node on a line's interior is a T joint.
    for (i, p) in graph.points.iter().enumerate().filter(|(_, p)| p.node) {
        for (l, line) in graph.lines.iter().enumerate() {
            if line.page == p.page && on_interior(p.at, line.from, line.to) {
                dsu.union(i, line_end(l, 0));
            }
        }
    }

    // Rules 1 and 3: facing runs. Points are binned by the coordinate a run
    // keeps (rows for horizontal runs, columns for vertical ones); a run
    // looks into the neighbouring bins for the 0.2 mm line tolerance.
    let bin = |v: i64| v.div_euclid(TOL_UM);
    let mut rows: BTreeMap<(u32, i64), Vec<usize>> = BTreeMap::new();
    let mut cols: BTreeMap<(u32, i64), Vec<usize>> = BTreeMap::new();
    for (i, p) in graph.points.iter().enumerate() {
        rows.entry((p.page, bin(um(p.at.y)))).or_default().push(i);
        cols.entry((p.page, bin(um(p.at.x)))).or_default().push(i);
    }
    let is_interruption: Vec<bool> = {
        let mut v = vec![false; n];
        for it in &graph.interruptions {
            if let Some(&i) = index.get(&it.point) {
                v[i] = true;
            }
        }
        v
    };
    let mut backed = vec![vec![false; 4]; n];
    let slot = |d: Dir| match d {
        Dir::Left => 0,
        Dir::Right => 1,
        Dir::Up => 2,
        Dir::Down => 3,
    };
    for (i, p) in graph.points.iter().enumerate() {
        for &d in &p.facing {
            let horizontal = matches!(d, Dir::Left | Dir::Right);
            let sign = if matches!(d, Dir::Right | Dir::Up) {
                1
            } else {
                -1
            };
            let (index_of, cross, along) = if horizontal {
                (&rows, um(p.at.y), um(p.at.x))
            } else {
                (&cols, um(p.at.x), um(p.at.y))
            };
            // Candidates ahead on the same line, with their distance.
            let mut ahead: Vec<(i64, usize)> = Vec::new();
            for b in bin(cross) - 1..=bin(cross) + 1 {
                for &j in index_of.get(&(p.page, b)).into_iter().flatten() {
                    let q = &graph.points[j];
                    let (qc, qa) = if horizontal {
                        (um(q.at.y), um(q.at.x))
                    } else {
                        (um(q.at.x), um(q.at.y))
                    };
                    let same_owner = p.owner.is_some() && p.owner == q.owner;
                    let delta = (qa - along) * sign;
                    if j != i && !same_owner && (qc - cross).abs() <= TOL_UM && delta > 1 {
                        ahead.push((delta, j));
                    }
                }
            }
            let Some(nearest) = ahead.iter().map(|&(delta, _)| delta).min() else {
                continue;
            };
            // Every port within the tolerance of the nearest distance is a
            // candidate; one that faces back connects. Any other port in the
            // way blocks the run.
            let mut joined = false;
            for &(delta, j) in &ahead {
                let q = &graph.points[j];
                if delta - nearest > TOL_UM
                    || !q.facing.contains(&d.opposite())
                    || (is_interruption[i] && is_interruption[j])
                    || graph
                        .blockers
                        .iter()
                        .any(|b| b.page == p.page && crosses(b, p.at, q.at))
                {
                    continue;
                }
                dsu.union(i, j);
                runs.push((p.key.clone(), q.key.clone()));
                joined = true;
                backed[i][slot(d)] = true;
                backed[j][slot(d.opposite())] = true;
            }
            if !joined
                && let Some(&(_, j)) = ahead.iter().find(|&&(delta, _)| delta == nearest)
                && !(is_interruption[i] && is_interruption[j])
            {
                issues.push(NetIssue::BlockedRun {
                    from: p.key.clone(),
                    stopped_by: graph.points[j].key.clone(),
                });
            }
        }
    }
    // A leg is also backed by a drawn line leaving the node that way.
    for (i, p) in graph.points.iter().enumerate().filter(|(_, p)| p.node) {
        for &d in &p.facing {
            let (sx, sy) = d.step();
            let probe = Pt::new(p.at.x + sx * 1e-2, p.at.y + sy * 1e-2);
            let drawn = graph
                .lines
                .iter()
                .any(|l| l.page == p.page && on_interior(probe, l.from, l.to));
            // A line that merely ends at the node and heads off that way.
            let leaves = graph.lines.iter().any(|l| {
                l.page == p.page && {
                    let (near, far) = if cell(l.page, l.from) == cell(p.page, p.at) {
                        (l.from, l.to)
                    } else if cell(l.page, l.to) == cell(p.page, p.at) {
                        (l.to, l.from)
                    } else {
                        return false;
                    };
                    let (dx, dy) = (far.x - near.x, far.y - near.y);
                    dx * sx + dy * sy > 0.0 && (dx * sy - dy * sx).abs() < 1e-3
                }
            });
            if drawn || leaves {
                backed[i][slot(d)] = true;
            }
        }
    }

    // Rule 4: terminals.
    let mut conductors: HashMap<&str, usize> = HashMap::new();
    for (i, p) in graph.points.iter().enumerate() {
        if let Some(c) = &p.conductor {
            match conductors.get(c.as_str()) {
                Some(&first) => dsu.union(first, i),
                None => {
                    conductors.insert(c, i);
                }
            }
        }
    }

    // Rule 5: interruption points pair in `seq` order.
    let mut by_name: BTreeMap<&str, Vec<(u32, usize)>> = BTreeMap::new();
    let mut interruption_points = BTreeSet::new();
    for it in &graph.interruptions {
        match index.get(&it.point) {
            Some(&i) => {
                by_name.entry(&it.name).or_default().push((it.seq, i));
                interruption_points.insert(i);
            }
            None => issues.push(NetIssue::UnknownPin {
                key: it.point.clone(),
            }),
        }
    }
    for (name, mut members) in by_name {
        members.sort_unstable();
        for pair in members.chunks(2) {
            match pair {
                [(_, a), (_, b)] => dsu.union(*a, *b),
                [(_, lone)] => issues.push(NetIssue::UnpairedInterruption {
                    name: name.to_owned(),
                    point: graph.points[*lone].key.clone(),
                }),
                _ => {}
            }
        }
        if members.len() > 2 {
            issues.push(NetIssue::AmbiguousInterruption {
                name: name.to_owned(),
                count: members.len(),
            });
        }
    }

    // Rule 6.
    for (a, b) in &graph.bridges {
        match (index.get(a), index.get(b)) {
            (Some(&i), Some(&j)) => dsu.union(i, j),
            (ia, ib) => {
                for (found, key) in [(ia, a), (ib, b)] {
                    if found.is_none() {
                        issues.push(NetIssue::UnknownPin { key: key.clone() });
                    }
                }
            }
        }
    }

    // Collect nets of real points, in input order.
    let mut net_of_root: HashMap<usize, usize> = HashMap::new();
    let mut nets: Vec<ReconNet> = Vec::new();
    let mut by_pin = HashMap::new();
    let mut root_of_point = vec![0usize; n];
    for (i, p) in graph.points.iter().enumerate() {
        let root = dsu.find(i);
        root_of_point[i] = root;
        if p.node {
            continue;
        }
        let id = *net_of_root.entry(root).or_insert_with(|| {
            nets.push(ReconNet {
                pins: Vec::new(),
                pages: Vec::new(),
            });
            nets.len() - 1
        });
        nets[id].pins.push(p.key.clone());
        if !nets[id].pages.contains(&p.page) {
            nets[id].pages.push(p.page);
        }
        by_pin.entry(p.key.clone()).or_insert(id);
    }
    for net in &mut nets {
        net.pages.sort_unstable();
    }

    // Findings.
    for (i, p) in graph.points.iter().enumerate() {
        if p.node {
            for &d in &p.facing {
                if !backed[i][slot(d)] {
                    issues.push(NetIssue::DanglingLeg {
                        node: p.key.clone(),
                        dir: d,
                    });
                }
            }
        } else if !p.facing.is_empty()
            && !interruption_points.contains(&i)
            && nets[net_of_root[&root_of_point[i]]].pins.len() == 1
            && !touches_line(graph, p)
        {
            issues.push(NetIssue::OpenPin { pin: p.key.clone() });
        }
    }
    for (page, x_um, y_um) in lone_line_ends {
        issues.push(NetIssue::DanglingLineEnd { page, x_um, y_um });
    }
    ReconNets {
        nets,
        issues,
        runs,
        by_pin,
    }
}

/// Whether a drawn line ends on the point (so it is not open, only unfinished).
fn touches_line(graph: &ConnGraph, p: &ConnPoint) -> bool {
    let here = cell(p.page, p.at);
    graph
        .lines
        .iter()
        .any(|l| cell(l.page, l.from) == here || cell(l.page, l.to) == here)
}

/// Where the reconstruction and KoperCAD's nets disagree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetComparison {
    /// Pairs joined by EPLAN's rules that KoperCAD's nets keep apart: an
    /// import that lost a wire, or a rule KoperCAD does not have.
    pub import_only: Vec<(PinKey, PinKey)>,
    /// Pairs KoperCAD joins that the reconstruction keeps apart: same-name
    /// terminals, a name used by three interruption points, an extra wire.
    pub project_only: Vec<(PinKey, PinKey)>,
    /// Points the caller could not map to a KoperCAD endpoint; not compared.
    pub unmapped: Vec<PinKey>,
    /// Points compared.
    pub compared: usize,
}

impl NetComparison {
    /// Both resolutions give the same nets over the compared points.
    pub fn agrees(&self) -> bool {
        self.import_only.is_empty() && self.project_only.is_empty()
    }
}

/// Compares `recon` with `netlist` over the points `map` can place in the
/// project. Each disagreement is reported once as a representative pair.
pub fn compare(
    recon: &ReconNets,
    netlist: &Netlist,
    map: impl Fn(&PinKey) -> Option<Endpoint>,
) -> NetComparison {
    let mut out = NetComparison::default();
    // Per project net: the reconstruction nets met, each with a pin.
    let mut project_side: BTreeMap<novo_core::NetId, Vec<(usize, &PinKey)>> = BTreeMap::new();
    for (idx, net) in recon.nets.iter().enumerate() {
        let mut groups: BTreeMap<Option<novo_core::NetId>, &PinKey> = BTreeMap::new();
        for pin in &net.pins {
            let Some(end) = map(pin) else {
                out.unmapped.push(pin.clone());
                continue;
            };
            out.compared += 1;
            let id = netlist.net_of(end);
            groups.entry(id).or_insert(pin);
            if let Some(id) = id {
                let seen = project_side.entry(id).or_default();
                if !seen.iter().any(|(n, _)| *n == idx) {
                    seen.push((idx, pin));
                }
            }
        }
        let mut reps = groups.into_values();
        if let Some(first) = reps.next() {
            out.import_only
                .extend(reps.map(|other| (first.clone(), other.clone())));
        }
    }
    for seen in project_side.into_values() {
        if let Some(&(_, first)) = seen.first() {
            out.project_only.extend(
                seen.iter()
                    .skip(1)
                    .map(|&(_, other)| (first.clone(), other.clone())),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(key: &str, x: f64, y: f64, d: Dir) -> ConnPoint {
        ConnPoint::pin(key, 0, x, y, d)
    }

    #[test]
    fn facing_pins_join_over_a_gap() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("b", 80.0, 0.0, Dir::Left),
            ],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert!(r.connected(&"a".into(), &"b".into()));
    }

    #[test]
    fn pins_not_facing_back_do_not_join() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("b", 80.0, 0.0, Dir::Right),
            ],
            ..ConnGraph::default()
        };
        assert!(!rebuild_nets(&g).connected(&"a".into(), &"b".into()));
    }

    #[test]
    fn a_pin_in_between_stops_the_run() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("mid", 40.0, 0.0, Dir::Up),
                pin("b", 80.0, 0.0, Dir::Left),
            ],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert!(!r.connected(&"a".into(), &"b".into()));
    }

    #[test]
    fn a_body_in_between_stops_the_run() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("b", 80.0, 0.0, Dir::Left),
            ],
            blockers: vec![Blocker {
                page: 0,
                min: Pt::new(30.0, -5.0),
                max: Pt::new(50.0, 5.0),
            }],
            ..ConnGraph::default()
        };
        assert!(!rebuild_nets(&g).connected(&"a".into(), &"b".into()));
    }

    #[test]
    fn a_body_beside_the_run_does_not() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("b", 80.0, 0.0, Dir::Left),
            ],
            blockers: vec![Blocker {
                page: 0,
                min: Pt::new(30.0, 1.0),
                max: Pt::new(50.0, 9.0),
            }],
            ..ConnGraph::default()
        };
        assert!(rebuild_nets(&g).connected(&"a".into(), &"b".into()));
    }

    #[test]
    fn runs_stay_on_their_page() {
        let g = ConnGraph {
            points: vec![
                ConnPoint::pin("a", 0, 0.0, 0.0, Dir::Right),
                ConnPoint::pin("b", 1, 80.0, 0.0, Dir::Left),
            ],
            ..ConnGraph::default()
        };
        assert!(!rebuild_nets(&g).connected(&"a".into(), &"b".into()));
    }

    #[test]
    fn lines_with_a_corner_node_join_their_ends() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("b", 50.0, 40.0, Dir::Down),
                ConnPoint::node("n", 0, 50.0, 0.0, &[Dir::Left, Dir::Up]),
            ],
            lines: vec![
                ConnLine {
                    page: 0,
                    from: Pt::new(0.0, 0.0),
                    to: Pt::new(50.0, 0.0),
                },
                ConnLine {
                    page: 0,
                    from: Pt::new(50.0, 0.0),
                    to: Pt::new(50.0, 40.0),
                },
            ],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert!(r.connected(&"a".into(), &"b".into()));
        assert!(r.issues.is_empty(), "{:?}", r.issues);
        // The node is not a pin.
        assert_eq!(r.nets[0].pins.len(), 2);
    }

    #[test]
    fn a_node_on_a_line_makes_a_t_but_a_pin_does_not() {
        let line = ConnLine {
            page: 0,
            from: Pt::new(0.0, 0.0),
            to: Pt::new(100.0, 0.0),
        };
        let t = ConnGraph {
            points: vec![
                pin("l", 0.0, 0.0, Dir::Right),
                pin("r", 100.0, 0.0, Dir::Left),
                pin("branch", 50.0, 30.0, Dir::Down),
                ConnPoint::node("n", 0, 50.0, 0.0, &[Dir::Left, Dir::Right, Dir::Up]),
            ],
            lines: vec![
                line,
                ConnLine {
                    page: 0,
                    from: Pt::new(50.0, 0.0),
                    to: Pt::new(50.0, 30.0),
                },
            ],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&t);
        assert_eq!(r.nets.len(), 1);
        assert!(r.issues.is_empty(), "{:?}", r.issues);

        // The same drawing without the node: the branch stays apart.
        let mut no_node = t.clone();
        no_node.points.pop();
        let r = rebuild_nets(&no_node);
        assert!(r.connected(&"l".into(), &"r".into()));
        assert!(!r.connected(&"l".into(), &"branch".into()));
    }

    #[test]
    fn crossing_lines_do_not_join() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("b", 100.0, 0.0, Dir::Left),
                pin("c", 50.0, -50.0, Dir::Up),
                pin("d", 50.0, 50.0, Dir::Down),
            ],
            lines: vec![
                ConnLine {
                    page: 0,
                    from: Pt::new(0.0, 0.0),
                    to: Pt::new(100.0, 0.0),
                },
                ConnLine {
                    page: 0,
                    from: Pt::new(50.0, -50.0),
                    to: Pt::new(50.0, 50.0),
                },
            ],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert!(r.connected(&"a".into(), &"b".into()));
        assert!(r.connected(&"c".into(), &"d".into()));
        assert!(!r.connected(&"a".into(), &"c".into()));
    }

    #[test]
    fn dangling_ends_and_legs_are_reported() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                ConnPoint::node("n", 0, 50.0, 0.0, &[Dir::Left, Dir::Up]),
            ],
            lines: vec![ConnLine {
                page: 0,
                from: Pt::new(0.0, 0.0),
                to: Pt::new(50.0, 0.0),
            }],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert!(r.issues.contains(&NetIssue::DanglingLeg {
            node: "n".into(),
            dir: Dir::Up
        }));
        assert!(
            !r.issues
                .iter()
                .any(|i| matches!(i, NetIssue::DanglingLeg { dir: Dir::Left, .. }))
        );

        let loose = ConnGraph {
            points: vec![pin("a", 0.0, 0.0, Dir::Right)],
            lines: vec![ConnLine {
                page: 0,
                from: Pt::new(0.0, 0.0),
                to: Pt::new(30.0, 0.0),
            }],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&loose);
        assert!(r.issues.contains(&NetIssue::DanglingLineEnd {
            page: 0,
            x_um: 30_000,
            y_um: 0
        }));
        // The pin has a line on it: unfinished, not open.
        assert!(
            !r.issues
                .iter()
                .any(|i| matches!(i, NetIssue::OpenPin { .. }))
        );
    }

    #[test]
    fn runs_tolerate_a_fifth_of_a_millimetre_off_axis() {
        let near = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("b", 80.0, 0.15, Dir::Left),
            ],
            ..ConnGraph::default()
        };
        assert!(rebuild_nets(&near).connected(&"a".into(), &"b".into()));
        let far = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("b", 80.0, 0.5, Dir::Left),
            ],
            ..ConnGraph::default()
        };
        assert!(!rebuild_nets(&far).connected(&"a".into(), &"b".into()));
    }

    #[test]
    fn two_interruption_points_never_join_by_facing() {
        let mut g = interruption_graph(&[(0, "=A-X"), (1, "=A-Y")]);
        g.points[0] = pin("i0", 0.0, 0.0, Dir::Right);
        g.points[1] = pin("i1", 50.0, 0.0, Dir::Left);
        assert!(!rebuild_nets(&g).connected(&"i0".into(), &"i1".into()));
    }

    #[test]
    fn ports_of_one_symbol_ignore_each_other() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right).with_owner("s"),
                pin("b", 10.0, 0.0, Dir::Left).with_owner("s"),
            ],
            ..ConnGraph::default()
        };
        assert!(!rebuild_nets(&g).connected(&"a".into(), &"b".into()));
    }

    #[test]
    fn skew_lines_are_counted_not_used() {
        let g = ConnGraph {
            points: vec![pin("a", 0.0, 0.0, Dir::Up), pin("b", 30.0, 40.0, Dir::Down)],
            lines: vec![ConnLine {
                page: 0,
                from: Pt::new(0.0, 0.0),
                to: Pt::new(30.0, 40.0),
            }],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert!(!r.connected(&"a".into(), &"b".into()));
        assert!(r.issues.contains(&NetIssue::SkewLine { page: 0 }));
    }

    #[test]
    fn a_run_stopped_by_a_port_that_does_not_face_back_is_reported() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("mid", 40.0, 0.0, Dir::Up),
                pin("b", 80.0, 0.0, Dir::Left),
            ],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert!(r.issues.contains(&NetIssue::BlockedRun {
            from: "a".into(),
            stopped_by: "mid".into()
        }));
    }

    #[test]
    fn a_lone_facing_pin_is_open() {
        let g = ConnGraph {
            points: vec![pin("a", 0.0, 0.0, Dir::Right)],
            ..ConnGraph::default()
        };
        assert_eq!(
            rebuild_nets(&g).issues,
            vec![NetIssue::OpenPin { pin: "a".into() }]
        );
    }

    #[test]
    fn terminal_sides_are_one_conductor() {
        let g = ConnGraph {
            points: vec![
                pin("x1:1", 0.0, 0.0, Dir::Up).with_conductor("-X1:1"),
                pin("x1:1b", 0.0, -10.0, Dir::Down).with_conductor("-X1:1"),
                pin("k:13", 0.0, 50.0, Dir::Down),
            ],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert!(r.connected(&"x1:1".into(), &"x1:1b".into()));
        assert!(r.connected(&"x1:1".into(), &"k:13".into()));
    }

    fn interruption_graph(pages: &[(u32, &str)]) -> ConnGraph {
        let mut g = ConnGraph::default();
        for (seq, (page, name)) in pages.iter().enumerate() {
            let key = format!("i{seq}");
            g.points
                .push(ConnPoint::pin(&key, *page, 0.0, 0.0, Dir::Down));
            g.interruptions.push(Interruption {
                point: key.as_str().into(),
                name: (*name).to_owned(),
                seq: u32::try_from(seq).unwrap(),
            });
        }
        g
    }

    #[test]
    fn interruption_points_pair_two_by_two() {
        let g = interruption_graph(&[
            (0, "=A-X"),
            (1, "=A-X"),
            (2, "=A-X"),
            (3, "=A-X"),
            (4, "=A-Y"),
        ]);
        let r = rebuild_nets(&g);
        assert!(r.connected(&"i0".into(), &"i1".into()));
        assert!(r.connected(&"i2".into(), &"i3".into()));
        assert!(!r.connected(&"i1".into(), &"i2".into()));
        assert!(r.issues.contains(&NetIssue::AmbiguousInterruption {
            name: "=A-X".into(),
            count: 4
        }));
        assert!(r.issues.contains(&NetIssue::UnpairedInterruption {
            name: "=A-Y".into(),
            point: "i4".into()
        }));
    }

    #[test]
    fn an_odd_third_point_stays_unpaired() {
        let g = interruption_graph(&[(0, "=A-X"), (1, "=A-X"), (2, "=A-X")]);
        let r = rebuild_nets(&g);
        assert!(r.connected(&"i0".into(), &"i1".into()));
        assert!(!r.connected(&"i1".into(), &"i2".into()));
        assert!(r.issues.contains(&NetIssue::UnpairedInterruption {
            name: "=A-X".into(),
            point: "i2".into()
        }));
    }

    #[test]
    fn bridges_and_unknown_keys() {
        let g = ConnGraph {
            points: vec![pin("a", 0.0, 0.0, Dir::Up), pin("b", 0.0, 100.0, Dir::Up)],
            bridges: vec![("a".into(), "b".into()), ("a".into(), "zz".into())],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert!(r.connected(&"a".into(), &"b".into()));
        assert!(
            r.issues
                .contains(&NetIssue::UnknownPin { key: "zz".into() })
        );
    }

    #[test]
    fn duplicate_keys_are_reported_once_each() {
        let g = ConnGraph {
            points: vec![pin("a", 0.0, 0.0, Dir::Up), pin("a", 5.0, 0.0, Dir::Up)],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert_eq!(
            r.issues
                .iter()
                .filter(|i| matches!(i, NetIssue::DuplicateKey { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn spanning_links_chain_each_net() {
        let g = ConnGraph {
            points: vec![
                pin("a", 0.0, 0.0, Dir::Right),
                pin("b", 50.0, 0.0, Dir::Left),
                pin("c", 0.0, 20.0, Dir::Right),
                pin("d", 50.0, 20.0, Dir::Left),
                pin("lone", 0.0, 90.0, Dir::Up),
            ],
            ..ConnGraph::default()
        };
        let r = rebuild_nets(&g);
        assert_eq!(r.spanning_links().len(), 2);
        assert_eq!(r.connected_nets(), 2);
    }
}
