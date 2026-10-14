//! Camera: maps world coordinates (mm) to screen points and back, with pan,
//! zoom about the cursor and zoom to fit.

use crate::geom::Aabb;
use glam::Vec2;

/// A 2D camera. Screen coordinates are in points relative to the canvas'
/// top-left corner; world and screen y both point down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// World point shown at the centre of the canvas.
    pub center: Vec2,
    /// Screen points per world unit.
    pub zoom: f32,
    /// Canvas size in screen points.
    pub size: Vec2,
    pub min_zoom: f32,
    pub max_zoom: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 2.0,
            size: Vec2::new(800.0, 600.0),
            min_zoom: 0.01,
            max_zoom: 2000.0,
        }
    }
}

impl Viewport {
    pub fn new(size: Vec2) -> Self {
        Self {
            size,
            ..Default::default()
        }
    }

    pub fn world_to_screen(&self, w: Vec2) -> Vec2 {
        (w - self.center) * self.zoom + self.size * 0.5
    }

    pub fn screen_to_world(&self, s: Vec2) -> Vec2 {
        (s - self.size * 0.5) / self.zoom + self.center
    }

    /// Converts a screen distance (points) to world units.
    pub fn screen_to_world_len(&self, len: f32) -> f32 {
        len / self.zoom
    }

    /// The world rectangle currently visible.
    pub fn visible_world(&self) -> Aabb {
        Aabb::new(
            self.screen_to_world(Vec2::ZERO),
            self.screen_to_world(self.size),
        )
    }

    /// Moves the view by a screen delta, so content follows the cursor.
    pub fn pan_by_screen(&mut self, delta: Vec2) {
        self.center -= delta / self.zoom;
    }

    /// Multiplies zoom by `factor`, keeping the world point under `anchor`
    /// (screen points) fixed.
    pub fn zoom_at(&mut self, anchor: Vec2, factor: f32) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let before = self.screen_to_world(anchor);
        self.zoom = (self.zoom * factor).clamp(self.min_zoom, self.max_zoom);
        let after = self.screen_to_world(anchor);
        self.center += before - after;
    }

    /// Centres `bounds` and zooms so it fills the canvas, leaving `margin`
    /// screen points around it.
    pub fn fit(&mut self, bounds: &Aabb, margin: f32) {
        if bounds.is_empty() {
            return;
        }
        let avail = (self.size - Vec2::splat(2.0 * margin)).max(Vec2::ONE);
        let size = bounds.size().max(Vec2::splat(1e-3));
        let z = (avail / size).min_element();
        self.zoom = z.clamp(self.min_zoom, self.max_zoom);
        self.center = bounds.center();
    }

    /// Uniform data for the GPU: maps world to clip space.
    pub(crate) fn clip_transform(&self) -> (Vec2, Vec2) {
        // clip = (world - center) * scale, with y flipped for wgpu (y up in clip).
        let scale = Vec2::new(
            2.0 * self.zoom / self.size.x,
            -2.0 * self.zoom / self.size.y,
        );
        (self.center, scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let v = Viewport {
            center: Vec2::new(100.0, 50.0),
            zoom: 3.0,
            size: Vec2::new(800.0, 600.0),
            ..Default::default()
        };
        let w = Vec2::new(123.0, -45.0);
        assert!(v.screen_to_world(v.world_to_screen(w)).distance(w) < 1e-4);
        assert_eq!(v.world_to_screen(v.center), Vec2::new(400.0, 300.0));
    }

    #[test]
    fn zoom_keeps_anchor_fixed() {
        let mut v = Viewport::new(Vec2::new(800.0, 600.0));
        let anchor = Vec2::new(100.0, 500.0);
        let before = v.screen_to_world(anchor);
        v.zoom_at(anchor, 4.0);
        assert!(v.screen_to_world(anchor).distance(before) < 1e-4);
        assert_eq!(v.zoom, 8.0);
        v.zoom_at(anchor, 1e9);
        assert_eq!(v.zoom, v.max_zoom);
        v.zoom_at(anchor, 0.0);
        assert_eq!(v.zoom, v.max_zoom);
    }

    #[test]
    fn pan_follows_cursor() {
        let mut v = Viewport::new(Vec2::new(800.0, 600.0));
        let w = Vec2::new(10.0, 10.0);
        let s = v.world_to_screen(w);
        v.pan_by_screen(Vec2::new(30.0, -20.0));
        assert!(v.world_to_screen(w).distance(s + Vec2::new(30.0, -20.0)) < 1e-4);
    }

    #[test]
    fn fit_shows_whole_page() {
        let mut v = Viewport::new(Vec2::new(800.0, 600.0));
        // A3 landscape.
        let page = Aabb::new(Vec2::ZERO, Vec2::new(420.0, 297.0));
        v.fit(&page, 20.0);
        let vis = v.visible_world();
        assert!(vis.contains(&page));
        assert!((v.zoom - (760.0 / 420.0)).abs() < 1e-4);
    }
}
