//! Wire routing through ducts (WS-16). Pure geometry: given ducts and wire
//! jobs on a mounting plate, find the route, its length and the ducts it
//! uses. See `docs/design/wire-routing.md`.
//!
//! Millimetres, origin top left, x right, y down. Nothing here touches a
//! project; routes are derived and never stored.

pub mod entry;
mod graph;
pub mod model;
mod plan;
pub mod project_rules;

pub use graph::Network;
pub use plan::{DuctFill, Plan, Problem, plan};
pub use project_rules::WireRules;

/// Point on the plate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Point {
        Point { x, y }
    }
    pub fn manhattan(&self, o: &Point) -> f64 {
        (self.x - o.x).abs() + (self.y - o.y).abs()
    }
}

/// Point in space: x, y on the plate, z the height above it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub fn new(x: f64, y: f64, z: f64) -> Point3 {
        Point3 { x, y, z }
    }
}

/// A straight wiring duct. It must be horizontal or vertical; a diagonal
/// or zero-length duct is ignored by [`Network::new`] and reported there.
#[derive(Debug, Clone, PartialEq)]
pub struct Duct {
    pub id: u64,
    pub a: Point,
    pub b: Point,
    pub width: f64,
    pub height: f64,
}

/// The side of a device a terminal leaves from. `Any` means no preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Up,
    Down,
    Left,
    Right,
    Any,
}

/// A terminal position on the plate and the direction its wire leaves in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub pos: Point,
    pub facing: Facing,
    /// Height of the terminal above the plate. `None` is unknown and counts
    /// as 0 (a flat length), which the caller should report.
    pub z_mm: Option<f64>,
}

/// One connection that needs a physical wire.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: u64,
    pub from: Anchor,
    pub to: Anchor,
    /// Outer diameter of the wire. `None` (no part, no cross-section) uses
    /// [`DEFAULT_WIRE_DIAMETER_MM`] and is reported as a problem.
    pub diameter_mm: Option<f64>,
}

/// Outer diameter assumed for a wire of unknown size (about 1.5 mm2).
pub const DEFAULT_WIRE_DIAMETER_MM: f64 = 3.0;

/// Routing parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rules {
    /// Service loop added at each end of a wire.
    pub slack_mm: f64,
    /// Longest wire from a terminal to its duct that is preferred. Longer
    /// entries are used only when no duct is within this reach, so a wire
    /// does not cross the plate in free air to save length in a duct.
    pub max_stub_mm: f64,
    /// Share of a duct's cross-section that wires may fill (0..1).
    pub fill_limit: f64,
    /// Extra length per bend of 90 degrees (laying a wire round a corner
    /// takes more than the centre line). 0 leaves lengths on the centre line.
    pub bend_mm: f64,
}

impl Default for Rules {
    fn default() -> Rules {
        Rules {
            slack_mm: 50.0,
            max_stub_mm: 150.0,
            fill_limit: 0.5,
            bend_mm: 0.0,
        }
    }
}

/// How a route was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Through the ducts.
    Ducted,
    /// No duct path: a straight Manhattan line, to be flagged as a problem.
    FreeAir,
}

/// The chosen route for a job.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub job: u64,
    pub kind: Kind,
    /// Path from the first anchor to the second, without the slack loops.
    pub points: Vec<Point>,
    /// The same path in space: each point at the height the wire lies at
    /// (the terminal's, then the duct's, then the far terminal's).
    pub points3: Vec<Point3>,
    /// Ducts the wire lies in, in order, consecutive repeats removed.
    pub ducts: Vec<u64>,
    /// Extra length where a terminal faces away from its duct and the
    /// wire has to leave and come back, counted twice the backwards distance.
    pub detour_mm: f64,
    /// Length gained by the wire rising or falling between terminal height
    /// and duct height, in total over the whole route.
    pub rise_mm: f64,
    /// Number of 90 degree bends along the path.
    pub bends: usize,
    /// Cut length: path + detour + rise + bends + slack at both ends. Never
    /// zero.
    pub length_mm: f64,
}

/// Length of a polyline.
pub fn polyline_length(points: &[Point]) -> f64 {
    points.windows(2).map(|w| w[0].manhattan(&w[1])).sum()
}

/// Routes every job, in the given order.
pub fn route_all(network: &Network, jobs: &[Job], rules: &Rules) -> Vec<Route> {
    jobs.iter().map(|j| network.route(j, rules)).collect()
}
