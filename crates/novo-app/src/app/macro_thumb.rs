//! The picture of a macro in the browser: its symbols (from the IEC symbol
//! library, a box around the pins when a key is unknown), wires and
//! junctions, scaled to fit. Geometry comes from
//! [`novo_parts::thumbnail`]; this file only fits and paints it.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use novo_core::Point;
use novo_parts::Thumbnail;
use novo_symbols::{Graphic, Library, Pack};

/// Scale and offset that fit `bounds` (mm) into `size` (points) with
/// `margin` around, keeping the aspect ratio and centring. `None` for a
/// degenerate box.
pub(super) fn fit(
    bounds: (Point, Point),
    size: egui::Vec2,
    margin: f32,
) -> Option<(f32, egui::Vec2)> {
    let (min, max) = bounds;
    let (w, h) = ((max.x - min.x) as f32, (max.y - min.y) as f32);
    if w <= 0.0 || h <= 0.0 || size.x <= 2.0 * margin || size.y <= 2.0 * margin {
        return None;
    }
    let scale = ((size.x - 2.0 * margin) / w).min((size.y - 2.0 * margin) / h);
    let offset = egui::vec2(
        (size.x - w * scale) / 2.0 - min.x as f32 * scale,
        (size.y - h * scale) / 2.0 - min.y as f32 * scale,
    );
    Some((scale, offset))
}

/// Paints `thumb` into `rect`.
pub(super) fn paint(ui: &egui::Ui, rect: Rect, thumb: &Thumbnail) {
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, 4.0, visuals.extreme_bg_color);
    painter.rect_stroke(
        rect,
        4.0,
        visuals.widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let Some(bounds) = thumb.bounds else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Empty macro",
            egui::FontId::proportional(12.0),
            visuals.weak_text_color(),
        );
        return;
    };
    let Some((scale, offset)) = fit(bounds, rect.size(), 8.0) else {
        return;
    };
    let to = |x: f64, y: f64| {
        Pos2::new(
            rect.min.x + x as f32 * scale + offset.x,
            rect.min.y + y as f32 * scale + offset.y,
        )
    };
    let ink = visuals.text_color();
    let wire = Stroke::new(1.0, visuals.widgets.active.fg_stroke.color);
    let line = Stroke::new(1.0, ink);
    let library = Library::shared(Pack::Iec);

    for w in &thumb.wires {
        let pts: Vec<Pos2> = w.iter().map(|p| to(p.x, p.y)).collect();
        painter.add(egui::Shape::line(pts, wire));
    }
    for s in &thumb.symbols {
        let map = |p: [f64; 2]| {
            let q = s.placement.apply(Point::new(p[0], p[1]));
            to(q.x, q.y)
        };
        let Some(symbol) = library.get(&s.key) else {
            fallback_box(&painter, s, &to, line);
            continue;
        };
        for g in &symbol.graphics {
            match g {
                Graphic::Line { from, to: end, .. } => {
                    painter.line_segment([map(*from), map(*end)], line);
                }
                Graphic::Polyline { points, closed, .. } => {
                    let pts: Vec<Pos2> = points.iter().map(|p| map(*p)).collect();
                    painter.add(egui::Shape::Path(egui::epaint::PathShape {
                        points: pts,
                        closed: *closed,
                        fill: Color32::TRANSPARENT,
                        stroke: line.into(),
                    }));
                }
                Graphic::Rect { at, size, .. } => {
                    let c = [
                        *at,
                        [at[0] + size[0], at[1]],
                        [at[0] + size[0], at[1] + size[1]],
                        [at[0], at[1] + size[1]],
                    ];
                    painter.add(egui::Shape::closed_line(
                        c.iter().map(|p| map(*p)).collect(),
                        line,
                    ));
                }
                Graphic::Circle { center, r, .. } => {
                    painter.circle_stroke(map(*center), (*r as f32) * scale, line);
                }
                Graphic::Ellipse { center, rx, ry, .. } => {
                    let r = (*rx).max(*ry);
                    painter.circle_stroke(map(*center), r as f32 * scale, line);
                }
                Graphic::Arc {
                    center,
                    r,
                    start,
                    end,
                    ..
                } => {
                    let pts: Vec<Pos2> = (0..=16)
                        .map(|i| {
                            let a = (start + (end - start) * f64::from(i) / 16.0).to_radians();
                            map([center[0] + r * a.cos(), center[1] + r * a.sin()])
                        })
                        .collect();
                    painter.add(egui::Shape::line(pts, line));
                }
                Graphic::Text { .. } => {}
            }
        }
    }
    for j in &thumb.junctions {
        painter.circle_filled(to(j.x, j.y), 2.0, ink);
    }
}

/// A box around a symbol the library does not know, so the circuit still
/// shows its shape.
fn fallback_box(
    painter: &egui::Painter,
    s: &novo_parts::ThumbSymbol,
    to: &impl Fn(f64, f64) -> Pos2,
    stroke: Stroke,
) {
    let mut min = Point::new(s.placement.origin.x, s.placement.origin.y);
    let mut max = min;
    for p in &s.pins {
        min = Point::new(min.x.min(p.x), min.y.min(p.y));
        max = Point::new(max.x.max(p.x), max.y.max(p.y));
    }
    painter.rect_stroke(
        Rect::from_two_pos(to(min.x, min.y), to(max.x, max.y)),
        0.0,
        stroke,
        egui::StrokeKind::Middle,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_box_fits_by_width_and_centres_vertically() {
        let b = (Point::new(0.0, 0.0), Point::new(100.0, 20.0));
        let (scale, off) = fit(b, egui::vec2(216.0, 100.0), 8.0).unwrap();
        assert!((scale - 2.0).abs() < 1e-6);
        assert!((off.x - 8.0).abs() < 1e-4);
        // 20 mm * 2 = 40 high in 100: 30 above.
        assert!((off.y - 30.0).abs() < 1e-4);
    }

    #[test]
    fn a_box_not_at_the_origin_still_lands_inside() {
        let b = (Point::new(50.0, 60.0), Point::new(70.0, 80.0));
        let size = egui::vec2(100.0, 100.0);
        let (scale, off) = fit(b, size, 10.0).unwrap();
        let at = |x: f64, y: f64| (x as f32 * scale + off.x, y as f32 * scale + off.y);
        let (x0, y0) = at(50.0, 60.0);
        let (x1, y1) = at(70.0, 80.0);
        assert!(x0 >= 10.0 - 1e-3 && y0 >= 10.0 - 1e-3 && x1 <= 90.0 + 1e-3 && y1 <= 90.0 + 1e-3);
    }

    #[test]
    fn a_degenerate_box_does_not_fit() {
        let b = (Point::new(1.0, 1.0), Point::new(1.0, 5.0));
        assert!(fit(b, egui::vec2(100.0, 100.0), 8.0).is_none());
    }
}
