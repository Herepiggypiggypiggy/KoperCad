//! Drawing grid: snapping to it and choosing which grid lines to show at the
//! current zoom.

use crate::viewport::Viewport;
use glam::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridStyle {
    Lines,
    Dots,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// Snap spacing in world units. IEC schematics commonly use 2.5 mm or
    /// 4 mm; EPLAN's default grid C is 4 mm.
    pub spacing: f32,
    /// Grid origin in world units.
    pub origin: Vec2,
    /// Every n-th line is drawn as a major line.
    pub major_every: u32,
    pub visible: bool,
    pub style: GridStyle,
    /// Lines closer than this many screen points are thinned out.
    pub min_screen_spacing: f32,
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            spacing: 4.0,
            origin: Vec2::ZERO,
            major_every: 5,
            visible: true,
            style: GridStyle::Dots,
            min_screen_spacing: 10.0,
        }
    }
}

/// Grid lines to draw at the current zoom.
#[derive(Clone, Debug, PartialEq)]
pub struct VisibleGrid {
    /// Spacing actually drawn (a multiple of the snap spacing).
    pub step: f32,
    /// x of each vertical line, with whether it is major.
    pub xs: Vec<(f32, bool)>,
    /// y of each horizontal line, with whether it is major.
    pub ys: Vec<(f32, bool)>,
}

impl Grid {
    pub fn with_spacing(spacing: f32) -> Self {
        Self {
            spacing,
            ..Default::default()
        }
    }

    /// Nearest grid point.
    pub fn snap(&self, p: Vec2) -> Vec2 {
        if self.spacing <= 0.0 {
            return p;
        }
        ((p - self.origin) / self.spacing).round() * self.spacing + self.origin
    }

    /// The grid lines to draw, thinned so neighbours are at least
    /// `min_screen_spacing` points apart. Returns `None` when hidden.
    pub fn visible_lines(&self, view: &Viewport) -> Option<VisibleGrid> {
        if !self.visible || self.spacing <= 0.0 {
            return None;
        }
        let major = self.major_every.max(1);
        // Thin in steps of 1, 2, major, 2 * major, major^2, ... times the
        // snap spacing, so the drawn grid always lands on snap points.
        let major_u = major as u64;
        let mut level: u64 = 1;
        let mut mult = 1u64;
        while self.spacing * mult as f32 * view.zoom < self.min_screen_spacing {
            mult = if mult == level && major_u > 2 {
                level * 2
            } else {
                level * major_u.max(2)
            };
            if mult == level * major_u.max(2) {
                level = mult;
            }
            if mult > 1 << 40 {
                return None;
            }
        }
        let step = self.spacing * mult as f32;
        let major_step = (self.spacing * major as f32).max(step);
        let vis = view.visible_world();
        let lines = |lo: f32, hi: f32, o: f32| -> Vec<(f32, bool)> {
            let first = ((lo - o) / step).floor() as i64;
            let last = ((hi - o) / step).ceil() as i64;
            (first..=last)
                .map(|i| {
                    let v = o + i as f32 * step;
                    let is_major = ((v - o) / major_step).round() * major_step;
                    (v, (is_major - (v - o)).abs() < step * 0.25)
                })
                .collect()
        };
        Some(VisibleGrid {
            step,
            xs: lines(vis.min.x, vis.max.x, self.origin.x),
            ys: lines(vis.min.y, vis.max.y, self.origin.y),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_to_nearest_point() {
        let g = Grid::with_spacing(4.0);
        assert_eq!(g.snap(Vec2::new(5.9, -1.9)), Vec2::new(4.0, -0.0));
        assert_eq!(g.snap(Vec2::new(6.1, -2.1)), Vec2::new(8.0, -4.0));
        let g = Grid {
            origin: Vec2::new(1.0, 1.0),
            ..Grid::with_spacing(2.5)
        };
        assert_eq!(g.snap(Vec2::new(3.4, 1.2)), Vec2::new(3.5, 1.0));
    }

    #[test]
    fn grid_thins_out_when_zoomed_out() {
        let g = Grid::with_spacing(4.0);
        let mut v = Viewport::new(Vec2::new(1000.0, 800.0));
        v.zoom = 10.0;
        let near = g.visible_lines(&v).unwrap();
        assert_eq!(near.step, 4.0);
        v.zoom = 0.05;
        let far = g.visible_lines(&v).unwrap();
        assert!(far.step * v.zoom >= g.min_screen_spacing);
        assert!(far.xs.len() <= (1000.0 / g.min_screen_spacing) as usize + 3);
        // The drawn step is always a multiple of the snap spacing.
        let ratio = far.step / g.spacing;
        assert!((ratio - ratio.round()).abs() < 1e-3);
    }

    #[test]
    fn line_count_stays_bounded_at_every_zoom() {
        let g = Grid::default();
        let mut v = Viewport::new(Vec2::new(1920.0, 1080.0));
        for i in 0..60 {
            v.zoom = 0.01 * 1.25f32.powi(i);
            if let Some(vis) = g.visible_lines(&v) {
                assert!(vis.xs.len() <= 1920 / 10 + 3, "zoom {}", v.zoom);
                assert!(vis.ys.len() <= 1080 / 10 + 3);
            }
        }
    }

    #[test]
    fn major_lines_marked() {
        let g = Grid::with_spacing(4.0);
        let mut v = Viewport::new(Vec2::new(400.0, 400.0));
        v.zoom = 10.0;
        let vis = g.visible_lines(&v).unwrap();
        for (x, major) in vis.xs {
            let expect = (x / 20.0 - (x / 20.0).round()).abs() < 1e-3;
            assert_eq!(major, expect, "x = {x}");
        }
    }
}
