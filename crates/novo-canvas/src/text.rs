//! Text labels (device tags, terminal numbers, page texts).
//!
//! Labels are positioned in world units and scale with zoom like the rest of
//! the drawing. The egui widget renders them with egui's font atlas on top
//! of the GPU geometry and skips labels too small to read. Picking and box
//! selection use an estimated text box, so they work without fonts.

use crate::geom::{self, Aabb};
use crate::style::Color;
use glam::Vec2;

/// Which point of the text box sits at [`Label::pos`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    #[default]
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    /// Fractions of the box (0, 0.5 or 1) along x and y.
    pub fn fractions(self) -> Vec2 {
        use Anchor::*;
        let x = match self {
            TopLeft | CenterLeft | BottomLeft => 0.0,
            TopCenter | Center | BottomCenter => 0.5,
            TopRight | CenterRight | BottomRight => 1.0,
        };
        let y = match self {
            TopLeft | TopCenter | TopRight => 0.0,
            CenterLeft | Center | CenterRight => 0.5,
            BottomLeft | BottomCenter | BottomRight => 1.0,
        };
        Vec2::new(x, y)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub pos: Vec2,
    pub text: String,
    /// Font size in world units (mm). IEC drawings typically use 2.5 or 3.5.
    pub height: f32,
    pub color: Color,
    pub anchor: Anchor,
    /// Radians, clockwise on screen (`-FRAC_PI_2` reads bottom to top).
    pub rotation: f32,
    /// Keeps its own colour while its item is hovered or selected (a
    /// warning should not disappear the moment the user looks at it).
    pub keep_color: bool,
    /// Drawn bold (a table header). The screen fakes it with a second pass
    /// shifted sideways; a print backend picks a bold face.
    pub bold: bool,
    /// What is hidden first when the screen is too small to read everything
    /// (see [`crate::declutter`]).
    pub priority: crate::declutter::Priority,
    /// Meant to sit on its own item's drawing (the `M` inside a motor, the
    /// cells of a contact table). The text audit does not count such a label
    /// over its own item; every other label does.
    pub inside: bool,
}

/// Average glyph advance relative to the font size, used for the estimated
/// text box.
pub const ADVANCE: f32 = 0.6;

/// Average glyph advance of bold text, relative to the font size. Bold
/// faces run wider than regular ones (Helvetica-Bold about 10%, Inter
/// SemiBold about 5%), and the faux-bold pass adds [`BOLD_SHIFT`]; this
/// covers all three.
pub const ADVANCE_BOLD: f32 = 0.66;

/// How far the faux-bold second pass is shifted along the baseline,
/// relative to the font size, when no bold face is loaded.
pub const BOLD_SHIFT: f32 = 0.04;

/// [`ADVANCE`], or [`ADVANCE_BOLD`] for bold text.
pub fn advance(bold: bool) -> f32 {
    if bold { ADVANCE_BOLD } else { ADVANCE }
}

impl Label {
    /// [`ADVANCE`], or [`ADVANCE_BOLD`] for bold text.
    pub fn advance(&self) -> f32 {
        advance(self.bold)
    }

    pub fn new(pos: impl Into<Vec2>, text: impl Into<String>, height: f32) -> Self {
        Self {
            pos: pos.into(),
            text: text.into(),
            height,
            color: Color::BLACK,
            anchor: Anchor::BottomLeft,
            rotation: 0.0,
            keep_color: false,
            bold: false,
            priority: crate::declutter::Priority::Primary,
            inside: false,
        }
    }

    /// Marks the label as part of its item's drawing (see [`Label::inside`]).
    pub fn inside(mut self) -> Self {
        self.inside = true;
        self
    }

    /// Marks the label as secondary text: the first to go when the drawing
    /// is too small to read it.
    pub fn secondary(mut self) -> Self {
        self.priority = crate::declutter::Priority::Secondary;
        self
    }

    /// See [`Label::keep_color`].
    pub fn keeping_color(mut self) -> Self {
        self.keep_color = true;
        self
    }

    /// See [`Label::bold`].
    pub fn bolded(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    pub fn anchored(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    pub fn rotated(mut self, rotation: f32) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn colored(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Estimated unrotated size of the text box in world units.
    pub fn size(&self) -> Vec2 {
        let lines = self.text.lines().count().max(1);
        let cols = self
            .text
            .lines()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0);
        Vec2::new(
            cols as f32 * self.height * self.advance(),
            lines as f32 * self.height,
        )
    }

    /// Corners of the (rotated) text box, clockwise from the top left.
    pub fn corners(&self) -> [Vec2; 4] {
        let size = self.size();
        let origin = -self.anchor.fractions() * size;
        let rot = Vec2::from_angle(self.rotation);
        let local = [
            origin,
            origin + Vec2::new(size.x, 0.0),
            origin + size,
            origin + Vec2::new(0.0, size.y),
        ];
        local.map(|p| self.pos + rot.rotate(p))
    }

    pub fn bounds(&self) -> Aabb {
        Aabb::from_points(self.corners())
    }

    /// Distance from `p` to the text box (zero inside).
    pub fn distance(&self, p: Vec2) -> f32 {
        let c = self.corners();
        if geom::point_in_polygon(p, &c) {
            return 0.0;
        }
        (0..4)
            .map(|i| geom::dist_point_segment(p, c[i], c[(i + 1) % 4]))
            .fold(f32::INFINITY, f32::min)
    }

    /// True when the text box touches `rect`.
    pub fn intersects(&self, rect: &Aabb) -> bool {
        let c = self.corners();
        if !Aabb::from_points(c).intersects(rect) {
            return false;
        }
        geom::point_in_polygon(rect.center(), &c)
            || c.iter().any(|p| rect.contains_point(*p))
            || (0..4).any(|i| geom::segment_intersects_aabb(c[i], c[(i + 1) % 4], rect))
    }

    /// Applies a placement to the label position and rotation. Mirroring
    /// never mirrors glyphs (that would be unreadable): the text keeps
    /// reading left to right or bottom to top, and the anchor moves so the
    /// box lands on the mirrored side.
    pub fn transformed(&self, t: &crate::shape::Placement) -> Label {
        use Anchor::*;
        let mut l = self.clone();
        l.pos = t.affine().transform_point2(self.pos);
        l.height *= t.scale;
        if t.mirror {
            // Text direction after mirroring about the local y axis.
            let mirrored = std::f32::consts::PI - self.rotation;
            let leftward = mirrored.cos() < -1e-4;
            if leftward {
                // Turn it around and swap the anchor along the text.
                l.rotation = mirrored - std::f32::consts::PI;
                l.anchor = match self.anchor {
                    TopLeft => TopRight,
                    TopRight => TopLeft,
                    CenterLeft => CenterRight,
                    CenterRight => CenterLeft,
                    BottomLeft => BottomRight,
                    BottomRight => BottomLeft,
                    a => a,
                };
            } else {
                // Same reading direction; the box flips across the text line.
                l.rotation = (mirrored + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                l.anchor = match self.anchor {
                    TopLeft => BottomLeft,
                    BottomLeft => TopLeft,
                    TopCenter => BottomCenter,
                    BottomCenter => TopCenter,
                    TopRight => BottomRight,
                    BottomRight => TopRight,
                    a => a,
                };
            }
        }
        l.rotation += t.rotation;
        l
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn box_follows_anchor() {
        let l = Label::new(Vec2::new(10.0, 10.0), "-K1", 2.5);
        let b = l.bounds();
        // Bottom left at the anchor: the box extends right and up.
        assert!((b.min.x - 10.0).abs() < 1e-5 && (b.max.y - 10.0).abs() < 1e-5);
        assert!((b.size().x - 3.0 * 2.5 * ADVANCE).abs() < 1e-5);
        let c = l.clone().anchored(Anchor::Center).bounds();
        assert!(c.center().distance(Vec2::new(10.0, 10.0)) < 1e-5);
    }

    /// Bold text is estimated wider, whichever way it is drawn (a bold
    /// face or the faux-bold pass), and covers the faux shift.
    #[test]
    fn bold_text_is_estimated_wider() {
        let l = Label::new(Vec2::ZERO, "Header", 2.5);
        let b = l.clone().bolded(true);
        assert!((b.size().x - 6.0 * 2.5 * ADVANCE_BOLD).abs() < 1e-5);
        assert!(b.size().x - l.size().x >= BOLD_SHIFT * 2.5);
    }

    #[test]
    fn rotation_turns_the_box() {
        let l = Label::new(Vec2::ZERO, "ABCD", 2.0).rotated(-FRAC_PI_2);
        let b = l.bounds();
        // Reading bottom to top: the long side is now vertical.
        assert!(b.size().y > b.size().x);
        assert!(b.max.y <= 1e-5);
    }

    #[test]
    fn picking_and_crossing() {
        let l = Label::new(Vec2::ZERO, "-Q1", 3.0);
        assert_eq!(l.distance(Vec2::new(1.0, -1.0)), 0.0);
        assert!((l.distance(Vec2::new(-2.0, -1.0)) - 2.0).abs() < 1e-5);
        assert!(l.intersects(&Aabb::new(Vec2::new(1.0, -2.0), Vec2::new(30.0, -1.0))));
        assert!(!l.intersects(&Aabb::new(Vec2::new(20.0, 0.0), Vec2::new(30.0, 5.0))));
    }

    #[test]
    fn mirror_keeps_text_readable() {
        let l = Label::new(Vec2::new(5.0, 0.0), "X", 2.0);
        let t = crate::shape::Placement {
            mirror: true,
            ..Default::default()
        };
        let m = l.transformed(&t);
        assert_eq!(m.pos, Vec2::new(-5.0, 0.0));
        assert_eq!(m.anchor, Anchor::BottomRight);
        assert!(m.rotation.abs() < 1e-5);
        // Vertical text stays bottom to top; its box moves to the other side.
        let v = Label::new(Vec2::new(5.0, 0.0), "XYZ", 2.0).rotated(-FRAC_PI_2);
        let m = v.transformed(&t);
        assert!((m.rotation + FRAC_PI_2).abs() < 1e-5);
        let (b0, b1) = (v.bounds(), m.bounds());
        assert!(
            (b0.min.x + b1.max.x).abs() < 1e-4,
            "{b0:?} mirrors to {b1:?}"
        );
    }
}
