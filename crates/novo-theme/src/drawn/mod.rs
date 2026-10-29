//! The drawn icon set: 40 icons for what Phosphor has no picture for (the
//! workspaces, electrical components, wiring, cabinet, production, workflow
//! and analysis), as plain data on a 24 x 24 grid, and the engine that paints
//! them (docs/design/visual-redesign.md section 9).
//!
//! Rules of the set: stroke 1.5 units, round caps and joins, outline only
//! (the junction dot is the one filled shape), live area 2 to 22, one colour:
//! the colour you pass in, so hover, active and disabled need no extra
//! assets. There is no image loader and no SVG parser in the app; the data
//! in [`set`] is generated from the designer's Python source by
//! `docs/design/visual-redesign/tools/icons_to_rust.py`.
//!
//! ```ignore
//! use novo_theme::icons::custom;
//! custom::paint(ui.painter(), rect, &custom::CONTACTOR, ui.visuals().text_color());
//! ```

use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Shape as Egui, Stroke, Ui, Vec2};

#[rustfmt::skip]
mod set;

mod runtime;

pub use runtime::{
    IconError, IconSpec, MAX_SHAPES, ShapeSpec, clear, lookup, register, registered,
    unregister_prefix, valid_name,
};
pub use set::*;

/// The grid every icon is drawn on.
pub const GRID: f32 = 24.0;
/// Stroke width in grid units: 6.25 percent, the ratio of Phosphor Regular,
/// so the two sets sit together.
pub const STROKE: f32 = 1.5;
/// The sizes the set is checked at, in points.
pub const SIZES: [f32; 8] = [12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 32.0, 48.0];

/// What an icon is for; also the prefix of its code name in the design doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    Workspace,
    Component,
    Wiring,
    Cabinet,
    Production,
    Workflow,
    Analysis,
    /// Registered at run time by an add-on ([`register`]); not part of the
    /// built-in set.
    Addon,
}

impl Group {
    /// The groups of the 40 built-in icons.
    pub const ALL: [Group; 7] = [
        Group::Workspace,
        Group::Component,
        Group::Wiring,
        Group::Cabinet,
        Group::Production,
        Group::Workflow,
        Group::Analysis,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Group::Workspace => "workspace",
            Group::Component => "component",
            Group::Wiring => "wiring",
            Group::Cabinet => "cabinet",
            Group::Production => "production",
            Group::Workflow => "workflow",
            Group::Analysis => "analysis",
            Group::Addon => "addon",
        }
    }
}

/// One drawn element. Coordinates are grid units, y down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// x1 y1 x2 y2.
    Line([f32; 4]),
    /// Points, and whether the last joins the first.
    Poly(&'static [[f32; 2]], bool),
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        filled: bool,
    },
    /// Lines, arcs and curves joined end to end. Each arc starts where the
    /// path is and ends where the next segment starts.
    Path {
        start: [f32; 2],
        segs: &'static [Seg],
        closed: bool,
    },
    /// The shape, drawn as dashes: `on` units of line, `off` units of gap.
    Dashed {
        shape: &'static Shape,
        on: f32,
        off: f32,
    },
}

/// A segment of a [`Shape::Path`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    /// A straight line to the point.
    Line([f32; 2]),
    /// An arc of an ellipse, in the centre form: angles in radians, growing
    /// clockwise on screen (y down). `to` may be below `from` for the other way.
    Arc {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        from: f32,
        to: f32,
    },
    /// A cubic bezier: two control points and the end.
    Cubic([[f32; 2]; 3]),
}

/// One icon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Icon {
    /// Lower-case, hyphenated; the id used by the presentation table.
    pub name: &'static str,
    pub group: Group,
    /// The name shown in a tooltip.
    pub title: &'static str,
    pub shapes: &'static [Shape],
}

impl Icon {
    /// The icon called `name`: a built-in one, or one an add-on registered.
    pub fn by_name(name: &str) -> Option<&'static Icon> {
        ALL.iter()
            .copied()
            .find(|i| i.name == name)
            .or_else(|| lookup(name))
    }

    /// The built-in icons of one group, in set order.
    pub fn of_group(group: Group) -> impl Iterator<Item = &'static Icon> {
        ALL.iter().copied().filter(move |i| i.group == group)
    }

    /// Number of drawn elements; a path counts once per sub-path, a dashed
    /// shape once. The SVG of the icon has the same count.
    pub fn shape_count(&self) -> usize {
        self.shapes.len()
    }

    /// The icon as lines and dots on the 24 grid, sampled finely enough for
    /// `scale` points per grid unit.
    pub fn prims(&self, scale: f32) -> Vec<Prim> {
        self.shapes.iter().flat_map(|s| s.prims(scale)).collect()
    }

    /// The smallest box holding every drawn centre line, in grid units.
    pub fn bounds(&self) -> Rect {
        let mut bounds = Rect::NOTHING;
        for prim in self.prims(8.0) {
            match prim {
                Prim::Stroke { points, .. } => {
                    for p in points {
                        bounds.extend_with(Pos2::new(p[0], p[1]));
                    }
                }
                Prim::Dot { centre, radius } => {
                    bounds.extend_with(Pos2::new(centre[0] - radius, centre[1] - radius));
                    bounds.extend_with(Pos2::new(centre[0] + radius, centre[1] + radius));
                }
            }
        }
        bounds
    }
}

/// What the painter draws: a stroked polyline (optionally dashed) or a
/// filled dot. Grid units.
#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    Stroke {
        points: Vec<[f32; 2]>,
        closed: bool,
        /// Dash and gap lengths.
        dash: Option<[f32; 2]>,
    },
    Dot {
        centre: [f32; 2],
        radius: f32,
    },
}

impl Shape {
    /// The shape as primitives. `scale` is points per grid unit and only
    /// decides how many points curves are sampled with.
    pub fn prims(&self, scale: f32) -> Vec<Prim> {
        let stroke = |points: Vec<[f32; 2]>, closed| Prim::Stroke {
            points,
            closed,
            dash: None,
        };
        match *self {
            Shape::Line([x1, y1, x2, y2]) => vec![stroke(vec![[x1, y1], [x2, y2]], false)],
            Shape::Poly(points, closed) => vec![stroke(points.to_vec(), closed)],
            Shape::Rect { x, y, w, h, r } => vec![stroke(rounded_rect(x, y, w, h, r, scale), true)],
            Shape::Circle {
                cx,
                cy,
                r,
                filled: true,
            } => vec![Prim::Dot {
                centre: [cx, cy],
                radius: r,
            }],
            Shape::Circle { cx, cy, r, .. } => {
                let mut points = Vec::new();
                sample_arc(&mut points, cx, cy, r, r, 0.0, std::f32::consts::TAU, scale);
                points.pop(); // the closed outline repeats the first point
                vec![stroke(points, true)]
            }
            Shape::Path {
                start,
                segs,
                closed,
            } => {
                let mut points = vec![start];
                for seg in segs {
                    match *seg {
                        Seg::Line(p) => points.push(p),
                        Seg::Arc {
                            cx,
                            cy,
                            rx,
                            ry,
                            from,
                            to,
                        } => sample_arc(&mut points, cx, cy, rx, ry, from, to, scale),
                        Seg::Cubic([c1, c2, end]) => {
                            let from = *points.last().unwrap_or(&start);
                            sample_cubic(&mut points, from, c1, c2, end, scale);
                        }
                    }
                }
                if closed && points.len() > 1 && near(points[0], points[points.len() - 1]) {
                    points.pop();
                }
                vec![stroke(points, closed)]
            }
            Shape::Dashed { shape, on, off } => shape
                .prims(scale)
                .into_iter()
                .map(|prim| match prim {
                    Prim::Stroke {
                        mut points, closed, ..
                    } => {
                        if closed && let Some(first) = points.first().copied() {
                            points.push(first); // dashes follow the outline all the way round
                        }
                        Prim::Stroke {
                            points,
                            closed: false,
                            dash: Some([on, off]),
                        }
                    }
                    dot => dot,
                })
                .collect(),
        }
    }
}

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
}

/// Points per arc: about one every 2 points of arc length on screen, never
/// fewer than 6 for a quarter turn.
fn steps(sweep: f32, radius: f32, scale: f32) -> usize {
    let length = sweep.abs() * radius * scale;
    let by_length = (length / 2.0).ceil();
    let by_angle = (sweep.abs() / std::f32::consts::FRAC_PI_2 * 6.0).ceil();
    by_length.max(by_angle).clamp(2.0, 128.0) as usize
}

/// Appends the arc's points, including both ends.
#[allow(clippy::too_many_arguments)]
fn sample_arc(
    out: &mut Vec<[f32; 2]>,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    from: f32,
    to: f32,
    scale: f32,
) {
    let n = steps(to - from, rx.max(ry), scale);
    for i in 0..=n {
        let a = from + (to - from) * i as f32 / n as f32;
        out.push([cx + rx * a.cos(), cy + ry * a.sin()]);
    }
}

fn sample_cubic(
    out: &mut Vec<[f32; 2]>,
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    scale: f32,
) {
    let chord = |a: [f32; 2], b: [f32; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    let length = chord(p0, p1) + chord(p1, p2) + chord(p2, p3);
    let n = ((length * scale / 2.0).ceil() as usize).clamp(4, 64);
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let at = |k: usize| {
            u * u * u * p0[k]
                + 3.0 * u * u * t * p1[k]
                + 3.0 * u * t * t * p2[k]
                + t * t * t * p3[k]
        };
        out.push([at(0), at(1)]);
    }
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32, scale: f32) -> Vec<[f32; 2]> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r <= 0.0 {
        return vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    }
    use std::f32::consts::{FRAC_PI_2, PI};
    let mut out = Vec::new();
    // Corners clockwise from the top left, each a quarter circle.
    for (cx, cy, from) in [
        (x + r, y + r, PI),
        (x + w - r, y + r, PI * 1.5),
        (x + w - r, y + h - r, 0.0),
        (x + r, y + h - r, FRAC_PI_2),
    ] {
        sample_arc(&mut out, cx, cy, r, r, from, from + FRAC_PI_2, scale);
    }
    out
}

/// Stroke width in points for an icon painted `size` points wide: the 1.5
/// unit rule, never below one physical pixel.
pub fn stroke_width(size: f32, pixels_per_point: f32) -> f32 {
    (STROKE * size / GRID).max(1.0 / pixels_per_point.max(0.1))
}

/// Paints `icon` in `rect` (the largest square that fits, centred) in one
/// colour.
pub fn paint(painter: &Painter, rect: Rect, icon: &Icon, color: Color32) {
    let size = rect.width().min(rect.height());
    if size <= 0.0 {
        return;
    }
    let scale = size / GRID;
    let origin = rect.center() - Vec2::splat(size / 2.0);
    let width = stroke_width(size, painter.ctx().pixels_per_point());
    let stroke = Stroke::new(width, color);
    let at = |p: [f32; 2]| Pos2::new(origin.x + p[0] * scale, origin.y + p[1] * scale);
    for prim in icon.prims(scale) {
        match prim {
            Prim::Dot { centre, radius } => {
                // A filled dot is as wide as the line that meets it, at least.
                painter.circle_filled(at(centre), (radius * scale).max(width * 0.75), color);
            }
            Prim::Stroke {
                points,
                closed,
                dash: None,
            } => {
                let points: Vec<Pos2> = points.into_iter().map(at).collect();
                if !closed {
                    // Round caps: egui strokes end square-cut.
                    for end in [points.first(), points.last()].into_iter().flatten() {
                        painter.circle_filled(*end, width / 2.0, color);
                    }
                    painter.add(Egui::line(points, stroke));
                } else {
                    painter.add(Egui::closed_line(points, stroke));
                }
            }
            Prim::Stroke {
                points,
                dash: Some([on, off]),
                ..
            } => {
                let points: Vec<Pos2> = points.into_iter().map(at).collect();
                painter.extend(Egui::dashed_line(&points, stroke, on * scale, off * scale));
            }
        }
    }
}

/// Reserves a square of `size` points, paints the icon in it and returns the
/// response (hover only; wrap it in a button for clicks).
pub fn show(ui: &mut Ui, icon: &Icon, size: f32, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), rect, icon, color);
    }
    response.on_hover_text(icon.title)
}

#[cfg(test)]
mod tests;
