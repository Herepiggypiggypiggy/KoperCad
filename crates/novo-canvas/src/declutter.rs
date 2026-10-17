//! Text floor and priority hiding for drawing text.
//!
//! Text is drawn in millimetres, so at "fit" on a small screen tags end up
//! at 5 px. The rule (design direction, drawing text): tags and values never
//! render below [`MIN_TEXT_PX`] on screen. A label that would be smaller is
//! enlarged to the floor, and when that does not work it is hidden by
//! priority instead of drawn unreadable:
//!
//! * a label lifted to the floor is dropped when it would overlap a label
//!   already placed (primary text is placed first, so secondary text gives
//!   way);
//! * secondary text (cross references, pin numbers, notes, wire and cable
//!   numbers) is dropped when reaching the floor would enlarge it by more
//!   than [`SECONDARY_MAX_BOOST`], because a blown-up note is worse than
//!   none.
//!
//! Labels already at or above the floor are never touched.

use glam::Vec2;

use crate::geom::Aabb;

/// Smallest text height on screen, in points.
pub const MIN_TEXT_PX: f32 = 9.0;

/// Clear space kept around enlarged text, in points.
const PAD: f32 = 2.0;

/// The most secondary text is enlarged to reach the floor.
pub const SECONDARY_MAX_BOOST: f32 = 2.0;

/// How much a label matters when the room runs out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Tags, values, column numbers: always kept.
    #[default]
    Primary,
    /// Cross references, pin numbers, notes, wire and cable numbers.
    Secondary,
}

/// One label that is big enough to be drawn at all.
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    /// Caller's handle, returned unchanged.
    pub key: usize,
    pub priority: Priority,
    /// Text height on screen at the drawing's scale.
    pub size: f32,
    /// Screen position of the anchor (the point the label grows from).
    pub anchor: Vec2,
    /// Screen box of the label at its own scale.
    pub bounds: Aabb,
}

/// A label that is drawn, and how much to enlarge it (1 when it is at the
/// floor already).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub key: usize,
    pub boost: f32,
}

/// Decides which labels are drawn and at what size. `floor` is
/// [`MIN_TEXT_PX`] in the app; larger labels come first within a priority.
pub fn plan(mut labels: Vec<Candidate>, floor: f32) -> Vec<Placed> {
    labels.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then(b.size.total_cmp(&a.size))
            .then(a.key.cmp(&b.key))
    });
    let mut taken = Boxes::default();
    let mut out = Vec::with_capacity(labels.len());
    for l in labels {
        let boost = if l.size >= floor {
            1.0
        } else {
            floor / l.size.max(1e-3)
        };
        if boost > 1.0 {
            if l.priority == Priority::Secondary && boost > SECONDARY_MAX_BOOST {
                continue;
            }
            let b = scaled(&l, boost);
            // Text boxes are estimates and glyphs need air: a gap of PAD
            // keeps neighbouring tags from running into each other.
            let padded = Aabb::new(b.min - Vec2::splat(PAD), b.max + Vec2::splat(PAD));
            if taken.hits(&padded) {
                continue;
            }
            taken.add(b);
        } else {
            taken.add(l.bounds);
        }
        out.push(Placed { key: l.key, boost });
    }
    out
}

fn scaled(l: &Candidate, boost: f32) -> Aabb {
    Aabb::new(
        l.anchor + (l.bounds.min - l.anchor) * boost,
        l.anchor + (l.bounds.max - l.anchor) * boost,
    )
}

/// Placed boxes in a coarse grid, so a page of thousands of labels does not
/// compare every pair.
#[derive(Default)]
struct Boxes {
    cells: std::collections::HashMap<(i32, i32), Vec<Aabb>>,
}

const CELL: f32 = 48.0;

impl Boxes {
    fn range(b: &Aabb) -> (std::ops::RangeInclusive<i32>, std::ops::RangeInclusive<i32>) {
        let c = |v: f32| (v / CELL).floor() as i32;
        (c(b.min.x)..=c(b.max.x), c(b.min.y)..=c(b.max.y))
    }

    fn add(&mut self, b: Aabb) {
        let (xs, ys) = Self::range(&b);
        for x in xs {
            for y in ys.clone() {
                self.cells.entry((x, y)).or_default().push(b);
            }
        }
    }

    fn hits(&self, b: &Aabb) -> bool {
        let (xs, ys) = Self::range(b);
        xs.into_iter().any(|x| {
            ys.clone().any(|y| {
                self.cells
                    .get(&(x, y))
                    .is_some_and(|v| v.iter().any(|o| o.intersects(b)))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(key: usize, priority: Priority, size: f32, at: Vec2, w: f32) -> Candidate {
        Candidate {
            key,
            priority,
            size,
            anchor: at,
            bounds: Aabb::new(at, at + Vec2::new(w, size)),
        }
    }

    #[test]
    fn small_text_is_lifted_to_the_floor() {
        let out = plan(vec![cand(0, Priority::Primary, 5.0, Vec2::ZERO, 20.0)], 9.0);
        assert_eq!(out.len(), 1);
        assert!((out[0].boost - 1.8).abs() < 1e-5);
    }

    #[test]
    fn text_at_or_over_the_floor_is_left_alone() {
        let out = plan(
            vec![
                cand(0, Priority::Primary, 9.0, Vec2::ZERO, 20.0),
                cand(1, Priority::Secondary, 14.0, Vec2::new(5.0, 3.0), 20.0),
            ],
            9.0,
        );
        assert_eq!(out.len(), 2, "native-size text is never culled");
        assert!(out.iter().all(|p| p.boost == 1.0));
    }

    #[test]
    fn secondary_text_gives_way_to_a_tag_it_would_overlap() {
        let out = plan(
            vec![
                cand(0, Priority::Secondary, 6.0, Vec2::new(4.0, 2.0), 20.0),
                cand(1, Priority::Primary, 5.0, Vec2::ZERO, 20.0),
            ],
            9.0,
        );
        assert_eq!(out.iter().map(|p| p.key).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn secondary_text_is_hidden_rather_than_blown_up() {
        let out = plan(
            vec![cand(0, Priority::Secondary, 3.0, Vec2::ZERO, 10.0)],
            9.0,
        );
        assert!(out.is_empty());
        let out = plan(vec![cand(0, Priority::Primary, 3.0, Vec2::ZERO, 10.0)], 9.0);
        assert_eq!(out.len(), 1, "a tag is enlarged however far it has to go");
    }

    #[test]
    fn separate_secondary_text_survives_when_there_is_room() {
        let out = plan(
            vec![
                cand(0, Priority::Primary, 5.0, Vec2::ZERO, 20.0),
                cand(1, Priority::Secondary, 5.0, Vec2::new(0.0, 100.0), 20.0),
            ],
            9.0,
        );
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn the_enlarged_box_grows_from_the_anchor() {
        // A 5 px label hanging under a tag with a gap: its anchor is the
        // bottom right corner, so growing 1.8x reaches up into the tag.
        let mut low = cand(1, Priority::Primary, 5.0, Vec2::new(0.0, 9.5), 10.0);
        low.anchor = low.bounds.max;
        let out = plan(
            vec![cand(0, Priority::Primary, 9.0, Vec2::ZERO, 10.0), low],
            9.0,
        );
        assert_eq!(out.len(), 1, "grown to 9 px it would sit on the first tag");
    }
}
