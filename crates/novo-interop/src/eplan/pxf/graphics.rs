//! The plain drawing on a page: lines, rectangles, arcs, polygons and the
//! location boxes (`O31`, `O89`, `O32`, `O34`, `O88`), and how they are shown.
//!
//! KoperCAD has no free-line object on a page, so this geometry becomes the
//! page's background drawing (the same property mechanism as an imported DXF,
//! `novo_interop::underlay`): it undoes, saves and diffs like a property and
//! never adds devices or wires. Exact: every position and size, in millimetres
//! on the sheet. Inferred: pen widths (one thin pen is used), the line type of
//! a dashed line (dashes are drawn in a fixed pattern), and the drawing of a
//! location box (a dashed frame with its location name).

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

use crate::dxf::{Drawing, Entity, EntityKind, HAlign, Point, TextAlign, VAlign, write_dxf};
use crate::underlay;
use novo_core::Value;

/// A sheet is A3 landscape, millimetres, y up from the lower left.
pub(super) const SHEET_W: f64 = 420.0;
pub(super) const SHEET_H: f64 = 297.0;

/// What a drawn shape is.
#[derive(Debug, Clone, PartialEq)]
pub enum ShapeKind {
    Line {
        a: (f64, f64),
        b: (f64, f64),
    },
    /// Axis-parallel rectangle between two opposite corners.
    Rect {
        a: (f64, f64),
        b: (f64, f64),
    },
    /// Counter-clockwise from `start` to `end`, radians.
    Arc {
        center: (f64, f64),
        radius: f64,
        start: f64,
        end: f64,
    },
    Polygon {
        points: Vec<(f64, f64)>,
        filled: bool,
    },
    /// A location (or function) box: a frame with the location's name.
    LocationBox {
        a: (f64, f64),
        b: (f64, f64),
        /// `=A2` and `+CAB` as EPLAN shows them, when the box has them.
        function: String,
        location: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PxfShape {
    /// Index into [`super::PxfFile::pages`].
    pub page: Option<usize>,
    pub kind: ShapeKind,
    /// Pen width in millimetres when the file states one.
    pub pen: Option<f64>,
    /// A line type other than continuous or "by layer".
    pub dashed: bool,
}

/// `x/y` as the file writes a point.
pub(super) fn point(s: &str) -> Option<(f64, f64)> {
    super::library::parse_point(s)
}

/// `x/y;x/y;...`
pub(super) fn points(s: &str) -> Vec<(f64, f64)> {
    s.split(';').filter_map(point).collect()
}

/// A page's background drawing, ready to store as page properties.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PageGraphics {
    pub properties: Vec<(&'static str, Value)>,
    pub shapes: usize,
}

/// What the graphics step did, for the report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphicsStats {
    pub pages: usize,
    pub lines: usize,
    pub rects: usize,
    pub arcs: usize,
    pub polygons: usize,
    pub location_boxes: usize,
    /// Shapes wholly outside the sheet (the height index marks at the edge).
    pub outside: usize,
    /// Shapes on pages that are not drawn.
    pub undrawn: usize,
    /// Pages whose drawing was too big for a page background.
    pub too_large: Vec<usize>,
}

const DASH: f64 = 3.0;
const GAP: f64 = 1.5;

/// Wholly on or beyond one edge of the sheet (the height index marks run
/// from just outside the sheet to its edge).
fn all_outside(pts: &[(f64, f64)]) -> bool {
    pts.iter().all(|p| p.0 >= SHEET_W)
        || pts.iter().all(|p| p.0 <= 0.0)
        || pts.iter().all(|p| p.1 >= SHEET_H)
        || pts.iter().all(|p| p.1 <= 0.0)
}

/// Coordinates a sheet drawing can sensibly have (the sheet is 420 x 297 mm
/// and some layout sheets keep a full-size copy a metre or two to one side).
/// Anything beyond is a damaged or hostile file: it is skipped before any
/// dash is generated, so work stays bounded by the sheet, not by the file.
const COORD_LIMIT: f64 = 5000.0;
/// Most drawing entities one page's background may take while it is built.
const MAX_ENTITIES: usize = 60_000;

fn wild(pts: &[(f64, f64)], extra: f64) -> bool {
    !extra.is_finite()
        || extra.abs() > COORD_LIMIT
        || pts
            .iter()
            .any(|p| !(p.0.abs() <= COORD_LIMIT && p.1.abs() <= COORD_LIMIT))
}

fn pt(p: (f64, f64)) -> Point {
    Point::new(p.0, p.1)
}

fn line(layer: &str, a: (f64, f64), b: (f64, f64), dashed: bool, out: &mut Drawing) {
    if !dashed {
        out.push(Entity::new(layer, EntityKind::Line { a: pt(a), b: pt(b) }));
        return;
    }
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    if len < DASH + GAP {
        out.push(Entity::new(layer, EntityKind::Line { a: pt(a), b: pt(b) }));
        return;
    }
    let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    let mut at = 0.0;
    while at < len {
        let end = (at + DASH).min(len);
        out.push(Entity::new(
            layer,
            EntityKind::Line {
                a: Point::new(a.0 + ux * at, a.1 + uy * at),
                b: Point::new(a.0 + ux * end, a.1 + uy * end),
            },
        ));
        at += DASH + GAP;
    }
}

fn frame(layer: &str, a: (f64, f64), b: (f64, f64), dashed: bool, out: &mut Drawing) {
    let c = [a, (b.0, a.1), b, (a.0, b.1)];
    if dashed {
        for i in 0..4 {
            line(layer, c[i], c[(i + 1) % 4], true, out);
        }
    } else {
        out.push(Entity::new(
            layer,
            EntityKind::Polyline {
                points: c.iter().map(|p| pt(*p)).collect(),
                closed: true,
            },
        ));
    }
}

/// Builds the background drawing of every page in `drawn` that has shapes.
pub(super) fn build(
    shapes: &[PxfShape],
    drawn: &BTreeSet<usize>,
) -> (BTreeMap<usize, PageGraphics>, GraphicsStats) {
    let mut stats = GraphicsStats::default();
    let mut by_page: BTreeMap<usize, Drawing> = BTreeMap::new();
    let mut count: BTreeMap<usize, usize> = BTreeMap::new();
    for s in shapes {
        let Some(page) = s.page.filter(|p| drawn.contains(p)) else {
            stats.undrawn += 1;
            continue;
        };
        let (pts, layer): (Vec<(f64, f64)>, &str) = match &s.kind {
            ShapeKind::Line { a, b } | ShapeKind::Rect { a, b } => (vec![*a, *b], "lines"),
            ShapeKind::Arc { center, .. } => (vec![*center], "lines"),
            ShapeKind::Polygon { points, .. } => (points.clone(), "lines"),
            ShapeKind::LocationBox { a, b, .. } => (vec![*a, *b], "boxes"),
        };
        let radius = match &s.kind {
            ShapeKind::Arc { radius, .. } => *radius,
            _ => 0.0,
        };
        if pts.is_empty() || all_outside(&pts) || wild(&pts, radius) {
            stats.outside += 1;
            continue;
        }
        let d = by_page.entry(page).or_default();
        if d.entities.len() >= MAX_ENTITIES {
            if !stats.too_large.contains(&page) {
                stats.too_large.push(page);
            }
            continue;
        }
        match &s.kind {
            ShapeKind::Line { a, b } => {
                line(layer, *a, *b, s.dashed, d);
                stats.lines += 1;
            }
            ShapeKind::Rect { a, b } => {
                frame(layer, *a, *b, s.dashed, d);
                stats.rects += 1;
            }
            ShapeKind::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let sweep = (end - start).rem_euclid(TAU);
                if sweep < 1e-6 || (TAU - sweep) < 1e-6 {
                    d.push(Entity::new(
                        layer,
                        EntityKind::Circle {
                            center: pt(*center),
                            radius: *radius,
                        },
                    ));
                } else {
                    d.push(Entity::new(
                        layer,
                        EntityKind::Arc {
                            center: pt(*center),
                            radius: *radius,
                            start_deg: start.to_degrees().rem_euclid(360.0),
                            end_deg: end.to_degrees().rem_euclid(360.0),
                        },
                    ));
                }
                stats.arcs += 1;
            }
            ShapeKind::Polygon { points, .. } => {
                if points.len() >= 2 {
                    d.push(Entity::new(
                        layer,
                        EntityKind::Polyline {
                            points: points.iter().map(|p| pt(*p)).collect(),
                            closed: true,
                        },
                    ));
                    stats.polygons += 1;
                }
            }
            ShapeKind::LocationBox {
                a,
                b,
                function,
                location,
            } => {
                frame(layer, *a, *b, true, d);
                let label = format!("{function}{location}");
                if !label.is_empty() {
                    let (x0, y1) = (a.0.min(b.0), a.1.max(b.1));
                    d.push(Entity::new(
                        layer,
                        EntityKind::Text {
                            pos: Point::new(x0 + 1.5, y1 - 1.5),
                            height: 3.5,
                            rotation_deg: 0.0,
                            text: label,
                            align: TextAlign {
                                h: HAlign::Left,
                                v: VAlign::Top,
                            },
                        },
                    ));
                }
                stats.location_boxes += 1;
            }
        }
        *count.entry(page).or_default() += 1;
    }
    let mut out = BTreeMap::new();
    for (page, drawing) in by_page {
        if stats.too_large.contains(&page) {
            continue;
        }
        let text = write_dxf(&drawing);
        if text.len() > underlay::MAX_BYTES {
            stats.too_large.push(page);
            continue;
        }
        out.insert(
            page,
            PageGraphics {
                properties: vec![
                    (underlay::KEY_DXF, Value::Text(text)),
                    (underlay::KEY_HEIGHT, Value::Float(SHEET_H)),
                    (
                        underlay::KEY_NAME,
                        Value::Text("EPLAN lines and boxes".into()),
                    ),
                    (underlay::KEY_DX, Value::Float(0.0)),
                    (underlay::KEY_DY, Value::Float(0.0)),
                    (underlay::KEY_SCALE, Value::Float(1.0)),
                    (underlay::KEY_VISIBLE, Value::Bool(true)),
                ],
                shapes: count.get(&page).copied().unwrap_or(0),
            },
        );
    }
    stats.pages = out.len();
    (out, stats)
}

/// The page properties (`underlay.*`) that show `shapes` as a page's
/// background drawing, in sheet millimetres (y up from the lower left, A3).
/// The shapes' `page` is ignored; `None` when nothing is left to draw or the
/// drawing is too big for a page background. For other readers (the AML
/// import) that have their own lines and rectangles.
pub fn background_properties(shapes: &[PxfShape]) -> Option<Vec<(&'static str, Value)>> {
    let flat: Vec<PxfShape> = shapes
        .iter()
        .map(|s| PxfShape {
            page: Some(0),
            ..s.clone()
        })
        .collect();
    let (mut pages, _) = build(&flat, &BTreeSet::from([0]));
    pages.remove(&0).map(|g| g.properties)
}
