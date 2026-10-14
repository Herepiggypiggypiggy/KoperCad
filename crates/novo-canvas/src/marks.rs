//! Marks on the drawing (docs/DESIGN.md §5): proposals, problems, overrides
//! and changes, laid out in screen space so they keep their size at every
//! zoom and cluster into a count when zoomed out.
//!
//! This module only decides where marks go ([`layout`]); the egui widget
//! paints them on top of the drawing and its text.

use crate::geom::Aabb;
use crate::scene::{ItemId, Scene};
use crate::viewport::Viewport;
use glam::Vec2;
use std::collections::BTreeMap;

/// What a mark says. Proposal, problem and override differ in colour, line
/// style and position at once, so they stay apart for colour-blind users.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MarkKind {
    /// A suggested object (terminals, cable, part): a dashed outline.
    Proposal,
    /// A small triangle at the top right of the object.
    Warning,
    /// A small octagon at the top right of the object.
    Error,
    /// A hollow diamond at the top left, shown only while asked for.
    Override,
    /// Changes mode: an outline in the added colour.
    Added,
    /// Changes mode: an outline in the modified colour.
    Modified,
    /// Changes mode: a dashed ghost where something was removed.
    Removed,
}

impl MarkKind {
    /// Drawn as a screen-sized glyph at a corner (rather than an outline).
    pub fn is_glyph(self) -> bool {
        matches!(
            self,
            MarkKind::Warning | MarkKind::Error | MarkKind::Override
        )
    }

    /// Outlines drawn dashed.
    pub fn is_dashed(self) -> bool {
        matches!(self, MarkKind::Proposal | MarkKind::Removed)
    }
}

/// What a mark is attached to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkTarget {
    /// A canvas item; the mark follows its bounds.
    Item(ItemId),
    /// A world area, for things no longer on the canvas (a removed ghost).
    Area(Aabb),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub kind: MarkKind,
    pub target: MarkTarget,
    /// Shown on hover (problems) or next to the nearest proposal.
    pub text: String,
}

impl Mark {
    pub fn on(kind: MarkKind, item: ItemId) -> Self {
        Self {
            kind,
            target: MarkTarget::Item(item),
            text: String::new(),
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }
}

/// The marks a canvas shows, and the switches the app flips.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Marks {
    pub list: Vec<Mark>,
    /// False hides every mark (Shift+P); the app keeps its counters.
    pub hidden: bool,
    /// Overrides are shown only while asked for (Shift+O held).
    pub show_overrides: bool,
}

/// Size of a glyph in screen points.
pub const GLYPH_SIZE: f32 = 10.0;
/// Glyphs of one kind closer than this (screen points) merge into a count.
pub const CLUSTER_PX: f32 = 24.0;
/// Outlines sit this far outside the object, in screen points.
pub const OUTLINE_GAP: f32 = 3.0;

/// A mark placed on screen (points, canvas-local).
#[derive(Clone, Debug, PartialEq)]
pub enum Placed {
    /// A glyph at `pos`, standing for `marks` (indices into the list); more
    /// than one means a cluster drawn with its count.
    Glyph {
        kind: MarkKind,
        pos: Vec2,
        marks: Vec<usize>,
    },
    Outline {
        kind: MarkKind,
        rect: Aabb,
        mark: usize,
    },
}

/// Places the visible marks for this view. Glyphs of the same kind that
/// land within [`CLUSTER_PX`] of each other become one cluster.
pub fn layout(marks: &Marks, scene: &Scene, view: &Viewport) -> Vec<Placed> {
    if marks.hidden {
        return Vec::new();
    }
    let screen = Aabb::new(Vec2::ZERO, view.size).expand(GLYPH_SIZE + OUTLINE_GAP);
    let mut out = Vec::new();
    let mut cells: BTreeMap<(MarkKind, i32, i32), (Vec2, Vec<usize>)> = BTreeMap::new();
    for (i, m) in marks.list.iter().enumerate() {
        if m.kind == MarkKind::Override && !marks.show_overrides {
            continue;
        }
        let world = match m.target {
            // The object's own ink, not its tag: a mark sits on the body.
            MarkTarget::Item(id) => match scene.get(id) {
                Some(item) => {
                    let ink = item
                        .primitives
                        .iter()
                        .fold(Aabb::EMPTY, |b, p| b.union(&p.bounds(f32::INFINITY)));
                    if ink.is_empty() {
                        scene.item_bounds(id).unwrap_or(Aabb::EMPTY)
                    } else {
                        ink
                    }
                }
                None => continue,
            },
            MarkTarget::Area(b) => b,
        };
        if world.is_empty() {
            continue;
        }
        let rect = Aabb::from_corners(
            view.world_to_screen(world.min),
            view.world_to_screen(world.max),
        );
        if !rect.intersects(&screen) {
            continue;
        }
        if m.kind.is_glyph() {
            let pos = match m.kind {
                MarkKind::Override => Vec2::new(rect.min.x, rect.min.y),
                _ => Vec2::new(rect.max.x, rect.min.y),
            };
            let key = (
                m.kind,
                (pos.x / CLUSTER_PX).floor() as i32,
                (pos.y / CLUSTER_PX).floor() as i32,
            );
            let cell = cells.entry(key).or_insert((pos, Vec::new()));
            cell.1.push(i);
        } else {
            out.push(Placed::Outline {
                kind: m.kind,
                rect: rect.expand(OUTLINE_GAP),
                mark: i,
            });
        }
    }
    for ((kind, ..), (pos, marks)) in cells {
        out.push(Placed::Glyph { kind, pos, marks });
    }
    out
}

/// The proposal outline nearest to `cursor` (screen points), whose text is
/// shown as the hint; the first one when there is no cursor.
pub fn nearest_proposal(placed: &[Placed], cursor: Option<Vec2>) -> Option<&Placed> {
    let proposals = placed.iter().filter(|p| {
        matches!(
            p,
            Placed::Outline {
                kind: MarkKind::Proposal,
                ..
            }
        )
    });
    match cursor {
        None => proposals.min_by(|a, b| top_left(a).total_cmp(&top_left(b))),
        Some(c) => proposals.min_by(|a, b| distance(a, c).total_cmp(&distance(b, c))),
    }
}

fn top_left(p: &Placed) -> f32 {
    match p {
        Placed::Outline { rect, .. } => rect.min.y * 1e4 + rect.min.x,
        Placed::Glyph { pos, .. } => pos.y * 1e4 + pos.x,
    }
}

fn distance(p: &Placed, c: Vec2) -> f32 {
    match p {
        Placed::Outline { rect, .. } => rect.distance_to_point(c),
        Placed::Glyph { pos, .. } => pos.distance(c),
    }
}

/// The glyph under `cursor` (screen points), for its hover text.
pub fn glyph_at(placed: &[Placed], cursor: Vec2) -> Option<&Placed> {
    placed.iter().find(|p| match p {
        Placed::Glyph { pos, .. } => pos.distance(cursor) <= GLYPH_SIZE * 0.75,
        Placed::Outline { .. } => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Item, LayerId};
    use crate::shape::{Primitive, Shape};
    use crate::style::Stroke;

    fn scene(n: u64, spacing: f32) -> Scene {
        let mut s = Scene::new();
        for i in 0..n {
            let x = i as f32 * spacing;
            s.insert(
                ItemId(i + 1),
                Item::new(LayerId(0)).with(Primitive::stroked(
                    Shape::rect(Vec2::new(x, 0.0), Vec2::new(x + 4.0, 4.0)),
                    Stroke::default(),
                )),
            );
        }
        s
    }

    fn view(zoom: f32) -> Viewport {
        Viewport {
            center: Vec2::new(10.0, 2.0),
            zoom,
            size: Vec2::new(800.0, 600.0),
            ..Default::default()
        }
    }

    #[test]
    fn glyphs_sit_on_corners_at_screen_size() {
        let s = scene(1, 10.0);
        let marks = Marks {
            list: vec![
                Mark::on(MarkKind::Warning, ItemId(1)),
                Mark::on(MarkKind::Override, ItemId(1)),
            ],
            show_overrides: true,
            ..Default::default()
        };
        for zoom in [1.0, 10.0] {
            let v = view(zoom);
            let placed = layout(&marks, &s, &v);
            let b = s.item_bounds(ItemId(1)).unwrap();
            let tr = v.world_to_screen(Vec2::new(b.max.x, b.min.y));
            let tl = v.world_to_screen(b.min);
            assert!(placed.iter().any(|p| matches!(p,
                Placed::Glyph { kind: MarkKind::Warning, pos, .. } if pos.distance(tr) < 1e-3)));
            assert!(placed.iter().any(|p| matches!(p,
                Placed::Glyph { kind: MarkKind::Override, pos, .. } if pos.distance(tl) < 1e-3)));
        }
    }

    #[test]
    fn overrides_and_everything_hide_on_request() {
        let s = scene(1, 10.0);
        let mut marks = Marks {
            list: vec![
                Mark::on(MarkKind::Override, ItemId(1)),
                Mark::on(MarkKind::Error, ItemId(1)),
            ],
            ..Default::default()
        };
        let placed = layout(&marks, &s, &view(4.0));
        assert_eq!(placed.len(), 1, "overrides wait for Shift+O");
        marks.hidden = true;
        assert!(layout(&marks, &s, &view(4.0)).is_empty());
    }

    #[test]
    fn zoomed_out_glyphs_cluster_into_a_count() {
        let s = scene(20, 10.0);
        let marks = Marks {
            list: (1..=20)
                .map(|i| Mark::on(MarkKind::Warning, ItemId(i)))
                .collect(),
            ..Default::default()
        };
        let near = layout(&marks, &s, &view(8.0));
        let far = layout(&marks, &s, &view(0.2));
        let glyphs = |p: &[Placed]| {
            p.iter()
                .filter(|p| matches!(p, Placed::Glyph { .. }))
                .count()
        };
        assert!(
            glyphs(&far) < glyphs(&near),
            "{} vs {}",
            glyphs(&far),
            glyphs(&near)
        );
        // Every mark is still counted somewhere.
        let counted: usize = far
            .iter()
            .map(|p| match p {
                Placed::Glyph { marks, .. } => marks.len(),
                _ => 0,
            })
            .sum();
        assert_eq!(counted, 20);
    }

    #[test]
    fn proposals_outline_and_the_nearest_gets_the_hint() {
        let s = scene(3, 10.0);
        let marks = Marks {
            list: (1..=3)
                .map(|i| Mark::on(MarkKind::Proposal, ItemId(i)).with_text("↵ accept · Tab next"))
                .collect(),
            ..Default::default()
        };
        let v = view(4.0);
        let placed = layout(&marks, &s, &v);
        assert_eq!(placed.len(), 3);
        let cursor = v.world_to_screen(Vec2::new(22.0, 2.0));
        let Some(Placed::Outline { mark, rect, .. }) = nearest_proposal(&placed, Some(cursor))
        else {
            panic!()
        };
        assert_eq!(*mark, 2);
        // The outline stands off the object by the gap.
        let b = s.item_bounds(ItemId(3)).unwrap();
        assert!((v.world_to_screen(b.min).x - rect.min.x - OUTLINE_GAP).abs() < 1e-3);
    }

    #[test]
    fn removed_ghosts_use_an_area_and_offscreen_marks_are_skipped() {
        let s = scene(1, 10.0);
        let marks = Marks {
            list: vec![
                Mark {
                    kind: MarkKind::Removed,
                    target: MarkTarget::Area(Aabb::new(Vec2::ZERO, Vec2::splat(5.0))),
                    text: String::new(),
                },
                Mark {
                    kind: MarkKind::Added,
                    target: MarkTarget::Area(Aabb::new(Vec2::splat(5000.0), Vec2::splat(5001.0))),
                    text: String::new(),
                },
                Mark::on(MarkKind::Error, ItemId(99)),
            ],
            ..Default::default()
        };
        let placed = layout(&marks, &s, &view(4.0));
        assert_eq!(placed.len(), 1);
        assert!(matches!(
            placed[0],
            Placed::Outline {
                kind: MarkKind::Removed,
                ..
            }
        ));
    }
}
