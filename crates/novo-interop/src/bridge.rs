//! Page scene to DXF drawing. See `docs/design/interop.md` §3.
//!
//! The page scene is millimetres, y down, angles clockwise on screen. The
//! drawing is DXF-native, y up, so every point is mirrored about the page
//! height and every angle is negated. Symbol artwork is drawn as plain
//! geometry (no blocks), so a recipient needs nothing from KoperCAD.

use novo_canvas::model::{ModelStyle, PageScene, SymbolArt};
use novo_canvas::{Item, Label, Primitive, Scene, Shape, Vec2};
use novo_core::{PageId, Project};

use crate::dxf::{Drawing, Entity, EntityKind, HAlign, Layer, Point, TextAlign, VAlign};

/// AutoCAD colour index per scene layer name; unknown layers are white/black.
fn layer_color(name: &str) -> u16 {
    match name {
        "Frame" => 8,
        "Wires" => 3,
        "Texts" => 2,
        _ => 7,
    }
}

/// The visible layers and items of `scene` as a drawing in a page
/// `page_height_mm` tall. Items are written in layer order, then id, so the
/// result is reproducible.
pub fn scene_to_drawing(scene: &Scene, page_height_mm: f64) -> Drawing {
    let mut drawing = Drawing::default();
    let flip = |p: Vec2| Point::new(f64::from(p.x), page_height_mm - f64::from(p.y));
    let mut items: Vec<_> = scene
        .iter()
        .filter(|(_, it)| scene.layer(it.layer).is_some_and(|l| l.visible))
        .collect();
    items.sort_by_key(|(id, it)| (scene.layer(it.layer).map_or(0, |l| l.order), *id));
    for layer in scene.layers().iter().filter(|l| l.visible) {
        drawing
            .layers
            .push(Layer::new(layer.name.clone(), layer_color(&layer.name)));
    }
    for (_, item) in items {
        let name = scene
            .layer(item.layer)
            .map_or("0", |l| l.name.as_str())
            .to_string();
        push_item(&mut drawing, &name, item, &flip, page_height_mm);
    }
    drawing
}

fn push_item(
    d: &mut Drawing,
    layer: &str,
    item: &Item,
    flip: &impl Fn(Vec2) -> Point,
    page_h: f64,
) {
    for p in &item.primitives {
        if let Some(kind) = primitive(p, flip) {
            d.push(Entity::new(layer, kind));
        }
    }
    for l in &item.labels {
        label(d, layer, l, page_h);
    }
}

fn primitive(p: &Primitive, flip: &impl Fn(Vec2) -> Point) -> Option<EntityKind> {
    let stroked = p.stroke.is_some_and(|s| s.color.0[3] > 0 && s.width > 0.0);
    let filled = p.fill.is_some_and(|c| c.0[3] > 0);
    if !stroked && !filled {
        return None;
    }
    Some(match &p.shape {
        Shape::Line { a, b } => EntityKind::Line {
            a: flip(*a),
            b: flip(*b),
        },
        Shape::Polyline { points, closed } => {
            if points.len() < 2 {
                return None;
            }
            EntityKind::Polyline {
                points: points.iter().map(|q| flip(*q)).collect(),
                closed: *closed,
            }
        }
        Shape::Rect { min, max } => EntityKind::Polyline {
            points: vec![
                flip(*min),
                flip(Vec2::new(max.x, min.y)),
                flip(*max),
                flip(Vec2::new(min.x, max.y)),
            ],
            closed: true,
        },
        Shape::Circle { center, radius } => EntityKind::Circle {
            center: flip(*center),
            radius: f64::from(*radius),
        },
        Shape::Arc {
            center,
            radius,
            start,
            sweep,
        } => {
            // y-down angle t is -t in y-up; a positive sweep (clockwise on
            // screen) then runs from -(start+sweep) up to -start.
            let a = -f64::from(*start).to_degrees();
            let b = -f64::from(start + sweep).to_degrees();
            let (start_deg, end_deg) = if *sweep >= 0.0 { (b, a) } else { (a, b) };
            EntityKind::Arc {
                center: flip(*center),
                radius: f64::from(*radius),
                start_deg: start_deg.rem_euclid(360.0),
                end_deg: end_deg.rem_euclid(360.0),
            }
        }
    })
}

/// One DXF `TEXT` per line, each anchored on its own line box so multi-line
/// labels keep the label's anchor.
fn label(d: &mut Drawing, layer: &str, l: &Label, page_h: f64) {
    if l.height <= 0.0 || l.color.0[3] == 0 {
        return;
    }
    let lines: Vec<&str> = l.text.lines().collect();
    if lines.iter().all(|s| s.trim().is_empty()) {
        return;
    }
    let frac = l.anchor.fractions();
    let (h, n) = (f64::from(l.height), lines.len() as f64);
    // Down the lines, in y-down page millimetres.
    let down = Vec2::new(-l.rotation.sin(), l.rotation.cos());
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let offset = -f64::from(frac.y) * n * h + (i as f64 + 0.5) * h;
        let pos = l.pos + down * offset as f32;
        d.push(Entity::new(
            layer,
            EntityKind::Text {
                pos: Point::new(f64::from(pos.x), page_h - f64::from(pos.y)),
                height: h,
                rotation_deg: (-f64::from(l.rotation).to_degrees()).rem_euclid(360.0),
                text: (*line).to_string(),
                align: TextAlign {
                    h: if frac.x < 0.25 {
                        HAlign::Left
                    } else if frac.x < 0.75 {
                        HAlign::Center
                    } else {
                        HAlign::Right
                    },
                    v: VAlign::Middle,
                },
            },
        ));
    }
}

/// The drawing of one project page, as the canvas shows it (frame, title
/// block, symbols, wires, tags, texts). `None` when the page does not exist.
/// The project is only read.
pub fn page_drawing(
    project: &Project,
    page: PageId,
    art: &dyn SymbolArt,
    style: ModelStyle,
) -> Option<Drawing> {
    let height = project.page(page)?.format.size_mm().1;
    let mut scene = PageScene::new(project, page, art);
    scene.set_style(style, project, art);
    Some(scene_to_drawing(scene.scene(), height))
}
