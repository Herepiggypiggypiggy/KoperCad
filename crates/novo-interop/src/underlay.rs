//! A DXF as a page background (`docs/design/dxf-background.md`, option B).
//!
//! The drawing lives in the page's own properties, so it needs no model
//! change and undoes, saves and diffs like any property:
//!
//! | key | value |
//! | --- | --- |
//! | `underlay.dxf` | the cleaned drawing as ASCII DXF, millimetres, lower-left corner at 0,0 |
//! | `underlay.h` | its height in mm (to put y up on a y-down page) |
//! | `underlay.name` | the file it came from, for the menu |
//! | `underlay.dx`, `underlay.dy` | where its top-left corner sits on the page, mm |
//! | `underlay.scale` | extra scale, 1.0 true size |
//! | `underlay.visible` | shown or hidden |
//!
//! Importing never creates devices or wires: it is reference geometry.

use std::collections::BTreeMap;
use std::f32::consts::PI;

use novo_canvas::model::Underlay;
use novo_canvas::{Anchor, Color, Label, Primitive, Shape, Stroke, Vec2};
use novo_core::{Properties, Value};

use crate::dxf::{Drawing, DxfError, EntityKind, HAlign, Point, VAlign, read_dxf, write_dxf};

pub const KEY_DXF: &str = "underlay.dxf";
pub const KEY_HEIGHT: &str = "underlay.h";
pub const KEY_NAME: &str = "underlay.name";
pub const KEY_DX: &str = "underlay.dx";
pub const KEY_DY: &str = "underlay.dy";
pub const KEY_SCALE: &str = "underlay.scale";
pub const KEY_VISIBLE: &str = "underlay.visible";

/// Most stored text per page (bytes). Bigger drawings are refused with a
/// hint instead of bloating the project file and every snapshot.
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

/// Where a fresh underlay's top-left corner goes: the inner frame corner.
const DEFAULT_OFFSET_MM: f64 = 10.0;

#[derive(Debug, thiserror::Error)]
pub enum UnderlayError {
    #[error(transparent)]
    Dxf(#[from] DxfError),
    #[error("the drawing has nothing KoperCAD can show (only unsupported entities)")]
    Empty,
    #[error(
        "the drawing is {size} KB, more than the {max} KB a page can hold; hide or purge layers in the source and export again"
    )]
    TooLarge { size: usize, max: usize },
}

/// What an import produced: the properties to set on the page and what was
/// left out, for the dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct UnderlayImport {
    /// `(key, value)` pairs for the page, every `underlay.*` key included.
    pub properties: Vec<(&'static str, Value)>,
    pub entities: usize,
    /// Entity types not imported, with counts.
    pub skipped: BTreeMap<String, usize>,
    pub warnings: Vec<String>,
}

impl UnderlayImport {
    /// One line for the message bar.
    pub fn summary(&self, name: &str) -> String {
        let mut s = format!("Imported {} shapes from {name}", self.entities);
        let skipped: usize = self.skipped.values().sum();
        if skipped > 0 {
            let list: Vec<String> = self
                .skipped
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect();
            s.push_str(&format!("; not imported: {}", list.join(", ")));
        }
        s
    }
}

/// Reads DXF `bytes` as a background. Hidden layers are dropped, units are
/// converted to millimetres, and the drawing is moved to the origin.
pub fn import_underlay(bytes: &[u8], name: &str) -> Result<UnderlayImport, UnderlayError> {
    let read = read_dxf(bytes)?;
    let mut drawing = read.drawing.scaled_to_mm();
    let hidden: Vec<String> = drawing
        .layers
        .iter()
        .filter(|l| l.hidden)
        .map(|l| l.name.clone())
        .collect();
    drawing.entities.retain(|e| !hidden.contains(&e.layer));
    let (min, max) = drawing.bounds().ok_or(UnderlayError::Empty)?;
    move_by(&mut drawing, -min.x, -min.y);
    let text = write_dxf(&drawing);
    if text.len() > MAX_BYTES {
        return Err(UnderlayError::TooLarge {
            size: text.len() / 1024,
            max: MAX_BYTES / 1024,
        });
    }
    Ok(UnderlayImport {
        properties: vec![
            (KEY_DXF, Value::Text(text)),
            (KEY_HEIGHT, Value::Float(max.y - min.y)),
            (KEY_NAME, Value::Text(name.to_string())),
            (KEY_DX, Value::Float(DEFAULT_OFFSET_MM)),
            (KEY_DY, Value::Float(DEFAULT_OFFSET_MM)),
            (KEY_SCALE, Value::Float(1.0)),
            (KEY_VISIBLE, Value::Bool(true)),
        ],
        entities: drawing.entities.len(),
        skipped: read.skipped,
        warnings: read.warnings,
    })
}

fn move_by(d: &mut Drawing, dx: f64, dy: f64) {
    let m = |p: &mut Point| {
        p.x += dx;
        p.y += dy;
    };
    for e in &mut d.entities {
        match &mut e.kind {
            EntityKind::Line { a, b } => {
                m(a);
                m(b);
            }
            EntityKind::Polyline { points, .. } => points.iter_mut().for_each(m),
            EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => m(center),
            EntityKind::Text { pos, .. } => m(pos),
        }
    }
}

/// The text a page stores, when it has an underlay.
pub fn stored_text(props: &Properties) -> Option<&str> {
    match props.get(KEY_DXF) {
        Some(Value::Text(t)) if !t.is_empty() => Some(t),
        _ => None,
    }
}

/// The file name the underlay came from.
pub fn stored_name(props: &Properties) -> Option<&str> {
    match props.get(KEY_NAME) {
        Some(Value::Text(t)) => Some(t),
        _ => None,
    }
}

/// Whether the page's underlay is shown (true when it has no flag).
pub fn is_visible(props: &Properties) -> bool {
    !matches!(props.get(KEY_VISIBLE), Some(Value::Bool(false)))
}

fn number(props: &Properties, key: &str, default: f64) -> f64 {
    match props.get(key) {
        Some(Value::Float(f)) => *f,
        Some(Value::Int(i)) => *i as f64,
        _ => default,
    }
}

/// A key that changes whenever what [`underlay_of`] would draw changes, so
/// a caller can skip parsing on every frame.
pub fn cache_key(props: &Properties) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let text = stored_text(props)?;
    if !is_visible(props) {
        return None;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    for k in [KEY_HEIGHT, KEY_DX, KEY_DY, KEY_SCALE] {
        number(props, k, 0.0).to_bits().hash(&mut h);
    }
    Some(h.finish())
}

/// The canvas underlay a page's properties describe: grey, thin, in page
/// millimetres. `None` when there is none or it is hidden.
pub fn underlay_of(props: &Properties) -> Option<Underlay> {
    if !is_visible(props) {
        return None;
    }
    let drawing = read_dxf(stored_text(props)?.as_bytes()).ok()?.drawing;
    let height = number(props, KEY_HEIGHT, 0.0);
    let (dx, dy) = (number(props, KEY_DX, 10.0), number(props, KEY_DY, 10.0));
    let k = number(props, KEY_SCALE, 1.0).max(1e-6);
    let at = |p: Point| Vec2::new((dx + k * p.x) as f32, (dy + k * (height - p.y)) as f32);
    let ink = Color::rgb(130, 140, 150);
    let stroke = Stroke::world(0.18, ink);
    let mut out = Underlay::default();
    for e in &drawing.entities {
        match &e.kind {
            EntityKind::Line { a, b } => out
                .primitives
                .push(Primitive::stroked(Shape::line(at(*a), at(*b)), stroke)),
            EntityKind::Polyline { points, closed } if points.len() >= 2 => {
                out.primitives.push(Primitive::stroked(
                    Shape::Polyline {
                        points: points.iter().map(|p| at(*p)).collect(),
                        closed: *closed,
                    },
                    stroke,
                ))
            }
            EntityKind::Circle { center, radius } => out.primitives.push(Primitive::stroked(
                Shape::circle(at(*center), (k * radius) as f32),
                stroke,
            )),
            EntityKind::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                // Counter-clockwise in y up is clockwise on the y-down page,
                // from the mirrored end angle through the sweep.
                let sweep = (end_deg - start_deg).rem_euclid(360.0) as f32 * PI / 180.0;
                out.primitives.push(Primitive::stroked(
                    Shape::Arc {
                        center: at(*center),
                        radius: (k * radius) as f32,
                        start: (-end_deg) as f32 * PI / 180.0,
                        sweep,
                    },
                    stroke,
                ));
            }
            EntityKind::Text {
                pos,
                height: h,
                rotation_deg,
                text,
                align,
            } => {
                let anchor = match (align.v, align.h) {
                    (VAlign::Top, HAlign::Left) => Anchor::TopLeft,
                    (VAlign::Top, HAlign::Center) => Anchor::TopCenter,
                    (VAlign::Top, HAlign::Right) => Anchor::TopRight,
                    (VAlign::Middle, HAlign::Left) => Anchor::CenterLeft,
                    (VAlign::Middle, HAlign::Center) => Anchor::Center,
                    (VAlign::Middle, HAlign::Right) => Anchor::CenterRight,
                    (_, HAlign::Left) => Anchor::BottomLeft,
                    (_, HAlign::Center) => Anchor::BottomCenter,
                    (_, HAlign::Right) => Anchor::BottomRight,
                };
                let mut label = Label::new(at(*pos), text.clone(), (k * h) as f32)
                    .anchored(anchor)
                    .colored(ink);
                label.rotation = (-rotation_deg) as f32 * PI / 180.0;
                out.labels.push(label);
            }
            _ => {}
        }
    }
    Some(out)
}
