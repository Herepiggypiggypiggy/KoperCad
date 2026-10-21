//! Canvas primitives and labels to a PDF content stream.
//!
//! The stream first sets a transform from page millimetres (y down, origin
//! top left, as on the canvas) to PDF points (y up, origin bottom left), so
//! every coordinate below is written in the drawing's own millimetres.

use crate::font;
use crate::inter::{self, Inter};
use novo_canvas::{Color, Label, Primitive, Shape, Stroke, Vec2, WidthUnit};
use pdf_writer::types::{LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Name, Str};
use std::f32::consts::{FRAC_PI_2, TAU};

/// PDF points per millimetre.
pub const PT_PER_MM: f32 = 72.0 / 25.4;

/// Millimetres per point, for screen-unit strokes (which are in points).
const MM_PER_PT: f32 = 25.4 / 72.0;

/// Writes one page.
pub struct PageWriter<'a> {
    content: Content,
    stroke: Option<(Color, f32)>,
    fill: Option<Color>,
    /// The export's Inter subset, for lines outside WinAnsi.
    inter: &'a mut Inter,
    uses_inter: bool,
    uses_bold: bool,
}

impl<'a> PageWriter<'a> {
    /// A new page of `height_mm`, with the millimetre transform set.
    pub fn new(height_mm: f32, inter: &'a mut Inter) -> Self {
        let mut content = Content::new();
        content.transform([PT_PER_MM, 0.0, 0.0, -PT_PER_MM, 0.0, height_mm * PT_PER_MM]);
        // Wiring diagrams read best with round ends and corners, like pen
        // plots; it also closes the tiny gaps where wires meet pins.
        content.set_line_cap(LineCapStyle::RoundCap);
        content.set_line_join(LineJoinStyle::RoundJoin);
        Self {
            content,
            stroke: None,
            fill: None,
            inter,
            uses_inter: false,
            uses_bold: false,
        }
    }

    /// Whether the page set any text in Inter (its resources then need the
    /// font).
    pub fn uses_inter(&self) -> bool {
        self.uses_inter
    }

    /// Whether the page set any text in Helvetica-Bold.
    pub fn uses_bold(&self) -> bool {
        self.uses_bold
    }

    /// Restricts everything drawn until [`PageWriter::end_clip`] to the
    /// rectangle (page millimetres).
    pub fn begin_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.content.save_state();
        self.content.rect(x, y, w, h);
        self.content.clip_nonzero();
        self.content.end_path();
    }

    /// Ends the clip of [`PageWriter::begin_clip`].
    pub fn end_clip(&mut self) {
        self.content.restore_state();
        // The restore also undid the colours and widths set inside the clip.
        self.stroke = None;
        self.fill = None;
    }

    pub fn finish(self) -> Vec<u8> {
        self.content.finish().to_vec()
    }

    fn set_stroke(&mut self, stroke: &Stroke) {
        let width = match stroke.unit {
            WidthUnit::World => stroke.width,
            WidthUnit::Screen => stroke.width * MM_PER_PT,
        };
        if self.stroke == Some((stroke.color, width)) {
            return;
        }
        let [r, g, b] = rgb(stroke.color);
        self.content.set_stroke_rgb(r, g, b);
        self.content.set_line_width(width);
        self.stroke = Some((stroke.color, width));
    }

    fn set_fill(&mut self, color: Color) {
        if self.fill == Some(color) {
            return;
        }
        let [r, g, b] = rgb(color);
        self.content.set_fill_rgb(r, g, b);
        self.fill = Some(color);
    }

    /// Draws one primitive. Fills apply to closed shapes only, as on the
    /// canvas; fully transparent strokes and fills are skipped.
    pub fn primitive(&mut self, p: &Primitive) {
        let closed = matches!(
            p.shape,
            Shape::Rect { .. } | Shape::Circle { .. } | Shape::Polyline { closed: true, .. }
        );
        let fill = p.fill.filter(|c| closed && c.0[3] > 0);
        let stroke = p.stroke.filter(|s| s.color.0[3] > 0 && s.width > 0.0);
        if fill.is_none() && stroke.is_none() {
            return;
        }
        if let Some(c) = fill {
            self.set_fill(c);
        }
        if let Some(s) = &stroke {
            self.set_stroke(s);
        }
        if !self.path(&p.shape) {
            return;
        }
        match (fill.is_some(), stroke.is_some()) {
            (true, true) => self.content.fill_nonzero_and_stroke(),
            (true, false) => self.content.fill_nonzero(),
            _ => self.content.stroke(),
        };
    }

    /// Builds the path of a shape. Returns false when there is nothing to
    /// draw (a polyline without points).
    fn path(&mut self, shape: &Shape) -> bool {
        let c = &mut self.content;
        match shape {
            Shape::Line { a, b } => {
                c.move_to(a.x, a.y);
                c.line_to(b.x, b.y);
            }
            Shape::Polyline { points, closed } => {
                let Some(first) = points.first() else {
                    return false;
                };
                c.move_to(first.x, first.y);
                if points.len() == 1 {
                    // A dot: a zero-length segment shows as a round cap.
                    c.line_to(first.x, first.y);
                }
                for p in &points[1..] {
                    c.line_to(p.x, p.y);
                }
                if *closed {
                    c.close_path();
                }
            }
            Shape::Rect { min, max } => {
                c.rect(min.x, min.y, max.x - min.x, max.y - min.y);
            }
            Shape::Circle { center, radius } => {
                arc_path(c, *center, *radius, 0.0, TAU, true);
                c.close_path();
            }
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => arc_path(c, *center, *radius, *start, *sweep, true),
        }
        true
    }

    /// Draws a label in Helvetica, anchored and rotated like the canvas
    /// draws it. Lines are aligned within the text box by the anchor. A
    /// line Helvetica cannot carry is set in Inter (see [`inter`]). A bold
    /// label is set in Helvetica-Bold (Inter has no bold here, so a bold
    /// line outside WinAnsi prints regular).
    pub fn label(&mut self, label: &Label) {
        let size = label.height;
        if size <= 0.0 || label.color.0[3] == 0 || label.text.trim().is_empty() {
            return;
        }
        // (font, encoded line, width in mm)
        let lines: Vec<(&[u8], Vec<u8>, f32)> = label
            .text
            .lines()
            .map(|line| {
                if font::is_winansi(line) && label.bold {
                    let bytes = font::encode(line);
                    let w = font::bold_text_width(&bytes) * size;
                    self.uses_bold = true;
                    (font::BOLD_NAME, bytes, w)
                } else if font::is_winansi(line) {
                    let bytes = font::encode(line);
                    let w = font::text_width(&bytes) * size;
                    (font::FONT_NAME, bytes, w)
                } else {
                    let (bytes, w) = self.inter.encode(line);
                    self.uses_inter = true;
                    (inter::FONT_NAME, bytes, w * size)
                }
            })
            .collect();
        let widths: Vec<f32> = lines.iter().map(|l| l.2).collect();
        let box_w = widths.iter().copied().fold(0.0, f32::max);
        let box_h = lines.len() as f32 * size;
        let frac = label.anchor.fractions();
        let origin = Vec2::new(-frac.x * box_w, -frac.y * box_h);
        // Along the text and down the lines, in page millimetres (y down).
        let u = Vec2::from_angle(label.rotation);
        let v = Vec2::new(-u.y, u.x);
        self.set_fill(label.color);
        let c = &mut self.content;
        c.begin_text();
        let mut current: Option<&[u8]> = None;
        for (i, ((name, line, _), w)) in lines.iter().zip(&widths).enumerate() {
            if line.is_empty() {
                continue;
            }
            if current != Some(*name) {
                c.set_font(Name(name), 1.0);
                current = Some(*name);
            }
            let local =
                origin + Vec2::new(frac.x * (box_w - w), (i as f32 + font::BASELINE) * size);
            let at = label.pos + u * local.x + v * local.y;
            // Glyph space is y up; on this y-down page "up" is -v.
            c.set_text_matrix([size * u.x, size * u.y, -size * v.x, -size * v.y, at.x, at.y]);
            c.show(Str(line));
        }
        c.end_text();
    }
}

fn rgb(c: Color) -> [f32; 3] {
    let [r, g, b, _] = c.0;
    [r, g, b].map(|v| f32::from(v) / 255.0)
}

/// Appends a circular arc as cubic Béziers of at most a quarter turn each.
/// Angles run from +x towards +y (clockwise on the y-down page), as in
/// [`Shape::Arc`].
fn arc_path(c: &mut Content, center: Vec2, radius: f32, start: f32, sweep: f32, move_first: bool) {
    let at = |a: f32| center + Vec2::new(a.cos(), a.sin()) * radius;
    let tangent = |a: f32| Vec2::new(-a.sin(), a.cos()) * radius;
    let p0 = at(start);
    if move_first {
        c.move_to(p0.x, p0.y);
    }
    let segments = (sweep.abs() / FRAC_PI_2).ceil().max(1.0) as usize;
    let step = sweep / segments as f32;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    for i in 0..segments {
        let a0 = start + step * i as f32;
        let a1 = a0 + step;
        let (q0, q3) = (at(a0), at(a1));
        let q1 = q0 + tangent(a0) * k;
        let q2 = q3 - tangent(a1) * k;
        c.cubic_to(q1.x, q1.y, q2.x, q2.y, q3.x, q3.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_canvas::Anchor;

    fn text(writer: PageWriter) -> String {
        String::from_utf8_lossy(&writer.finish()).into_owned()
    }

    #[test]
    fn page_starts_with_the_millimetre_transform() {
        let mut inter = Inter::new();
        let s = text(PageWriter::new(297.0, &mut inter));
        let first = s.lines().next().unwrap();
        assert!(first.ends_with(" cm"), "{first}");
        assert!(first.starts_with("2.83"), "{first}");
    }

    #[test]
    fn circles_are_four_beziers_and_filled_when_asked() {
        let mut inter = Inter::new();
        let mut w = PageWriter::new(100.0, &mut inter);
        w.primitive(&Primitive::filled(
            Shape::circle(Vec2::new(10.0, 10.0), 1.0),
            Color::BLACK,
        ));
        let s = text(w);
        assert_eq!(s.lines().filter(|l| l.ends_with(" c")).count(), 4, "{s}");
        assert_eq!(s.lines().last(), Some("f"), "{s}");
    }

    #[test]
    fn fills_on_open_shapes_are_ignored() {
        let mut inter = Inter::new();
        let mut w = PageWriter::new(100.0, &mut inter);
        w.primitive(
            &Primitive::stroked(Shape::line(Vec2::ZERO, Vec2::ONE), Stroke::default())
                .with_fill(Color::BLACK),
        );
        let s = text(w);
        assert_eq!(s.lines().last(), Some("S"), "{s}");
    }

    #[test]
    fn bold_labels_are_set_in_helvetica_bold() {
        let mut inter = Inter::new();
        let mut w = PageWriter::new(100.0, &mut inter);
        w.label(&Label::new(Vec2::ZERO, "Qty", 2.5).bolded(true));
        w.label(&Label::new(Vec2::ZERO, "12", 2.5));
        assert!(w.uses_bold());
        assert!(!w.uses_inter());
        let s = text(w);
        let fonts: Vec<&str> = s.lines().filter(|l| l.ends_with(" Tf")).collect();
        assert_eq!(fonts, ["/F3 1 Tf", "/F1 1 Tf"], "{s}");
    }

    #[test]
    fn label_text_is_written_and_anchored() {
        let mut inter = Inter::new();
        let mut w = PageWriter::new(100.0, &mut inter);
        w.label(&Label::new(Vec2::new(50.0, 20.0), "-K1", 2.5).anchored(Anchor::BottomRight));
        let s = text(w);
        assert!(s.contains("(-K1) Tj"), "{s}");
        // Right-anchored: the text starts left of the anchor point.
        let tm = s.lines().find(|l| l.ends_with(" Tm")).unwrap();
        let x: f32 = tm.split(' ').nth(4).unwrap().parse().unwrap();
        let width = font::text_width(b"-K1") * 2.5;
        assert!((x - (50.0 - width)).abs() < 1e-3, "{tm}");
    }

    #[test]
    fn a_line_outside_winansi_switches_to_inter_and_back() {
        let mut inter = Inter::new();
        let mut w = PageWriter::new(100.0, &mut inter);
        w.label(&Label::new(Vec2::ZERO, "-R1\n10 kΩ\n5 W", 2.5));
        assert!(w.uses_inter());
        let s = text(w);
        let fonts: Vec<&str> = s.lines().filter(|l| l.ends_with(" Tf")).collect();
        assert_eq!(fonts, ["/F1 1 Tf", "/F2 1 Tf", "/F1 1 Tf"], "{s}");
        assert!(s.contains("(-R1) Tj"), "{s}");
        assert!(s.contains("(5 W) Tj"), "{s}");
    }

    #[test]
    fn arcs_split_into_quarter_turns() {
        let mut inter = Inter::new();
        let mut w = PageWriter::new(100.0, &mut inter);
        w.primitive(&Primitive::stroked(
            Shape::Arc {
                center: Vec2::ZERO,
                radius: 5.0,
                start: 0.0,
                sweep: -3.0,
            },
            Stroke::default(),
        ));
        assert_eq!(text(w).lines().filter(|l| l.ends_with(" c")).count(), 2);
    }
}
