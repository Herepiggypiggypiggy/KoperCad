//! Tab cycles orientations (J3).
//!
//! EPLAN gives every corner and T orientation its own shortcut, spread over
//! the keyboard, and a selected T can't be turned with Tab. Here J picks up
//! a connection piece and Tab walks all nine shapes in one fixed order:
//! four corners, four Ts, then the cross. Shift+Tab walks back. R rotates
//! within the shape and X mirrors, so the familiar keys still work.
//!
//! Symbols and texts cycle the same way: Tab steps through the four
//! rotations, then the four mirrored ones.

use novo_core::{Direction, Placement, Rotation};

/// Which sides of a connection piece have a wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Arms {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

impl Arms {
    pub fn count(self) -> usize {
        [self.up, self.down, self.left, self.right]
            .iter()
            .filter(|a| **a)
            .count()
    }

    pub fn has(self, direction: Direction) -> bool {
        match direction {
            Direction::Up => self.up,
            Direction::Down => self.down,
            Direction::Left => self.left,
            Direction::Right => self.right,
        }
    }

    /// The arms leaving in these directions.
    pub fn from_directions(directions: impl IntoIterator<Item = Direction>) -> Arms {
        let mut arms = Arms::default();
        for d in directions {
            match d {
                Direction::Up => arms.up = true,
                Direction::Down => arms.down = true,
                Direction::Left => arms.left = true,
                Direction::Right => arms.right = true,
            }
        }
        arms
    }

    /// Turned a quarter counter-clockwise, as [`Rotation::R90`] turns.
    fn rotated(self) -> Arms {
        Arms {
            up: self.right,
            left: self.up,
            down: self.left,
            right: self.down,
        }
    }

    /// Mirrored about the vertical axis, as [`Placement::mirrored`].
    fn mirrored(self) -> Arms {
        Arms {
            left: self.right,
            right: self.left,
            ..self
        }
    }
}

/// A corner, T or cross where wires meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionPiece {
    /// `┌`: wires leave down and right.
    CornerDownRight,
    /// `┐`
    CornerDownLeft,
    /// `┘`
    CornerUpLeft,
    /// `└`
    CornerUpRight,
    /// `┬`: the stem points down.
    TDown,
    /// `┤`
    TLeft,
    /// `┴`
    TUp,
    /// `├`
    TRight,
    /// `┼`
    Cross,
}

impl ConnectionPiece {
    /// The Tab order.
    pub const ALL: [ConnectionPiece; 9] = [
        ConnectionPiece::CornerDownRight,
        ConnectionPiece::CornerDownLeft,
        ConnectionPiece::CornerUpLeft,
        ConnectionPiece::CornerUpRight,
        ConnectionPiece::TDown,
        ConnectionPiece::TLeft,
        ConnectionPiece::TUp,
        ConnectionPiece::TRight,
        ConnectionPiece::Cross,
    ];

    fn index(self) -> usize {
        ConnectionPiece::ALL
            .iter()
            .position(|p| *p == self)
            .expect("every piece is in ALL")
    }

    /// Tab.
    pub fn next(self) -> ConnectionPiece {
        ConnectionPiece::ALL[(self.index() + 1) % 9]
    }

    /// Shift+Tab.
    pub fn previous(self) -> ConnectionPiece {
        ConnectionPiece::ALL[(self.index() + 8) % 9]
    }

    pub fn arms(self) -> Arms {
        let a = |up, down, left, right| Arms {
            up,
            down,
            left,
            right,
        };
        match self {
            ConnectionPiece::CornerDownRight => a(false, true, false, true),
            ConnectionPiece::CornerDownLeft => a(false, true, true, false),
            ConnectionPiece::CornerUpLeft => a(true, false, true, false),
            ConnectionPiece::CornerUpRight => a(true, false, false, true),
            ConnectionPiece::TDown => a(false, true, true, true),
            ConnectionPiece::TLeft => a(true, true, true, false),
            ConnectionPiece::TUp => a(true, false, true, true),
            ConnectionPiece::TRight => a(true, true, false, true),
            ConnectionPiece::Cross => a(true, true, true, true),
        }
    }

    /// The piece with exactly these arms. Two opposite arms are a straight
    /// wire, not a piece, and fewer than two are no piece at all.
    pub fn from_arms(arms: Arms) -> Option<ConnectionPiece> {
        ConnectionPiece::ALL.into_iter().find(|p| p.arms() == arms)
    }

    /// The piece a junction needs for the wires that leave it, so a
    /// T-junction never asks for a direction (J3).
    pub fn for_wires(directions: impl IntoIterator<Item = Direction>) -> Option<ConnectionPiece> {
        ConnectionPiece::from_arms(Arms::from_directions(directions))
    }

    /// R: a quarter turn counter-clockwise, staying a corner or a T.
    pub fn rotate(self) -> ConnectionPiece {
        ConnectionPiece::from_arms(self.arms().rotated()).expect("rotation keeps the shape")
    }

    /// X: mirrored left to right.
    pub fn mirror(self) -> ConnectionPiece {
        ConnectionPiece::from_arms(self.arms().mirrored()).expect("mirroring keeps the shape")
    }

    pub fn is_corner(self) -> bool {
        self.arms().count() == 2
    }

    pub fn is_t(self) -> bool {
        self.arms().count() == 3
    }

    /// A box-drawing character for menus and the status bar.
    pub fn glyph(self) -> char {
        match self {
            ConnectionPiece::CornerDownRight => '┌',
            ConnectionPiece::CornerDownLeft => '┐',
            ConnectionPiece::CornerUpLeft => '┘',
            ConnectionPiece::CornerUpRight => '└',
            ConnectionPiece::TDown => '┬',
            ConnectionPiece::TLeft => '┤',
            ConnectionPiece::TUp => '┴',
            ConnectionPiece::TRight => '├',
            ConnectionPiece::Cross => '┼',
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ConnectionPiece::CornerDownRight => "Corner down-right",
            ConnectionPiece::CornerDownLeft => "Corner down-left",
            ConnectionPiece::CornerUpLeft => "Corner up-left",
            ConnectionPiece::CornerUpRight => "Corner up-right",
            ConnectionPiece::TDown => "T down",
            ConnectionPiece::TLeft => "T left",
            ConnectionPiece::TUp => "T up",
            ConnectionPiece::TRight => "T right",
            ConnectionPiece::Cross => "Cross",
        }
    }
}

/// One of the eight orientations of a symbol or text: four rotations, each
/// plain or mirrored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Orientation {
    pub rotation: Rotation,
    pub mirrored: bool,
}

impl Orientation {
    /// The Tab order.
    pub const ALL: [Orientation; 8] = [
        Orientation::new(Rotation::R0, false),
        Orientation::new(Rotation::R90, false),
        Orientation::new(Rotation::R180, false),
        Orientation::new(Rotation::R270, false),
        Orientation::new(Rotation::R0, true),
        Orientation::new(Rotation::R90, true),
        Orientation::new(Rotation::R180, true),
        Orientation::new(Rotation::R270, true),
    ];

    pub const fn new(rotation: Rotation, mirrored: bool) -> Self {
        Orientation { rotation, mirrored }
    }

    pub fn of(placement: &Placement) -> Self {
        Orientation::new(placement.rotation, placement.mirrored)
    }

    /// The placement turned to this orientation, at the same origin.
    pub fn apply_to(self, placement: Placement) -> Placement {
        Placement {
            rotation: self.rotation,
            mirrored: self.mirrored,
            ..placement
        }
    }

    fn index(self) -> usize {
        Orientation::ALL
            .iter()
            .position(|o| *o == self)
            .expect("every orientation is in ALL")
    }

    /// Tab.
    pub fn next(self) -> Orientation {
        Orientation::ALL[(self.index() + 1) % 8]
    }

    /// Shift+Tab.
    pub fn previous(self) -> Orientation {
        Orientation::ALL[(self.index() + 7) % 8]
    }

    /// R: a quarter turn counter-clockwise on the page.
    pub fn rotate(self) -> Orientation {
        Orientation::new(self.rotation.then(Rotation::R90), self.mirrored)
    }

    /// X: mirror left to right on the page. Since a placement mirrors
    /// before it rotates, mirroring on the page also reverses the rotation.
    pub fn mirror(self) -> Orientation {
        let reversed = Rotation::from_degrees(-i32::from(self.rotation.degrees()))
            .expect("a quarter turn reversed is a quarter turn");
        Orientation::new(reversed, !self.mirrored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::Point;
    use std::collections::HashSet;

    #[test]
    fn tab_reaches_every_piece_and_comes_back() {
        let mut seen = HashSet::new();
        let mut piece = ConnectionPiece::CornerDownRight;
        for _ in 0..9 {
            seen.insert(piece);
            piece = piece.next();
        }
        assert_eq!(seen.len(), 9);
        assert_eq!(piece, ConnectionPiece::CornerDownRight);
        for p in ConnectionPiece::ALL {
            assert_eq!(p.next().previous(), p);
        }
    }

    #[test]
    fn tab_order_is_corners_then_ts_then_cross() {
        let shapes: Vec<usize> = ConnectionPiece::ALL
            .iter()
            .map(|p| p.arms().count())
            .collect();
        assert_eq!(shapes, [2, 2, 2, 2, 3, 3, 3, 3, 4]);
    }

    #[test]
    fn rotate_keeps_the_shape_and_four_turns_are_identity() {
        for p in ConnectionPiece::ALL {
            let r = p.rotate();
            assert_eq!(r.arms().count(), p.arms().count());
            assert_eq!(r.rotate().rotate().rotate(), p);
            assert_eq!(p.mirror().mirror(), p);
        }
        // ┬ turned a quarter counter-clockwise points its stem right: ├.
        assert_eq!(ConnectionPiece::TDown.rotate(), ConnectionPiece::TRight);
        assert_eq!(ConnectionPiece::TLeft.mirror(), ConnectionPiece::TRight);
    }

    #[test]
    fn rotate_matches_the_model_rotation() {
        // An arm vector turned by Rotation::R90 lands on the rotated piece's arm.
        for p in ConnectionPiece::ALL {
            let turned = p.rotate();
            for d in [
                Direction::Up,
                Direction::Down,
                Direction::Left,
                Direction::Right,
            ] {
                if p.arms().has(d) {
                    let placement = Placement {
                        rotation: Rotation::R90,
                        ..Placement::at(Point::new(0.0, 0.0))
                    };
                    assert!(
                        turned.arms().has(placement.apply_direction(d)),
                        "{p:?} {d:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn junction_shape_follows_the_wires() {
        use Direction::*;
        assert_eq!(
            ConnectionPiece::for_wires([Left, Right, Down]),
            Some(ConnectionPiece::TDown)
        );
        assert_eq!(ConnectionPiece::for_wires([Up, Down]), None);
        assert_eq!(
            ConnectionPiece::for_wires([Up, Down, Left, Right]),
            Some(ConnectionPiece::Cross)
        );
    }

    #[test]
    fn symbol_tab_walks_all_eight_orientations() {
        let mut o = Orientation::default();
        let mut seen = HashSet::new();
        for _ in 0..8 {
            seen.insert(o);
            o = o.next();
        }
        assert_eq!(seen.len(), 8);
        assert_eq!(o, Orientation::default());
        for o in Orientation::ALL {
            assert_eq!(o.next().previous(), o);
            assert_eq!(o.mirror().mirror(), o);
            assert_eq!(o.rotate().rotate().rotate().rotate(), o);
        }
    }

    #[test]
    fn mirror_is_a_page_mirror() {
        // A pin to the right of the origin ends up on the left after X,
        // whatever the rotation.
        for o in Orientation::ALL {
            let before = o.apply_to(Placement::default());
            let after = o.mirror().apply_to(Placement::default());
            let p = before.apply(Point::new(3.0, 1.0));
            let q = after.apply(Point::new(3.0, 1.0));
            assert_eq!(q, Point::new(-p.x, p.y), "{o:?}");
        }
    }
}
