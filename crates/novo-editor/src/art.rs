//! Real IEC 60617 artwork on the canvas: draws placed symbols from the WS-05
//! library instead of the canvas placeholder boxes.

use std::collections::HashMap;

use novo_canvas::model::SymbolArt;
use novo_canvas::{Anchor, Color, Label, Primitive, Shape, Stroke, Vec2};
use novo_core::SymbolInstance;
use novo_symbols::{Align, Graphic, Library, Symbol};

/// Key of the labelled box the importers draw for a symbol with no library match.
pub const BOX_KEY: &str = "import/box";

/// Dash and gap length of dashed artwork lines (mechanical links), in mm.
const DASH: f32 = 1.5;
const GAP: f32 = 1.0;

/// Symbol artwork from a symbol library, converted once per symbol.
pub struct LibraryArt {
    stroke: Stroke,
    /// Connection points of each symbol, in library order.
    pins: HashMap<String, Vec<Vec2>>,
    shapes: HashMap<String, Vec<Primitive>>,
    labels: HashMap<String, Vec<Label>>,
    tags: HashMap<String, (Vec2, Anchor)>,
}

impl LibraryArt {
    /// Converts every symbol of `library` with the given line style.
    pub fn new(library: &Library, stroke: Stroke) -> Self {
        let pins = library
            .iter()
            .map(|s| {
                (
                    s.id.clone(),
                    s.connections.iter().map(|c| v(c.at)).collect(),
                )
            })
            .collect();
        let shapes = library
            .iter()
            .map(|s| (s.id.clone(), primitives(s, stroke)))
            .collect();
        let labels = library
            .iter()
            .map(|s| (s.id.clone(), labels(s)))
            .filter(|(_, l)| !l.is_empty())
            .collect();
        let tags = library
            .iter()
            .filter_map(|s| {
                let a = s.anchors.iter().find(|a| a.role == "tag")?;
                Some((s.id.clone(), (v(a.at), anchor(a.align))))
            })
            .collect();
        LibraryArt {
            stroke,
            pins,
            shapes,
            labels,
            tags,
        }
    }
}

/// How far (mm) a pin may sit from the library's before the artwork follows it.
const PIN_TOLERANCE: f32 = 0.05;

impl LibraryArt {
    /// The scale that fits the library artwork to the pins of this instance,
    /// and the library pins; `None` when the pins are where the library has
    /// them (the usual case) or are not the library's pins at all.
    ///
    /// Pins that sit elsewhere (a symbol imported from a drawing with its own
    /// pin pitch) scale the artwork about the symbol origin, so a 16 mm
    /// contact drawn with 8 mm between its pins is a smaller contact, not one
    /// whose leads miss the pins.
    fn fit(&self, symbol: &SymbolInstance) -> Option<(f32, &[Vec2])> {
        let lib = self.pins.get(&symbol.symbol.0)?;
        if lib.len() != symbol.pins.len() {
            return None;
        }
        let at = |i: usize| v([symbol.pins[i].offset.x, symbol.pins[i].offset.y]);
        if (0..lib.len()).all(|i| (at(i) - lib[i]).length() <= PIN_TOLERANCE) {
            return None;
        }
        let (dot, norm) = (0..lib.len()).fold((0.0f32, 0.0f32), |(d, n), i| {
            (d + at(i).dot(lib[i]), n + lib[i].dot(lib[i]))
        });
        let scale = if norm > 1e-3 {
            (dot / norm).clamp(0.2, 5.0)
        } else {
            1.0
        };
        Some((scale, lib))
    }
}

fn scaled(shape: &Shape, k: f32) -> Shape {
    match shape {
        Shape::Line { a, b } => Shape::Line {
            a: *a * k,
            b: *b * k,
        },
        Shape::Polyline { points, closed } => Shape::Polyline {
            points: points.iter().map(|p| *p * k).collect(),
            closed: *closed,
        },
        Shape::Rect { min, max } => Shape::Rect {
            min: *min * k,
            max: *max * k,
        },
        Shape::Circle { center, radius } => Shape::Circle {
            center: *center * k,
            radius: radius * k,
        },
        Shape::Arc {
            center,
            radius,
            start,
            sweep,
        } => Shape::Arc {
            center: *center * k,
            radius: radius * k,
            start: *start,
            sweep: *sweep,
        },
    }
}

/// How far (mm) a pin's lead reaches into a labelled box.
const LEAD: f32 = 3.0;
/// The smallest a labelled box is drawn.
const BOX_MIN: f32 = 3.5;

/// Properties an importer sets on a box that has a drawn size of its own
/// (a busbar frame, a black box, a PLC box): the rectangle's corners in
/// symbol coordinates, millimetres, and whether the frame is dashed.
pub const BOX_X0: &str = "box.x0";
pub const BOX_Y0: &str = "box.y0";
pub const BOX_X1: &str = "box.x1";
pub const BOX_Y1: &str = "box.y1";
pub const BOX_DASHED: &str = "box.dashed";

/// The rectangle a box was given by its source, when it has one.
fn given_frame(symbol: &SymbolInstance) -> Option<(Vec2, Vec2)> {
    let get = |k: &str| match symbol.properties.get(k)? {
        novo_core::Value::Float(f) if f.is_finite() => Some(*f as f32),
        novo_core::Value::Int(i) => Some(*i as f32),
        _ => None,
    };
    let (x0, y0, x1, y1) = (get(BOX_X0)?, get(BOX_Y0)?, get(BOX_X1)?, get(BOX_Y1)?);
    Some((
        Vec2::new(x0.min(x1), y0.min(y1)),
        Vec2::new(x0.max(x1), y0.max(y1)),
    ))
}

/// The outline of a box with a given rectangle, dashed when it says so.
fn frame_art(symbol: &SymbolInstance, min: Vec2, max: Vec2, stroke: Stroke) -> Vec<Primitive> {
    let dashed = matches!(
        symbol.properties.get(BOX_DASHED),
        Some(novo_core::Value::Bool(true))
    );
    if !dashed {
        return vec![Primitive::stroked(Shape::rect(min, max), stroke)];
    }
    let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
    dashes(&Shape::polyline(corners, true))
        .into_iter()
        .map(|s| Primitive::stroked(s, stroke))
        .collect()
}

/// The artwork of a symbol that has no library drawing: a box with each pin
/// on the side it faces and a short lead from the pin to the box. Pins that
/// face up sit on the top edge, so two rows of pins (the poles of a
/// breaker) give a box between the rows, not one that runs over its
/// neighbours.
pub fn box_art(symbol: &SymbolInstance, stroke: Stroke) -> Vec<Primitive> {
    use novo_core::Direction;
    let pins: Vec<(Vec2, Option<Direction>)> = symbol
        .pins
        .iter()
        .map(|p| (v([p.offset.x, p.offset.y]), p.direction))
        .collect();
    if pins.is_empty() {
        return given_frame(symbol)
            .map_or_else(Vec::new, |(lo, hi)| frame_art(symbol, lo, hi, stroke));
    }
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for (p, _) in &pins {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    // Each side is where its pins' leads end; a side without pins is the
    // pins' hull plus half the smallest box.
    let side = |dir: Direction, pick: fn(Vec2) -> f32, take: fn(f32, f32) -> f32, margin: f32| {
        pins.iter()
            .filter(|(_, d)| *d == Some(dir))
            .map(|(p, _)| pick(*p) + margin)
            .reduce(take)
    };
    let top = side(Direction::Up, |p| p.y, f32::min, LEAD).unwrap_or(lo.y - BOX_MIN / 2.0);
    let bottom = side(Direction::Down, |p| p.y, f32::max, -LEAD).unwrap_or(hi.y + BOX_MIN / 2.0);
    let left = side(Direction::Left, |p| p.x, f32::min, LEAD).unwrap_or(lo.x - BOX_MIN / 2.0);
    let right = side(Direction::Right, |p| p.x, f32::max, -LEAD).unwrap_or(hi.x + BOX_MIN / 2.0);
    let (mut min, mut max) = (Vec2::new(left, top), Vec2::new(right, bottom));
    for axis in 0..2 {
        if max[axis] - min[axis] < BOX_MIN {
            let mid = (max[axis] + min[axis]) / 2.0;
            min[axis] = mid - BOX_MIN / 2.0;
            max[axis] = mid + BOX_MIN / 2.0;
        }
    }
    let mut out = match given_frame(symbol) {
        Some((lo, hi)) => {
            (min, max) = (lo, hi);
            frame_art(symbol, lo, hi, stroke)
        }
        None => vec![Primitive::stroked(Shape::rect(min, max), stroke)],
    };
    let framed = given_frame(symbol).is_some();
    for (p, _) in pins {
        // A framed box keeps its pins where its source put them, no leads.
        if framed || (p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y) {
            continue;
        }
        out.push(Primitive::stroked(
            Shape::line(p, p.clamp(min, max)),
            stroke,
        ));
    }
    out
}

/// The drawing an importer stored on the instance (`novo_symbols::drawn`),
/// when it has one: the artwork the source program drew, with its pins where
/// that program had them, which the library's drawing must not replace.
fn carried(symbol: &SymbolInstance) -> Option<Vec<Graphic>> {
    match symbol.properties.get(novo_symbols::drawn::ART_PROPERTY)? {
        novo_core::Value::Text(t) => novo_symbols::drawn::decode(t),
        _ => None,
    }
}

impl SymbolArt for LibraryArt {
    fn cache_key(&self, symbol: &SymbolInstance) -> Option<novo_canvas::model::ArtKey> {
        if symbol
            .properties
            .contains_key(novo_symbols::drawn::ART_PROPERTY)
        {
            return None;
        }
        // A box with a rectangle of its own differs from its twins by it.
        if symbol.symbol.0 == BOX_KEY && given_frame(symbol).is_some() {
            return None;
        }
        Some(novo_canvas::model::ArtKey::of(symbol))
    }

    fn artwork(&self, symbol: &SymbolInstance) -> Option<Vec<Primitive>> {
        if let Some(g) = carried(symbol) {
            return Some(primitives_of(&g, self.stroke));
        }
        if symbol.symbol.0 == BOX_KEY {
            return Some(box_art(symbol, self.stroke));
        }
        let base = self.shapes.get(&symbol.symbol.0)?;
        let Some((k, lib)) = self.fit(symbol) else {
            return Some(base.clone());
        };
        let mut out: Vec<Primitive> = base
            .iter()
            .map(|p| Primitive {
                shape: scaled(&p.shape, k),
                ..p.clone()
            })
            .collect();
        // A lead from where the scaled artwork ends to where the pin is.
        let stroke = base.iter().find_map(|p| p.stroke);
        for (i, l) in lib.iter().enumerate() {
            let pin = v([symbol.pins[i].offset.x, symbol.pins[i].offset.y]);
            if let Some(stroke) = stroke
                && (pin - *l * k).length() > PIN_TOLERANCE
            {
                out.push(Primitive::stroked(Shape::line(*l * k, pin), stroke));
            }
        }
        Some(out)
    }

    fn labels(&self, symbol: &SymbolInstance) -> Vec<Label> {
        if let Some(g) = carried(symbol) {
            return labels_of(&g);
        }
        let mut labels = self
            .labels
            .get(&symbol.symbol.0)
            .cloned()
            .unwrap_or_default();
        if let Some((k, _)) = self.fit(symbol) {
            for l in &mut labels {
                l.pos *= k;
                l.height *= k.clamp(0.6, 1.0);
            }
        }
        labels
    }

    fn tag_anchor(&self, symbol: &SymbolInstance) -> Option<(Vec2, Anchor)> {
        if symbol
            .properties
            .contains_key(novo_symbols::drawn::ART_PROPERTY)
        {
            return None;
        }
        let (at, anchor) = *self.tags.get(&symbol.symbol.0)?;
        let k = self.fit(symbol).map_or(1.0, |(k, _)| k);
        Some((at * k, anchor))
    }
}

/// The library's text alignment as a canvas anchor: texts are vertically
/// centred on their point (FORMAT.md).
fn anchor(align: Align) -> Anchor {
    match align {
        Align::Left => Anchor::CenterLeft,
        Align::Center => Anchor::Center,
        Align::Right => Anchor::CenterRight,
    }
}

/// Fixed artwork text (the M and 3~ of a motor) in symbol coordinates. The
/// canvas places it with the symbol and draws it in the symbol colour.
pub fn labels(symbol: &Symbol) -> Vec<Label> {
    labels_of(&symbol.graphics)
}

/// [`labels`] for a bare drawing.
pub fn labels_of(graphics: &[Graphic]) -> Vec<Label> {
    graphics
        .iter()
        .filter_map(|g| match g {
            Graphic::Text {
                at,
                text,
                size,
                align,
            } => Some(Label::new(v(*at), text.clone(), *size as f32).anchored(anchor(*align))),
            _ => None,
        })
        .collect()
}

fn v(p: [f64; 2]) -> Vec2 {
    Vec2::new(p[0] as f32, p[1] as f32)
}

/// The artwork of one symbol as canvas primitives in symbol coordinates.
/// Its fixed text comes separately from [`labels`].
pub fn primitives(symbol: &Symbol, stroke: Stroke) -> Vec<Primitive> {
    primitives_of(&symbol.graphics, stroke)
}

/// [`primitives`] for a bare drawing.
pub fn primitives_of(graphics: &[Graphic], stroke: Stroke) -> Vec<Primitive> {
    let mut out = Vec::new();
    let fill = |filled: bool| filled.then_some(stroke.color);
    let mut push = |shape: Shape, filled: Option<Color>, dashed: bool| {
        if dashed {
            out.extend(
                dashes(&shape)
                    .into_iter()
                    .map(|s| Primitive::stroked(s, stroke)),
            );
            return;
        }
        let mut p = Primitive::stroked(shape, stroke);
        if let Some(c) = filled {
            p = p.with_fill(c);
        }
        out.push(p);
    };
    for g in graphics {
        match g {
            Graphic::Line {
                from,
                to,
                stroke: s,
            } => {
                push(Shape::line(v(*from), v(*to)), None, is_dashed(*s));
            }
            Graphic::Polyline {
                points,
                closed,
                fill: f,
                stroke: s,
            } => push(
                Shape::polyline(points.iter().map(|p| v(*p)), *closed),
                fill(*f && *closed),
                is_dashed(*s),
            ),
            Graphic::Rect {
                at,
                size,
                fill: f,
                stroke: s,
            } => {
                let a = v(*at);
                let b = a + Vec2::new(size[0] as f32, size[1] as f32);
                if is_dashed(*s) {
                    let pts = [a, Vec2::new(b.x, a.y), b, Vec2::new(a.x, b.y)];
                    push(Shape::polyline(pts, true), None, true);
                } else {
                    push(Shape::rect(a, b), fill(*f), false);
                }
            }
            Graphic::Circle {
                center,
                r,
                fill: f,
                stroke: s,
            } => push(
                Shape::circle(v(*center), *r as f32),
                fill(*f),
                is_dashed(*s),
            ),
            Graphic::Ellipse {
                center,
                rx,
                ry,
                fill: f,
                stroke: s,
            } => {
                let c = v(*center);
                let pts = (0..48).map(|i| {
                    let a = i as f32 / 48.0 * std::f32::consts::TAU;
                    c + Vec2::new(*rx as f32 * a.cos(), *ry as f32 * a.sin())
                });
                push(Shape::polyline(pts, true), fill(*f), is_dashed(*s));
            }
            Graphic::Arc {
                center,
                r,
                start,
                end,
                stroke: s,
            } => push(
                Shape::Arc {
                    center: v(*center),
                    radius: *r as f32,
                    start: (*start as f32).to_radians(),
                    sweep: ((*end - *start) as f32).to_radians(),
                },
                None,
                is_dashed(*s),
            ),
            Graphic::Text { .. } => {}
        }
    }
    out
}

fn is_dashed(s: novo_symbols::Stroke) -> bool {
    s == novo_symbols::Stroke::Dashed
}

/// Splits straight parts of a shape into dashes. Curves stay solid.
pub(crate) fn dashes(shape: &Shape) -> Vec<Shape> {
    let points: Vec<Vec2> = match shape {
        Shape::Line { a, b } => vec![*a, *b],
        Shape::Polyline { points, closed } => {
            let mut p = points.clone();
            if *closed && let Some(first) = points.first() {
                p.push(*first);
            }
            p
        }
        other => return vec![other.clone()],
    };
    let mut out = Vec::new();
    for seg in points.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let len = a.distance(b);
        if len <= f32::EPSILON {
            continue;
        }
        let dir = (b - a) / len;
        let mut t = 0.0;
        while t < len {
            let end = (t + DASH).min(len);
            out.push(Shape::line(a + dir * t, a + dir * end));
            t = end + GAP;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_library_symbol_has_artwork() {
        let library = Library::builtin().unwrap();
        let art = LibraryArt::new(&library, Stroke::default());
        for s in library.iter() {
            let prims = art.shapes.get(&s.id).unwrap();
            assert!(!prims.is_empty(), "{} has no artwork", s.id);
        }
    }

    #[test]
    fn motor_text_and_tag_anchor_come_from_the_library() {
        let library = Library::builtin().unwrap();
        let art = LibraryArt::new(&library, Stroke::default());
        let motor = library.get("iec/motors/motor_3ph").unwrap();
        let texts: Vec<String> = labels(motor).into_iter().map(|l| l.text).collect();
        assert!(texts.iter().any(|t| t == "M"), "{texts:?}");
        let tag = motor.anchors.iter().find(|a| a.role == "tag").unwrap();
        assert_eq!(art.tags[&motor.id], (v(tag.at), anchor(tag.align)));
        // Every Text graphic becomes one label, and only those.
        for s in library.iter() {
            let n = s
                .graphics
                .iter()
                .filter(|g| matches!(g, Graphic::Text { .. }))
                .count();
            assert_eq!(art.labels.get(&s.id).map_or(0, Vec::len), n, "{}", s.id);
        }
    }

    fn framed_box(frame: Option<[f64; 4]>, dashed: bool) -> SymbolInstance {
        use novo_core::{Pin, Placement, SymbolId, SymbolKey, SymbolRole, Value};
        let mut properties = novo_core::Properties::new();
        if let Some([x0, y0, x1, y1]) = frame {
            for (k, v) in [(BOX_X0, x0), (BOX_Y0, y0), (BOX_X1, x1), (BOX_Y1, y1)] {
                properties.insert(k.to_string(), Value::Float(v));
            }
            properties.insert(BOX_DASHED.to_string(), Value::Bool(dashed));
        }
        SymbolInstance {
            id: SymbolId::from(1),
            page: novo_core::PageId::from(1),
            symbol: SymbolKey(BOX_KEY.to_string()),
            placement: Placement::default(),
            role: SymbolRole::Unassigned,
            pins: vec![Pin::new("1", 0.0, 0.0)],
            bridges: Vec::new(),
            pass_through: Vec::new(),
            feeds: Vec::new(),
            properties,
        }
    }

    #[test]
    fn a_box_with_a_given_rectangle_is_drawn_at_that_size() {
        let stroke = Stroke::default();
        let rect_of = |prims: &[Primitive]| {
            prims.iter().find_map(|p| match p.shape {
                Shape::Rect { min, max } => Some((min, max)),
                _ => None,
            })
        };
        let plain = box_art(&framed_box(None, false), stroke);
        let (lo, hi) = rect_of(&plain).unwrap();
        assert!(hi.x - lo.x < 10.0, "a pin-sized box without a rectangle");

        let given = box_art(&framed_box(Some([0.0, 0.0, 356.0, 16.0]), false), stroke);
        let (lo, hi) = rect_of(&given).unwrap();
        assert_eq!((lo, hi), (Vec2::new(0.0, 0.0), Vec2::new(356.0, 16.0)));
        // no lead from the pin: it sits on the frame's corner
        assert_eq!(given.len(), 1);

        let dashed = box_art(&framed_box(Some([0.0, 0.0, 356.0, 16.0]), true), stroke);
        assert!(dashed.len() > 8, "dashes, not one rectangle");
        assert!(rect_of(&dashed).is_none());
    }

    #[test]
    fn boxes_with_their_own_rectangle_do_not_share_a_drawing() {
        let library = Library::builtin().unwrap();
        let art = LibraryArt::new(&library, Stroke::default());
        assert!(
            art.cache_key(&framed_box(Some([0.0, 0.0, 9.0, 9.0]), false))
                .is_none()
        );
        assert!(art.cache_key(&framed_box(None, false)).is_some());
    }

    #[test]
    fn dashed_lines_are_split() {
        let d = dashes(&Shape::line(Vec2::ZERO, Vec2::new(10.0, 0.0)));
        assert_eq!(d.len(), 4);
    }
}
