//! Snapping the cursor to connection points, geometry and the grid.

use crate::geom::Aabb;
use crate::grid::Grid;
use crate::scene::{ItemId, Scene};
use crate::viewport::Viewport;
use glam::Vec2;

/// What the cursor snapped to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SnapKind {
    /// A connection point declared by an item (symbol pins, wire ends).
    Connection,
    Endpoint,
    Center,
    Midpoint,
    /// Somewhere along a [`SnapLine`](crate::scene::SnapLine) (a rail, a
    /// duct).
    OnLine,
    Grid,
    /// No snap applied.
    Free,
}

impl SnapKind {
    /// Lower weight wins when several targets are in range: a connection
    /// point slightly further away beats a midpoint right under the cursor.
    fn weight(self) -> f32 {
        match self {
            SnapKind::Connection => 0.4,
            SnapKind::Endpoint => 0.7,
            SnapKind::Center => 0.9,
            SnapKind::Midpoint => 1.0,
            // Points on or near a line win over sliding along it.
            SnapKind::OnLine => 1.5,
            SnapKind::Grid | SnapKind::Free => f32::INFINITY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapSettings {
    pub grid: bool,
    /// Snap to item connection points.
    pub connections: bool,
    /// Snap to geometric points (ends, centres, midpoints).
    pub geometry: bool,
    /// Capture radius for object snaps, in screen points.
    pub radius: f32,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            grid: true,
            connections: true,
            geometry: true,
            radius: 10.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapped {
    pub point: Vec2,
    pub kind: SnapKind,
    /// The item owning the snap target, for object snaps.
    pub item: Option<ItemId>,
}

/// Snaps `cursor` (world units). Object snaps in range win over the grid.
/// Items in `exclude` (for example the ones being dragged) are ignored.
pub fn snap(
    scene: &Scene,
    view: &Viewport,
    grid: Option<&Grid>,
    settings: &SnapSettings,
    cursor: Vec2,
    exclude: &[ItemId],
) -> Snapped {
    let radius = view.screen_to_world_len(settings.radius);
    let mut best: Option<(f32, Snapped)> = None;
    if settings.connections || settings.geometry {
        let rect = Aabb::from_center(cursor, Vec2::splat(radius));
        let mut pts = Vec::new();
        scene.for_each_snap_candidate(&rect, |id, item| {
            if exclude.contains(&id) {
                return;
            }
            pts.clear();
            if settings.connections {
                pts.extend(item.snap_points.iter().map(|s| (s.pos, s.kind)));
            }
            if settings.geometry {
                for p in &item.primitives {
                    p.shape.snap_points(&mut pts);
                }
            }
            if settings.connections {
                pts.extend(
                    item.snap_lines
                        .iter()
                        .map(|l| (l.nearest(cursor), SnapKind::OnLine)),
                );
            }
            for &(p, kind) in &pts {
                let d = p.distance(cursor);
                if d > radius {
                    continue;
                }
                let score = d * kind.weight();
                if best.is_none_or(|(b, _)| score < b) {
                    best = Some((
                        score,
                        Snapped {
                            point: p,
                            kind,
                            item: Some(id),
                        },
                    ));
                }
            }
        });
    }
    if let Some((_, s)) = best {
        return s;
    }
    match grid {
        Some(g) if settings.grid => Snapped {
            point: g.snap(cursor),
            kind: SnapKind::Grid,
            item: None,
        },
        _ => Snapped {
            point: cursor,
            kind: SnapKind::Free,
            item: None,
        },
    }
}

/// Constrains `to` to a horizontal or vertical line through `from`,
/// whichever is closer: the orthogonal mode used for wire drawing.
pub fn ortho(from: Vec2, to: Vec2) -> Vec2 {
    let d = to - from;
    if d.x.abs() >= d.y.abs() {
        Vec2::new(to.x, from.y)
    } else {
        Vec2::new(from.x, to.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Item, LayerId, SnapPoint};
    use crate::shape::{Primitive, Shape};
    use crate::style::Stroke;

    fn scene() -> Scene {
        let mut s = Scene::new();
        s.insert(
            ItemId(1),
            Item::new(LayerId(0))
                .with(Primitive::stroked(
                    Shape::line(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)),
                    Stroke::default(),
                ))
                .with_snap(SnapPoint::connection(Vec2::new(6.0, 0.0))),
        );
        s
    }

    fn view() -> Viewport {
        Viewport {
            zoom: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn connection_beats_nearer_midpoint() {
        let s = scene();
        let r = snap(
            &s,
            &view(),
            None,
            &SnapSettings::default(),
            Vec2::new(5.0, 1.0),
            &[],
        );
        assert_eq!(r.kind, SnapKind::Connection);
        assert_eq!(r.point, Vec2::new(6.0, 0.0));
        assert_eq!(r.item, Some(ItemId(1)));
    }

    #[test]
    fn endpoint_and_grid_fallback() {
        let s = scene();
        let g = Grid::with_spacing(4.0);
        let r = snap(
            &s,
            &view(),
            Some(&g),
            &SnapSettings::default(),
            Vec2::new(10.5, 0.5),
            &[],
        );
        assert_eq!(r.kind, SnapKind::Endpoint);
        let r = snap(
            &s,
            &view(),
            Some(&g),
            &SnapSettings::default(),
            Vec2::new(50.5, 49.0),
            &[],
        );
        assert_eq!(r.kind, SnapKind::Grid);
        assert_eq!(r.point, Vec2::new(52.0, 48.0));
        let off = SnapSettings {
            grid: false,
            ..Default::default()
        };
        let r = snap(&s, &view(), Some(&g), &off, Vec2::new(50.5, 49.0), &[]);
        assert_eq!(r.kind, SnapKind::Free);
    }

    #[test]
    fn radius_is_in_screen_points() {
        let s = scene();
        let mut v = view();
        v.zoom = 0.5; // 10 pt radius = 20 world units
        let r = snap(
            &s,
            &v,
            None,
            &SnapSettings::default(),
            Vec2::new(25.0, 0.0),
            &[],
        );
        assert_eq!(r.kind, SnapKind::Connection);
        v.zoom = 4.0;
        let r = snap(
            &s,
            &v,
            None,
            &SnapSettings::default(),
            Vec2::new(25.0, 0.0),
            &[],
        );
        assert_eq!(r.kind, SnapKind::Free);
    }

    #[test]
    fn excluded_items_are_ignored() {
        let s = scene();
        let r = snap(
            &s,
            &view(),
            None,
            &SnapSettings::default(),
            Vec2::new(6.0, 0.0),
            &[ItemId(1)],
        );
        assert_eq!(r.kind, SnapKind::Free);
    }

    #[test]
    fn ortho_picks_dominant_axis() {
        assert_eq!(
            ortho(Vec2::ZERO, Vec2::new(10.0, 3.0)),
            Vec2::new(10.0, 0.0)
        );
        assert_eq!(
            ortho(Vec2::ZERO, Vec2::new(-2.0, -7.0)),
            Vec2::new(0.0, -7.0)
        );
    }

    /// A rail: the cursor slides along it, on its pitch when it has one,
    /// never past its ends; a point target in range still wins.
    #[test]
    fn snaps_onto_a_line_along_its_pitch() {
        use crate::scene::SnapLine;
        let mut s = Scene::new();
        s.insert(
            ItemId(7),
            Item::new(LayerId(0))
                .with_snap_line(SnapLine::new((0.0, 50.0), (100.0, 50.0)).stepped(9.0)),
        );
        let at = |s: &Scene, c: Vec2| snap(s, &view(), None, &SnapSettings::default(), c, &[]);
        let r = at(&s, Vec2::new(40.0, 53.0));
        assert_eq!((r.kind, r.item), (SnapKind::OnLine, Some(ItemId(7))));
        assert_eq!(r.point, Vec2::new(36.0, 50.0));
        // Past the end: the last whole step, not the end itself.
        let r = at(&s, Vec2::new(103.0, 50.0));
        assert_eq!(r.point, Vec2::new(99.0, 50.0));
        // Too far from the line: free.
        assert_eq!(at(&s, Vec2::new(40.0, 70.0)).kind, SnapKind::Free);
        // Without a pitch, straight under the cursor.
        let free = SnapLine::new((0.0, 0.0), (0.0, 30.0));
        assert_eq!(free.nearest(Vec2::new(4.0, 12.5)), Vec2::new(0.0, 12.5));
        assert_eq!(
            SnapLine::new((0.0, 0.0), (0.0, 0.0)).nearest(Vec2::ONE),
            Vec2::ZERO
        );
        assert_eq!(free.stepped(-1.0).step, None);
        // A connection point near the line beats the line.
        s.insert(
            ItemId(8),
            Item::new(LayerId(0)).with_snap(SnapPoint::connection(Vec2::new(42.0, 51.0))),
        );
        let r = at(&s, Vec2::new(40.0, 53.0));
        assert_eq!(r.kind, SnapKind::Connection);
    }
}
