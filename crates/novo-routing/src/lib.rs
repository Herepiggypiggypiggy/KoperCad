//! Wire routing through ducts (WS-16). Pure geometry: given ducts and wire
//! jobs on a mounting plate, find the route, its length and the ducts it
//! uses. See `docs/design/wire-routing.md`.
//!
//! Millimetres, origin top left, x right, y down. Nothing here touches a
//! project; routes are derived and never stored.

mod graph;

pub use graph::Network;

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
}

/// One connection that needs a physical wire.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: u64,
    pub from: Anchor,
    pub to: Anchor,
}

/// Routing parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rules {
    /// Service loop added at each end of a wire.
    pub slack_mm: f64,
    /// Longest wire from a terminal to its duct that is preferred. Longer
    /// entries are used only when no duct is within this reach, so a wire
    /// does not cross the plate in free air to save length in a duct.
    pub max_stub_mm: f64,
}

impl Default for Rules {
    fn default() -> Rules {
        Rules {
            slack_mm: 50.0,
            max_stub_mm: 150.0,
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
    /// Ducts the wire lies in, in order, consecutive repeats removed.
    pub ducts: Vec<u64>,
    /// Extra length where a terminal faces away from its duct and the
    /// wire has to leave and come back, counted twice the backwards distance.
    pub detour_mm: f64,
    /// Cut length: path + detour + slack at both ends. Never zero.
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
