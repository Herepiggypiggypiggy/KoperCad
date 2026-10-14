//! Page geometry.
//!
//! Coordinates are millimetres on the page. The origin is the top-left corner
//! of the page and `y` grows downwards, the same orientation as the screen.
//! Rotations are counter-clockwise as seen on the page.

use std::ops::{Add, Neg, Sub};

use serde::{Deserialize, Serialize};

/// A point or offset in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    /// Distance to another point.
    pub fn distance(self, other: Point) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    /// Whether two points coincide within `tolerance` millimetres on each axis.
    pub fn approx_eq(self, other: Point, tolerance: f64) -> bool {
        (self.x - other.x).abs() <= tolerance && (self.y - other.y).abs() <= tolerance
    }
}

impl Add for Point {
    type Output = Point;
    fn add(self, rhs: Point) -> Point {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, rhs: Point) -> Point {
        Point::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Neg for Point {
    type Output = Point;
    fn neg(self) -> Point {
        Point::new(-self.x, -self.y)
    }
}

/// Rotation in quarter turns, counter-clockwise as seen on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Rotation {
    #[default]
    #[serde(rename = "0")]
    R0,
    #[serde(rename = "90")]
    R90,
    #[serde(rename = "180")]
    R180,
    #[serde(rename = "270")]
    R270,
}

impl Rotation {
    /// Rotation in degrees (0, 90, 180 or 270).
    pub fn degrees(self) -> u16 {
        match self {
            Rotation::R0 => 0,
            Rotation::R90 => 90,
            Rotation::R180 => 180,
            Rotation::R270 => 270,
        }
    }

    /// Builds a rotation from degrees; any multiple of 90 is accepted.
    pub fn from_degrees(degrees: i32) -> Option<Rotation> {
        if degrees % 90 != 0 {
            return None;
        }
        Some(match degrees.rem_euclid(360) {
            0 => Rotation::R0,
            90 => Rotation::R90,
            180 => Rotation::R180,
            _ => Rotation::R270,
        })
    }

    /// This rotation followed by another.
    pub fn then(self, other: Rotation) -> Rotation {
        Rotation::from_degrees(i32::from(self.degrees()) + i32::from(other.degrees()))
            .expect("sum of quarter turns is a quarter turn")
    }

    /// Rotates an offset. With `y` pointing down, a visual counter-clockwise
    /// quarter turn maps `(x, y)` to `(y, -x)`.
    pub fn apply(self, p: Point) -> Point {
        match self {
            Rotation::R0 => p,
            Rotation::R90 => Point::new(p.y, -p.x),
            Rotation::R180 => Point::new(-p.x, -p.y),
            Rotation::R270 => Point::new(-p.y, p.x),
        }
    }
}

/// Where and how a symbol sits on its page: mirror (about the symbol's
/// vertical axis) first, then rotate, then move to `origin`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Placement {
    pub origin: Point,
    #[serde(default)]
    pub rotation: Rotation,
    #[serde(default)]
    pub mirrored: bool,
}

impl Placement {
    pub fn at(origin: Point) -> Self {
        Placement {
            origin,
            ..Placement::default()
        }
    }

    /// Maps a point in symbol coordinates to page coordinates.
    pub fn apply(&self, local: Point) -> Point {
        let mirrored = if self.mirrored {
            Point::new(-local.x, local.y)
        } else {
            local
        };
        self.origin + self.rotation.apply(mirrored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_turn_is_visually_counter_clockwise() {
        // A pin pointing right ends up pointing up (negative y) after 90 degrees.
        assert_eq!(
            Rotation::R90.apply(Point::new(1.0, 0.0)),
            Point::new(0.0, -1.0)
        );
        // Pointing down turns to pointing right.
        assert_eq!(
            Rotation::R90.apply(Point::new(0.0, 1.0)),
            Point::new(1.0, 0.0)
        );
    }

    #[test]
    fn four_quarter_turns_are_identity() {
        let p = Point::new(3.0, -7.5);
        let mut q = p;
        for _ in 0..4 {
            q = Rotation::R90.apply(q);
        }
        assert_eq!(p, q);
        assert_eq!(Rotation::R270.then(Rotation::R180), Rotation::R90);
        assert_eq!(Rotation::from_degrees(-90), Some(Rotation::R270));
        assert_eq!(Rotation::from_degrees(45), None);
    }

    #[test]
    fn placement_mirrors_then_rotates_then_moves() {
        let placement = Placement {
            origin: Point::new(100.0, 50.0),
            rotation: Rotation::R90,
            mirrored: true,
        };
        // (2, 0) mirrors to (-2, 0), rotates to (0, 2), moves to (100, 52).
        assert_eq!(
            placement.apply(Point::new(2.0, 0.0)),
            Point::new(100.0, 52.0)
        );
    }
}
