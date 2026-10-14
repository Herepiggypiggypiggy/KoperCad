//! Tab cycles orientations.
//!
//! While placing or with a symbol or text selected, Tab steps through the
//! four rotations and then the four mirrored ones; Shift+Tab walks back. R
//! rotates and X mirrors on the page. The canvas reports the key as
//! `cycle_orientation` (WS-03) and the editor (WS-06) asks this table for
//! the next orientation, so the order is defined once.
//!
//! Connection pieces (corners and Ts) have no table here: the wire tool
//! makes junctions itself, from where the wires go.

use novo_core::{Placement, Rotation};

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
